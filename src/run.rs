//! Helper functions extracted from `main.rs` for the scan pipeline orchestration.
//!
//! These functions handle MCP dispatch, project bootstrapping, scanning,
//! output rendering, and quality gate checks.

use std::process::ExitCode;

use crate::cli::{Cli, FailOn};

/// Exit code for scan errors (project doesn't compile, discovery fails).
pub const EXIT_SCAN_ERROR: u8 = 2;
/// Exit code for quality gate failures (score below threshold, --fail-on).
pub const EXIT_GATE_FAILURE: u8 = 3;
use crate::diagnostics::ScanResult;
use crate::{config, deps, discovery, fixer, output, plan, sarif, scan};

/// Run the interactive setup wizard. Returns exit code.
pub fn handle_setup() -> ExitCode {
    match crate::setup::run_setup() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Dispatch `--mcp` flag: start the MCP server or report a compile-time error.
/// Returns `Some(ExitCode)` if MCP was handled, `None` to continue normal flow.
pub fn handle_mcp_flag(cli: &Cli) -> Option<ExitCode> {
    #[cfg(feature = "mcp")]
    if cli.mcp {
        return Some(match crate::mcp::run_mcp_server() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("Error: MCP server failed: {e}");
                ExitCode::FAILURE
            }
        });
    }

    #[cfg(not(feature = "mcp"))]
    if cli.mcp {
        eprintln!("Error: MCP support not compiled in. Rebuild with `--features mcp`.");
        return Some(ExitCode::FAILURE);
    }

    None
}

/// Resolve directory, discover the project, and load file-based configuration.
pub fn bootstrap_project(
    cli: &Cli,
) -> Result<
    (
        std::path::PathBuf,
        discovery::ProjectInfo,
        Option<config::FileConfig>,
    ),
    crate::error::BootstrapError,
> {
    discovery::bootstrap_project(&cli.directory, cli.offline)
}

/// Run the scan passes and return the result.
pub fn run_scan(
    cli: &Cli,
    project_info: &discovery::ProjectInfo,
    resolved: &config::ResolvedConfig,
) -> Result<ScanResult, crate::error::ScanError> {
    let suppress_spinner = cli.score || cli.json || cli.sarif;
    scan::scan_project(
        project_info,
        resolved,
        cli.offline,
        &cli.project,
        suppress_spinner,
    )
}

/// Render the appropriate output format (score, JSON, SARIF, or terminal).
pub fn emit_output(
    cli: &Cli,
    scan_result: &ScanResult,
    resolved: &config::ResolvedConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    if cli.score {
        output::render_score(scan_result);
    } else if cli.json {
        output::render_json(scan_result)?;
    } else if cli.sarif {
        let sarif_json = sarif::render_sarif(scan_result)?;
        println!("{sarif_json}");
    } else {
        output::render_terminal(scan_result, resolved.verbose);
    }
    Ok(())
}

/// Returns `Some(ExitCode)` with `EXIT_GATE_FAILURE` if the score is below the configured threshold.
pub fn check_score_gate(scan_result: &ScanResult, threshold: Option<u32>) -> Option<ExitCode> {
    if let Some(threshold) = threshold {
        if scan_result.score < threshold {
            eprintln!(
                "Score {} is below the configured threshold of {}",
                scan_result.score, threshold
            );
            return Some(ExitCode::from(EXIT_GATE_FAILURE));
        }
    }
    None
}

/// Check and install missing external tools. Returns appropriate exit code.
pub fn handle_install_deps() -> ExitCode {
    deps::print_status();
    let all_ok = deps::install_missing_tools();
    if all_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Apply auto-fixes if `--fix` was requested.
pub fn apply_fixes_if_requested(cli: &Cli, scan_result: &ScanResult) {
    if cli.fix {
        let applied = fixer::apply_fixes(&scan_result.diagnostics, &cli.directory);
        if applied > 0 {
            eprintln!("Applied {applied} fix(es).");
        } else {
            eprintln!("No machine-applicable fixes available.");
        }
    }
}

/// Show remediation plan if `--plan` was requested.
pub fn emit_plan_if_requested(cli: &Cli, scan_result: &ScanResult) {
    if cli.plan {
        let items = plan::generate_plan(scan_result);
        let plan_text = plan::format_plan_markdown(&items, scan_result);
        eprintln!("\n{plan_text}");
    }
}

/// Returns `Some(ExitCode)` with `EXIT_GATE_FAILURE` if any diagnostic exceeds the `--fail-on` severity level.
pub fn check_fail_on_gate(scan_result: &ScanResult, fail_on: FailOn) -> Option<ExitCode> {
    let should_fail = match fail_on {
        FailOn::Error => scan_result.error_count > 0,
        FailOn::Warning => scan_result.error_count > 0 || scan_result.warning_count > 0,
        FailOn::Info => {
            scan_result.error_count > 0
                || scan_result.warning_count > 0
                || scan_result.info_count > 0
        }
        FailOn::None => false,
    };
    if should_fail {
        Some(ExitCode::from(EXIT_GATE_FAILURE))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{DimensionScores, ScoreLabel};
    use std::time::Duration;

    fn make_scan_result(score: u32, errors: usize, warnings: usize, infos: usize) -> ScanResult {
        ScanResult {
            diagnostics: vec![],
            score,
            score_label: ScoreLabel::Great,
            dimension_scores: DimensionScores {
                security: 100,
                reliability: 100,
                maintainability: 100,
                performance: 100,
                dependencies: 100,
            },
            source_file_count: 10,
            elapsed: Duration::from_secs(1),
            skipped_passes: vec![],
            error_count: errors,
            warning_count: warnings,
            info_count: infos,
            pass_timings: vec![],
        }
    }

    // --- check_score_gate ---

    #[test]
    fn test_score_gate_below_threshold_fails() {
        let result = make_scan_result(75, 0, 0, 0);
        assert!(check_score_gate(&result, Some(80)).is_some());
    }

    #[test]
    fn test_score_gate_above_threshold_passes() {
        let result = make_scan_result(85, 0, 0, 0);
        assert!(check_score_gate(&result, Some(80)).is_none());
    }

    #[test]
    fn test_score_gate_exact_threshold_passes() {
        let result = make_scan_result(80, 0, 0, 0);
        assert!(check_score_gate(&result, Some(80)).is_none());
    }

    #[test]
    fn test_score_gate_no_threshold_passes() {
        let result = make_scan_result(10, 0, 0, 0);
        assert!(check_score_gate(&result, None).is_none());
    }

    // --- check_fail_on_gate ---

    #[test]
    fn test_fail_on_error_with_errors() {
        let result = make_scan_result(50, 1, 0, 0);
        assert!(check_fail_on_gate(&result, FailOn::Error).is_some());
    }

    #[test]
    fn test_fail_on_error_without_errors() {
        let result = make_scan_result(50, 0, 5, 3);
        assert!(check_fail_on_gate(&result, FailOn::Error).is_none());
    }

    #[test]
    fn test_fail_on_warning_with_warnings() {
        let result = make_scan_result(50, 0, 1, 0);
        assert!(check_fail_on_gate(&result, FailOn::Warning).is_some());
    }

    #[test]
    fn test_fail_on_warning_with_errors_too() {
        let result = make_scan_result(50, 1, 0, 0);
        assert!(check_fail_on_gate(&result, FailOn::Warning).is_some());
    }

    #[test]
    fn test_fail_on_info_with_info() {
        let result = make_scan_result(50, 0, 0, 1);
        assert!(check_fail_on_gate(&result, FailOn::Info).is_some());
    }

    #[test]
    fn test_fail_on_none_never_fails() {
        let result = make_scan_result(50, 10, 20, 30);
        assert!(check_fail_on_gate(&result, FailOn::None).is_none());
    }
}
