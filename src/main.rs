mod cli;
mod config;
mod discovery;

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
        if !resolved.ignore_rules.is_empty() {
            eprintln!("Ignoring rules: {}", resolved.ignore_rules.join(", "));
        }
        if !resolved.ignore_files.is_empty() {
            eprintln!("Ignoring files: {}", resolved.ignore_files.join(", "));
        }
        eprintln!("Fail on: {}", resolved.fail_on);
    }

    // Placeholder: future stories will add scan orchestration here
    println!(
        "rust-doctor: scanning '{}'...",
        project_info.root_dir.display()
    );
}
