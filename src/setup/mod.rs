//! Interactive setup wizard for configuring rust-doctor with AI coding agents.
//!
//! Supports two installation modes:
//! - **MCP Server**: configures the `rust-doctor --mcp` stdio server in each agent's config
//! - **CLI + Skills**: installs a `SKILL.md` that teaches the agent to use the CLI

mod detect;
mod mcp_config;
mod skill;

use crate::error::SetupError;
use detect::DetectedAgent;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, MultiSelect, Select};
use owo_colors::{OwoColorize, Stream};
use std::io::IsTerminal;
use std::process::{Command, Stdio};

/// Installation mode selected by the user.
enum Mode {
    /// Configure the MCP stdio server in the agent's config file.
    Mcp,
    /// Install a SKILL.md that teaches the agent to use the CLI.
    CliSkills,
}

/// A file that was successfully installed.
struct InstalledFile {
    path: String,
    kind: &'static str,
}

/// Run the interactive setup wizard.
///
/// # Errors
///
/// Returns an error if interactive prompts fail (e.g., stdin is not a TTY)
/// or if file I/O fails during installation.
pub fn run_setup() -> Result<(), SetupError> {
    if !std::io::stderr().is_terminal() {
        return Err(SetupError::NotInteractive(
            "`rust-doctor setup` requires an interactive terminal.\n\
             Hint: run this command directly in your shell, not via a script or pipe."
                .to_string(),
        ));
    }

    print_banner();

    // Step 1: Choose installation mode
    let mode = select_mode()?;

    // Step 2: Detect installed agents
    let agents = detect::detect_agents();
    if agents.is_empty() {
        eprintln!(
            "\n{}",
            "No supported AI agents detected on this system."
                .if_supports_color(Stream::Stderr, |t| t.yellow())
        );
        eprintln!("Supported agents: Claude Code, Cursor, Windsurf");
        eprintln!("Install one of these and run `rust-doctor setup` again.");
        return Ok(());
    }

    eprintln!(
        "\n  Detected {} agent(s):\n",
        agents.len().if_supports_color(Stream::Stderr, |t| t.bold())
    );
    for agent in &agents {
        let status = if agent.mcp_already_configured {
            " (MCP already configured)"
        } else {
            ""
        };
        eprintln!(
            "    {} {} — {}{}",
            "✓".if_supports_color(Stream::Stderr, |t| t.green()),
            agent.name.if_supports_color(Stream::Stderr, |t| t.bold()),
            agent
                .description
                .if_supports_color(Stream::Stderr, |t| t.dimmed()),
            status.if_supports_color(Stream::Stderr, |t| t.dimmed()),
        );
    }
    eprintln!();

    // Step 3: Select which agents to configure
    let selected = select_agents(&agents, &mode)?;
    if selected.is_empty() {
        eprintln!("No agents selected. Exiting.");
        return Ok(());
    }

    // Step 4: Confirm
    let agent_names: Vec<&str> = selected.iter().map(|a| a.name).collect();
    let mode_label = match mode {
        Mode::Mcp => "MCP server",
        Mode::CliSkills => "CLI + Skills",
    };
    eprintln!(
        "\n  Will install {} for: {}",
        mode_label.if_supports_color(Stream::Stderr, |t| t.bold()),
        agent_names
            .join(", ")
            .if_supports_color(Stream::Stderr, |t| t.cyan())
    );

    if !Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("  Proceed?")
        .default(true)
        .interact()?
    {
        eprintln!("Cancelled.");
        return Ok(());
    }

    eprintln!();

    // Step 5: Install
    let installed = match mode {
        Mode::Mcp => install_mcp(&selected),
        Mode::CliSkills => install_skills(&selected),
    };

    // Step 6: Recap
    print_recap(&installed, &mode);

    Ok(())
}

fn print_banner() {
    eprintln!(
        "\n  {}",
        "rust-doctor setup".if_supports_color(Stream::Stderr, |t| t.bold())
    );
    eprintln!(
        "  {}",
        "Configure rust-doctor for your AI coding agent"
            .if_supports_color(Stream::Stderr, |t| t.dimmed())
    );
}

fn select_mode() -> Result<Mode, dialoguer::Error> {
    let items = &[
        "CLI + Skills \u{2014} Installs a skill file that guides your agent to use the CLI (recommended)",
        "MCP Server \u{2014} Agent calls rust-doctor tools via MCP protocol",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("  How should your agent access rust-doctor?")
        .items(items)
        .default(0)
        .interact()?;

    Ok(if selection == 0 {
        Mode::CliSkills
    } else {
        Mode::Mcp
    })
}

fn select_agents<'a>(
    agents: &'a [DetectedAgent],
    mode: &Mode,
) -> Result<Vec<&'a DetectedAgent>, dialoguer::Error> {
    // If only one agent detected, skip the selection
    if agents.len() == 1 {
        return Ok(agents.iter().collect());
    }

    // Ask: all agents or pick specific ones?
    let scope_items = &[
        format!("All detected agents ({})", agents.len()),
        "Select specific agents...".to_string(),
    ];

    let scope = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("  Install for which agents?")
        .items(scope_items)
        .default(0)
        .interact()?;

    if scope == 0 {
        return Ok(agents.iter().collect());
    }

    // Specific selection: space to toggle, enter to confirm
    let labels: Vec<String> = agents
        .iter()
        .map(|a| {
            let status = match mode {
                Mode::Mcp if a.mcp_already_configured => " (will overwrite)",
                Mode::CliSkills if a.skill_already_installed => " (will overwrite)",
                _ => "",
            };
            format!("{} \u{2014} {}{status}", a.name, a.description)
        })
        .collect();

    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("  Select agents (space to toggle, enter to confirm)")
        .items(&labels)
        .interact()?;

    Ok(selections
        .into_iter()
        .filter_map(|i| agents.get(i))
        .collect())
}

fn install_mcp(agents: &[&DetectedAgent]) -> Vec<InstalledFile> {
    let (cmd, args) = detect_command();
    let mut installed = Vec::new();

    for agent in agents {
        if agent.mcp_already_configured {
            eprintln!(
                "  {} already has rust-doctor MCP configured.",
                agent.name.if_supports_color(Stream::Stderr, |t| t.cyan())
            );
            let replace = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("  Replace with new configuration? (recommended)")
                .default(true)
                .interact()
                .unwrap_or(false);
            if !replace {
                eprintln!("  Skipped.");
                continue;
            }
        }

        eprint!(
            "  Configuring {} ... ",
            agent.name.if_supports_color(Stream::Stderr, |t| t.cyan())
        );

        match mcp_config::write_mcp_config(&agent.mcp_config_path, &cmd, &args) {
            Ok(()) => {
                eprintln!(
                    "{}",
                    "done".if_supports_color(Stream::Stderr, |t| t.green())
                );
                installed.push(InstalledFile {
                    path: agent.mcp_config_path.display().to_string(),
                    kind: "MCP config",
                });
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    format!("failed: {e}").if_supports_color(Stream::Stderr, |t| t.red())
                );
            }
        }
    }

    installed
}

fn install_skills(agents: &[&DetectedAgent]) -> Vec<InstalledFile> {
    let mut installed = Vec::new();

    for agent in agents {
        let Some(ref skills_dir) = agent.skills_dir else {
            eprintln!(
                "  {} \u{2014} {}",
                agent.name.if_supports_color(Stream::Stderr, |t| t.cyan()),
                "no skills support, skipping".if_supports_color(Stream::Stderr, |t| t.yellow())
            );
            continue;
        };

        if agent.skill_already_installed {
            eprintln!(
                "  {} already has the rust-doctor skill installed.",
                agent.name.if_supports_color(Stream::Stderr, |t| t.cyan())
            );
            let replace = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("  Replace with latest version? (recommended)")
                .default(true)
                .interact()
                .unwrap_or(false);
            if !replace {
                eprintln!("  Skipped.");
                continue;
            }
        }

        eprint!(
            "  Installing skill for {} ... ",
            agent.name.if_supports_color(Stream::Stderr, |t| t.cyan())
        );

        match skill::write_skill(skills_dir) {
            Ok(path) => {
                eprintln!(
                    "{}",
                    "done".if_supports_color(Stream::Stderr, |t| t.green())
                );
                installed.push(InstalledFile {
                    path: path.display().to_string(),
                    kind: "Skill file",
                });
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    format!("failed: {e}").if_supports_color(Stream::Stderr, |t| t.red())
                );
            }
        }
    }

    installed
}

/// Detect whether `rust-doctor` is directly available in PATH,
/// or whether we should use `npx` as a fallback for the MCP command.
fn detect_command() -> (String, Vec<String>) {
    let is_available = Command::new("rust-doctor")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());

    if is_available {
        ("rust-doctor".into(), vec!["--mcp".into()])
    } else {
        (
            "npx".into(),
            vec!["-y".into(), "rust-doctor@latest".into(), "--mcp".into()],
        )
    }
}

fn print_recap(installed: &[InstalledFile], mode: &Mode) {
    eprintln!();

    if installed.is_empty() {
        eprintln!(
            "  {}",
            "No files were installed.".if_supports_color(Stream::Stderr, |t| t.yellow())
        );
        return;
    }

    eprintln!(
        "  {}",
        "Setup complete!".if_supports_color(Stream::Stderr, |t| t.green())
    );
    eprintln!();
    eprintln!("  Installed files:");
    for file in installed {
        eprintln!(
            "    {} {} ({})",
            "\u{2713}".if_supports_color(Stream::Stderr, |t| t.green()),
            file.path.if_supports_color(Stream::Stderr, |t| t.dimmed()),
            file.kind,
        );
    }

    eprintln!();
    match mode {
        Mode::Mcp => {
            eprintln!("  Restart your AI agent to activate the MCP server.");
            eprintln!(
                "  The agent will have access to: {}, {}, {}, {}",
                "scan".if_supports_color(Stream::Stderr, |t| t.bold()),
                "score".if_supports_color(Stream::Stderr, |t| t.bold()),
                "explain_rule".if_supports_color(Stream::Stderr, |t| t.bold()),
                "list_rules".if_supports_color(Stream::Stderr, |t| t.bold()),
            );
        }
        Mode::CliSkills => {
            eprintln!("  Your agent can now use rust-doctor via CLI commands.");
            eprintln!(
                "  Try asking: {}",
                "\"Run rust-doctor on this project\""
                    .if_supports_color(Stream::Stderr, |t| t.dimmed())
            );
        }
    }
    eprintln!();
}
