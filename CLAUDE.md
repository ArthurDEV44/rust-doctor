# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

rust-doctor is a unified code health tool for Rust. It scans projects for security, performance, correctness, architecture, and dependency issues, producing a 0–100 health score with actionable diagnostics. Distributed as a CLI binary, library crate, MCP server, npm package, and GitHub Action.

**Edition:** Rust 2024, MSRV 1.85, single crate (not a workspace).

## Build & Test Commands

```bash
cargo build                        # Debug build (with MCP server)
cargo build --no-default-features  # Build without MCP server
cargo test                         # All tests (unit + integration + snapshots)
cargo test test_name               # Single test
cargo test --test integration      # Integration tests only
cargo test --test snapshots        # Snapshot tests only
cargo insta review                 # Review snapshot changes (after test failures)
cargo clippy --all-targets -- -W clippy::all -W clippy::pedantic -W clippy::nursery -D warnings  # CI lint check
cargo fmt --check                  # Format check
```

The `RUSTFLAGS="-Zproc-macro-backtrace-on-nightly-only"` env var is NOT needed — `proc-macro2` span-locations feature handles source mapping.

## Architecture

### Execution Pipeline

```
main.rs → --mcp flag? → mcp::run_mcp_server() (stdio transport, rmcp SDK)
        → otherwise  → discovery::bootstrap_project()  → config resolution
                      → scan::scan_project()            → orchestrator
                      → output::render_*()              → terminal/json/score/sarif
```

### Scan Pipeline (`scan.rs`)

1. `resolve_scan_roots()` — workspace members or single project
2. `run_passes()` — parallel over scan roots (rayon), then parallel passes per root (std::thread::scope)
3. `dedup_diagnostics()` → `filter_to_changed_files()` (if --diff) → `apply_inline_suppressions()`
4. Score calculation → output

### Module Visibility

- **Public API** (`pub mod`): `cli`, `config`, `diagnostics`, `discovery`, `error`, `fixer`, `mcp`, `output`, `plan`, `sarif`, `scan`
- **Internal** (`pub(crate) mod`): `passes` (re-exported as `audit`, `clippy`, `rules`, etc.), `cache`, `diff`, `process`, `scanner`, `suppression`, `workspace`

### Analysis Passes (`scanner.rs`)

All passes implement `AnalysisPass` trait (`name()` + `run()` → `Vec<Diagnostic>`). Passes run in parallel via `std::thread::scope`. `PassError::Skipped` is used when external tools aren't installed — emits an Info diagnostic instead of failing. Passes are grouped by domain under `src/passes/`: `security/` (audit, deny, geiger), `static_analysis/` (clippy, rules), `quality/` (coverage, msrv, machete, semver_checks).

### Custom Rule System (`src/passes/static_analysis/rules/`)

Rules implement `CustomRule` trait in `rules/mod.rs`. Each rule uses `syn::visit::Visit` to walk the AST. Rules are organized by category across submodules:
- `error_handling.rs` — unwrap, panic, box-dyn-error, result-unit-error
- `performance.rs` — clone, string-from-literal, collect-iterate, large-enum, allocation
- `complexity.rs` — high-cyclomatic-complexity
- `security.rs` — hardcoded-secrets, unsafe-block-audit, sql-injection
- `async_rules.rs` — blocking-in-async, block-on-in-async
- `framework.rs` — tokio-main, axum-handler, actix-blocking, tokio-spawn

Rules are collected by `all_custom_rules()`. Each rule runs inside `catch_unwind` — a panicking rule emits a warning and doesn't crash the scan. Framework-specific and async rules are conditionally included based on `ProjectInfo.frameworks` (detected from dependencies at discovery time).

### Score Calculation (`output/score.rs`)

Dimension-based weighted scoring across 5 dimensions (Security ×2.0, Reliability ×1.5, Maintainability ×1.0, Performance ×1.0, Dependencies ×1.0). Counts **unique rules** violated, not occurrences. Clamped to [0, 100]. Terminal rendering in `output/terminal.rs`, simple renderers (score/json) in `output/mod.rs`.

### Clippy Integration (`clippy.rs`)

Spawns `cargo clippy --message-format=json` with 120s timeout. 55+ lints in static `LINT_REGISTRY` with category/severity overrides. Unlisted lints inherit clippy defaults and map to `Category::Style`.

### MCP Server (`src/mcp/`, feature-gated)

4 tools (scan, score, explain_rule, list_rules), all read-only. Split into `mod.rs` (server + entry), `types.rs` (schemas), `tools.rs` (handlers), `helpers.rs` (formatting), `rules.rs` (rule docs). Security hardening: directory must be under `$HOME`, 5-minute timeout, offline mode by default, path sanitization in errors. Built with rmcp v1.2.0 over stdio transport.

### Configuration Priority

CLI flags > `rust-doctor.toml` > `[package.metadata.rust-doctor]` in Cargo.toml > defaults

### Output Routing

Diagnostics → **stderr**, score box → **stdout**. This is intentional for piping (`--score` prints bare integer to stdout).

## Testing Patterns

- **Unit tests**: inline `#[cfg(test)]` in every module
- **Integration tests** (`tests/integration.rs`): create temp Rust projects with known violations via `tempfile::TempDir`, use `fast_config()` to skip external tool passes
- **Snapshot tests** (`tests/snapshots.rs`): `insta` JSON snapshots for serialization stability
- **Self-scan tests**: several modules scan the rust-doctor codebase itself as a sanity check

## Compaction Guidance

When compacting, always preserve: the list of modified files, current test commands, any error types or trait impls being changed, and the scan pipeline stage being worked on.

## Key Design Decisions

- `proc-macro2` with `span-locations` feature enables source line/column from `syn` AST nodes
- Two levels of parallelism: rayon for scan roots, std::thread::scope for passes within a root
- Incremental cache (`.rust-doctor-cache.json`) keyed by config hash + file content hash
- `--no-project-config` bypasses file config (used in MCP for untrusted projects)
- Release profile uses `opt-level = "z"` (size-optimized) for npm binary distribution
- Clippy pedantic is enabled project-wide with specific allows (`must_use_candidate`, `module_name_repetitions`, `missing_errors_doc`, `missing_panics_doc`)

## Recommended Hooks

Critical rules that benefit from deterministic enforcement via `.claude/settings.json` hooks rather than advisory CLAUDE.md instructions:

- `cargo fmt --check` — pre-commit: formatting is non-negotiable
- `cargo clippy --all-targets -- -W clippy::all -W clippy::pedantic -W clippy::nursery -D warnings` — post-edit: lint pedantic obligatory
- Verify no `unsafe` in modified files — post-edit: production unsafe is banned
