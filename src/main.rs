mod audit;
mod cli;
mod clippy;
mod config;
mod diagnostics;
mod diff;
mod discovery;
mod machete;
mod output;
mod rules;
mod scanner;
mod workspace;

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

    // Load configuration
    let file_config =
        config::load_file_config(&project_info.root_dir, Some(&project_info.package_metadata));
    let resolved = config::resolve_config(&cli, file_config.as_ref());

    // Validate ignored rule names
    let known_rules = clippy::known_lint_names();
    config::validate_ignored_rules(&resolved.ignore_rules, &known_rules);

    // Resolve workspace members (US-017)
    let scan_roots: Vec<std::path::PathBuf> = if project_info.is_workspace {
        match workspace::resolve_members(&project_info.workspace_members, &cli.project) {
            Ok(members) => {
                if resolved.verbose {
                    eprintln!(
                        "Workspace: scanning {} of {} members",
                        members.len(),
                        project_info.member_count
                    );
                }
                members.iter().map(|m| m.root_dir.clone()).collect()
            }
            Err(e) => {
                eprintln!("Error: {e}");
                process::exit(1);
            }
        }
    } else {
        vec![project_info.root_dir.clone()]
    };

    // Resolve diff mode (US-016)
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

    if resolved.verbose {
        eprintln!(
            "Project: {} v{} (edition {})",
            project_info.name, project_info.version, project_info.edition
        );
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

    // Build custom rules
    let mut custom_rules: Vec<Box<dyn rules::CustomRule>> = Vec::new();
    custom_rules.extend(rules::error_handling::all_rules());
    custom_rules.extend(rules::performance::all_rules());
    custom_rules.extend(rules::security::all_rules());

    let has_async_runtime = project_info.frameworks.iter().any(|f| {
        matches!(
            f,
            discovery::Framework::Tokio
                | discovery::Framework::AsyncStd
                | discovery::Framework::Smol
        )
    });
    if has_async_runtime {
        custom_rules.extend(rules::async_rules::all_rules());
    }
    custom_rules.extend(rules::framework::rules_for_frameworks(
        &project_info.frameworks,
    ));

    // Scan each root (single project or workspace members)
    let suppress_spinner = cli.score || cli.json;
    let mut all_diagnostics = Vec::new();
    let mut total_source_files = 0;
    let mut all_skipped_passes = Vec::new();
    let mut total_elapsed = std::time::Duration::ZERO;

    for scan_root in &scan_roots {
        let source_file_count = if let Some(ref ctx) = diff_context {
            ctx.changed_files.len()
        } else {
            scanner::count_source_files(scan_root)
        };
        total_source_files += source_file_count;

        // Build passes per scan root
        let mut passes: Vec<Box<dyn scanner::AnalysisPass>> = vec![
            Box::new(clippy::ClippyPass),
            Box::new(rules::RuleEnginePass::new(
                // Clone rules for each scan root — rules are stateless so this is safe
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
            )),
        ];
        if !is_diff_mode {
            passes.push(Box::new(audit::AuditPass));
            passes.push(Box::new(machete::MachetePass));
        }

        let orchestrator = scanner::ScanOrchestrator::new(passes);
        let result = orchestrator.run(scan_root, &resolved, source_file_count, suppress_spinner);

        all_diagnostics.extend(result.diagnostics);
        all_skipped_passes.extend(result.skipped_passes);
        total_elapsed += result.elapsed;
    }

    // In diff mode, filter to changed files
    if let Some(ref ctx) = diff_context {
        all_diagnostics = diff::filter_to_changed_files(all_diagnostics, &ctx.changed_files);
    }

    // Calculate final score
    let error_count = all_diagnostics
        .iter()
        .filter(|d| d.severity == diagnostics::Severity::Error)
        .count();
    let warning_count = all_diagnostics
        .iter()
        .filter(|d| d.severity == diagnostics::Severity::Warning)
        .count();
    let (score, score_label) = output::calculate_score(&all_diagnostics);

    // Deduplicate skipped passes
    all_skipped_passes.sort();
    all_skipped_passes.dedup();

    let scan_result = diagnostics::ScanResult {
        diagnostics: all_diagnostics,
        score,
        score_label,
        source_file_count: total_source_files,
        elapsed: total_elapsed,
        skipped_passes: all_skipped_passes,
        error_count,
        warning_count,
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
