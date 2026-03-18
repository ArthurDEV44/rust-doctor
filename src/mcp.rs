use crate::diagnostics::{Diagnostic, DimensionScores, ScoreLabel};
use crate::{clippy, config, discovery, rules, scan};
use rmcp::handler::server::router::prompt::PromptRouter;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{
    AnnotateAble, CallToolResult, Content, GetPromptRequestParams, GetPromptResult,
    ListPromptsResult, ListResourcesResult, PaginatedRequestParams, PromptMessage,
    PromptMessageRole, RawResource, ReadResourceRequestParams, ReadResourceResult, Resource,
    ResourceContents, ServerCapabilities, ServerInfo,
};
use rmcp::service::{RequestContext, ServiceExt};
use rmcp::{
    ErrorData as McpError, RoleServer, prompt, prompt_handler, prompt_router, tool, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// MCP server struct
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct RustDoctorServer {
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

// ---------------------------------------------------------------------------
// Input schemas (schemars-derived)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct ScanInput {
    /// Absolute path to the Rust project directory (must contain a Cargo.toml).
    #[schemars(
        description = "Absolute path to the Rust project directory to analyze. Must contain a Cargo.toml file."
    )]
    pub directory: String,
    /// Only scan files changed vs this base branch (e.g. "main"). Omit to scan all files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        description = "Git branch name to diff against (e.g. 'main', 'develop'). When set, only files changed vs this branch are scanned. Omit to scan all files."
    )]
    pub diff: Option<String>,
    /// Run in offline mode (no network fetches). Defaults to true in MCP mode for security.
    #[serde(default = "default_mcp_offline")]
    #[schemars(
        description = "When true, cargo-audit runs with --no-fetch (no network access). Defaults to true in MCP mode for security."
    )]
    pub offline: bool,
    /// Ignore the project's rust-doctor.toml config file.
    #[serde(default)]
    #[schemars(
        description = "When true, ignores the project's rust-doctor.toml config file. Useful for untrusted projects."
    )]
    pub ignore_project_config: bool,
}

const fn default_mcp_offline() -> bool {
    true
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct ScoreInput {
    /// Absolute path to the Rust project directory.
    #[schemars(
        description = "Absolute path to the Rust project directory to score. Must contain a Cargo.toml file."
    )]
    pub directory: String,
    /// Run in offline mode (no network fetches). Defaults to true in MCP mode for security.
    #[serde(default = "default_mcp_offline")]
    #[schemars(
        description = "When true, cargo-audit runs with --no-fetch (no network access). Defaults to true in MCP mode."
    )]
    pub offline: bool,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct ExplainRuleInput {
    /// The rule ID (e.g. "unwrap-in-production", "clippy::expect_used", "blocking-in-async").
    #[schemars(
        description = "Rule identifier to explain. Accepts custom rule IDs (e.g. 'unwrap-in-production') or clippy lint names (e.g. 'clippy::expect_used'). Use list_rules to discover available IDs."
    )]
    pub rule: String,
}

// ---------------------------------------------------------------------------
// Output schemas (schemars-derived for MCP structured output)
// ---------------------------------------------------------------------------

/// Structured output for the scan tool.
#[derive(Serialize, JsonSchema)]
pub struct ScanOutput {
    /// All diagnostic findings.
    pub diagnostics: Vec<Diagnostic>,
    /// Health score from 0 (critical) to 100 (perfect).
    pub score: u32,
    /// Human-readable score label.
    pub score_label: ScoreLabel,
    /// Per-dimension health scores.
    pub dimension_scores: DimensionScores,
    /// Number of source files that were analyzed.
    pub source_file_count: usize,
    /// Total scan duration in seconds.
    pub elapsed_secs: f64,
    /// Analysis passes that were skipped or failed.
    pub skipped_passes: Vec<String>,
    /// Total number of error-severity findings.
    pub error_count: usize,
    /// Total number of warning-severity findings.
    pub warning_count: usize,
    /// Total number of info-severity findings.
    pub info_count: usize,
}

/// Structured output for the score tool.
#[derive(Serialize, JsonSchema)]
pub struct ScoreOutput {
    /// Health score from 0 (critical) to 100 (perfect).
    pub score: u32,
    /// Human-readable score label.
    pub score_label: ScoreLabel,
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

#[tool_router]
#[prompt_router]
impl RustDoctorServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            prompt_router: Self::prompt_router(),
        }
    }

    #[tool(
        name = "scan",
        description = "Run a full Rust code health analysis on a project directory. \
Use this tool when you need detailed diagnostics — it returns all findings with file:line precision. \
Takes 5-30 seconds depending on project size. \
Returns JSON with: diagnostics array (each has rule, severity, message, file_path, line, column, help), \
score (0-100), score_label, source_file_count, elapsed_secs, error_count, warning_count, info_count, skipped_passes. \
Severity levels: error (bugs/security), warning (code smells), info (suggestions). \
Runs 4 passes in parallel: clippy (55+ lints), 18 custom AST rules, cargo-audit (CVEs), cargo-machete (unused deps). \
Set 'diff' to a branch name to only scan changed files. \
After scanning, use explain_rule on any rule ID to get fix guidance.",
        annotations(read_only_hint = true)
    )]
    async fn scan(
        &self,
        meta: rmcp::model::Meta,
        client: rmcp::Peer<RoleServer>,
        params: Parameters<ScanInput>,
    ) -> Result<Json<ScanOutput>, McpError> {
        let input = params.0;
        let progress_token = meta.get_progress_token();

        // Send start progress if client supports it
        if let Some(ref token) = progress_token {
            let _ = client
                .notify_progress(rmcp::model::ProgressNotificationParam {
                    progress_token: token.clone(),
                    progress: 0.0,
                    total: Some(2.0),
                    message: Some("Bootstrapping project...".to_string()),
                })
                .await;
        }

        let (_dir, project_info, mut resolved) =
            discover_and_resolve(&input.directory, input.ignore_project_config)?;

        if let Some(diff_base) = input.diff {
            resolved.diff = Some(diff_base);
        }

        // Send scanning progress
        if let Some(ref token) = progress_token {
            let _ = client
                .notify_progress(rmcp::model::ProgressNotificationParam {
                    progress_token: token.clone(),
                    progress: 1.0,
                    total: Some(2.0),
                    message: Some(
                        "Running analysis passes (clippy, rules, audit, machete)...".to_string(),
                    ),
                })
                .await;
        }

        // Run the CPU-bound scan on a blocking thread to avoid starving the tokio runtime
        let offline = input.offline;
        let result = tokio::task::spawn_blocking(move || {
            scan::scan_project(&project_info, &resolved, offline, &[], true)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("scan task failed: {e}"), None))?
        .map_err(|e| {
            eprintln!("MCP scan error: {e}");
            McpError::internal_error(
                "scan failed — check project compiles with `cargo check`",
                None,
            )
        })?;

        // Send completion progress
        if let Some(ref token) = progress_token {
            let _ = client
                .notify_progress(rmcp::model::ProgressNotificationParam {
                    progress_token: token.clone(),
                    progress: 2.0,
                    total: Some(2.0),
                    message: Some(format!(
                        "Scan complete: score {}/100, {} findings",
                        result.score,
                        result.diagnostics.len()
                    )),
                })
                .await;
        }

        Ok(Json(ScanOutput {
            diagnostics: result.diagnostics,
            score: result.score,
            score_label: result.score_label,
            dimension_scores: result.dimension_scores,
            source_file_count: result.source_file_count,
            elapsed_secs: result.elapsed.as_secs_f64(),
            skipped_passes: result.skipped_passes,
            error_count: result.error_count,
            warning_count: result.warning_count,
            info_count: result.info_count,
        }))
    }

    #[tool(
        name = "score",
        description = "Get just the health score of a Rust project (0-100 integer). \
Use this tool for a quick pass/fail check without full diagnostics. \
IMPORTANT: runs the same full analysis as scan internally, so takes the same 5-30 seconds. \
Score thresholds: >=75 'Great', >=50 'Needs work', <50 'Critical'. \
Scoring: each unique error-severity rule violated costs 1.5 points, each warning costs 0.75 points. \
If you also need the diagnostics, use scan instead — it includes the score too.",
        annotations(read_only_hint = true)
    )]
    async fn score(&self, params: Parameters<ScoreInput>) -> Result<Json<ScoreOutput>, McpError> {
        let input = params.0;
        let (_dir, project_info, resolved) = discover_and_resolve(&input.directory, false)?;

        // Run the CPU-bound scan on a blocking thread to avoid starving the tokio runtime
        let offline = input.offline;
        let result = tokio::task::spawn_blocking(move || {
            scan::scan_project(&project_info, &resolved, offline, &[], true)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("scan task failed: {e}"), None))?
        .map_err(|e| {
            eprintln!("MCP score error: {e}");
            McpError::internal_error(
                "scan failed — check project compiles with `cargo check`",
                None,
            )
        })?;

        Ok(Json(ScoreOutput {
            score: result.score,
            score_label: result.score_label,
        }))
    }

    #[tool(
        name = "explain_rule",
        description = "Get a detailed markdown explanation of a specific rust-doctor rule. \
Use this after scan to understand what a rule detects and how to fix violations. \
Returns: rule name, category, severity, description, and fix guidance. \
Accepts custom rule IDs (e.g. 'unwrap-in-production') and clippy lint names (e.g. 'clippy::expect_used'). \
Instant response — no project scanning required. \
For unknown rules, returns guidance to use list_rules.",
        annotations(read_only_hint = true)
    )]
    async fn explain_rule(
        &self,
        params: Parameters<ExplainRuleInput>,
    ) -> Result<CallToolResult, McpError> {
        let explanation = get_rule_explanation(&params.0.rule);
        Ok(CallToolResult::success(vec![Content::text(explanation)]))
    }

    #[tool(
        name = "list_rules",
        description = "List all available rust-doctor rules as formatted markdown. \
Use this to discover which checks exist before scanning, or to find a rule ID for explain_rule. \
Instant response — no project scanning required. \
Returns: 18 custom AST rules (grouped by Error Handling, Performance, Security, Async, Framework), \
55+ clippy lints with custom severity overrides, and 2 external tools (cargo-audit, cargo-machete). \
Each entry shows rule ID, severity, and one-line summary.",
        annotations(read_only_hint = true)
    )]
    async fn list_rules(&self) -> Result<CallToolResult, McpError> {
        let listing = get_all_rules_listing();
        Ok(CallToolResult::success(vec![Content::text(listing)]))
    }

    // ── Prompts ──────────────────────────────────────────────────────────

    #[prompt(
        name = "health-check",
        description = "Run a full health check on a Rust project: scan, generate a prioritized \
remediation plan, and optionally apply fixes. Combines scan + plan + fix into one structured workflow."
    )]
    async fn health_check(&self, params: Parameters<HealthCheckArgs>) -> GetPromptResult {
        GetPromptResult::new(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            format!(
                r#"Run a comprehensive health audit on the Rust project at '{directory}'.

## Phase 1: Scan
Use the `scan` tool to get all diagnostics and the health score.

## Phase 2: Remediation Plan
From the scan results, generate a prioritized remediation plan:
- **P0 Critical**: Security vulnerabilities, correctness bugs → fix immediately
- **P1 High**: Error handling issues, dependency problems → fix before release
- **P2 Medium**: Performance issues, architecture smells → plan for next sprint
- **P3 Low**: Style, info-level suggestions → nice-to-have

For each item, show:
1. Rule name and occurrence count
2. Affected files
3. Concrete fix action (use `explain_rule` for detailed guidance)
4. Estimated effort (trivial / small / medium / large)

## Phase 3: Confirmation
Present the full plan as a structured task list and ask:
"Do you want me to proceed with fixing these issues? I'll work through them by priority, starting with P0."

## Phase 4: Execution (if confirmed)
If the user confirms:
1. Use task tracking (plan mode) to organize the work
2. Start with P0 items, then P1, P2, P3
3. For each item:
   - Read the affected files
   - Apply the fix following the `explain_rule` guidance
   - Verify the fix compiles (`cargo check`)
4. After all fixes, re-run `scan` to verify the score improved
5. Commit the changes with a conventional commit message

If the user declines or wants partial fixes, respect their choice and only fix the items they approve."#,
                directory = params.0.directory
            ),
        )])
        .with_description("Full health audit with prioritized remediation plan and structured fix workflow")
    }
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct HealthCheckArgs {
    /// Absolute path to the Rust project directory.
    #[schemars(description = "Absolute path to the Rust project directory to check.")]
    pub directory: String,
}

// ---------------------------------------------------------------------------
// ServerHandler implementation
// ---------------------------------------------------------------------------

#[rmcp::tool_handler]
#[prompt_handler(router = self.prompt_router)]
impl rmcp::handler::server::ServerHandler for RustDoctorServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        .with_instructions(
            "rust-doctor is a Rust code health scanner. It analyzes projects for security, \
             performance, correctness, architecture, and dependency issues.\n\n\
             ## Recommended workflow\n\
             1. `scan` a project directory → get diagnostics + score (5-30s)\n\
             2. `explain_rule` for any rule you want to understand → instant\n\
             3. `list_rules` to browse all available checks → instant\n\
             4. `score` for a quick pass/fail without diagnostics (same 5-30s as scan)\n\n\
             ## Resources\n\
             - `rule://` — read rule documentation by URI (e.g. `rule://unwrap-in-production`)\n\n\
             ## Prompts\n\
             - `health-check` — full health check workflow with fix guidance\n\n\
             ## Tips\n\
             - Prefer `scan` over `score` — it includes the score plus full diagnostics\n\
             - Use `diff` parameter in scan to focus on changed files only\n\
             - All tools are read-only and safe to call repeatedly\n\
             - `explain_rule` and `list_rules` are instant (no project scanning)",
        )
    }

    async fn list_resources(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let docs = rule_docs();
        let resources: Vec<Resource> = docs
            .iter()
            .map(|doc| {
                RawResource::new(format!("rule://{}", doc.name), doc.name)
                    .with_description(format!("[{}] {}", doc.severity, doc.description))
                    .with_mime_type("text/markdown")
                    .no_annotation()
            })
            .collect();

        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let uri = request.uri.as_str();
        let rule_name = uri.strip_prefix("rule://").ok_or_else(|| {
            McpError::invalid_params(
                format!("Unknown URI scheme: {uri}. Expected rule://{{rule-name}}"),
                None,
            )
        })?;

        let explanation = get_rule_explanation(rule_name);
        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(explanation, uri).with_mime_type("text/markdown"),
        ]))
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Typed error enum for MCP server failures — replaces `Box<dyn Error>` so
/// callers can match on specific failure modes.
#[derive(Debug, thiserror::Error)]
pub enum McpServerError {
    #[error("failed to create tokio runtime: {0}")]
    RuntimeCreation(#[from] std::io::Error),

    #[error("MCP server initialization failed: {0}")]
    Initialize(#[from] Box<rmcp::service::ServerInitializeError>),

    #[error("MCP server task failed: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),
}

/// Run the MCP server over stdio. Called from main when `--mcp` is passed.
///
/// # Errors
///
/// Returns an error if the tokio runtime cannot be created, the MCP transport
/// fails to initialize, or the server encounters a fatal error.
pub fn run_mcp_server() -> Result<(), McpServerError> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let server = RustDoctorServer::new();
        let transport = rmcp::transport::io::stdio();
        let service = server.serve(transport).await.map_err(Box::new)?;
        service.waiting().await?;
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Discover project + load file config + resolve with defaults.
/// Validates that the directory is under `$HOME` to prevent scanning arbitrary paths.
fn discover_and_resolve(
    directory: &str,
    ignore_project_config: bool,
) -> Result<
    (
        std::path::PathBuf,
        discovery::ProjectInfo,
        config::ResolvedConfig,
    ),
    McpError,
> {
    // Validate directory scope: must be under $HOME (fail closed)
    let canonical = std::path::Path::new(directory)
        .canonicalize()
        .map_err(|_| {
            McpError::invalid_params("directory path is invalid or does not exist", None)
        })?;

    if let Ok(home) = std::env::var("HOME") {
        let home_canonical = std::path::Path::new(&home).canonicalize().map_err(|_| {
            McpError::internal_error(
                "$HOME path is invalid; cannot validate directory scope",
                None,
            )
        })?;
        if !canonical.starts_with(&home_canonical) {
            return Err(McpError::invalid_params(
                "directory must be under $HOME",
                None,
            ));
        }
    }
    // If $HOME is not set (e.g. containers): allow — no scope to validate against

    // Pass the already-canonicalized path to avoid TOCTOU between validation and use
    let (target_dir, project_info, file_config) = discovery::bootstrap_project(&canonical, false)
        .map_err(|e| {
        // Sanitize: return a hint but NOT the raw error text (which may contain paths)
        let hint = match &e {
            crate::error::BootstrapError::InvalidDirectory { .. } => {
                "invalid directory — use an absolute path like /home/user/project"
            }
            crate::error::BootstrapError::NoCargo { .. } => {
                "no Cargo.toml found — ensure the directory contains a Cargo.toml"
            }
            crate::error::BootstrapError::Discovery(_) => {
                "project discovery failed — check that `cargo metadata` runs successfully"
            }
        };
        eprintln!("MCP bootstrap error: {e}");
        McpError::invalid_params(hint.to_string(), None)
    })?;

    let effective_config = if ignore_project_config {
        None
    } else {
        // Warn if security rules are suppressed by project config
        if let Some(ref fc) = file_config {
            let security_rules = [
                "hardcoded-secrets",
                "sql-injection-risk",
                "unsafe-block-audit",
            ];
            for rule in &fc.ignore.rules {
                if security_rules.contains(&rule.as_str()) {
                    eprintln!("Warning: project config suppresses security rule '{rule}'");
                }
            }
        }
        file_config.as_ref()
    };
    let resolved = config::resolve_config_defaults(effective_config);

    Ok((target_dir, project_info, resolved))
}

// ---------------------------------------------------------------------------
// Rule knowledge base (derived from trait implementations at runtime)
// ---------------------------------------------------------------------------

struct RuleDoc {
    name: &'static str,
    category: String,
    severity: String,
    description: &'static str,
    fix: &'static str,
}

/// Return cached rule docs. Computed once on first call since rules are static.
fn rule_docs() -> &'static [RuleDoc] {
    static DOCS: std::sync::OnceLock<Vec<RuleDoc>> = std::sync::OnceLock::new();
    DOCS.get_or_init(|| {
        rules::all_custom_rules()
            .iter()
            .map(|rule| RuleDoc {
                name: rule.name(),
                category: rule.category().to_string(),
                severity: rule.severity().to_string(),
                description: rule.description(),
                fix: rule.fix_hint(),
            })
            .collect()
    })
}

fn get_rule_explanation(rule: &str) -> String {
    // Look up in the data-driven registry first
    let docs = rule_docs();
    if let Some(doc) = docs.iter().find(|d| d.name == rule) {
        return format!(
            "## {}\n\n**Category:** {} | **Severity:** {}\n\n{}\n\n**Fix:** {}",
            doc.name, doc.category, doc.severity, doc.description, doc.fix
        );
    }

    // Fall back to clippy lint lookup
    let lint_name = rule.strip_prefix("clippy::").unwrap_or(rule);
    if clippy::known_lint_names().contains(&lint_name) {
        format!(
            "## {rule}\n\nThis is a Clippy lint tracked by rust-doctor with custom severity/category mapping.\n\nSee full documentation: https://rust-lang.github.io/rust-clippy/master/index.html#{lint_name}"
        )
    } else {
        format!("Unknown rule: `{rule}`\n\nUse the `list_rules` tool to see all available rules.")
    }
}

fn get_all_rules_listing() -> String {
    let mut text = String::from("# rust-doctor Rules\n\n## Custom Rules (AST-based via syn)\n\n");

    use std::fmt::Write;
    let docs = rule_docs();
    let mut current_category = String::new();
    for doc in docs {
        if doc.category != current_category {
            if !current_category.is_empty() {
                text.push('\n');
            }
            let _ = writeln!(text, "### {}", doc.category);
            current_category.clone_from(&doc.category);
        }
        let _ = writeln!(
            text,
            "- `{}` ({}) — {}",
            doc.name,
            doc.severity.to_lowercase(),
            doc.description
                .split(". ")
                .next()
                .unwrap_or(doc.description)
        );
    }

    text.push_str("\n## Clippy Lints (55+ with category/severity overrides)\n\n");
    text.push_str(
        "rust-doctor runs `cargo clippy` with pedantic, nursery, and cargo lint groups.\n",
    );
    text.push_str("55+ lints have explicit category and severity overrides across:\n");
    text.push_str(
        "Error Handling, Performance, Security, Correctness, Architecture, Cargo, Async, Style\n",
    );
    text.push_str("\nUse `explain_rule` with a clippy lint name for details.\n");

    text.push_str("\n## External Tools\n\n");
    text.push_str("- **cargo-audit** — Vulnerability scanning for dependencies (install: `cargo install cargo-audit`)\n");
    text.push_str("- **cargo-machete** — Unused dependency detection (install: `cargo install cargo-machete`)\n");

    text
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- RULE_DOCS completeness ---

    #[test]
    fn test_rule_docs_covers_all_custom_rules() {
        let docs = rule_docs();
        let expected: Vec<String> = crate::scan::custom_rule_names()
            .into_iter()
            .filter(|name| name != "unused-dependency") // external tool rule, not AST
            .collect();

        for rule_name in &expected {
            assert!(
                docs.iter().any(|doc| doc.name == *rule_name),
                "rule_docs() is missing entry for custom rule '{rule_name}'"
            );
        }
    }

    #[test]
    fn test_rule_docs_has_no_duplicates() {
        let docs = rule_docs();
        let mut seen = std::collections::HashSet::new();
        for doc in docs {
            assert!(
                seen.insert(&doc.name),
                "rule_docs() has duplicate entry for '{}'",
                doc.name
            );
        }
    }

    #[test]
    fn test_rule_docs_fields_not_empty() {
        let docs = rule_docs();
        for doc in docs {
            assert!(!doc.name.is_empty(), "Rule has empty name");
            assert!(
                !doc.category.is_empty(),
                "Rule '{}' has empty category",
                doc.name
            );
            assert!(
                !doc.severity.is_empty(),
                "Rule '{}' has empty severity",
                doc.name
            );
            assert!(
                !doc.description.is_empty(),
                "Rule '{}' has empty description",
                doc.name
            );
            assert!(!doc.fix.is_empty(), "Rule '{}' has empty fix", doc.name);
        }
    }

    // --- get_rule_explanation ---

    #[test]
    fn test_explain_known_custom_rule() {
        let explanation = get_rule_explanation("unwrap-in-production");
        assert!(explanation.contains("## unwrap-in-production"));
        assert!(explanation.contains("Error Handling"));
        assert!(explanation.contains("warning"));
        assert!(explanation.contains("Fix:"));
    }

    #[test]
    fn test_explain_known_clippy_lint() {
        let explanation = get_rule_explanation("clippy::expect_used");
        assert!(explanation.contains("clippy::expect_used"));
        assert!(explanation.contains("Clippy lint"));
        assert!(explanation.contains("rust-lang.github.io"));
    }

    #[test]
    fn test_explain_clippy_lint_without_prefix() {
        let explanation = get_rule_explanation("expect_used");
        assert!(explanation.contains("expect_used"));
        assert!(explanation.contains("Clippy lint"));
    }

    #[test]
    fn test_explain_unknown_rule() {
        let explanation = get_rule_explanation("nonexistent-rule-xyz");
        assert!(explanation.contains("Unknown rule"));
        assert!(explanation.contains("list_rules"));
    }

    // --- get_all_rules_listing ---

    #[test]
    fn test_rules_listing_has_all_sections() {
        let listing = get_all_rules_listing();
        assert!(listing.contains("# rust-doctor Rules"));
        assert!(listing.contains("## Custom Rules"));
        assert!(listing.contains("## Clippy Lints"));
        assert!(listing.contains("## External Tools"));
    }

    #[test]
    fn test_rules_listing_contains_all_categories() {
        let listing = get_all_rules_listing();
        assert!(listing.contains("### Error Handling"));
        assert!(listing.contains("### Performance"));
        assert!(listing.contains("### Security"));
        assert!(listing.contains("### Async"));
        assert!(listing.contains("### Framework"));
    }

    #[test]
    fn test_rules_listing_contains_all_custom_rules() {
        let listing = get_all_rules_listing();
        let docs = rule_docs();
        for doc in docs {
            assert!(
                listing.contains(doc.name),
                "Rules listing is missing '{}'",
                doc.name
            );
        }
    }

    // --- ServerInfo ---

    #[test]
    fn test_server_info_has_instructions() {
        let server = RustDoctorServer::new();
        let info = <RustDoctorServer as rmcp::handler::server::ServerHandler>::get_info(&server);
        let instructions = info.instructions.as_deref().unwrap_or("");
        assert!(!instructions.is_empty());
        assert!(instructions.contains("scan"));
        assert!(instructions.contains("explain_rule"));
        assert!(instructions.contains("list_rules"));
        assert!(instructions.contains("score"));
    }

    // --- Tool registration ---

    #[test]
    fn test_tool_router_has_all_tools() {
        let server = RustDoctorServer::new();
        let tools = server.tool_router.list_all();
        let names: Vec<&str> = tools.iter().map(|t| &*t.name).collect();
        assert!(names.contains(&"scan"), "Missing scan tool");
        assert!(names.contains(&"score"), "Missing score tool");
        assert!(names.contains(&"explain_rule"), "Missing explain_rule tool");
        assert!(names.contains(&"list_rules"), "Missing list_rules tool");
        assert_eq!(
            names.len(),
            4,
            "Expected exactly 4 tools, got {}",
            names.len()
        );
    }

    #[test]
    fn test_scan_tool_has_output_schema() {
        let server = RustDoctorServer::new();
        let tools = server.tool_router.list_all();
        let scan = tools.iter().find(|t| t.name == "scan").unwrap();
        assert!(
            scan.output_schema.is_some(),
            "scan tool should have outputSchema from Json<ScanOutput>"
        );
    }

    #[test]
    fn test_score_tool_has_output_schema() {
        let server = RustDoctorServer::new();
        let tools = server.tool_router.list_all();
        let score = tools.iter().find(|t| t.name == "score").unwrap();
        assert!(
            score.output_schema.is_some(),
            "score tool should have outputSchema from Json<ScoreOutput>"
        );
    }

    // --- discover_and_resolve error mapping ---

    #[test]
    fn test_discover_and_resolve_invalid_path() {
        let result = discover_and_resolve("/nonexistent/path/to/project", false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Should be invalid_params (not internal_error) for bad input
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[test]
    fn test_discover_and_resolve_error_does_not_contain_raw_path() {
        let result = discover_and_resolve("/nonexistent/path/to/project", false);
        let err = result.unwrap_err();
        let msg = err.message.to_string();
        // Sanitized: must NOT contain the raw filesystem path
        assert!(
            !msg.contains("/nonexistent/path"),
            "MCP error should not contain raw path, got: {msg}"
        );
    }

    #[test]
    fn test_discover_and_resolve_outside_home() {
        // /tmp is typically outside $HOME — should be rejected
        if std::env::var("HOME").is_ok() {
            let result = discover_and_resolve("/etc", false);
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
        }
    }

    // --- MCP e2e: scan + score on a real project ---

    #[test]
    fn test_scan_tool_on_self() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let result = discover_and_resolve(manifest_dir, false);
        assert!(result.is_ok(), "discover_and_resolve failed: {result:?}");
        let (_dir, project_info, resolved) = result.unwrap();
        let scan_result = scan::scan_project(&project_info, &resolved, true, &[], true);
        assert!(scan_result.is_ok(), "scan_project failed: {scan_result:?}");
        let result = scan_result.unwrap();
        // Verify ScanOutput structure
        assert!(result.score <= 100);
        assert!(result.source_file_count > 0);
    }

    #[test]
    fn test_score_output_structure() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let (_dir, project_info, resolved) = discover_and_resolve(manifest_dir, false).unwrap();
        let result = scan::scan_project(&project_info, &resolved, true, &[], true).unwrap();
        let output = ScoreOutput {
            score: result.score,
            score_label: result.score_label,
        };
        assert!(output.score <= 100);
        // Verify it serializes correctly
        let json = serde_json::to_value(&output).unwrap();
        assert!(json.get("score").is_some());
        assert!(json.get("score_label").is_some());
    }
}
