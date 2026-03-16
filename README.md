# rust-doctor

A unified code health tool for Rust — scan, score, and fix your codebase.

rust-doctor scans Rust projects for security, performance, correctness, architecture, and dependency issues, producing a 0–100 health score with actionable diagnostics.

## Features

- **700+ clippy lints** with severity overrides and category mapping
- **18 custom AST rules** via syn: error handling, performance, security, async, framework anti-patterns
- **Async anti-pattern detection**: blocking calls in async, block_on in async context
- **Framework-specific rules**: tokio, axum, actix-web
- **Dependency auditing**: CVE detection via cargo-audit, unused deps via cargo-machete
- **Health score**: 0–100 with ASCII doctor face output
- **MCP server**: integrate with Claude Code, Cursor, or any MCP-compatible AI tool
- **Diff mode**: scan only changed files for fast CI feedback
- **Workspace support**: scan all crates or select specific members
- **Inline suppression**: `// rust-doctor-disable-next-line <rule>`
- **Multiple output modes**: terminal, `--json`, `--score`
- **Library crate**: use rust-doctor programmatically via `lib.rs`
- **NO_COLOR support**: respects the NO_COLOR environment variable

## Installation

### cargo install (from source)

```bash
cargo install rust-doctor
```

### cargo binstall (pre-built binary)

```bash
cargo binstall rust-doctor
```

### Shell installer (Linux/macOS)

```bash
curl -fsSL https://github.com/ArthurDEV44/rust-doctor/releases/latest/download/install.sh | bash
```

### PowerShell installer (Windows)

```powershell
irm https://github.com/ArthurDEV44/rust-doctor/releases/latest/download/install.ps1 | iex
```

### GitHub Releases

Download pre-built binaries from [GitHub Releases](https://github.com/ArthurDEV44/rust-doctor/releases).

Available platforms:
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

## Usage

```bash
# Scan current directory
rust-doctor

# Scan a specific directory
rust-doctor /path/to/project

# Get bare score for CI
rust-doctor --score

# JSON output
rust-doctor --json

# Scan only changed files
rust-doctor --diff

# Scan against a specific branch
rust-doctor --diff main

# Fail CI on errors
rust-doctor --fail-on error

# Scan specific workspace members
rust-doctor --project core,api

# Verbose output with file:line details
rust-doctor --verbose

# Run as MCP server
rust-doctor --mcp
```

## MCP Server

rust-doctor includes a built-in [Model Context Protocol](https://modelcontextprotocol.io/) server, allowing AI coding assistants to scan and analyze Rust projects directly.

### Tools

| Tool | Description |
|------|-------------|
| `scan` | Scan a Rust project for code health issues. Returns diagnostics with a 0–100 health score. |
| `score` | Get the health score (0–100) of a Rust project as a single integer. |
| `explain_rule` | Get a detailed explanation of a rule: what it checks, why it matters, and how to fix violations. |
| `list_rules` | List all available rules with their categories and severities. |

All tools are read-only (`readOnlyHint: true`).

### Claude Code

Add to your `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "rust-doctor": {
      "command": "rust-doctor",
      "args": ["--mcp"]
    }
  }
}
```

### Cursor

Add to your `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "rust-doctor": {
      "command": "rust-doctor",
      "args": ["--mcp"]
    }
  }
}
```

### Other MCP clients

rust-doctor uses stdio transport. Any MCP client that supports stdio can connect by running `rust-doctor --mcp`.

Built with [rmcp](https://crates.io/crates/rmcp) v1.x (official Rust MCP SDK).

## GitHub Actions

```yaml
- uses: ArthurDEV44/rust-doctor@v1
  with:
    token: ${{ secrets.GITHUB_TOKEN }}
    fail-on: warning
```

The action posts a PR comment with the health score, error/warning counts, and top diagnostics.

## Configuration

Create a `rust-doctor.toml` in your project root, or add `[package.metadata.rust-doctor]` to your `Cargo.toml`:

```toml
# rust-doctor.toml
verbose = false
fail_on = "none"

[ignore]
rules = ["excessive-clone", "string-from-literal"]
files = ["**/generated/**", "tests/**"]
```

CLI flags override config file values.

## Inline Suppression

```rust
// rust-doctor-disable-next-line unwrap-in-production
let value = some_option.unwrap();

let x = risky_call(); // rust-doctor-disable-line
```

## Rules

### Custom AST Rules (18 rules)

| Category | Rule | Severity |
|----------|------|----------|
| Error Handling | `unwrap-in-production` | Warning |
| Error Handling | `panic-in-library` | Error |
| Error Handling | `box-dyn-error-in-public-api` | Warning |
| Error Handling | `result-unit-error` | Warning |
| Performance | `excessive-clone` | Warning |
| Performance | `string-from-literal` | Warning |
| Performance | `collect-then-iterate` | Warning |
| Performance | `large-enum-variant` | Warning |
| Performance | `unnecessary-allocation` | Warning |
| Security | `hardcoded-secrets` | Error |
| Security | `unsafe-block-audit` | Warning |
| Security | `sql-injection-risk` | Error |
| Async | `blocking-in-async` | Warning |
| Async | `block-on-in-async` | Error |
| Framework | `tokio-main-missing` | Error |
| Framework | `tokio-spawn-without-move` | Warning |
| Framework | `axum-handler-not-async` | Warning |
| Framework | `actix-blocking-handler` | Error |

### Clippy Lints (55+ with overrides)

rust-doctor runs `cargo clippy` with pedantic, nursery, and cargo lint groups. 55+ lints have explicit category and severity overrides across: Error Handling, Performance, Security, Correctness, Architecture, Cargo, Async, Style.

### External Tools

- **cargo-audit** — Vulnerability scanning for dependencies
- **cargo-machete** — Unused dependency detection

## Library Usage

rust-doctor is available as a library crate:

```rust
use rust_doctor::{config, discovery, scan};

let manifest = std::path::Path::new("/path/to/project/Cargo.toml");
let project_info = discovery::discover_project(manifest, false)?;

let file_config = config::load_file_config(&project_info.root_dir, Some(&project_info.package_metadata));
let resolved = config::resolve_config_defaults(file_config.as_ref());

let result = scan::scan_project(&project_info, &resolved, false, &[], true)?;
println!("Score: {}/100 ({})", result.score, result.score_label);
```

## Score Calculation

Score = `100 - (unique_error_rules × 1.5) - (unique_warning_rules × 0.75)`, clamped to 0–100.

The score counts unique rules violated, not occurrences — fixing one instance of `.unwrap()` won't change the score, but eliminating all `.unwrap()` calls removes the penalty entirely.

| Score | Label | Doctor |
|-------|-------|--------|
| 75–100 | Great | ◠ ◠ |
| 50–74 | Needs work | • • |
| 0–49 | Critical | x x |

## License

MIT OR Apache-2.0
