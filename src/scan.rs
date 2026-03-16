use crate::config::ResolvedConfig;
use crate::diagnostics::{ScanResult, Severity};
use crate::discovery::ProjectInfo;
use crate::{audit, clippy, config, diff, machete, output, rules, scanner, suppression, workspace};

/// Known custom rule names for config validation.
pub const CUSTOM_RULE_NAMES: &[&str] = &[
    "unwrap-in-production",
    "panic-in-library",
    "box-dyn-error-in-public-api",
    "result-unit-error",
    "excessive-clone",
    "string-from-literal",
    "collect-then-iterate",
    "large-enum-variant",
    "unnecessary-allocation",
    "hardcoded-secrets",
    "unsafe-block-audit",
    "sql-injection-risk",
    "blocking-in-async",
    "block-on-in-async",
    "tokio-main-missing",
    "tokio-spawn-without-move",
    "axum-handler-not-async",
    "actix-blocking-handler",
    "unused-dependency",
];

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
    // Validate ignored rules
    let mut known_rules = clippy::known_lint_names();
    known_rules.extend_from_slice(CUSTOM_RULE_NAMES);
    config::validate_ignored_rules(&resolved.ignore_rules, &known_rules);

    // Resolve workspace members
    let scan_roots: Vec<std::path::PathBuf> = if project_info.is_workspace {
        let members = workspace::resolve_members(&project_info.workspace_members, project_filter)
            .map_err(crate::error::ScanError::Workspace)?;
        if resolved.verbose {
            eprintln!(
                "Workspace: scanning {} of {} members",
                members.len(),
                project_info.member_count
            );
        }
        members.iter().map(|m| m.root_dir.clone()).collect()
    } else {
        if !project_filter.is_empty() {
            eprintln!("Warning: --project is only applicable to Cargo workspaces; ignoring");
        }
        vec![project_info.root_dir.clone()]
    };

    // Print project info
    if resolved.verbose {
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

    // Resolve diff context
    let diff_context = resolved.diff.as_ref().and_then(|base_hint| {
        match diff::resolve_diff(&project_info.root_dir, base_hint) {
            Ok(ctx) => Some(ctx),
            Err(e) => {
                eprintln!("Warning: {e}");
                None
            }
        }
    });
    let is_diff_mode = diff_context.is_some();

    if let Some(ref ctx) = diff_context {
        eprintln!(
            "Diff mode: scanning {} changed file(s) vs {}",
            ctx.changed_files.len(),
            ctx.base,
        );
    }

    // Detect async runtime for conditional rule activation
    let has_async_runtime = project_info.frameworks.iter().any(|f| {
        matches!(
            f,
            crate::discovery::Framework::Tokio
                | crate::discovery::Framework::AsyncStd
                | crate::discovery::Framework::Smol
        )
    });

    // Scan each root (single project or workspace members)
    let mut all_diagnostics = Vec::new();
    let mut total_source_files = 0;
    let mut all_skipped_passes = Vec::new();
    let mut total_elapsed = std::time::Duration::ZERO;

    // In diff mode, count changed files once (not per scan root)
    if let Some(ref ctx) = diff_context {
        total_source_files = ctx.changed_files.len();
    }

    for scan_root in &scan_roots {
        if diff_context.is_none() {
            total_source_files += scanner::count_source_files(scan_root);
        }

        // Build passes per scan root, respecting config flags
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
                    .collect(),
                resolved.ignore_files.clone(),
            )));
        }
        if resolved.dependencies && !is_diff_mode {
            passes.push(Box::new(audit::AuditPass { offline }));
            passes.push(Box::new(machete::MachetePass));
        }

        let orchestrator = scanner::ScanOrchestrator::new(passes);
        let pass_result = orchestrator.run(scan_root, resolved, suppress_spinner);

        all_diagnostics.extend(pass_result.diagnostics);
        all_skipped_passes.extend(pass_result.skipped_passes);
        total_elapsed += pass_result.elapsed;
    }

    // Deduplicate diagnostics from overlapping workspace scans.
    // When a workspace root is also a member (e.g., members = [".", "sub"]),
    // clippy at the root compiles sub as a dependency and lints it, then
    // clippy runs again on sub's own Cargo.toml producing duplicates.
    //
    // The key includes `message` to avoid collapsing genuinely different
    // diagnostics that share the same position (e.g., two unused deps in
    // the same Cargo.toml both have rule="unused-dependency", line=None).
    all_diagnostics.sort_by(|a, b| {
        a.file_path
            .cmp(&b.file_path)
            .then(a.rule.cmp(&b.rule))
            .then(a.line.cmp(&b.line))
            .then(a.column.cmp(&b.column))
            .then(a.message.cmp(&b.message))
    });
    all_diagnostics.dedup_by(|a, b| {
        a.file_path == b.file_path
            && a.rule == b.rule
            && a.line == b.line
            && a.column == b.column
            && a.message == b.message
    });

    // In diff mode, filter to changed files
    if let Some(ref ctx) = diff_context {
        all_diagnostics = diff::filter_to_changed_files(all_diagnostics, &ctx.changed_files);
    }

    // Apply inline suppression comments
    let (all_diagnostics, suppressed_count) =
        suppression::apply_inline_suppressions(all_diagnostics, &project_info.root_dir);
    if resolved.verbose && suppressed_count > 0 {
        eprintln!("Suppressed {suppressed_count} diagnostic(s) via inline comments");
    }

    // Calculate final score
    let error_count = all_diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let warning_count = all_diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .count();
    let (score, score_label) = output::calculate_score(&all_diagnostics);

    // Deduplicate skipped passes
    all_skipped_passes.sort();
    all_skipped_passes.dedup();

    Ok(ScanResult {
        diagnostics: all_diagnostics,
        score,
        score_label,
        source_file_count: total_source_files,
        elapsed: total_elapsed,
        skipped_passes: all_skipped_passes,
        error_count,
        warning_count,
    })
}
