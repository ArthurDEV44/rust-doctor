use crate::diagnostics::Diagnostic;
use crate::{clippy, config, discovery, scan};
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{CallToolResult, Content, ServerInfo, ServerCapabilities};
use rmcp::service::ServiceExt;
use rmcp::{tool, tool_router, ErrorData as McpError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

// ---------------------------------------------------------------------------
// MCP server struct
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct RustDoctorServer {
    tool_router: ToolRouter<Self>,
}

// ---------------------------------------------------------------------------
// Input schemas (schemars-derived)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct ScanInput {
    /// Absolute path to the Rust project directory (must contain a Cargo.toml).
    #[schemars(description = "Absolute path to the Rust project directory to analyze. Must contain a Cargo.toml file.")]
    pub directory: String,
    /// Only scan files changed vs this base branch (e.g. "main"). Omit to scan all files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Git branch name to diff against (e.g. 'main', 'develop'). When set, only files changed vs this branch are scanned. Omit to scan all files.")]
    pub diff: Option<String>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct ScoreInput {
    /// Absolute path to the Rust project directory.
    #[schemars(description = "Absolute path to the Rust project directory to score. Must contain a Cargo.toml file.")]
    pub directory: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct ExplainRuleInput {
    /// The rule ID (e.g. "unwrap-in-production", "clippy::expect_used", "blocking-in-async").
    #[schemars(description = "Rule identifier to explain. Accepts custom rule IDs (e.g. 'unwrap-in-production') or clippy lint names (e.g. 'clippy::expect_used'). Use list_rules to discover available IDs.")]
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
    /// Human-readable score label: "Great", "Needs work", or "Critical".
    pub score_label: String,
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
}

/// Structured output for the score tool.
#[derive(Serialize, JsonSchema)]
pub struct ScoreOutput {
    /// Health score from 0 (critical) to 100 (perfect).
    pub score: u32,
    /// Human-readable score label: "Great", "Needs work", or "Critical".
    pub score_label: String,
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

#[tool_router]
impl RustDoctorServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "scan",
        description = "Run a full Rust code health analysis on a project directory. \
Use this tool when you need detailed diagnostics — it returns all findings with file:line precision. \
Takes 5-30 seconds depending on project size. \
Returns JSON with: diagnostics array (each has rule, severity, message, file_path, line, column, help), \
score (0-100), score_label, source_file_count, elapsed_secs, error_count, warning_count, skipped_passes. \
Runs 4 passes in parallel: clippy (55+ lints), 18 custom AST rules, cargo-audit (CVEs), cargo-machete (unused deps). \
Set 'diff' to a branch name to only scan changed files. \
After scanning, use explain_rule on any rule ID to get fix guidance.",
        annotations(read_only_hint = true)
    )]
    async fn scan(&self, params: Parameters<ScanInput>) -> Result<Json<ScanOutput>, McpError> {
        let input = params.0;
        let (_dir, project_info, mut resolved) = discover_and_resolve(&input.directory)?;

        if let Some(diff_base) = input.diff {
            resolved.diff = Some(diff_base);
        }

        let result = scan::scan_project(&project_info, &resolved, false, &[], true)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(Json(ScanOutput {
            diagnostics: result.diagnostics,
            score: result.score,
            score_label: result.score_label.to_string(),
            source_file_count: result.source_file_count,
            elapsed_secs: result.elapsed.as_secs_f64(),
            skipped_passes: result.skipped_passes,
            error_count: result.error_count,
            warning_count: result.warning_count,
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
        let (_dir, project_info, resolved) = discover_and_resolve(&input.directory)?;

        let result = scan::scan_project(&project_info, &resolved, false, &[], true)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(Json(ScoreOutput {
            score: result.score,
            score_label: result.score_label.to_string(),
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
}

// ---------------------------------------------------------------------------
// ServerHandler implementation
// ---------------------------------------------------------------------------

#[rmcp::tool_handler]
impl rmcp::handler::server::ServerHandler for RustDoctorServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(
                "rust-doctor is a Rust code health scanner. It analyzes projects for security, \
                 performance, correctness, architecture, and dependency issues.\n\n\
                 ## Recommended workflow\n\
                 1. `scan` a project directory → get diagnostics + score (5-30s)\n\
                 2. `explain_rule` for any rule you want to understand → instant\n\
                 3. `list_rules` to browse all available checks → instant\n\
                 4. `score` for a quick pass/fail without diagnostics (same 5-30s as scan)\n\n\
                 ## Tips\n\
                 - Prefer `scan` over `score` — it includes the score plus full diagnostics\n\
                 - Use `diff` parameter in scan to focus on changed files only\n\
                 - All tools are read-only and safe to call repeatedly\n\
                 - `explain_rule` and `list_rules` are instant (no project scanning)",
            )
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the MCP server over stdio. Called from main when `--mcp` is passed.
pub fn run_mcp_server() {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async {
        let server = RustDoctorServer::new();
        let transport = rmcp::transport::io::stdio();
        let service = server.serve(transport).await.expect("MCP server failed");
        service.waiting().await.expect("MCP server error");
    });
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Discover project + load file config + resolve with defaults.
fn discover_and_resolve(
    directory: &str,
) -> Result<
    (
        std::path::PathBuf,
        discovery::ProjectInfo,
        config::ResolvedConfig,
    ),
    McpError,
> {
    let (target_dir, project_info, file_config) =
        discovery::bootstrap_project(Path::new(directory), false)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

    let resolved = config::resolve_config_defaults(file_config.as_ref());

    Ok((target_dir, project_info, resolved))
}

// ---------------------------------------------------------------------------
// Rule knowledge base (data-driven)
// ---------------------------------------------------------------------------

struct RuleDoc {
    name: &'static str,
    category: &'static str,
    severity: &'static str,
    description: &'static str,
    fix: &'static str,
}

static RULE_DOCS: &[RuleDoc] = &[
    // ── Error Handling ──────────────────────────────────────────
    RuleDoc {
        name: "unwrap-in-production",
        category: "Error Handling",
        severity: "Warning",
        description: "Flags `.unwrap()` and `.expect()` calls outside of test code. These calls panic at runtime if the value is `None` or `Err`, crashing your application.",
        fix: "Use the `?` operator to propagate errors, or handle them with `match`, `if let`, `.unwrap_or()`, or `.unwrap_or_else()`.",
    },
    RuleDoc {
        name: "panic-in-library",
        category: "Error Handling",
        severity: "Error",
        description: "Flags `panic!()`, `todo!()`, and `unimplemented!()` macros in library code. Libraries should return errors rather than panicking, since callers cannot recover from a panic across crate boundaries.",
        fix: "Return `Result<T, E>` or `Option<T>` instead of panicking.",
    },
    RuleDoc {
        name: "box-dyn-error-in-public-api",
        category: "Error Handling",
        severity: "Warning",
        description: "Flags `pub fn` returning `Result<_, Box<dyn Error>>`. This erases error type information, making it impossible for callers to match on specific error variants.",
        fix: "Define a custom error enum with `thiserror` or return a concrete error type.",
    },
    RuleDoc {
        name: "result-unit-error",
        category: "Error Handling",
        severity: "Warning",
        description: "Flags `pub fn` returning `Result<_, ()>`. A unit error carries no information about what went wrong.",
        fix: "Use a meaningful error type that describes the failure.",
    },
    // ── Performance ─────────────────────────────────────────────
    RuleDoc {
        name: "excessive-clone",
        category: "Performance",
        severity: "Warning",
        description: "Flags `.clone()` calls that may indicate unnecessary heap allocations. Each clone copies the entire value, which is expensive for `String`, `Vec`, and other heap-allocated types.",
        fix: "Use references (`&T`) or `Cow<T>` instead of cloning. Consider restructuring ownership to avoid the clone.",
    },
    RuleDoc {
        name: "string-from-literal",
        category: "Performance",
        severity: "Warning",
        description: "Flags `String::from(\"literal\")` and `\"literal\".to_string()`. While not wrong, these allocate on the heap when a `&str` reference might suffice.",
        fix: "If the function accepts `&str`, pass the literal directly. If you need an owned `String`, this warning can be safely ignored or suppressed.",
    },
    RuleDoc {
        name: "collect-then-iterate",
        category: "Performance",
        severity: "Warning",
        description: "Flags `.collect::<Vec<_>>()` immediately followed by `.iter()`. This allocates a temporary vector unnecessarily since the original iterator could be used directly.",
        fix: "Remove the `.collect()` and chain the iterator operations directly.",
    },
    RuleDoc {
        name: "large-enum-variant",
        category: "Performance",
        severity: "Warning",
        description: "Flags enums where variants have significantly different sizes (>3x field count disparity). The enum's size equals its largest variant, wasting memory for smaller variants.",
        fix: "Box the large variant's data: `LargeVariant(Box<LargeData>)`.",
    },
    RuleDoc {
        name: "unnecessary-allocation",
        category: "Performance",
        severity: "Warning",
        description: "Flags `Vec::new()` or `String::new()` inside loops. Each iteration allocates a new buffer, which is expensive.",
        fix: "Move the allocation outside the loop and use `.clear()` to reuse it.",
    },
    // ── Security ────────────────────────────────────────────────
    RuleDoc {
        name: "hardcoded-secrets",
        category: "Security",
        severity: "Error",
        description: "Flags string literals assigned to variables named `api_key`, `password`, `token`, `secret`, etc. (length > 8 chars). Hardcoded secrets in source code can be extracted from compiled binaries or version control.",
        fix: "Use environment variables, a secrets manager, or config files excluded from version control.",
    },
    RuleDoc {
        name: "unsafe-block-audit",
        category: "Security",
        severity: "Warning",
        description: "Flags `unsafe {}` blocks and `unsafe fn` declarations. Unsafe code bypasses Rust's memory safety guarantees and must be carefully audited. Skipped if the crate declares `#![forbid(unsafe_code)]`.",
        fix: "Verify the safety invariants are documented and correct. Consider safe abstractions or crates like `zerocopy` to eliminate unsafe.",
    },
    RuleDoc {
        name: "sql-injection-risk",
        category: "Security",
        severity: "Error",
        description: "Flags `format!()` output passed to `.query()`, `.execute()`, or `.raw()` methods. String interpolation in SQL queries enables SQL injection attacks.",
        fix: "Use parameterized queries (`$1`, `?`) provided by your database library (sqlx, diesel, sea-orm).",
    },
    // ── Async ───────────────────────────────────────────────────
    RuleDoc {
        name: "blocking-in-async",
        category: "Async",
        severity: "Error",
        description: "Flags blocking `std` calls inside `async fn`: `std::thread::sleep`, `std::fs::*`, `std::net::*`. These block the async runtime's thread pool, reducing concurrency and potentially causing deadlocks.",
        fix: "Use async equivalents: `tokio::time::sleep`, `tokio::fs::*`, `tokio::net::*`. For CPU-bound work, use `tokio::task::spawn_blocking`.",
    },
    RuleDoc {
        name: "block-on-in-async",
        category: "Async",
        severity: "Error",
        description: "Flags `Runtime::block_on()` or `futures::executor::block_on()` called inside `async fn`. This blocks the current thread waiting for a future, which can deadlock the runtime if all worker threads are blocked.",
        fix: "Use `.await` instead of `block_on()`. If you need to call async code from sync context, restructure to avoid nesting runtimes.",
    },
    // ── Framework ───────────────────────────────────────────────
    RuleDoc {
        name: "tokio-main-missing",
        category: "Framework",
        severity: "Error",
        description: "Flags `async fn main()` without `#[tokio::main]` (or equivalent runtime attribute). Without it, the async runtime is not initialized and the program won't compile or will panic.",
        fix: "Add `#[tokio::main]` above `async fn main()`.",
    },
    RuleDoc {
        name: "tokio-spawn-without-move",
        category: "Framework",
        severity: "Error",
        description: "Flags `tokio::spawn(async { ... })` without the `move` keyword. Without `move`, the spawned task borrows from the enclosing scope, which often fails to compile due to lifetime requirements ('static bound on spawn).",
        fix: "Use `tokio::spawn(async move { ... })`.",
    },
    RuleDoc {
        name: "axum-handler-not-async",
        category: "Framework",
        severity: "Warning",
        description: "Flags non-async handler functions in axum. Web framework handlers run on the async runtime and must not block.",
        fix: "Make the handler `async fn` and use async I/O operations.",
    },
    RuleDoc {
        name: "actix-blocking-handler",
        category: "Framework",
        severity: "Error",
        description: "Flags blocking calls in actix-web handler functions. Web framework handlers run on the async runtime and must not block.",
        fix: "Make the handler `async fn` and use async I/O operations.",
    },
];

fn get_rule_explanation(rule: &str) -> String {
    // Look up in the data-driven registry first
    if let Some(doc) = RULE_DOCS.iter().find(|d| d.name == rule) {
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
        format!(
            "Unknown rule: `{rule}`\n\nUse the `list_rules` tool to see all available rules."
        )
    }
}

fn get_all_rules_listing() -> String {
    let mut text = String::from("# rust-doctor Rules\n\n## Custom Rules (AST-based via syn)\n\n");

    use std::fmt::Write;
    let mut current_category = "";
    for doc in RULE_DOCS {
        if doc.category != current_category {
            if !current_category.is_empty() {
                text.push('\n');
            }
            let _ = writeln!(text, "### {}", doc.category);
            current_category = doc.category;
        }
        let _ = writeln!(
            text,
            "- `{}` ({}) — {}",
            doc.name,
            doc.severity.to_lowercase(),
            doc.description.split(". ").next().unwrap_or(doc.description)
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
        let expected = crate::scan::CUSTOM_RULE_NAMES
            .iter()
            .filter(|name| **name != "unused-dependency") // external tool rule, not AST
            .collect::<Vec<_>>();

        for rule_name in &expected {
            assert!(
                RULE_DOCS.iter().any(|doc| doc.name == **rule_name),
                "RULE_DOCS is missing entry for custom rule '{rule_name}'"
            );
        }
    }

    #[test]
    fn test_rule_docs_has_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for doc in RULE_DOCS {
            assert!(
                seen.insert(doc.name),
                "RULE_DOCS has duplicate entry for '{}'",
                doc.name
            );
        }
    }

    #[test]
    fn test_rule_docs_fields_not_empty() {
        for doc in RULE_DOCS {
            assert!(!doc.name.is_empty(), "Rule has empty name");
            assert!(!doc.category.is_empty(), "Rule '{}' has empty category", doc.name);
            assert!(!doc.severity.is_empty(), "Rule '{}' has empty severity", doc.name);
            assert!(!doc.description.is_empty(), "Rule '{}' has empty description", doc.name);
            assert!(!doc.fix.is_empty(), "Rule '{}' has empty fix", doc.name);
        }
    }

    // --- get_rule_explanation ---

    #[test]
    fn test_explain_known_custom_rule() {
        let explanation = get_rule_explanation("unwrap-in-production");
        assert!(explanation.contains("## unwrap-in-production"));
        assert!(explanation.contains("Error Handling"));
        assert!(explanation.contains("Warning"));
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
        for doc in RULE_DOCS {
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
        assert_eq!(names.len(), 4, "Expected exactly 4 tools, got {}", names.len());
    }

    #[test]
    fn test_scan_tool_has_output_schema() {
        let server = RustDoctorServer::new();
        let tools = server.tool_router.list_all();
        let scan = tools.iter().find(|t| t.name == "scan").unwrap();
        assert!(scan.output_schema.is_some(), "scan tool should have outputSchema from Json<ScanOutput>");
    }

    #[test]
    fn test_score_tool_has_output_schema() {
        let server = RustDoctorServer::new();
        let tools = server.tool_router.list_all();
        let score = tools.iter().find(|t| t.name == "score").unwrap();
        assert!(score.output_schema.is_some(), "score tool should have outputSchema from Json<ScoreOutput>");
    }

    // --- discover_and_resolve error mapping ---

    #[test]
    fn test_discover_and_resolve_invalid_path() {
        let result = discover_and_resolve("/nonexistent/path/to/project");
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Should be invalid_params (not internal_error) for bad input
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}
