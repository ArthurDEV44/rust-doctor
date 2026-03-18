use crate::config::ResolvedConfig;
use crate::diagnostics::{Diagnostic, ScanResult, Severity};
use crate::discovery::ProjectInfo;
use crate::{
    audit, clippy, config, deny, diff, machete, msrv, output, rules, scanner, suppression,
    workspace,
};
use std::path::PathBuf;
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
pub fn scan_project(
    project_info: &ProjectInfo,
    resolved: &ResolvedConfig,
    offline: bool,
    project_filter: &[String],
    suppress_spinner: bool,
) -> Result<ScanResult, crate::error::ScanError> {
    validate_config(resolved);

    let scan_roots = resolve_scan_roots(project_info, resolved, project_filter)?;
    log_project_info(project_info, resolved);

    let diff_context = resolve_diff_context(project_info, resolved);

    let (mut all_diagnostics, total_source_files, all_skipped_passes, total_elapsed) = run_passes(
        project_info,
        resolved,
        &scan_roots,
        diff_context.as_ref(),
        offline,
        suppress_spinner,
    );

    dedup_diagnostics(&mut all_diagnostics);

    if let Some(ref ctx) = diff_context {
        all_diagnostics = diff::filter_to_changed_files(all_diagnostics, &ctx.changed_files);
    }

    let all_diagnostics = apply_suppressions(all_diagnostics, project_info, resolved);

    Ok(build_result(
        all_diagnostics,
        total_source_files,
        all_skipped_passes,
        total_elapsed,
    ))
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
        passes.push(Box::new(rules::RuleEnginePass::new(
            rules::error_handling::all_rules()
                .into_iter()
                .chain(rules::performance::all_rules())
                .chain(rules::security::all_rules())
                .chain(if has_async_runtime {
                    rules::async_rules::all_rules()
                } else {
                    vec![]
                })
                .chain(rules::framework::rules_for_frameworks(
                    &project_info.frameworks,
                ))
                .filter(|rule| rule.default_enabled())
                .collect(),
            resolved.ignore_files.clone(),
        )));
    }

    if resolved.dependencies && !is_diff_mode {
        // Prefer cargo-deny (advisory + license + ban + source checks).
        // Fall back to cargo-audit for advisory-only checks when cargo-deny
        // is not installed.
        let deny_pass = deny::DenyPass { offline };
        if deny::is_cargo_deny_available() {
            passes.push(Box::new(deny_pass));
        } else {
            passes.push(Box::new(deny_pass)); // still push to emit the Skipped diagnostic
            passes.push(Box::new(audit::AuditPass { offline }));
        }
        passes.push(Box::new(machete::MachetePass));
    }

    // MSRV validation always runs (not gated by config flags).
    passes.push(Box::new(msrv::MsrvPass {
        rust_version: project_info.rust_version.clone(),
    }));

    passes
}

fn run_passes(
    project_info: &ProjectInfo,
    resolved: &ResolvedConfig,
    scan_roots: &[PathBuf],
    diff_context: Option<&diff::DiffContext>,
    offline: bool,
    suppress_spinner: bool,
) -> (Vec<Diagnostic>, usize, Vec<String>, Duration) {
    let is_diff_mode = diff_context.is_some();
    let mut all_diagnostics = Vec::new();
    let mut total_source_files = 0;
    let mut all_skipped_passes = Vec::new();
    let mut total_elapsed = Duration::ZERO;

    // In diff mode, count changed files once (not per scan root)
    if let Some(ctx) = diff_context {
        total_source_files = ctx.changed_files.len();
    }

    for scan_root in scan_roots {
        if diff_context.is_none() {
            total_source_files += scanner::count_source_files(scan_root);
        }

        let passes = build_passes(project_info, resolved, is_diff_mode, offline);
        let orchestrator = scanner::ScanOrchestrator::new(passes);
        let pass_result = orchestrator.run(scan_root, resolved, suppress_spinner);

        all_diagnostics.extend(pass_result.diagnostics);
        all_skipped_passes.extend(pass_result.skipped_passes);
        total_elapsed += pass_result.elapsed;
    }

    (
        all_diagnostics,
        total_source_files,
        all_skipped_passes,
        total_elapsed,
    )
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
    }
}
