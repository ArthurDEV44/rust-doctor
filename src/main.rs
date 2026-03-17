use clap::Parser;
use rust_doctor::cli::{Cli, FailOn};
use rust_doctor::{config, discovery, mcp, output, scan};
use std::process;

fn main() {
    let cli = Cli::parse();

    // MCP mode: run as a stdio MCP server for AI tool integration
    if cli.mcp {
        mcp::run_mcp_server();
        return;
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

    // Merge CLI flags with file config
    let resolved = config::resolve_config(&cli, file_config.as_ref());

    // Run scan
    let suppress_spinner = cli.score || cli.json;
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

    // Output
    if cli.score {
        output::render_score(&scan_result);
    } else if cli.json {
        if let Err(e) = output::render_json(&scan_result) {
            eprintln!("Error: failed to serialize scan results: {e}");
            process::exit(1);
        }
    } else {
        output::render_terminal(&scan_result, resolved.verbose);
    }

    // Exit code
    let fail_on = resolved.fail_on;
    let should_fail = match fail_on {
        FailOn::Error => scan_result.error_count > 0,
        FailOn::Warning => scan_result.error_count > 0 || scan_result.warning_count > 0,
        FailOn::None => false,
    };
    if should_fail {
        process::exit(1);
    }
}
