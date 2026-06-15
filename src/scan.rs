use crate::config::ResolvedConfig;
use crate::diagnostics::{Diagnostic, ScanResult, Severity};
use crate::discovery::ProjectInfo;
use crate::{
    audit, clippy, config, coverage, deny, diff, geiger, machete, msrv, output, rules, scanner,
    semver_checks, suppression, workspace,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Derive custom rule names from the rule registry at runtime.
/// Includes the external "unused-dependency" rule which is not AST-based.
pub fn custom_rule_names() -> Vec<String> {
    let mut names: Vec<String> = rules::all_custom_rules()
        .iter()
        .map(|r| r.name().to_string())
        .collect();
    names.push("unused-dependency".to_string());
    names
}

/// Run a complete scan on a discovered Rust project.
///
/// This is the core scanning pipeline used by both the CLI and MCP server.
/// The caller is responsible for project discovery and config resolution.
///
/// Pipeline: validate → resolve roots → run passes (parallel) → dedup → diff filter → suppress → score
pub fn scan_project(
    project_info: &ProjectInfo,
    resolved: &ResolvedConfig,
    offline: bool,
    project_filter: &[String],
    suppress_spinner: bool,
) -> Result<ScanResult, crate::error::ScanError> {
    // CLI path: no external cancellation, so pass a flag that is never set.
    let cancel = Arc::new(AtomicBool::new(false));
    scan_project_cancellable(
        project_info,
        resolved,
        offline,
        project_filter,
        suppress_spinner,
        &cancel,
    )
}

/// Cancellable variant of [`scan_project`].
///
/// The `cancel` flag is polled between scan-root batches and propagated to the
/// subprocess passes. When it is set (e.g. by the MCP 5-minute timeout), the scan
/// stops launching new passes and any in-flight `cargo` subprocess tree is killed
/// instead of being detached to run in the background (US-007 / US-008).
pub fn scan_project_cancellable(
    project_info: &ProjectInfo,
    resolved: &ResolvedConfig,
    offline: bool,
    project_filter: &[String],
    suppress_spinner: bool,
    cancel: &Arc<AtomicBool>,
) -> Result<ScanResult, crate::error::ScanError> {
    tracing::info!(project = %project_info.name, "starting scan");

    // Step 1: Verify ignored rules are known — warns on typos in config
    validate_config(resolved);

    // Step 2: Resolve workspace members or single project root
    let scan_roots = resolve_scan_roots(project_info, resolved, project_filter)?;
    log_project_info(project_info, resolved);

    // Step 3: Parse git diff if --diff was specified (narrows scope to changed files)
    let diff_context = resolve_diff_context(project_info, resolved);

    // Step 4: Run all analysis passes
    // Three levels of parallelism, all OS-thread / rayon layered so rayon workers
    // never block on a join: bounded OS threads over scan roots, std::thread::scope
    // over passes within a root, rayon par_iter over files in the rule engine.
    // See the invariant comment in `run_passes` for why root-level rayon is banned.
    let mut passes_output = run_passes(
        project_info,
        resolved,
        &scan_roots,
        diff_context.as_ref(),
        offline,
        suppress_spinner,
        cancel,
    );

    // Step 5: Deduplicate — same rule+file+line from overlapping workspace scans = one diagnostic
    dedup_diagnostics(&mut passes_output.diagnostics);

    // Step 6: In diff mode, drop diagnostics outside changed files
    if let Some(ref ctx) = diff_context {
        passes_output.diagnostics =
            diff::filter_to_changed_files(passes_output.diagnostics, &ctx.changed_files);
    }

    // Step 7: Apply inline suppressions (// rust-doctor-disable-next-line <rule>)
    let all_diagnostics = apply_suppressions(passes_output.diagnostics, project_info, resolved);

    // Step 8: Calculate score and build the final result
    let result = build_result(
        all_diagnostics,
        passes_output.source_file_count,
        passes_output.skipped_passes,
        passes_output.elapsed,
        passes_output.pass_timings,
    );

    tracing::info!(
        score = result.score,
        diagnostics = result.diagnostics.len(),
        "scan complete"
    );

    Ok(result)
}

// ---------------------------------------------------------------------------
// Pipeline stages
// ---------------------------------------------------------------------------

fn validate_config(resolved: &ResolvedConfig) {
    let mut known_rules: Vec<&str> = clippy::known_lint_names();
    let custom_names = custom_rule_names();
    known_rules.extend(custom_names.iter().map(String::as_str));
    config::validate_ignored_rules(&resolved.ignore_rules, &known_rules);
}

fn resolve_scan_roots(
    project_info: &ProjectInfo,
    resolved: &ResolvedConfig,
    project_filter: &[String],
) -> Result<Vec<PathBuf>, crate::error::ScanError> {
    if project_info.is_workspace {
        let members = workspace::resolve_members(&project_info.workspace_members, project_filter)?;
        if resolved.verbose {
            eprintln!(
                "Workspace: scanning {} of {} members",
                members.len(),
                project_info.member_count
            );
        }
        Ok(members.iter().map(|m| m.root_dir.clone()).collect())
    } else {
        if !project_filter.is_empty() {
            eprintln!("Warning: --project is only applicable to Cargo workspaces; ignoring");
        }
        Ok(vec![project_info.root_dir.clone()])
    }
}

fn log_project_info(project_info: &ProjectInfo, resolved: &ResolvedConfig) {
    if !resolved.verbose {
        return;
    }
    eprintln!(
        "Project: {} v{} (edition {})",
        project_info.name, project_info.version, project_info.edition
    );
    if !project_info.frameworks.is_empty() {
        let fw_list: Vec<String> = project_info
            .frameworks
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        eprintln!("Frameworks: {}", fw_list.join(", "));
    }
    if project_info.is_no_std {
        eprintln!("Mode: no_std");
    }
    if project_info.has_build_script {
        eprintln!("Build script: yes");
    }
    if let Some(ref rv) = project_info.rust_version {
        eprintln!("MSRV: {rv}");
    }
}

fn resolve_diff_context(
    project_info: &ProjectInfo,
    resolved: &ResolvedConfig,
) -> Option<diff::DiffContext> {
    let ctx = resolved.diff.as_ref().and_then(|base_hint| {
        match diff::resolve_diff(&project_info.root_dir, base_hint) {
            Ok(ctx) => Some(ctx),
            Err(e) => {
                eprintln!("Warning: {e}");
                None
            }
        }
    });

    if let Some(ref ctx) = ctx {
        eprintln!(
            "Diff mode: scanning {} changed file(s) vs {}",
            ctx.changed_files.len(),
            ctx.base,
        );
    }

    ctx
}

/// Construct the set of analysis passes based on project info and config.
/// Lint passes (clippy + custom rules) are always included when lint=true.
/// Dependency passes (audit, deny, geiger, machete, semver, coverage) run
/// only when dependencies=true and NOT in diff mode (they scan the whole project).
fn build_passes(
    project_info: &ProjectInfo,
    resolved: &ResolvedConfig,
    is_diff_mode: bool,
    offline: bool,
) -> Vec<Box<dyn scanner::AnalysisPass>> {
    let has_async_runtime = project_info.frameworks.iter().any(|f| {
        matches!(
            f,
            crate::discovery::Framework::Tokio
                | crate::discovery::Framework::AsyncStd
                | crate::discovery::Framework::Smol
        )
    });

    let mut passes: Vec<Box<dyn scanner::AnalysisPass>> = Vec::new();

    if resolved.lint {
        passes.push(Box::new(clippy::ClippyPass));
        passes.push(Box::new(rules::RuleEnginePass::with_config(
            rules::error_handling::all_rules()
                .into_iter()
                .chain(rules::performance::all_rules())
                .chain(rules::complexity::all_rules())
                .chain(rules::security::all_rules())
                .chain(if has_async_runtime {
                    rules::async_rules::all_rules()
                } else {
                    vec![]
                })
                .chain(rules::framework::rules_for_frameworks(
                    &project_info.frameworks,
                ))
                .filter(|rule| {
                    rule.default_enabled()
                        || resolved.enable_rules.contains(&rule.name().to_string())
                })
                .collect(),
            resolved.ignore_files.clone(),
            resolved.ignore_rules.clone(),
            resolved.enable_rules.clone(),
        )));
    }

    if resolved.dependencies && !is_diff_mode {
        // Prefer cargo-deny (advisory + license + ban + source checks).
        // Fall back to cargo-audit for advisory-only checks when cargo-deny
        // is not installed.
        let deny_pass = deny::DenyPass { offline };
        passes.push(Box::new(deny_pass));
        if !deny::is_cargo_deny_available() {
            passes.push(Box::new(audit::AuditPass { offline }));
        }
        passes.push(Box::new(machete::MachetePass));
        passes.push(Box::new(geiger::GeigerPass));
        passes.push(Box::new(semver_checks::SemVerPass));
        passes.push(Box::new(coverage::CoveragePass));
    }

    // MSRV validation always runs (not gated by config flags).
    passes.push(Box::new(msrv::MsrvPass {
        rust_version: project_info.rust_version.clone(),
    }));

    passes
}

/// Aggregated output from running all analysis passes across scan roots.
struct PassesOutput {
    diagnostics: Vec<Diagnostic>,
    source_file_count: usize,
    skipped_passes: Vec<String>,
    elapsed: Duration,
    pass_timings: Vec<(String, Duration)>,
}

fn run_passes(
    project_info: &ProjectInfo,
    resolved: &ResolvedConfig,
    scan_roots: &[PathBuf],
    diff_context: Option<&diff::DiffContext>,
    offline: bool,
    suppress_spinner: bool,
    cancel: &Arc<AtomicBool>,
) -> PassesOutput {
    let is_diff_mode = diff_context.is_some();
    let mut all_diagnostics = Vec::new();
    let mut total_source_files = 0;
    let mut all_skipped_passes = Vec::new();
    let mut total_elapsed = Duration::ZERO;
    let mut all_pass_timings = Vec::new();

    // In diff mode, count changed files once (not per scan root)
    if let Some(ctx) = diff_context {
        total_source_files = ctx.changed_files.len();
    }

    // INVARIANT: parallelize scan roots with OS threads (`std::thread::scope`),
    // bounded to `available_parallelism` per batch — NEVER with rayon
    // (`par_iter`/`rayon::scope`). Each root runs its passes via an inner
    // `std::thread::scope` (`ScanOrchestrator::run_passes_parallel`), and the rule
    // engine fans out file work with an inner rayon `par_iter` (`rules/mod.rs`).
    // A rayon iterator at THIS level would park a rayon worker on the inner
    // `thread::scope.join()` whose rule engine awaits inner rayon work on the same
    // global pool; with workspace members ≥ cores and a cold cache, every worker
    // parks on `join` and the inner `par_iter` starves → permanent hang (EP-001).
    // OS threads sidestep this: rayon pool workers are never the threads that block
    // on a join, so the pool always makes progress regardless of the member/core
    // ratio. Root-level parallelism is KEPT (it overlaps each root's blocking
    // `cargo clippy` wait — sequential roots measured ~20% slower on a cold
    // multi-member workspace) but bounded, so a huge workspace cannot spawn
    // unbounded threads or `cargo` subprocesses.
    // DO NOT reintroduce rayon above a `thread::scope` that itself contains rayon.
    let max_parallel = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    for batch in scan_roots.chunks(max_parallel) {
        // US-007: cooperative cancellation, polled BETWEEN batches (never mid-pass).
        // When the MCP timeout sets the flag, stop launching new root batches.
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let batch_results = std::thread::scope(|s| {
            #[expect(
                clippy::needless_collect,
                reason = "all roots must be spawned before any is joined, else they run serially"
            )]
            let handles: Vec<_> = batch
                .iter()
                .map(|scan_root| {
                    s.spawn(move || {
                        // Skip roots whose work hasn't started once cancellation fires.
                        if cancel.load(Ordering::Relaxed) {
                            return (0, scanner::ScanPassResult::default());
                        }
                        let source_files = if diff_context.is_none() {
                            scanner::count_source_files(scan_root)
                        } else {
                            0
                        };
                        let passes = build_passes(project_info, resolved, is_diff_mode, offline);
                        let orchestrator = scanner::ScanOrchestrator::new(passes);
                        let pass_result = orchestrator.run(scan_root, resolved, suppress_spinner);
                        (source_files, pass_result)
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| {
                    h.join().unwrap_or_else(|_| {
                        // Pass panics are already caught inside the orchestrator
                        // (PassError::Panicked), so a join failure here is a rare
                        // root-level panic. Keep the other roots' results instead of
                        // aborting the whole scan (US-001 AC5).
                        eprintln!(
                            "Warning: a scan root worker panicked; its diagnostics are omitted"
                        );
                        (0, scanner::ScanPassResult::default())
                    })
                })
                .collect::<Vec<_>>()
        });

        // Roots within a batch run in parallel (wall-clock ≈ max); batches run
        // sequentially (wall-clock ≈ sum of per-batch maxima).
        let mut batch_elapsed = Duration::ZERO;
        for (source_files, pass_result) in batch_results {
            total_source_files += source_files;
            all_diagnostics.extend(pass_result.diagnostics);
            all_skipped_passes.extend(pass_result.skipped_passes);
            batch_elapsed = batch_elapsed.max(pass_result.elapsed);
            all_pass_timings.extend(pass_result.pass_timings);
        }
        total_elapsed += batch_elapsed;
    }

    PassesOutput {
        diagnostics: all_diagnostics,
        source_file_count: total_source_files,
        skipped_passes: all_skipped_passes,
        elapsed: total_elapsed,
        pass_timings: all_pass_timings,
    }
}

/// Deduplicate diagnostics from overlapping workspace scans.
fn dedup_diagnostics(diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.sort_by(|a, b| {
        a.file_path
            .cmp(&b.file_path)
            .then(a.rule.cmp(&b.rule))
            .then(a.line.cmp(&b.line))
            .then(a.column.cmp(&b.column))
            .then(a.message.cmp(&b.message))
    });
    diagnostics.dedup_by(|a, b| {
        a.file_path == b.file_path
            && a.rule == b.rule
            && a.line == b.line
            && a.column == b.column
            && a.message == b.message
    });
}

fn apply_suppressions(
    diagnostics: Vec<Diagnostic>,
    project_info: &ProjectInfo,
    resolved: &ResolvedConfig,
) -> Vec<Diagnostic> {
    let (diagnostics, suppressed_count) =
        suppression::apply_inline_suppressions(diagnostics, &project_info.root_dir);
    if resolved.verbose && suppressed_count > 0 {
        eprintln!("Suppressed {suppressed_count} diagnostic(s) via inline comments");
    }
    diagnostics
}

fn build_result(
    diagnostics: Vec<Diagnostic>,
    source_file_count: usize,
    mut skipped_passes: Vec<String>,
    elapsed: Duration,
    pass_timings: Vec<(String, Duration)>,
) -> ScanResult {
    let error_count = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let warning_count = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .count();
    let info_count = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Info)
        .count();
    let (score, score_label, dimension_scores) = output::calculate_score(&diagnostics);

    skipped_passes.sort();
    skipped_passes.dedup();

    ScanResult {
        diagnostics,
        score,
        score_label,
        dimension_scores,
        source_file_count,
        elapsed,
        skipped_passes,
        error_count,
        warning_count,
        info_count,
        pass_timings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Category;
    use std::path::PathBuf;

    fn make_diagnostic(rule: &str, severity: Severity, line: Option<u32>) -> Diagnostic {
        Diagnostic {
            file_path: PathBuf::from("src/main.rs"),
            rule: rule.to_string(),
            category: Category::Correctness,
            severity,
            message: format!("test diagnostic for {rule}"),
            help: None,
            line,
            column: None,
            fix: None,
        }
    }

    #[test]
    fn custom_rule_names_includes_all_rules() {
        let names = custom_rule_names();
        // Must include custom AST rules + the special "unused-dependency" rule
        assert!(names.contains(&"unwrap-in-production".to_string()));
        assert!(names.contains(&"high-cyclomatic-complexity".to_string()));
        assert!(names.contains(&"hardcoded-secrets".to_string()));
        assert!(names.contains(&"unused-dependency".to_string()));
        // At least 15+ rules (5 error_handling + 5 performance + 1 complexity + 3 security + ...)
        assert!(
            names.len() >= 15,
            "Expected >= 15 rules, got {}",
            names.len()
        );
    }

    #[test]
    fn dedup_removes_duplicate_diagnostics() {
        let mut diags = vec![
            make_diagnostic("rule-a", Severity::Warning, Some(10)),
            make_diagnostic("rule-a", Severity::Warning, Some(10)), // duplicate
            make_diagnostic("rule-b", Severity::Error, Some(20)),
        ];
        dedup_diagnostics(&mut diags);
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].rule, "rule-a");
        assert_eq!(diags[1].rule, "rule-b");
    }

    #[test]
    fn dedup_keeps_different_lines() {
        let mut diags = vec![
            make_diagnostic("rule-a", Severity::Warning, Some(10)),
            make_diagnostic("rule-a", Severity::Warning, Some(20)), // same rule, different line
        ];
        dedup_diagnostics(&mut diags);
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn dedup_handles_empty() {
        let mut diags: Vec<Diagnostic> = vec![];
        dedup_diagnostics(&mut diags);
        assert!(diags.is_empty());
    }

    #[test]
    fn build_result_counts_severities() {
        let diags = vec![
            make_diagnostic("err1", Severity::Error, Some(1)),
            make_diagnostic("err2", Severity::Error, Some(2)),
            make_diagnostic("warn1", Severity::Warning, Some(3)),
            make_diagnostic("info1", Severity::Info, Some(4)),
        ];
        let result = build_result(diags, 10, vec![], Duration::from_secs(1), vec![]);
        assert_eq!(result.error_count, 2);
        assert_eq!(result.warning_count, 1);
        assert_eq!(result.info_count, 1);
        assert_eq!(result.source_file_count, 10);
    }

    #[test]
    fn build_result_deduplicates_skipped_passes() {
        let skipped = vec![
            "cargo-deny".to_string(),
            "cargo-audit".to_string(),
            "cargo-deny".to_string(), // duplicate
        ];
        let result = build_result(vec![], 0, skipped, Duration::ZERO, vec![]);
        assert_eq!(result.skipped_passes.len(), 2);
        assert_eq!(result.skipped_passes[0], "cargo-audit"); // sorted
        assert_eq!(result.skipped_passes[1], "cargo-deny");
    }

    #[test]
    fn build_result_empty_diagnostics_gives_perfect_score() {
        let result = build_result(vec![], 5, vec![], Duration::from_millis(100), vec![]);
        assert_eq!(result.score, 100);
        assert_eq!(result.error_count, 0);
        assert_eq!(result.warning_count, 0);
        assert_eq!(result.info_count, 0);
    }
}
