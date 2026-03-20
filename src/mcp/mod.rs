mod helpers;
mod prompts;
mod rules;
mod tools;
mod types;

// Re-export the public API
pub use types::{
    DeepAuditArgs, DiagnosticExample, DiagnosticGroup, ExplainRuleInput, HealthCheckArgs,
    ScanInput, ScoreInput, ScoreOutput,
};

use rmcp::handler::server::router::prompt::PromptRouter;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::{
    AnnotateAble, GetPromptRequestParams, GetPromptResult, ListPromptsResult, ListResourcesResult,
    PaginatedRequestParams, RawResource, ReadResourceRequestParams, ReadResourceResult, Resource,
    ResourceContents, ServerCapabilities, ServerInfo,
};
use rmcp::service::{RequestContext, ServiceExt};
use rmcp::{ErrorData as McpError, RoleServer, prompt_handler};

use rules::{get_rule_explanation, rule_docs};

// ---------------------------------------------------------------------------
// MCP server struct
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct RustDoctorServer {
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
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
                .enable_logging()
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
             - `deep-audit` — comprehensive expert audit: explores codebase, scans, deep code review, \
             web research for best practices, synthesis report, then offers to implement all fixes / generate PRD / manual\n\
             - `health-check` — quick health check with scan + prioritized remediation plan + fix workflow\n\n\
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
// Error type & public entry point
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

#[cfg(test)]
mod tests {
    use super::helpers::{discover_and_resolve, format_scan_report, group_diagnostics};
    use super::rules::{get_all_rules_listing, get_rule_explanation, rule_docs};
    use super::types::{DeepAuditArgs, HealthCheckArgs, MAX_EXAMPLES_PER_GROUP, ScoreOutput};
    use super::*;
    use crate::diagnostics::Diagnostic;
    use crate::scan;
    use rmcp::handler::server::wrapper::Parameters;

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

    // --- Prompt registration ---

    #[test]
    fn test_prompt_router_has_all_prompts() {
        let server = RustDoctorServer::new();
        let prompts = server.prompt_router.list_all();
        let names: Vec<&str> = prompts.iter().map(|p| &*p.name).collect();
        assert!(names.contains(&"deep-audit"), "Missing deep-audit prompt");
        assert!(
            names.contains(&"health-check"),
            "Missing health-check prompt"
        );
        assert_eq!(
            names.len(),
            2,
            "Expected exactly 2 prompts, got {}",
            names.len()
        );
    }

    #[test]
    fn test_deep_audit_prompt_registered_with_description() {
        let server = RustDoctorServer::new();
        let prompts = server.prompt_router.list_all();
        let deep_audit = prompts.iter().find(|p| p.name == "deep-audit").unwrap();
        let desc = deep_audit.description.as_deref().unwrap_or("");
        assert!(
            desc.contains("audit"),
            "deep-audit description should mention audit"
        );
        assert!(
            desc.contains("best practices"),
            "deep-audit description should mention best practices"
        );
    }

    #[test]
    fn test_server_info_mentions_deep_audit() {
        let server = RustDoctorServer::new();
        let info = <RustDoctorServer as rmcp::handler::server::ServerHandler>::get_info(&server);
        let instructions = info.instructions.as_deref().unwrap_or("");
        assert!(
            instructions.contains("deep-audit"),
            "Server instructions should mention deep-audit prompt"
        );
        assert!(
            instructions.contains("health-check"),
            "Server instructions should mention health-check prompt"
        );
    }

    /// Extract text from a `PromptMessageContent::Text` variant.
    fn extract_prompt_text(content: &rmcp::model::PromptMessageContent) -> &str {
        match content {
            rmcp::model::PromptMessageContent::Text { text } => text,
            _ => panic!("expected Text content in prompt message"),
        }
    }

    #[tokio::test]
    async fn test_deep_audit_prompt_content() {
        let server = RustDoctorServer::new();
        let result = server
            .deep_audit(Parameters(DeepAuditArgs {
                directory: "/home/user/my-project".to_string(),
            }))
            .await;
        assert_eq!(result.messages.len(), 1);
        assert!(result.description.is_some());
        let text = extract_prompt_text(&result.messages[0].content);
        // Directory is interpolated
        assert!(
            text.contains("/home/user/my-project"),
            "directory should be interpolated into prompt"
        );
        // All 6 phases present
        for phase in 1..=6 {
            assert!(
                text.contains(&format!("PHASE {phase}")),
                "Missing PHASE {phase} in prompt"
            );
        }
        // Decision options present
        assert!(text.contains("Implement all fixes"));
        assert!(text.contains("Generate a PRD"));
        assert!(text.contains("Manual"));
        // Hard rules present
        assert!(text.contains("HARD RULES"));
    }

    #[tokio::test]
    async fn test_health_check_prompt_content() {
        let server = RustDoctorServer::new();
        let result = server
            .health_check(Parameters(HealthCheckArgs {
                directory: "/home/user/test".to_string(),
            }))
            .await;
        assert_eq!(result.messages.len(), 1);
        let text = extract_prompt_text(&result.messages[0].content);
        assert!(
            text.contains("/home/user/test"),
            "directory should be interpolated"
        );
        assert!(text.contains("Phase 1"));
        assert!(text.contains("Phase 4"));
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
    fn test_scan_tool_returns_call_tool_result() {
        // scan returns CallToolResult (text summary + structuredContent),
        // not Json<T>, so it has no auto-generated outputSchema.
        let server = RustDoctorServer::new();
        let tools = server.tool_router.list_all();
        let scan = tools.iter().find(|t| t.name == "scan").unwrap();
        assert!(
            scan.output_schema.is_none(),
            "scan uses CallToolResult, not Json<T>"
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

    // --- Tool annotations ---

    #[test]
    fn test_all_tools_have_correct_annotations() {
        let server = RustDoctorServer::new();
        let tools = server.tool_router.list_all();
        for tool in &tools {
            let ann = tool
                .annotations
                .as_ref()
                .unwrap_or_else(|| panic!("tool '{}' missing annotations", tool.name));
            assert_eq!(
                ann.read_only_hint,
                Some(true),
                "tool '{}' should be read-only",
                tool.name
            );
            assert_eq!(
                ann.destructive_hint,
                Some(false),
                "tool '{}' should not be destructive",
                tool.name
            );
            assert_eq!(
                ann.idempotent_hint,
                Some(true),
                "tool '{}' should be idempotent",
                tool.name
            );
            assert_eq!(
                ann.open_world_hint,
                Some(false),
                "tool '{}' should be closed-world",
                tool.name
            );
            assert!(
                ann.title.is_some(),
                "tool '{}' should have a title",
                tool.name
            );
        }
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
        // Verify ScanResult structure
        assert!(result.score <= 100);
        assert!(result.source_file_count > 0);
    }

    // --- Diagnostic grouping unit tests ---

    fn make_diagnostic(
        rule: &str,
        severity: crate::diagnostics::Severity,
        help: Option<&str>,
    ) -> Diagnostic {
        Diagnostic {
            file_path: std::path::PathBuf::from("src/lib.rs"),
            rule: rule.to_string(),
            category: crate::diagnostics::Category::ErrorHandling,
            severity,
            message: format!("test finding for {rule}"),
            help: help.map(String::from),
            line: Some(1),
            column: None,
            fix: None,
        }
    }

    #[test]
    fn test_group_diagnostics_empty() {
        let groups = group_diagnostics(&[]);
        assert!(groups.is_empty());
    }

    #[test]
    fn test_group_diagnostics_single() {
        let diag = make_diagnostic("rule-a", crate::diagnostics::Severity::Error, None);
        let groups = group_diagnostics(&[diag]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].count, 1);
        assert_eq!(groups[0].examples.len(), 1);
    }

    #[test]
    fn test_group_diagnostics_caps_examples() {
        let diags: Vec<_> = (0..10)
            .map(|_| make_diagnostic("rule-a", crate::diagnostics::Severity::Warning, None))
            .collect();
        let groups = group_diagnostics(&diags);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].count, 10);
        assert_eq!(groups[0].examples.len(), MAX_EXAMPLES_PER_GROUP);
    }

    #[test]
    fn test_group_diagnostics_sorts_errors_first() {
        let diags = vec![
            make_diagnostic("warn-rule", crate::diagnostics::Severity::Warning, None),
            make_diagnostic("info-rule", crate::diagnostics::Severity::Info, None),
            make_diagnostic("err-rule", crate::diagnostics::Severity::Error, None),
        ];
        let groups = group_diagnostics(&diags);
        assert_eq!(groups[0].severity, "error");
        assert_eq!(groups[1].severity, "warning");
        assert_eq!(groups[2].severity, "info");
    }

    #[test]
    fn test_group_diagnostics_help_finds_first_non_none() {
        let diags = vec![
            make_diagnostic("rule-a", crate::diagnostics::Severity::Warning, None),
            make_diagnostic(
                "rule-a",
                crate::diagnostics::Severity::Warning,
                Some("fix it"),
            ),
            make_diagnostic("rule-a", crate::diagnostics::Severity::Warning, None),
        ];
        let groups = group_diagnostics(&diags);
        assert_eq!(groups[0].help.as_deref(), Some("fix it"));
    }

    // --- Integration test: grouping on real project ---

    #[test]
    fn test_scan_output_grouping() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let (_dir, project_info, resolved) = discover_and_resolve(manifest_dir, false).unwrap();
        let result = scan::scan_project(&project_info, &resolved, true, &[], true).unwrap();

        let total = result.diagnostics.len();
        let grouped = group_diagnostics(&result.diagnostics);
        let report = format_scan_report(&result, &grouped);

        // Grouping reduces count
        assert!(
            grouped.len() < total,
            "grouping should compress: {} groups from {} diagnostics",
            grouped.len(),
            total
        );
        // Each group has examples
        for g in &grouped {
            assert!(!g.examples.is_empty(), "group '{}' has no examples", g.rule);
            assert!(
                g.examples.len() <= MAX_EXAMPLES_PER_GROUP,
                "group '{}' has too many examples",
                g.rule
            );
            assert!(g.count > 0);
        }
        // Report is non-empty and contains score
        assert!(report.contains(&result.score.to_string()));
        // Sorted: errors before warnings before info
        let severities: Vec<&str> = grouped.iter().map(|g| g.severity.as_str()).collect();
        for window in severities.windows(2) {
            let ord = |s: &str| -> u8 {
                match s {
                    "error" => 0,
                    "warning" => 1,
                    _ => 2,
                }
            };
            assert!(
                ord(window[0]) <= ord(window[1]),
                "groups not sorted by severity: {} before {}",
                window[0],
                window[1]
            );
        }
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
