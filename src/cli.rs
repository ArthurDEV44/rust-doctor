use clap::{Parser, ValueEnum};
use std::path::PathBuf;

/// Diagnose your Rust project's health with a single command.
///
/// rust-doctor scans Rust codebases for security, performance, correctness,
/// architecture, and dependency issues, producing a 0-100 health score
/// with actionable diagnostics.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Directory to scan (defaults to current directory)
    #[arg(default_value = ".")]
    pub directory: PathBuf,

    /// Show detailed file:line information per diagnostic
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Print only the bare integer score (for CI piping)
    #[arg(long, conflicts_with = "json")]
    pub score: bool,

    /// Output full scan results as JSON
    #[arg(long, conflicts_with = "score")]
    pub json: bool,

    /// Scan only changed files vs a base branch
    #[arg(long, num_args = 0..=1, default_missing_value = "auto", value_name = "BASE")]
    pub diff: Option<String>,

    /// Exit with code 1 when this severity is reached
    #[arg(long, value_enum)]
    pub fail_on: Option<FailOn>,

    /// Skip network-dependent checks (cargo-audit advisory DB fetch, etc.)
    #[arg(long)]
    pub offline: bool,

    /// Run as an MCP (Model Context Protocol) stdio server for AI tool integration
    #[arg(long, conflicts_with_all = ["score", "json"])]
    pub mcp: bool,

    /// Skip all interactive prompts (auto-yes)
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    /// Scan only specific workspace members (comma-separated)
    #[arg(long, value_delimiter = ',', value_name = "NAMES", value_parser = parse_non_empty)]
    pub project: Vec<String>,
}

/// Reject empty project name segments (e.g. `--project ,api` or `--project core,`)
fn parse_non_empty(s: &str) -> Result<String, String> {
    if s.is_empty() {
        Err("project name cannot be empty".to_string())
    } else {
        Ok(s.to_string())
    }
}

/// When to exit with a non-zero status code
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailOn {
    /// Exit 1 if any errors found
    Error,
    /// Exit 1 if any errors or warnings found
    Warning,
    /// Always exit 0 (unless rust-doctor itself crashes)
    None,
}

impl std::fmt::Display for FailOn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
            Self::None => write!(f, "none"),
        }
    }
}

/// Environment variables that indicate an automated/CI environment.
/// When any of these are set, interactive prompts are skipped.
#[allow(dead_code)] // Used by should_skip_prompts — reserved for US-017 interactive mode
const AUTOMATED_ENV_VARS: &[&str] = &["CI", "CLAUDECODE", "CURSOR_AGENT", "CODEX_CI"];

/// Returns `true` if running in a CI or automated agent environment.
#[allow(dead_code)] // Reserved for US-017 interactive mode
pub fn is_automated_environment() -> bool {
    check_automated_env(|var| std::env::var_os(var).is_some())
}

/// Testable CI detection — accepts a lookup function to avoid env mutation in tests.
#[allow(dead_code)] // Used by tests and is_automated_environment
fn check_automated_env(lookup: impl Fn(&str) -> bool) -> bool {
    AUTOMATED_ENV_VARS.iter().any(|var| lookup(var))
}

/// Returns `true` if interactive prompts should be skipped.
///
/// Prompts are skipped when:
/// - `--yes` / `-y` flag is passed
/// - A known CI/agent env var is set
/// - stdin is not a TTY (e.g., piped input)
#[allow(dead_code)] // Reserved for US-017 interactive mode
pub fn should_skip_prompts(cli: &Cli) -> bool {
    cli.yes || is_automated_environment() || !std::io::IsTerminal::is_terminal(&std::io::stdin())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_default_directory() {
        let cli = Cli::try_parse_from(["rust-doctor"]).unwrap();
        assert_eq!(cli.directory, PathBuf::from("."));
    }

    #[test]
    fn test_custom_directory() {
        let cli = Cli::try_parse_from(["rust-doctor", "/some/path"]).unwrap();
        assert_eq!(cli.directory, PathBuf::from("/some/path"));
    }

    #[test]
    fn test_score_flag() {
        let cli = Cli::try_parse_from(["rust-doctor", "--score"]).unwrap();
        assert!(cli.score);
    }

    #[test]
    fn test_json_flag() {
        let cli = Cli::try_parse_from(["rust-doctor", "--json"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn test_score_and_json_conflict() {
        let result = Cli::try_parse_from(["rust-doctor", "--score", "--json"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_verbose_flag() {
        let cli = Cli::try_parse_from(["rust-doctor", "--verbose"]).unwrap();
        assert!(cli.verbose);
    }

    #[test]
    fn test_yes_short_flag() {
        let cli = Cli::try_parse_from(["rust-doctor", "-y"]).unwrap();
        assert!(cli.yes);
    }

    #[test]
    fn test_yes_long_flag() {
        let cli = Cli::try_parse_from(["rust-doctor", "--yes"]).unwrap();
        assert!(cli.yes);
    }

    #[test]
    fn test_offline_flag() {
        let cli = Cli::try_parse_from(["rust-doctor", "--offline"]).unwrap();
        assert!(cli.offline);
    }

    #[test]
    fn test_fail_on_default() {
        let cli = Cli::try_parse_from(["rust-doctor"]).unwrap();
        assert_eq!(cli.fail_on, Option::None);
    }

    #[test]
    fn test_fail_on_error() {
        let cli = Cli::try_parse_from(["rust-doctor", "--fail-on", "error"]).unwrap();
        assert_eq!(cli.fail_on, Some(FailOn::Error));
    }

    #[test]
    fn test_fail_on_warning() {
        let cli = Cli::try_parse_from(["rust-doctor", "--fail-on", "warning"]).unwrap();
        assert_eq!(cli.fail_on, Some(FailOn::Warning));
    }

    #[test]
    fn test_fail_on_none() {
        let cli = Cli::try_parse_from(["rust-doctor", "--fail-on", "none"]).unwrap();
        assert_eq!(cli.fail_on, Some(FailOn::None));
    }

    #[test]
    fn test_fail_on_invalid() {
        let result = Cli::try_parse_from(["rust-doctor", "--fail-on", "critical"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_diff_without_value() {
        let cli = Cli::try_parse_from(["rust-doctor", "--diff"]).unwrap();
        assert_eq!(cli.diff, Some("auto".to_string()));
    }

    #[test]
    fn test_diff_with_value() {
        let cli = Cli::try_parse_from(["rust-doctor", "--diff", "main"]).unwrap();
        assert_eq!(cli.diff, Some("main".to_string()));
    }

    #[test]
    fn test_diff_absent() {
        let cli = Cli::try_parse_from(["rust-doctor"]).unwrap();
        assert_eq!(cli.diff, Option::None);
    }

    #[test]
    fn test_project_single() {
        let cli = Cli::try_parse_from(["rust-doctor", "--project", "core"]).unwrap();
        assert_eq!(cli.project, vec!["core"]);
    }

    #[test]
    fn test_project_comma_separated() {
        let cli = Cli::try_parse_from(["rust-doctor", "--project", "core,api,web"]).unwrap();
        assert_eq!(cli.project, vec!["core", "api", "web"]);
    }

    #[test]
    fn test_project_empty_by_default() {
        let cli = Cli::try_parse_from(["rust-doctor"]).unwrap();
        assert!(cli.project.is_empty());
    }

    #[test]
    fn test_project_rejects_empty_name() {
        let result = Cli::try_parse_from(["rust-doctor", "--project", ",api"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_version_flag() {
        let result = Cli::try_parse_from(["rust-doctor", "--version"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn test_help_flag() {
        let result = Cli::try_parse_from(["rust-doctor", "--help"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn test_all_flags_combined() {
        let cli = Cli::try_parse_from([
            "rust-doctor",
            "/my/project",
            "--verbose",
            "--score",
            "--diff",
            "develop",
            "--fail-on",
            "warning",
            "--offline",
            "-y",
            "--project",
            "core,api",
        ])
        .unwrap();

        assert_eq!(cli.directory, PathBuf::from("/my/project"));
        assert!(cli.verbose);
        assert!(cli.score);
        assert!(!cli.json);
        assert_eq!(cli.diff, Some("develop".to_string()));
        assert_eq!(cli.fail_on, Some(FailOn::Warning));
        assert!(cli.offline);
        assert!(cli.yes);
        assert_eq!(cli.project, vec!["core", "api"]);
    }

    // CI detection tests use check_automated_env with injected lookup
    // to avoid unsafe env mutation and test races.

    #[test]
    fn test_ci_detection_with_ci_var() {
        assert!(check_automated_env(|var| var == "CI"));
    }

    #[test]
    fn test_ci_detection_with_claudecode_var() {
        assert!(check_automated_env(|var| var == "CLAUDECODE"));
    }

    #[test]
    fn test_ci_detection_with_cursor_agent_var() {
        assert!(check_automated_env(|var| var == "CURSOR_AGENT"));
    }

    #[test]
    fn test_ci_detection_with_codex_ci_var() {
        assert!(check_automated_env(|var| var == "CODEX_CI"));
    }

    #[test]
    fn test_ci_detection_no_vars_set() {
        assert!(!check_automated_env(|_| false));
    }
}
