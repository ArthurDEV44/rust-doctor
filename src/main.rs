mod audit;
mod cli;
mod clippy;
mod config;
mod diagnostics;
mod diff;
mod discovery;
mod machete;
mod mcp;
mod output;
mod rules;
mod scan;
mod scanner;
mod suppression;
mod workspace;

use clap::Parser;
use cli::Cli;
use std::process;

fn main() {
    let cli = Cli::parse();

    // MCP mode: run as a stdio MCP server for AI tool integration
    if cli.mcp {
        mcp::run_mcp_server();
        return;
    }

    // Resolve the target directory to an absolute path
    let target_dir = match cli.directory.canonicalize() {
        Ok(path) => path,
        Err(e) => {
            eprintln!(
                "Error: cannot access directory '{}': {e}",
                cli.directory.display()
            );
            process::exit(1);
        }
    };

    // Check that a Cargo.toml exists in the target directory
    let cargo_toml = target_dir.join("Cargo.toml");
    let cargo_toml_exists = cargo_toml.try_exists().unwrap_or(false);
    if !cargo_toml_exists {
        eprintln!(
            "Error: no Cargo.toml found in '{}'\n\n\
             rust-doctor must be run in a Rust project directory.\n\
             Either pass a directory containing a Cargo.toml, or run from a project root:\n\n\
             \x20 rust-doctor /path/to/project\n\
             \x20 cd /path/to/project && rust-doctor",
            target_dir.display()
        );
        process::exit(1);
    }

    // Discover project characteristics
    let project_info = match discovery::discover_project(&cargo_toml, cli.offline) {
        Ok(info) => info,
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    };

    // Load configuration
    let file_config =
        config::load_file_config(&project_info.root_dir, Some(&project_info.package_metadata));
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
        output::render_json(&scan_result);
    } else {
        output::render_terminal(&scan_result, resolved.verbose);
    }

    // Exit code
    let fail_on = resolved.fail_on;
    let should_fail = match fail_on {
        cli::FailOn::Error => scan_result.error_count > 0,
        cli::FailOn::Warning => scan_result.error_count > 0 || scan_result.warning_count > 0,
        cli::FailOn::None => false,
    };
    if should_fail {
        process::exit(1);
    }
}
