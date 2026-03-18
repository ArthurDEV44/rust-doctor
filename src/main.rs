#![forbid(unsafe_code)]

use clap::Parser;
use rust_doctor::cli::{Cli, FailOn};
use rust_doctor::{config, discovery, fixer, output, plan, sarif, scan};
use std::process;

fn main() {
    let cli = Cli::parse();

    // MCP mode: run as a stdio MCP server for AI tool integration
    #[cfg(feature = "mcp")]
    if cli.mcp {
        if let Err(e) = rust_doctor::mcp::run_mcp_server() {
            eprintln!("Error: MCP server failed: {e}");
            process::exit(1);
        }
        return;
    }

    #[cfg(not(feature = "mcp"))]
    if cli.mcp {
        eprintln!("Error: MCP support not compiled in. Rebuild with `--features mcp`.");
        process::exit(1);
    }

    // Bootstrap: resolve directory, discover project, load file config
    let (_target_dir, project_info, file_config) =
        match discovery::bootstrap_project(&cli.directory, cli.offline) {
            Ok(result) => result,
            Err(e) => {
                eprintln!("Error: {e}");
                process::exit(1);
            }
        };

    // Merge CLI flags with file config (skip project config if --no-project-config)
    let effective_config = if cli.no_project_config {
        None
    } else {
        file_config.as_ref()
    };
    let resolved = config::resolve_config(&cli, effective_config);

    // Run scan
    let suppress_spinner = cli.score || cli.json || cli.sarif;
    let scan_result = match scan::scan_project(
        &project_info,
        &resolved,
        cli.offline,
        &cli.project,
        suppress_spinner,
    ) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    };

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
    if cli.score {
        output::render_score(&scan_result);
    } else if cli.json {
        if let Err(e) = output::render_json(&scan_result) {
            eprintln!("Error: failed to serialize scan results: {e}");
            process::exit(1);
        }
    } else if cli.sarif {
        match sarif::render_sarif(&scan_result) {
            Ok(sarif_json) => println!("{sarif_json}"),
            Err(e) => {
                eprintln!("Error: failed to serialize SARIF output: {e}");
                process::exit(1);
            }
        }
    } else {
        output::render_terminal(&scan_result, resolved.verbose);
    }

    // Show remediation plan if requested
    if cli.plan {
        let items = plan::generate_plan(&scan_result);
        let plan_text = plan::format_plan_markdown(&items, &scan_result);
        eprintln!("\n{plan_text}");
    }

    // Score quality gate
    if let Some(threshold) = resolved.score_fail_below {
        if scan_result.score < threshold {
            eprintln!(
                "Score {} is below the configured threshold of {}",
                scan_result.score, threshold
            );
            process::exit(1);
        }
    }

    // Exit code
    let fail_on = resolved.fail_on;
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
