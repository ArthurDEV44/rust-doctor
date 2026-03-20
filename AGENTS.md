# AGENTS.md

> rust-doctor — Rust code health scanner. CLI binary, library crate, MCP server, npm package, GitHub Action.
> Rust 2024, MSRV 1.85, single crate (not a workspace).

## Commands

```bash
cargo build                        # Debug build (with MCP server)
cargo build --no-default-features  # Build without MCP server
cargo test                         # All tests (unit + integration + snapshots)
cargo test test_name               # Single test
cargo test --test integration      # Integration tests only
cargo test --test snapshots        # Snapshot tests only
cargo insta review                 # Review snapshot changes
cargo clippy --all-targets -- -W clippy::all -W clippy::pedantic -W clippy::nursery -D warnings
cargo fmt --check
```

## Project Structure

```
src/
├── main.rs            # CLI entry point, --mcp flag dispatches to mcp::run_mcp_server()
├── lib.rs             # Module declarations + crate-root re-exports for passes
├── scan.rs            # Scan orchestrator: resolve roots → run passes → dedup → score
├── scanner.rs         # AnalysisPass trait + ScanOrchestrator
├── diagnostics.rs     # Diagnostic, ScanResult, Severity, Category — central types
├── config.rs          # Config resolution (CLI > TOML > Cargo.toml metadata > defaults)
├── output/            # Score calculation + terminal/JSON rendering
│   ├── mod.rs         # render_score(), render_json(), re-exports, tests
│   ├── score.rs       # calculate_score(), dimension weights, score_label()
│   └── terminal.rs    # render_terminal(), print_score_box(), print_diagnostics()
├── mcp/               # MCP server (rmcp v1.2.0, stdio, feature-gated)
│   ├── mod.rs         # Server struct, entry point, ServerHandler impl
│   ├── types.rs       # Input/output schemas (ScanInput, ScoreInput, etc.)
│   ├── tools.rs       # Tool + prompt handler implementations
│   ├── helpers.rs     # discover_and_resolve(), format_scan_report(), group_diagnostics()
│   └── rules.rs       # Rule documentation (explain_rule, list_rules)
├── passes/            # Analysis passes grouped by domain
│   ├── security/      # Security-focused passes
│   │   ├── audit.rs   # cargo-audit pass
│   │   ├── deny.rs    # cargo-deny pass
│   │   └── geiger.rs  # cargo-geiger pass
│   ├── static_analysis/  # Code analysis passes
│   │   ├── clippy/    # Clippy integration (55+ lint registry)
│   │   │   ├── mod.rs
│   │   │   └── lint_registry.rs
│   │   └── rules/     # Custom AST rules (syn::visit::Visit)
│   │       ├── mod.rs         # RulesPass + CustomRule trait + all_custom_rules()
│   │       ├── error_handling.rs
│   │       ├── performance.rs
│   │       ├── complexity.rs  # Cyclomatic complexity rule
│   │       ├── security.rs
│   │       ├── async_rules.rs
│   │       └── framework.rs
│   └── quality/       # Quality & dependency passes
│       ├── coverage.rs
│       ├── msrv.rs
│       ├── machete.rs
│       └── semver_checks.rs
├── discovery.rs       # Project detection (frameworks, dependencies, workspace)
├── diff.rs            # Git diff filtering
├── cache.rs           # Incremental cache (.rust-doctor-cache.json)
├── suppression.rs     # Inline suppression (// rust-doctor-disable-next-line)
├── process.rs         # Subprocess runner with timeout
├── fixer.rs           # Auto-fix suggestions
├── plan.rs            # Remediation plan generation
├── sarif.rs           # SARIF output format
├── deps.rs            # Dependency analysis
├── workspace.rs       # Cargo workspace resolution
├── error.rs           # 7 thiserror error types
└── cli.rs             # clap CLI definition
tests/
├── integration.rs     # Temp Rust projects with known violations
└── snapshots.rs       # insta JSON snapshot tests
```

Note: `lib.rs` re-exports pass modules at the crate root (`pub(crate) use passes::security::audit`, etc.) so that `use crate::audit` paths work throughout the codebase.

## Code Style

- Clippy pedantic enabled (`must_use_candidate`, `module_name_repetitions`, `missing_errors_doc`, `missing_panics_doc` allowed)
- Custom errors with `thiserror::Error` — no `anyhow`, no `Box<dyn Error>` in library code
- `Result<T, E>` + `?` operator everywhere — `unwrap()` only in tests
- Two parallelism levels: rayon for scan roots, `std::thread::scope` for passes within a root
- `PassError::Skipped` for missing external tools — graceful degradation, not failure

## Testing

- Unit tests: inline `#[cfg(test)]` modules in each source file
- Integration tests: temp Rust projects via `tempfile::TempDir`, `fast_config()` skips external passes
- Snapshot tests: `insta` JSON snapshots for serialization stability
- After changing snapshots: `cargo insta review`
- Self-scan tests: several modules scan rust-doctor itself as sanity check

## Architecture Rules

### Always
- New analysis passes implement `AnalysisPass` trait (`name()` + `run()` → `Vec<Diagnostic>`)
- New custom rules implement `CustomRule` trait in the appropriate `passes/static_analysis/rules/` submodule
- Diagnostics go to stderr, score to stdout (intentional for piping)
- MCP tools are read-only — no filesystem writes, directory under `$HOME` only
- Run `catch_unwind` around custom rules — a panicking rule must not crash the scan

### Ask First
- Changing score weights (Security ×2.0, Reliability ×1.5, Maintainability ×1.0, Performance ×1.0, Dependencies ×1.0)
- Adding new MCP tools or modifying security hardening
- Changing module visibility (pub vs pub(crate))

### Never
- Use `anyhow` — this project uses typed errors with `thiserror`
- Add `unsafe` blocks in production code
- Skip `catch_unwind` on custom rules
- Break the stderr/stdout output routing convention
