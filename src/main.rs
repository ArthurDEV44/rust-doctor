mod cli;
mod config;
mod diagnostics;
mod discovery;
mod output;
mod scanner;

use clap::Parser;
use cli::Cli;
use std::process;

fn main() {
    let cli = Cli::parse();

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

    let _skip_prompts = cli::should_skip_prompts(&cli);

    // Discover project characteristics
    let project_info = match discovery::discover_project(&cargo_toml, cli.offline) {
        Ok(info) => info,
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    };

    // Load configuration (rust-doctor.toml > [package.metadata.rust-doctor] > defaults)
    let file_config =
        config::load_file_config(&project_info.root_dir, Some(&project_info.package_metadata));

    // Merge CLI flags with file config
    let resolved = config::resolve_config(&cli, file_config.as_ref());

    // Validate ignored rule names against known rules (registry grows in US-008+)
    let known_rules: &[&str] = &[];
    config::validate_ignored_rules(&resolved.ignore_rules, known_rules);

    if resolved.verbose {
        eprintln!(
            "Project: {} v{} (edition {})",
            project_info.name, project_info.version, project_info.edition
        );
        if project_info.is_workspace {
            eprintln!("Workspace: {} members", project_info.member_count);
        }
        if !project_info.frameworks.is_empty() {
            let fw_list: Vec<String> = project_info
                .frameworks
                .iter()
                .map(|f| f.to_string())
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

    // Count source files
    let source_file_count = scanner::count_source_files(&project_info.root_dir);

    // Build analysis passes
    let passes: Vec<Box<dyn scanner::AnalysisPass>> = vec![
        Box::new(scanner::ClippyPass),
        Box::new(scanner::CustomRulesPass),
        Box::new(scanner::DependencyPass),
    ];

    // Run scan orchestrator
    let suppress_spinner = cli.score || cli.json;
    let orchestrator = scanner::ScanOrchestrator::new(passes);
    let scan_result = orchestrator.run(
        &project_info.root_dir,
        &resolved,
        source_file_count,
        suppress_spinner,
    );

    // Output results based on mode
    if cli.score {
        output::render_score(&scan_result);
    } else if cli.json {
        output::render_json(&scan_result);
    } else {
        output::render_terminal(&scan_result, resolved.verbose);
    }

    // Exit code based on fail_on config
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
