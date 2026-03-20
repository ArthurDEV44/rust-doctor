use crate::scan;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{
    CallToolResult, Content, GetPromptResult, LoggingLevel, LoggingMessageNotificationParam,
    PromptMessage, PromptMessageRole,
};
use rmcp::{ErrorData as McpError, RoleServer, prompt, prompt_router, tool, tool_router};

use super::RustDoctorServer;
use super::helpers::{discover_and_resolve, format_scan_report, group_diagnostics};
use super::rules::{get_all_rules_listing, get_rule_explanation};
use super::types::{
    DeepAuditArgs, ExplainRuleInput, HealthCheckArgs, ScanInput, ScoreInput, ScoreOutput,
};

// ---------------------------------------------------------------------------
// Tool and prompt implementations
// ---------------------------------------------------------------------------

#[tool_router]
#[prompt_router]
impl RustDoctorServer {
    pub(super) fn new() -> Self {
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
Runs 4 passes in parallel: clippy (55+ lints), 19 custom AST rules, cargo-audit (CVEs), cargo-machete (unused deps). \
Set 'diff' to a branch name to only scan changed files. \
After scanning, use explain_rule on any rule ID to get fix guidance.",
        annotations(
            title = "Scan Project",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    async fn scan(
        &self,
        meta: rmcp::model::Meta,
        client: rmcp::Peer<RoleServer>,
        params: Parameters<ScanInput>,
    ) -> Result<CallToolResult, McpError> {
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
        let _ = client
            .notify_logging_message(LoggingMessageNotificationParam {
                level: LoggingLevel::Info,
                logger: Some("rust-doctor".into()),
                data: serde_json::json!("Bootstrapping project..."),
            })
            .await;

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
        let _ = client
            .notify_logging_message(LoggingMessageNotificationParam {
                level: LoggingLevel::Info,
                logger: Some("rust-doctor".into()),
                data: serde_json::json!(
                    "Running 4 analysis passes (clippy, AST rules, cargo-audit, cargo-machete)..."
                ),
            })
            .await;

        // Run the CPU-bound scan on a blocking thread with a 5-minute absolute timeout
        let offline = input.offline;
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(300),
            tokio::task::spawn_blocking(move || {
                scan::scan_project(&project_info, &resolved, offline, &[], true)
            }),
        )
        .await
        .map_err(|_| {
            McpError::internal_error(
                "scan timed out after 5 minutes — project may be too large or a subprocess is hanging",
                None,
            )
        })?
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
        let _ = client
            .notify_logging_message(LoggingMessageNotificationParam {
                level: LoggingLevel::Info,
                logger: Some("rust-doctor".into()),
                data: serde_json::Value::String(format!(
                    "Scan complete: {}/100 ({}) — {} errors, {} warnings, {} info in {:.1}s",
                    result.score,
                    result.score_label,
                    result.error_count,
                    result.warning_count,
                    result.info_count,
                    result.elapsed.as_secs_f64()
                )),
            })
            .await;

        let grouped = group_diagnostics(&result.diagnostics);
        let report = format_scan_report(&result, &grouped);

        Ok(CallToolResult::success(vec![Content::text(report)]))
    }

    #[tool(
        name = "score",
        description = "Get just the health score of a Rust project (0-100 integer). \
Use this tool for a quick pass/fail check without full diagnostics. \
IMPORTANT: runs the same full analysis as scan internally, so takes the same 5-30 seconds. \
Score thresholds: >=75 'Great', >=50 'Needs work', <50 'Critical'. \
Scoring: each unique error-severity rule violated costs 1.5 points, each warning costs 0.75 points. \
If you also need the diagnostics, use scan instead — it includes the score too.",
        annotations(
            title = "Score Project",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    async fn score(
        &self,
        meta: rmcp::model::Meta,
        client: rmcp::Peer<RoleServer>,
        params: Parameters<ScoreInput>,
    ) -> Result<Json<ScoreOutput>, McpError> {
        let input = params.0;
        let progress_token = meta.get_progress_token();

        if let Some(ref token) = progress_token {
            let _ = client
                .notify_progress(rmcp::model::ProgressNotificationParam {
                    progress_token: token.clone(),
                    progress: 0.0,
                    total: Some(1.0),
                    message: Some("Scoring project...".to_string()),
                })
                .await;
        }
        let _ = client
            .notify_logging_message(LoggingMessageNotificationParam {
                level: LoggingLevel::Info,
                logger: Some("rust-doctor".into()),
                data: serde_json::json!("Scoring project..."),
            })
            .await;

        let (_dir, project_info, resolved) = discover_and_resolve(&input.directory, false)?;

        // Run the CPU-bound scan on a blocking thread with a 5-minute absolute timeout
        let offline = input.offline;
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(300),
            tokio::task::spawn_blocking(move || {
                scan::scan_project(&project_info, &resolved, offline, &[], true)
            }),
        )
        .await
        .map_err(|_| {
            McpError::internal_error(
                "scan timed out after 5 minutes — project may be too large or a subprocess is hanging",
                None,
            )
        })?
        .map_err(|e| McpError::internal_error(format!("scan task failed: {e}"), None))?
        .map_err(|e| {
            eprintln!("MCP score error: {e}");
            McpError::internal_error(
                "scan failed — check project compiles with `cargo check`",
                None,
            )
        })?;

        if let Some(ref token) = progress_token {
            let _ = client
                .notify_progress(rmcp::model::ProgressNotificationParam {
                    progress_token: token.clone(),
                    progress: 1.0,
                    total: Some(1.0),
                    message: Some(format!(
                        "Score: {}/100 ({})",
                        result.score, result.score_label
                    )),
                })
                .await;
        }
        let _ = client
            .notify_logging_message(LoggingMessageNotificationParam {
                level: LoggingLevel::Info,
                logger: Some("rust-doctor".into()),
                data: serde_json::Value::String(format!(
                    "Score: {}/100 ({})",
                    result.score, result.score_label
                )),
            })
            .await;

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
        annotations(
            title = "Explain Rule",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
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
Returns: 19 custom AST rules (grouped by Error Handling, Performance, Architecture, Security, Async, Framework), \
55+ clippy lints with custom severity overrides, and 2 external tools (cargo-audit, cargo-machete). \
Each entry shows rule ID, severity, and one-line summary.",
        annotations(
            title = "List Rules",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    async fn list_rules(&self) -> Result<CallToolResult, McpError> {
        let listing = get_all_rules_listing();
        Ok(CallToolResult::success(vec![Content::text(listing)]))
    }

    // -- Prompts --------------------------------------------------------------

    #[prompt(
        name = "deep-audit",
        description = "Comprehensive Rust code audit: explores codebase architecture, runs rust-doctor \
analysis, performs deep code review against production best practices, researches current Rust patterns \
on the web, cross-references findings, and generates a full remediation report. Ends with a choice: \
implement all fixes, generate a PRD, or manual prompt. Use this for thorough, expert-level code audits \
that go far beyond linting."
    )]
    pub(super) async fn deep_audit(&self, params: Parameters<DeepAuditArgs>) -> GetPromptResult {
        GetPromptResult::new(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            super::prompts::deep_audit_prompt(&params.0.directory),
        )])
        .with_description(
            "Expert-level Rust audit: codebase exploration + static analysis + deep code review \
             + best practices research + synthesis report + actionable remediation choices",
        )
    }

    #[prompt(
        name = "health-check",
        description = "Run a full health check on a Rust project: scan, generate a prioritized \
remediation plan, and optionally apply fixes. Combines scan + plan + fix into one structured workflow."
    )]
    pub(super) async fn health_check(
        &self,
        params: Parameters<HealthCheckArgs>,
    ) -> GetPromptResult {
        GetPromptResult::new(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            super::prompts::health_check_prompt(&params.0.directory),
        )])
        .with_description(
            "Full health audit with prioritized remediation plan and structured fix workflow",
        )
    }
}
