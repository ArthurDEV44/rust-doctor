use crate::diagnostics::{Diagnostic, ScanResult};
use crate::{config, discovery};
use rmcp::ErrorData as McpError;
use std::collections::HashMap;

use super::types::{DiagnosticExample, DiagnosticGroup, MAX_EXAMPLES_PER_GROUP};

// ---------------------------------------------------------------------------
// Directory scope validation (fail-closed)
// ---------------------------------------------------------------------------

/// Pick the allowed scope root by precedence, without I/O: `RUST_DOCTOR_MCP_ROOT`
/// first (for containers/CI), then `$HOME`. Empty values are ignored. Returns
/// `None` when neither is available — the caller MUST then reject (fail-closed).
fn pick_scope_root(mcp_root: Option<&str>, home: Option<&str>) -> Option<String> {
    mcp_root
        .filter(|s| !s.is_empty())
        .or_else(|| home.filter(|s| !s.is_empty()))
        .map(str::to_string)
}

/// Pure scope check: the canonical target must live under the allowed root.
/// Both arguments must already be canonicalized (no I/O here → unit-testable).
fn is_within_scope(canonical_target: &std::path::Path, allowed_root: &std::path::Path) -> bool {
    canonical_target.starts_with(allowed_root)
}

// ---------------------------------------------------------------------------
// Project discovery helper
// ---------------------------------------------------------------------------

/// Discover project + load file config + resolve with defaults.
/// Validates that the directory is within the allowed scope (RUST_DOCTOR_MCP_ROOT
/// or $HOME) to prevent scanning arbitrary paths; fails closed if neither is set.
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
    // Validate directory scope (fail closed). The target is canonicalized first to
    // defeat `../` traversal and TOCTOU, then checked against an allowed root.
    let canonical = std::path::Path::new(directory)
        .canonicalize()
        .map_err(|_| {
            McpError::invalid_params("directory path is invalid or does not exist", None)
        })?;

    // Precedence: RUST_DOCTOR_MCP_ROOT (container/CI override) > $HOME. If neither
    // is set, REJECT — never fall open to scanning arbitrary paths like /etc, /proc.
    let mcp_root = std::env::var("RUST_DOCTOR_MCP_ROOT").ok();
    let home = std::env::var("HOME").ok();
    let allowed_root = pick_scope_root(mcp_root.as_deref(), home.as_deref()).ok_or_else(|| {
        McpError::invalid_params(
            "directory scope cannot be validated — set RUST_DOCTOR_MCP_ROOT to the allowed root \
             (no $HOME present)",
            None,
        )
    })?;
    let allowed_canonical = std::path::Path::new(&allowed_root)
        .canonicalize()
        .map_err(|_| {
            McpError::internal_error(
                "scope root path is invalid; cannot validate directory scope",
                None,
            )
        })?;
    if !is_within_scope(&canonical, &allowed_canonical) {
        // Do not echo the raw path back to the client.
        return Err(McpError::invalid_params(
            "directory is outside the allowed scope",
            None,
        ));
    }

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
        .filter_map(|(rule, diags)| {
            let first = diags.first()?;
            let examples: Vec<DiagnosticExample> = diags
                .iter()
                .take(MAX_EXAMPLES_PER_GROUP)
                .map(|d| DiagnosticExample {
                    file_path: d.file_path.display().to_string(),
                    line: d.line,
                    column: d.column,
                })
                .collect();

            Some(DiagnosticGroup {
                rule: rule.to_string(),
                severity: first.severity.to_string(),
                category: first.category.to_string(),
                count: diags.len(),
                message: first.message.clone(),
                help: diags.iter().find_map(|d| d.help.as_ref()).cloned(),
                examples,
            })
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

#[cfg(test)]
mod tests {
    use super::{is_within_scope, pick_scope_root};
    use std::path::Path;

    // --- US-006: directory scope precedence (RUST_DOCTOR_MCP_ROOT > $HOME) ---

    #[test]
    fn test_pick_scope_root_prefers_mcp_root() {
        assert_eq!(
            pick_scope_root(Some("/srv/work"), Some("/home/user")),
            Some("/srv/work".to_string()),
        );
    }

    #[test]
    fn test_pick_scope_root_falls_back_to_home() {
        assert_eq!(
            pick_scope_root(None, Some("/home/user")),
            Some("/home/user".to_string()),
        );
    }

    #[test]
    fn test_pick_scope_root_ignores_empty() {
        // Empty env values must not be treated as a valid root.
        assert_eq!(
            pick_scope_root(Some(""), Some("/home/user")),
            Some("/home/user".to_string()),
        );
        assert_eq!(pick_scope_root(Some(""), Some("")), None);
    }

    #[test]
    fn test_pick_scope_root_fail_closed_when_neither_set() {
        // Neither RUST_DOCTOR_MCP_ROOT nor $HOME → None → caller rejects (fail-closed).
        assert_eq!(pick_scope_root(None, None), None);
    }

    #[test]
    fn test_is_within_scope() {
        assert!(is_within_scope(
            Path::new("/home/user/project"),
            Path::new("/home/user")
        ));
        assert!(is_within_scope(
            Path::new("/home/user"),
            Path::new("/home/user")
        ));
        // Outside the allowed root is rejected.
        assert!(!is_within_scope(Path::new("/etc"), Path::new("/home/user")));
        assert!(!is_within_scope(
            Path::new("/home/userother"),
            Path::new("/home/user/")
        ));
    }
}
