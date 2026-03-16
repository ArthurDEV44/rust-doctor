use crate::{clippy, config, discovery, scan};
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
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
    pub directory: String,
    /// Only scan files changed vs this base branch (e.g. "main"). Omit to scan all files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct ScoreInput {
    /// Absolute path to the Rust project directory.
    pub directory: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct ExplainRuleInput {
    /// The rule ID (e.g. "unwrap-in-production", "clippy::expect_used", "blocking-in-async").
    pub rule: String,
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
        description = "Scan a Rust project for code health issues. Returns diagnostics with a 0-100 health score covering security, performance, correctness, architecture, and dependency issues.",
        annotations(read_only_hint = true)
    )]
    async fn scan(&self, params: Parameters<ScanInput>) -> Result<CallToolResult, McpError> {
        let input = params.0;
        let (_dir, project_info, mut resolved) = discover_and_resolve(&input.directory)?;

        if let Some(diff_base) = input.diff {
            resolved.diff = Some(diff_base);
        }

        let result = scan::scan_project(&project_info, &resolved, false, &[], true)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let json = serde_json::to_string(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        name = "score",
        description = "Get the health score (0-100) of a Rust project as a single integer.",
        annotations(read_only_hint = true)
    )]
    async fn score(&self, params: Parameters<ScoreInput>) -> Result<CallToolResult, McpError> {
        let input = params.0;
        let (_dir, project_info, resolved) = discover_and_resolve(&input.directory)?;

        let result = scan::scan_project(&project_info, &resolved, false, &[], true)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            result.score.to_string(),
        )]))
    }

    #[tool(
        name = "explain_rule",
        description = "Get a detailed explanation of a rust-doctor rule: what it checks, why it matters, and how to fix violations.",
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
        description = "List all available rust-doctor rules with their categories and severities.",
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
                "Rust code health scanner. Use `scan` to analyze a project, `score` for a quick \
                 health check, `explain_rule` for rule details, and `list_rules` to see all rules.",
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
    let target_dir = Path::new(directory)
        .canonicalize()
        .map_err(|e| McpError::invalid_params(format!("Invalid directory '{directory}': {e}"), None))?;

    let cargo_toml = target_dir.join("Cargo.toml");
    if !cargo_toml.try_exists().unwrap_or(false) {
        return Err(McpError::invalid_params(
            format!("No Cargo.toml found in '{}'", target_dir.display()),
            None,
        ));
    }

    let project_info = discovery::discover_project(&cargo_toml, false)
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    let file_config =
        config::load_file_config(&project_info.root_dir, Some(&project_info.package_metadata));
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
        severity: "Warning",
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
        severity: "Warning",
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
