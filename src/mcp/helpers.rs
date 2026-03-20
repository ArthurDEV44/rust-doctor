use crate::diagnostics::{Diagnostic, ScanResult};
use crate::{config, discovery};
use rmcp::ErrorData as McpError;
use std::collections::HashMap;

use super::types::{DiagnosticExample, DiagnosticGroup, MAX_EXAMPLES_PER_GROUP};

// ---------------------------------------------------------------------------
// Project discovery helper
// ---------------------------------------------------------------------------

/// Discover project + load file config + resolve with defaults.
/// Validates that the directory is under `$HOME` to prevent scanning arbitrary paths.
pub(super) fn discover_and_resolve(
    directory: &str,
    ignore_project_config: bool,
) -> Result<
    (
        std::path::PathBuf,
        discovery::ProjectInfo,
        config::ResolvedConfig,
    ),
    McpError,
> {
    // Validate directory scope: must be under $HOME (fail closed)
    let canonical = std::path::Path::new(directory)
        .canonicalize()
        .map_err(|_| {
            McpError::invalid_params("directory path is invalid or does not exist", None)
        })?;

    if let Ok(home) = std::env::var("HOME") {
        let home_canonical = std::path::Path::new(&home).canonicalize().map_err(|_| {
            McpError::internal_error(
                "$HOME path is invalid; cannot validate directory scope",
                None,
            )
        })?;
        if !canonical.starts_with(&home_canonical) {
            return Err(McpError::invalid_params(
                "directory must be under $HOME",
                None,
            ));
        }
    }
    // If $HOME is not set (e.g. containers): allow — no scope to validate against

    // Pass the already-canonicalized path to avoid TOCTOU between validation and use
    let (target_dir, project_info, file_config) = discovery::bootstrap_project(&canonical, false)
        .map_err(|e| {
        // Sanitize: return a hint but NOT the raw error text (which may contain paths)
        let hint = match &e {
            crate::error::BootstrapError::InvalidDirectory { .. } => {
                "invalid directory — use an absolute path like /home/user/project"
            }
            crate::error::BootstrapError::NoCargo { .. } => {
                "no Cargo.toml found — ensure the directory contains a Cargo.toml"
            }
            crate::error::BootstrapError::Discovery(_) => {
                "project discovery failed — check that `cargo metadata` runs successfully"
            }
        };
        eprintln!("MCP bootstrap error: {e}");
        McpError::invalid_params(hint.to_string(), None)
    })?;

    let effective_config = if ignore_project_config {
        None
    } else {
        // Warn if security rules are suppressed by project config
        if let Some(ref fc) = file_config {
            let security_rules = [
                "hardcoded-secrets",
                "sql-injection-risk",
                "unsafe-block-audit",
            ];
            for rule in &fc.ignore.rules {
                if security_rules.contains(&rule.as_str()) {
                    eprintln!("Warning: project config suppresses security rule '{rule}'");
                }
            }
        }
        file_config.as_ref()
    };
    let resolved = config::resolve_config_defaults(effective_config);

    Ok((target_dir, project_info, resolved))
}

// ---------------------------------------------------------------------------
// Diagnostic grouping (MCP output compression)
// ---------------------------------------------------------------------------

/// Group individual diagnostics by rule, sorted by severity then count.
/// Reduces thousands of findings to ~70 compact groups.
pub(super) fn group_diagnostics(diagnostics: &[Diagnostic]) -> Vec<DiagnosticGroup> {
    let mut groups: HashMap<&str, Vec<&Diagnostic>> = HashMap::new();
    for diag in diagnostics {
        groups.entry(&diag.rule).or_default().push(diag);
    }

    let mut result: Vec<DiagnosticGroup> = groups
        .into_iter()
        .map(|(rule, diags)| {
            let first = diags[0];
            let examples: Vec<DiagnosticExample> = diags
                .iter()
                .take(MAX_EXAMPLES_PER_GROUP)
                .map(|d| DiagnosticExample {
                    file_path: d.file_path.display().to_string(),
                    line: d.line,
                    column: d.column,
                })
                .collect();

            DiagnosticGroup {
                rule: rule.to_string(),
                severity: first.severity.to_string(),
                category: first.category.to_string(),
                count: diags.len(),
                message: first.message.clone(),
                help: diags.iter().find_map(|d| d.help.as_ref()).cloned(),
                examples,
            }
        })
        .collect();

    // Sort: errors first, then warnings, then info; within severity by count descending
    result.sort_by(|a, b| {
        let severity_ord = |s: &str| -> u8 {
            match s {
                "error" => 0,
                "warning" => 1,
                _ => 2,
            }
        };
        severity_ord(&a.severity)
            .cmp(&severity_ord(&b.severity))
            .then(b.count.cmp(&a.count))
    });

    result
}

/// Generate a complete markdown report of scan results.
/// This is the sole output of the scan tool — no JSON, pure readable text.
pub(super) fn format_scan_report(result: &ScanResult, groups: &[DiagnosticGroup]) -> String {
    use std::fmt::Write;

    let mut s = String::with_capacity(8192);

    // Header
    let _ = writeln!(
        s,
        "## {}/100 ({}) — {} files in {:.1}s",
        result.score,
        result.score_label,
        result.source_file_count,
        result.elapsed.as_secs_f64()
    );
    let _ = writeln!(
        s,
        "{} errors | {} warnings | {} info | {} rules triggered\n",
        result.error_count,
        result.warning_count,
        result.info_count,
        groups.len()
    );

    // Dimension scores
    let _ = writeln!(s, "### Dimensions");
    let d = &result.dimension_scores;
    let _ = writeln!(
        s,
        "Security: {} | Reliability: {} | Performance: {} | Dependencies: {} | Maintainability: {}\n",
        d.security, d.reliability, d.performance, d.dependencies, d.maintainability
    );

    // All diagnostics grouped by severity
    for &(severity_label, severity_filter) in &[
        ("Errors", "error"),
        ("Warnings", "warning"),
        ("Info", "info"),
    ] {
        let severity_groups: Vec<&DiagnosticGroup> = groups
            .iter()
            .filter(|g| g.severity == severity_filter)
            .collect();

        if severity_groups.is_empty() {
            continue;
        }

        let total_count: usize = severity_groups.iter().map(|g| g.count).sum();
        let _ = writeln!(
            s,
            "### {} ({} rules, {} findings)",
            severity_label,
            severity_groups.len(),
            total_count
        );

        for g in &severity_groups {
            // Rule line: name (category) x count -- message
            let _ = writeln!(
                s,
                "- `{}` ({}) \u{00d7}{} \u{2014} {}",
                g.rule, g.category, g.count, g.message
            );

            // Example locations on next line
            let locations: Vec<String> = g
                .examples
                .iter()
                .map(|ex| {
                    ex.line.map_or_else(
                        || ex.file_path.clone(),
                        |line| format!("{}:{line}", ex.file_path),
                    )
                })
                .collect();
            if !locations.is_empty() {
                let _ = writeln!(s, "  \u{2192} {}", locations.join(", "));
            }

            // Help text if available
            if let Some(ref help) = g.help {
                let _ = writeln!(s, "  fix: {help}");
            }
        }
        s.push('\n');
    }

    // Skipped passes
    if !result.skipped_passes.is_empty() {
        let _ = writeln!(s, "### Skipped");
        for pass in &result.skipped_passes {
            let _ = writeln!(s, "- {pass}");
        }
    }

    s
}
