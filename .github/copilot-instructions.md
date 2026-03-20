# rust-doctor

Rust code health scanner. CLI binary, library crate, MCP server.
Rust 2024, MSRV 1.85, single crate.

## Commands

```bash
cargo build                        # Debug build (with MCP server)
cargo build --no-default-features  # Build without MCP server
cargo test                         # All tests (unit + integration + snapshots)
cargo test test_name               # Single test
cargo clippy --all-targets -- -W clippy::all -W clippy::pedantic -W clippy::nursery -D warnings
cargo fmt --check
```

## Code Style

- Clippy pedantic enabled (`must_use_candidate`, `module_name_repetitions`, `missing_errors_doc`, `missing_panics_doc` allowed)
- Custom errors with `thiserror::Error` — no `anyhow`, no `Box<dyn Error>` in library code
- `Result<T, E>` + `?` operator everywhere — `unwrap()` only in tests
- New analysis passes implement `AnalysisPass` trait (`name()` + `run()` → `Vec<Diagnostic>`)
- New custom rules implement `CustomRule` trait using `syn::visit::Visit`
- Diagnostics → stderr, score → stdout (intentional for piping)

## Architecture

Passes grouped by domain under `src/passes/`: `security/` (audit, deny, geiger), `static_analysis/` (clippy, rules), `quality/` (coverage, msrv, machete, semver_checks). MCP server in `src/mcp/` (feature-gated).

## Boundaries

### Always
- Run `catch_unwind` around custom rules — a panicking rule must not crash the scan
- MCP tools are read-only — no filesystem writes, directory under `$HOME` only

### Ask First
- Changing score weights
- Adding new MCP tools or modifying security hardening

### Never
- Use `anyhow` — this project uses typed errors with `thiserror`
- Add `unsafe` blocks in production code
- Skip `catch_unwind` on custom rules
- Break the stderr/stdout output routing convention
