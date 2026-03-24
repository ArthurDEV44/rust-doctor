//! Detect installed AI coding agents by probing filesystem paths.

use std::path::{Path, PathBuf};

/// An AI coding agent detected on this system.
pub struct DetectedAgent {
    /// Human-readable name (e.g., "Claude Code")
    pub name: &'static str,
    /// Short description
    pub description: &'static str,
    /// Path to the MCP configuration file
    pub mcp_config_path: PathBuf,
    /// Path to the skills directory (None if agent doesn't support skills)
    pub skills_dir: Option<PathBuf>,
    /// Whether rust-doctor MCP is already configured
    pub mcp_already_configured: bool,
    /// Whether the rust-doctor skill is already installed
    pub skill_already_installed: bool,
}

/// Internal definition of a supported agent.
struct AgentDef {
    name: &'static str,
    description: &'static str,
    /// Paths to probe for detection (relative to home dir)
    probe_paths: &'static [&'static str],
    /// MCP config file path (relative to home dir)
    mcp_config_rel: &'static str,
    /// Skills directory (relative to home dir), if supported
    skills_dir_rel: Option<&'static str>,
}

const AGENTS: &[AgentDef] = &[
    AgentDef {
        name: "Claude Code",
        description: "Anthropic's CLI for Claude",
        probe_paths: &[".claude"],
        mcp_config_rel: ".claude.json",
        skills_dir_rel: Some(".claude/skills"),
    },
    AgentDef {
        name: "Cursor",
        description: "AI-first code editor",
        probe_paths: &[".cursor"],
        mcp_config_rel: ".cursor/mcp.json",
        skills_dir_rel: Some(".cursor/skills"),
    },
    AgentDef {
        name: "Windsurf",
        description: "AI-powered IDE by Codeium",
        probe_paths: &[".codeium/windsurf", ".windsurf"],
        mcp_config_rel: ".codeium/windsurf/mcp_config.json",
        skills_dir_rel: None,
    },
];

/// Detect which AI coding agents are installed on this system.
pub fn detect_agents() -> Vec<DetectedAgent> {
    let Some(home) = home_dir() else {
        return Vec::new();
    };

    AGENTS.iter().filter_map(|def| probe(def, &home)).collect()
}

fn probe(def: &AgentDef, home: &Path) -> Option<DetectedAgent> {
    let detected = def.probe_paths.iter().any(|p| home.join(p).exists());
    if !detected {
        return None;
    }

    let mcp_config_path = home.join(def.mcp_config_rel);
    let skills_dir = def.skills_dir_rel.map(|p| home.join(p));

    let mcp_already_configured = is_mcp_configured(&mcp_config_path);
    let skill_already_installed = skills_dir
        .as_ref()
        .is_some_and(|dir| dir.join("rust-doctor").join("SKILL.md").exists());

    Some(DetectedAgent {
        name: def.name,
        description: def.description,
        mcp_config_path,
        skills_dir,
        mcp_already_configured,
        skill_already_installed,
    })
}

/// Check if `rust-doctor` is already present in an MCP config file.
fn is_mcp_configured(path: &Path) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return false;
    };
    json.get("mcpServers")
        .and_then(|servers| servers.get("rust-doctor"))
        .is_some()
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agents_list_is_not_empty() {
        assert!(!AGENTS.is_empty());
    }

    #[test]
    fn all_agents_have_mcp_config_path() {
        for def in AGENTS {
            assert!(
                !def.mcp_config_rel.is_empty(),
                "{} missing mcp_config_rel",
                def.name
            );
        }
    }

    #[test]
    fn is_mcp_configured_returns_false_for_missing_file() {
        assert!(!is_mcp_configured(Path::new("/nonexistent/path.json")));
    }
}
