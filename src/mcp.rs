use crate::{config, discovery, scan};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, Write};

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "rust-doctor";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

/// JSON-RPC 2.0 response.
///
/// The `id` field uses `Option<Value>` to distinguish:
/// - `None` → notification response (should not happen, but omits `id` from JSON)
/// - `Some(Value::Null)` → serialized as `"id": null` (required by spec for parse errors)
/// - `Some(number/string)` → normal request id echo
#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

// ---------------------------------------------------------------------------
// Response helpers
// ---------------------------------------------------------------------------

fn ok_response(id: Option<Value>, result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
    }
}

fn error_response(id: Option<Value>, code: i32, message: &str) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message: message.to_string(),
        }),
    }
}

fn tool_result(id: Option<Value>, text: &str) -> JsonRpcResponse {
    ok_response(
        id,
        serde_json::json!({
            "content": [{ "type": "text", "text": text }]
        }),
    )
}

fn tool_error(id: Option<Value>, text: &str) -> JsonRpcResponse {
    ok_response(
        id,
        serde_json::json!({
            "content": [{ "type": "text", "text": text }],
            "isError": true
        }),
    )
}

fn send(writer: &mut impl Write, response: &JsonRpcResponse) {
    let json = serde_json::to_string(response).unwrap_or_else(|_| {
        r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"Serialization error"}}"#
            .to_string()
    });
    let _ = writeln!(writer, "{json}");
    let _ = writer.flush();
}

// ---------------------------------------------------------------------------
// Main server loop (stdio, newline-delimited JSON-RPC)
// ---------------------------------------------------------------------------

pub fn run_mcp_server() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let reader = stdin.lock();
    let mut writer = stdout.lock();

    for line in reader.lines() {
        let line = match line {
            Ok(l) if !l.trim().is_empty() => l,
            Ok(_) => continue,
            Err(_) => break,
        };

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                // Parse error: id must be null per JSON-RPC 2.0 (cannot determine request id)
                send(
                    &mut writer,
                    &error_response(Some(Value::Null), -32700, &format!("Parse error: {e}")),
                );
                continue;
            }
        };

        // Notifications have no id — don't respond
        let Some(id) = request.id else {
            continue;
        };
        let id = Some(id);

        let response = match request.method.as_str() {
            "initialize" => handle_initialize(id),
            "ping" => ok_response(id, serde_json::json!({})),
            "tools/list" => handle_tools_list(id),
            "tools/call" => handle_tools_call(id, &request.params),
            _ => error_response(id, -32601, &format!("Method not found: {}", request.method)),
        };

        send(&mut writer, &response);
    }
}

// ---------------------------------------------------------------------------
// Protocol handlers
// ---------------------------------------------------------------------------

fn handle_initialize(id: Option<Value>) -> JsonRpcResponse {
    ok_response(
        id,
        serde_json::json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": SERVER_VERSION
            }
        }),
    )
}

fn handle_tools_list(id: Option<Value>) -> JsonRpcResponse {
    ok_response(
        id,
        serde_json::json!({
            "tools": [
                {
                    "name": "scan",
                    "description": "Scan a Rust project for code health issues. Returns diagnostics with a 0-100 health score covering security, performance, correctness, architecture, and dependency issues.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "directory": {
                                "type": "string",
                                "description": "Absolute path to the Rust project directory (must contain a Cargo.toml)"
                            },
                            "diff": {
                                "type": "string",
                                "description": "Only scan files changed vs this base branch (e.g. 'main'). Omit to scan all files."
                            }
                        },
                        "required": ["directory"]
                    }
                },
                {
                    "name": "score",
                    "description": "Get the health score (0-100) of a Rust project as a single integer.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "directory": {
                                "type": "string",
                                "description": "Absolute path to the Rust project directory"
                            }
                        },
                        "required": ["directory"]
                    }
                },
                {
                    "name": "explain_rule",
                    "description": "Get a detailed explanation of a rust-doctor rule: what it checks, why it matters, and how to fix violations.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "rule": {
                                "type": "string",
                                "description": "The rule ID (e.g. 'unwrap-in-production', 'clippy::expect_used', 'blocking-in-async')"
                            }
                        },
                        "required": ["rule"]
                    }
                },
                {
                    "name": "list_rules",
                    "description": "List all available rust-doctor rules with their categories and severities.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                }
            ]
        }),
    )
}

fn handle_tools_call(id: Option<Value>, params: &Value) -> JsonRpcResponse {
    let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(Value::Object(Default::default()));

    match tool_name {
        "scan" => handle_scan_tool(id, &arguments),
        "score" => handle_score_tool(id, &arguments),
        "explain_rule" => handle_explain_rule_tool(id, &arguments),
        "list_rules" => handle_list_rules_tool(id),
        // Unknown tool name is a protocol-level invalid params error, not a tool execution error
        _ => error_response(id, -32602, &format!("Unknown tool: {tool_name}")),
    }
}

// ---------------------------------------------------------------------------
// Tool implementations
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
    String,
> {
    let target_dir = std::path::Path::new(directory)
        .canonicalize()
        .map_err(|e| format!("Invalid directory '{directory}': {e}"))?;

    let cargo_toml = target_dir.join("Cargo.toml");
    if !cargo_toml.try_exists().unwrap_or(false) {
        return Err(format!("No Cargo.toml found in '{}'", target_dir.display()));
    }

    let project_info = discovery::discover_project(&cargo_toml, false)?;
    let file_config =
        config::load_file_config(&project_info.root_dir, Some(&project_info.package_metadata));
    let resolved = config::resolve_config_defaults(file_config.as_ref());

    Ok((target_dir, project_info, resolved))
}

fn handle_scan_tool(id: Option<Value>, args: &Value) -> JsonRpcResponse {
    let Some(directory) = args.get("directory").and_then(|v| v.as_str()) else {
        return tool_error(id, "Missing required argument: directory");
    };

    let (_target_dir, project_info, mut resolved) = match discover_and_resolve(directory) {
        Ok(t) => t,
        Err(e) => return tool_error(id, &e),
    };

    // Apply diff override from tool arguments
    if let Some(diff_base) = args.get("diff").and_then(|v| v.as_str()) {
        resolved.diff = Some(diff_base.to_string());
    }

    match scan::scan_project(&project_info, &resolved, false, &[], true) {
        Ok(result) => {
            let json = serde_json::to_string_pretty(&result)
                .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"));
            tool_result(id, &json)
        }
        Err(e) => tool_error(id, &e),
    }
}

fn handle_score_tool(id: Option<Value>, args: &Value) -> JsonRpcResponse {
    let Some(directory) = args.get("directory").and_then(|v| v.as_str()) else {
        return tool_error(id, "Missing required argument: directory");
    };

    let (_target_dir, project_info, resolved) = match discover_and_resolve(directory) {
        Ok(t) => t,
        Err(e) => return tool_error(id, &e),
    };

    match scan::scan_project(&project_info, &resolved, false, &[], true) {
        Ok(result) => tool_result(id, &result.score.to_string()),
        Err(e) => tool_error(id, &e),
    }
}

fn handle_explain_rule_tool(id: Option<Value>, args: &Value) -> JsonRpcResponse {
    let Some(rule) = args.get("rule").and_then(|v| v.as_str()) else {
        return tool_error(id, "Missing required argument: rule");
    };
    tool_result(id, &get_rule_explanation(rule))
}

fn handle_list_rules_tool(id: Option<Value>) -> JsonRpcResponse {
    tool_result(id, &get_all_rules_listing())
}

// ---------------------------------------------------------------------------
// Rule knowledge base
// ---------------------------------------------------------------------------

fn get_rule_explanation(rule: &str) -> String {
    match rule {
        // ── Error Handling ──────────────────────────────────────────
        "unwrap-in-production" => "\
## unwrap-in-production

**Category:** Error Handling | **Severity:** Warning

Flags `.unwrap()` and `.expect()` calls outside of test code. These calls \
panic at runtime if the value is `None` or `Err`, crashing your application.

**Fix:** Use the `?` operator to propagate errors, or handle them with \
`match`, `if let`, `.unwrap_or()`, or `.unwrap_or_else()`."
            .into(),

        "panic-in-library" => "\
## panic-in-library

**Category:** Error Handling | **Severity:** Error

Flags `panic!()`, `todo!()`, and `unimplemented!()` macros in library code. \
Libraries should return errors rather than panicking, since callers cannot \
recover from a panic across crate boundaries.

**Fix:** Return `Result<T, E>` or `Option<T>` instead of panicking."
            .into(),

        "box-dyn-error-in-public-api" => "\
## box-dyn-error-in-public-api

**Category:** Error Handling | **Severity:** Warning

Flags `pub fn` returning `Result<_, Box<dyn Error>>`. This erases error \
type information, making it impossible for callers to match on specific \
error variants.

**Fix:** Define a custom error enum with `thiserror` or return a concrete error type."
            .into(),

        "result-unit-error" => "\
## result-unit-error

**Category:** Error Handling | **Severity:** Warning

Flags `pub fn` returning `Result<_, ()>`. A unit error carries no information \
about what went wrong.

**Fix:** Use a meaningful error type that describes the failure."
            .into(),

        // ── Performance ─────────────────────────────────────────────
        "excessive-clone" => "\
## excessive-clone

**Category:** Performance | **Severity:** Warning

Flags `.clone()` calls that may indicate unnecessary heap allocations. \
Each clone copies the entire value, which is expensive for `String`, `Vec`, \
and other heap-allocated types.

**Fix:** Use references (`&T`) or `Cow<T>` instead of cloning. Consider \
restructuring ownership to avoid the clone."
            .into(),

        "string-from-literal" => "\
## string-from-literal

**Category:** Performance | **Severity:** Warning

Flags `String::from(\"literal\")` and `\"literal\".to_string()`. While not \
wrong, these allocate on the heap when a `&str` reference might suffice.

**Fix:** If the function accepts `&str`, pass the literal directly. If you \
need an owned `String`, this warning can be safely ignored or suppressed."
            .into(),

        "collect-then-iterate" => "\
## collect-then-iterate

**Category:** Performance | **Severity:** Warning

Flags `.collect::<Vec<_>>()` immediately followed by `.iter()`. This \
allocates a temporary vector unnecessarily since the original iterator \
could be used directly.

**Fix:** Remove the `.collect()` and chain the iterator operations directly."
            .into(),

        "large-enum-variant" => "\
## large-enum-variant

**Category:** Performance | **Severity:** Warning

Flags enums where variants have significantly different sizes (>3x field \
count disparity). The enum's size equals its largest variant, wasting \
memory for smaller variants.

**Fix:** Box the large variant's data: `LargeVariant(Box<LargeData>)`."
            .into(),

        "unnecessary-allocation" => "\
## unnecessary-allocation

**Category:** Performance | **Severity:** Warning

Flags `Vec::new()` or `String::new()` inside loops. Each iteration \
allocates a new buffer, which is expensive.

**Fix:** Move the allocation outside the loop and use `.clear()` to reuse it."
            .into(),

        // ── Security ────────────────────────────────────────────────
        "hardcoded-secrets" => "\
## hardcoded-secrets

**Category:** Security | **Severity:** Error

Flags string literals assigned to variables named `api_key`, `password`, \
`token`, `secret`, etc. (length > 8 chars). Hardcoded secrets in source \
code can be extracted from compiled binaries or version control.

**Fix:** Use environment variables, a secrets manager, or config files \
excluded from version control."
            .into(),

        "unsafe-block-audit" => "\
## unsafe-block-audit

**Category:** Security | **Severity:** Warning

Flags `unsafe {}` blocks and `unsafe fn` declarations. Unsafe code bypasses \
Rust's memory safety guarantees and must be carefully audited. Skipped if \
the crate declares `#![forbid(unsafe_code)]`.

**Fix:** Verify the safety invariants are documented and correct. Consider \
safe abstractions or crates like `zerocopy` to eliminate unsafe."
            .into(),

        "sql-injection-risk" => "\
## sql-injection-risk

**Category:** Security | **Severity:** Error

Flags `format!()` output passed to `.query()`, `.execute()`, or `.raw()` \
methods. String interpolation in SQL queries enables SQL injection attacks.

**Fix:** Use parameterized queries (`$1`, `?`) provided by your database \
library (sqlx, diesel, sea-orm)."
            .into(),

        // ── Async ───────────────────────────────────────────────────
        "blocking-in-async" => "\
## blocking-in-async

**Category:** Async | **Severity:** Warning

Flags blocking `std` calls inside `async fn`: `std::thread::sleep`, \
`std::fs::*`, `std::net::*`. These block the async runtime's thread pool, \
reducing concurrency and potentially causing deadlocks.

**Fix:** Use async equivalents: `tokio::time::sleep`, `tokio::fs::*`, \
`tokio::net::*`. For CPU-bound work, use `tokio::task::spawn_blocking`."
            .into(),

        "block-on-in-async" => "\
## block-on-in-async

**Category:** Async | **Severity:** Error

Flags `Runtime::block_on()` or `futures::executor::block_on()` called \
inside `async fn`. This blocks the current thread waiting for a future, \
which can deadlock the runtime if all worker threads are blocked.

**Fix:** Use `.await` instead of `block_on()`. If you need to call async \
code from sync context, restructure to avoid nesting runtimes."
            .into(),

        // ── Framework ───────────────────────────────────────────────
        "tokio-main-missing" => "\
## tokio-main-missing

**Category:** Framework | **Severity:** Error

Flags `async fn main()` without `#[tokio::main]` (or equivalent runtime \
attribute). Without it, the async runtime is not initialized and the \
program won't compile or will panic.

**Fix:** Add `#[tokio::main]` above `async fn main()`."
            .into(),

        "tokio-spawn-without-move" => "\
## tokio-spawn-without-move

**Category:** Framework | **Severity:** Warning

Flags `tokio::spawn(async { ... })` without the `move` keyword. Without \
`move`, the spawned task borrows from the enclosing scope, which often \
fails to compile due to lifetime requirements ('static bound on spawn).

**Fix:** Use `tokio::spawn(async move { ... })`."
            .into(),

        "axum-handler-not-async" | "actix-blocking-handler" => {
            format!(
                "\
## {rule}

**Category:** Framework | **Severity:** Warning/Error

Flags handler functions in web frameworks (axum/actix-web) that are not \
async or contain blocking calls. Web framework handlers run on the async \
runtime and must not block.

**Fix:** Make the handler `async fn` and use async I/O operations."
            )
        }

        // ── Clippy lints ────────────────────────────────────────────
        _ => {
            let lint_name = rule.strip_prefix("clippy::").unwrap_or(rule);
            if crate::clippy::known_lint_names().contains(&lint_name) {
                format!(
                    "\
## {rule}

This is a Clippy lint tracked by rust-doctor with custom severity/category mapping.

See full documentation: https://rust-lang.github.io/rust-clippy/master/index.html#{lint_name}"
                )
            } else {
                format!(
                    "Unknown rule: `{rule}`\n\n\
                     Use the `list_rules` tool to see all available rules."
                )
            }
        }
    }
}

fn get_all_rules_listing() -> String {
    let mut text = String::from("# rust-doctor Rules\n\n");

    text.push_str("## Custom Rules (AST-based via syn)\n\n");

    text.push_str("### Error Handling\n");
    text.push_str("- `unwrap-in-production` (warning) — .unwrap()/.expect() in production code\n");
    text.push_str("- `panic-in-library` (error) — panic!/todo!/unimplemented! in library code\n");
    text.push_str(
        "- `box-dyn-error-in-public-api` (warning) — Box<dyn Error> in public API returns\n",
    );
    text.push_str("- `result-unit-error` (warning) — Result<_, ()> in public API returns\n");

    text.push_str("\n### Performance\n");
    text.push_str("- `excessive-clone` (warning) — .clone() calls that may be unnecessary\n");
    text.push_str(
        "- `string-from-literal` (warning) — String::from(\"lit\") or \"lit\".to_string()\n",
    );
    text.push_str(
        "- `collect-then-iterate` (warning) — .collect() immediately followed by .iter()\n",
    );
    text.push_str("- `large-enum-variant` (warning) — Enum variants with large size disparity\n");
    text.push_str("- `unnecessary-allocation` (warning) — Vec::new()/String::new() inside loops\n");

    text.push_str("\n### Security\n");
    text.push_str(
        "- `hardcoded-secrets` (error) — String literals assigned to secret-sounding variables\n",
    );
    text.push_str("- `unsafe-block-audit` (warning) — unsafe blocks and unsafe fn\n");
    text.push_str("- `sql-injection-risk` (error) — format!() passed to query/execute methods\n");

    text.push_str("\n### Async (activated when tokio/async-std/smol detected)\n");
    text.push_str("- `blocking-in-async` (warning) — Blocking std calls inside async fn\n");
    text.push_str("- `block-on-in-async` (error) — Runtime::block_on() inside async fn\n");

    text.push_str("\n### Framework (activated per detected framework)\n");
    text.push_str("- `tokio-main-missing` (error) — async fn main() without #[tokio::main]\n");
    text.push_str("- `tokio-spawn-without-move` (warning) — tokio::spawn without move keyword\n");
    text.push_str("- `axum-handler-not-async` (warning) — Non-async axum handlers\n");
    text.push_str("- `actix-blocking-handler` (error) — Blocking calls in actix handlers\n");

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
