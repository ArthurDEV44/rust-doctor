#![forbid(unsafe_code)]

use clap::Parser;
use rust_doctor::cli::{Cli, FailOn};
use rust_doctor::diagnostics::ScanResult;
use rust_doctor::{config, discovery, fixer, output, plan, sarif, scan};
use std::process;

fn main() {
    let cli = Cli::parse();

    // MCP mode: run as a stdio MCP server for AI tool integration
    handle_mcp_flag(&cli);

    // Bootstrap: resolve directory, discover project, load file config
    let (_target_dir, project_info, file_config) = bootstrap_project(&cli);

    // Merge CLI flags with file config (skip project config if --no-project-config)
    let effective_config = if cli.no_project_config {
        None
    } else {
        file_config.as_ref()
    };
    let resolved = config::resolve_config(&cli, effective_config);

    // Run scan
    let scan_result = run_scan(&cli, &project_info, &resolved);

    // Apply fixes if requested
    if cli.fix {
        let applied = fixer::apply_fixes(&scan_result.diagnostics, &cli.directory);
        if applied > 0 {
            eprintln!("Applied {applied} fix(es).");
        } else {
            eprintln!("No machine-applicable fixes available.");
        }
    }

    // Output
    emit_output(&cli, &scan_result, &resolved);

    // Show remediation plan if requested
    if cli.plan {
        let items = plan::generate_plan(&scan_result);
        let plan_text = plan::format_plan_markdown(&items, &scan_result);
        eprintln!("\n{plan_text}");
    }

    // Quality gates
    check_score_gate(&scan_result, resolved.score_fail_below);
    check_fail_on_gate(&scan_result, resolved.fail_on);
}

/// Dispatch `--mcp` flag: start the MCP server or report a compile-time error.
fn handle_mcp_flag(cli: &Cli) {
    #[cfg(feature = "mcp")]
    if cli.mcp {
        if let Err(e) = rust_doctor::mcp::run_mcp_server() {
            eprintln!("Error: MCP server failed: {e}");
            process::exit(1);
        }
        process::exit(0);
    }

    #[cfg(not(feature = "mcp"))]
    if cli.mcp {
        eprintln!("Error: MCP support not compiled in. Rebuild with `--features mcp`.");
        process::exit(1);
    }
}

/// Resolve directory, discover the project, and load file-based configuration.
fn bootstrap_project(
    cli: &Cli,
) -> (
    std::path::PathBuf,
    discovery::ProjectInfo,
    Option<config::FileConfig>,
) {
    match discovery::bootstrap_project(&cli.directory, cli.offline) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    }
}

/// Run the scan passes and return the result, or exit on failure.
fn run_scan(
    cli: &Cli,
    project_info: &discovery::ProjectInfo,
    resolved: &config::ResolvedConfig,
) -> ScanResult {
    let suppress_spinner = cli.score || cli.json || cli.sarif;
    match scan::scan_project(
        project_info,
        resolved,
        cli.offline,
        &cli.project,
        suppress_spinner,
    ) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    }
}

/// Render the appropriate output format (score, JSON, SARIF, or terminal).
fn emit_output(cli: &Cli, scan_result: &ScanResult, resolved: &config::ResolvedConfig) {
    if cli.score {
        output::render_score(scan_result);
    } else if cli.json {
        if let Err(e) = output::render_json(scan_result) {
            eprintln!("Error: failed to serialize scan results: {e}");
            process::exit(1);
        }
    } else if cli.sarif {
        match sarif::render_sarif(scan_result) {
            Ok(sarif_json) => println!("{sarif_json}"),
            Err(e) => {
                eprintln!("Error: failed to serialize SARIF output: {e}");
                process::exit(1);
            }
        }
    } else {
        output::render_terminal(scan_result, resolved.verbose);
    }
}

/// Exit with code 1 if the score is below the configured threshold.
fn check_score_gate(scan_result: &ScanResult, threshold: Option<u32>) {
    if let Some(threshold) = threshold {
        if scan_result.score < threshold {
            eprintln!(
                "Score {} is below the configured threshold of {}",
                scan_result.score, threshold
            );
            process::exit(1);
        }
    }
}

/// Exit with code 1 if any diagnostic exceeds the `--fail-on` severity level.
fn check_fail_on_gate(scan_result: &ScanResult, fail_on: FailOn) {
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
        process::exit(1);
    }
}
