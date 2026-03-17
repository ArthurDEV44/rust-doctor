[PRD]

## Changelog

| Version | Date       | Author | Changes                                        |
|---------|------------|--------|------------------------------------------------|
| 0.1     | 2026-03-17 | Claude | Initial PRD from /meta-best-practices audit    |

# PRD: rust-doctor Best Practices Remediation

## Problem Statement

A comprehensive best practices audit of rust-doctor (score: 62/100) identified 6 major gaps, 13 minor gaps, and 6 absent practices across security, performance, testing, CI, and documentation. The most critical issues are: (1) MCP tool handlers block the tokio runtime for 5-30 seconds per scan, degrading concurrent request handling; (2) the MCP `directory` parameter accepts arbitrary filesystem paths with no restriction, enabling potential code execution via crafted `Cargo.toml` build scripts; (3) the CI pipeline lacks `cargo audit` and applies weaker clippy lints than the tool itself applies to user projects, undermining credibility; (4) unconditional compilation of MCP dependencies (`tokio`, `rmcp`, `schemars`) penalizes CLI-only users and CI pipelines with ~150 extra transitive crates.

**Why now:** rust-doctor is pre-1.0 and gaining users via crates.io. Fixing these gaps before the API stabilizes avoids breaking changes. The MCP security gaps are urgent because MCP servers are increasingly invoked by AI agents that accept external input — the trust boundary is weaker than assumed.

## Overview

This initiative implements all 18 actionable findings from the `.meta/best-practices-report.md` audit, organized into 6 epics by priority tier. Quick wins (P0) can be completed in a single session. MCP security hardening (P0) addresses the trust boundary. Error handling, testing, and documentation improvements (P1) raise reliability. Strategic refactors (P2) reduce maintenance burden for future rule additions.

Target outcome: raise the best practices score from 62/100 to 85+/100.

## Goals

| Goal | Baseline (current) | Target | Timeframe |
|------|-------------------|--------|-----------|
| Best practices score | 62/100 | 85+/100 | After all P0+P1 stories |
| Security findings | 1 HIGH, 4 MEDIUM | 0 HIGH, 0 MEDIUM | After EP-001 + EP-002 |
| CI pipeline coverage | fmt + clippy + test | fmt + clippy(pedantic) + test + audit | After US-002 |
| MCP async correctness | Blocking tokio workers | Non-blocking via spawn_blocking | After US-004 |
| Build time (CLI-only) | Full build with tokio/rmcp | Conditional MCP deps via feature flag | After US-016 |
| Public API doc coverage | 0% (all doc lints suppressed) | Module-level + key types documented | After US-017 |

## Target Users

### Persona 1: rust-doctor Maintainer (Arthur)
- **Role:** Sole maintainer, responsible for quality and releases
- **Pain points:** Audit revealed gaps that could undermine trust if discovered by users or security researchers
- **Behavior:** Wants all findings addressed methodically, highest-risk first

### Persona 2: MCP Integration Consumer (AI Agent / IDE)
- **Role:** Calls rust-doctor via MCP server from Claude Code, Cursor, or other AI agents
- **Pain points:** Blocked tokio runtime causes timeout on concurrent requests. Arbitrary directory access is a security risk in shared environments
- **Behavior:** Expects fast, secure, non-blocking tool responses

### Persona 3: Library Consumer
- **Role:** Uses `rust_doctor` as a Rust library dependency via `use rust_doctor::{scan, discovery, config}`
- **Pain points:** No documentation on public API. MCP dependencies compiled even when unused
- **Behavior:** Expects documented, minimal, well-typed public API

## Research Findings

Research was conducted during the `/meta-best-practices` audit (2026-03-17) using 2 parallel agent-websearch instances (38 findings) and 2 parallel agent-explore instances (42 codebase findings). Key sources:

- [Best Practices for Tokio](https://www.oreateai.com/blog/best-practices-for-tokio-a-comprehensive-guide-to-writing-efficient-asynchronous-rust-code/fab15751330fc07d6632c61da87a5bab) — spawn_blocking for CPU-bound work in async contexts
- [Rust Security Best Practices 2025](https://corgea.com/Learn/rust-security-best-practices-2025) — forbid(unsafe_code), input validation
- [From println!() Disasters to Production: MCP Servers in Rust](https://dev.to/ejb503/from-println-disasters-to-production-building-mcp-servers-in-rust-imf) — MCP error-as-UI, structured output
- [Building a Fast and Reliable CI/CD Pipeline for Rust Crates](https://blog.nashtechglobal.com/building-a-fast-and-reliable-ci-cd-pipeline-for-rust-crates/) — fmt -> clippy -> test -> audit gates
- [How to Deal with Rust Dependencies](https://notgull.net/rust-dependencies/) — feature flags, minimal deps
- [Rust API Guidelines — Documentation](https://rust-lang.github.io/api-guidelines/documentation.html) — document every public item

Full audit report: `.meta/best-practices-report.md`

## Assumptions & Constraints

### Assumptions
- rust-doctor MCP server runs locally (same machine, same user) — not exposed over network
- `$HOME` restriction is sufficient for MCP directory validation in v0.x
- The existing test suite passes before starting this work
- `proc-macro2` with `span-locations` is required (verified: used for line/column in rule diagnostics)

### Constraints
- No breaking changes to CLI flags or exit codes (users may have CI scripts depending on them)
- No breaking changes to MCP tool schemas (existing MCP configurations must keep working)
- The `mcp` feature flag must default to enabled (`default = ["mcp"]`) to avoid breaking existing installs
- All changes must pass the quality gates defined below
- This PRD is READ-ONLY on the audit report — `.meta/best-practices-report.md` is not modified

## Quality Gates

The following commands must pass for every completed story:

```bash
cargo check && \
cargo clippy -- -W clippy::all -W clippy::pedantic -W clippy::nursery -D warnings && \
cargo fmt --check && \
cargo test
```

Note: After US-002 (CI hardening), these gates will also be enforced by CI with the addition of `cargo audit`.

---

## Epics & User Stories

### EP-001: Immediate Hardening (P0)

Quick wins from the audit — each addressable in minutes, high security/correctness impact.

#### US-001: Add `#![forbid(unsafe_code)]` to crate root

- **Description:** As a maintainer, I want the compiler to reject any `unsafe` code so that future contributions cannot accidentally introduce memory safety violations.
- **Priority:** P0
- **Size:** XS (1 point)
- **Blocked by:** none
- **Acceptance Criteria:**
  - [x]`src/lib.rs` contains `#![forbid(unsafe_code)]` as the first attribute
  - [x]`src/main.rs` contains `#![forbid(unsafe_code)]` as the first attribute
  - [x]`cargo check` passes (confirming no existing unsafe code)
  - [x]Adding an `unsafe {}` block anywhere in `src/` causes a compile error

#### US-002: Harden CI pipeline — add `cargo audit` and align clippy lints

- **Description:** As a maintainer, I want CI to run `cargo audit` for CVE detection and use the same clippy lint level that rust-doctor applies to user projects, so that the tool practices what it preaches.
- **Priority:** P0
- **Size:** S (2 points)
- **Blocked by:** none
- **Acceptance Criteria:**
  - [x]`.github/workflows/ci.yml` includes a `cargo audit` step after tests
  - [x]`.github/workflows/ci.yml` clippy step uses `-W clippy::all -W clippy::pedantic -W clippy::nursery -D warnings`
  - [x]CI still passes with the stricter lint configuration (fix any new warnings)
  - [x]If `cargo audit` is not installed, the step installs it via `cargo install cargo-audit`
  - [x]CI failure on a known CVE blocks the build (exit code 1)

#### US-003: Add file size cap before `syn::parse_file`

- **Description:** As an MCP consumer, I want rust-doctor to skip oversized `.rs` files (>10 MB) so that a crafted file cannot cause out-of-memory crashes.
- **Priority:** P0
- **Size:** S (2 points)
- **Blocked by:** none
- **Acceptance Criteria:**
  - [x]`collect_rs_files_recursive` in `src/scanner.rs` skips files larger than 10 MB
  - [x]A warning is printed to stderr when a file is skipped: `"Warning: skipping oversized file {path} ({size} bytes)"`
  - [x]The skipped file does NOT appear in diagnostics or source file count
  - [x]A unit test verifies the size check with a mock entry

#### US-004: Wrap `scan_project` in `spawn_blocking` for MCP handlers

- **Description:** As an MCP consumer, I want the scan and score tools to not block the tokio runtime so that progress notifications and concurrent requests are handled correctly.
- **Priority:** P0
- **Size:** M (3 points)
- **Blocked by:** none
- **Acceptance Criteria:**
  - [x]`mcp.rs` `scan` handler calls `scan_project` inside `tokio::task::spawn_blocking`
  - [x]`mcp.rs` `score` handler calls `scan_project` inside `tokio::task::spawn_blocking`
  - [x]Progress notifications in `scan` handler still work (sent before and after the blocking call)
  - [x]A `spawn_blocking` panic is caught and returned as `McpError::internal_error`
  - [x]Existing MCP unit tests pass without modification
  - [x]The `ScanInput`, `ProjectInfo`, and `ResolvedConfig` types satisfy `Send + 'static` (required by `spawn_blocking`)

---

### EP-002: MCP Trust Boundary (P0)

Addresses all security findings related to the MCP server's trust model.

#### US-005: Validate MCP `directory` parameter scope

- **Description:** As a security-conscious user, I want the MCP server to reject directory paths outside `$HOME` so that a prompt-injected LLM cannot scan arbitrary filesystem locations.
- **Priority:** P0
- **Size:** M (3 points)
- **Blocked by:** none
- **Acceptance Criteria:**
  - [x]`discover_and_resolve` in `mcp.rs` canonicalizes the path and verifies it starts with `$HOME`
  - [x]A path outside `$HOME` returns `McpError::invalid_params` with message "directory must be under $HOME"
  - [x]If `$HOME` is not set, the validation is skipped with a warning (graceful degradation)
  - [x]Symlink resolution happens BEFORE the `$HOME` check (via `canonicalize`)
  - [x]Existing tests for `discover_and_resolve_invalid_path` still pass
  - [x]A new test verifies rejection of `/etc/` and `/tmp/outside-home/`

#### US-006: Default MCP mode to `offline` for `cargo audit`

- **Description:** As a security-conscious user, I want MCP scans to NOT fetch from the network by default so that scanning an untrusted project cannot trigger outbound requests.
- **Priority:** P0
- **Size:** S (2 points)
- **Blocked by:** none
- **Acceptance Criteria:**
  - [x]`ScanInput` gains an optional `offline` field (default: `true` for MCP, `false` for CLI)
  - [x]When `offline` is true, `cargo audit` runs with `--no-fetch`
  - [x]The MCP `scan` tool description documents that offline mode is the default
  - [x]A new test verifies the offline default in MCP mode
  - [x]CLI `--offline` flag behavior is unchanged

#### US-007: Sanitize error messages in MCP responses

- **Description:** As a security-conscious user, I want MCP error responses to NOT contain internal filesystem paths or cargo error details so that system information is not leaked to untrusted clients.
- **Priority:** P0
- **Size:** S (2 points)
- **Blocked by:** none
- **Acceptance Criteria:**
  - [x]`discover_and_resolve` in `mcp.rs` logs the full error to stderr and returns a sanitized message to the client
  - [x]The sanitized message includes the hint but NOT the raw `cargo_metadata` error text
  - [x]Clippy stderr output included in diagnostics is truncated to 200 characters max
  - [x]A test verifies that an MCP error for a non-existent path does NOT contain the literal filesystem path
  - [x]Verbose/debug logging of full errors is available via `RUST_LOG` env var (for local debugging)

#### US-008: Config hardening — `--no-project-config` flag and glob pattern limits

- **Description:** As a CI operator, I want to bypass the project's `rust-doctor.toml` and limit glob pattern complexity so that a malicious project cannot disable security rules or cause CPU exhaustion.
- **Priority:** P0
- **Size:** M (3 points)
- **Blocked by:** none
- **Acceptance Criteria:**
  - [x]CLI gains a `--no-project-config` flag that skips `rust-doctor.toml` loading
  - [x]MCP `ScanInput` gains an optional `ignore_project_config` field (default: `false`)
  - [x]When a security-category rule (`hardcoded-secrets`, `sql-injection-risk`, `unsafe-block-audit`) is suppressed by project config, a warning is printed to stderr
  - [x]`build_glob_set` in `scanner.rs` caps patterns at 100 and individual pattern length at 256 chars
  - [x]Excess patterns are silently truncated with a warning
  - [x]A test verifies the flag prevents config loading
  - [x]A test verifies the glob pattern limits

---

### EP-003: Error Handling & Reliability (P1)

Improves error type fidelity and eliminates panics in production paths.

#### US-009: Replace `expect()` panics with `Result` in MCP entry point

- **Description:** As an MCP consumer, I want the MCP server to return proper errors instead of panicking so that IDE integrations receive structured error responses.
- **Priority:** P1
- **Size:** S (2 points)
- **Blocked by:** none
- **Acceptance Criteria:**
  - [x]`run_mcp_server()` signature changes to `fn run_mcp_server() -> Result<(), Box<dyn std::error::Error>>`
  - [x]All three `.expect()` calls are replaced with `?` operator
  - [x]`main.rs` handles the `Result` with `eprintln!` and `process::exit(1)`
  - [x]No `.expect()` or `.unwrap()` remains in `src/mcp.rs` outside of `#[cfg(test)]`

#### US-010: Type `ScanError::Workspace` and `ScanError::Diff` with proper error enums

- **Description:** As a library consumer, I want typed error variants for workspace and diff failures so that I can match on specific error causes programmatically.
- **Priority:** P1
- **Size:** M (3 points)
- **Blocked by:** none
- **Acceptance Criteria:**
  - [x]`error.rs` defines `WorkspaceError` enum with at least `UnknownMember` and `NoMembers` variants
  - [x]`error.rs` defines `DiffError` enum with at least `InvalidRef`, `GitNotFound`, and `MergeBaseFailed` variants
  - [x]`ScanError::Workspace` wraps `WorkspaceError` via `#[from]` (not `String`)
  - [x]`ScanError::Diff` wraps `DiffError` via `#[from]` (not `String`)
  - [x]All callers in `workspace.rs` and `diff.rs` return the new typed errors
  - [x]Existing tests pass with the new error types

#### US-011: Return `Result` from `load_file_config` instead of silently swallowing parse errors

- **Description:** As a user, I want to see a clear error when my `rust-doctor.toml` has a syntax error so that I don't unknowingly run with default config.
- **Priority:** P1
- **Size:** S (2 points)
- **Blocked by:** none
- **Acceptance Criteria:**
  - [x]`load_file_config` returns `Result<Option<FileConfig>, ConfigError>` instead of `Option<FileConfig>`
  - [x]`ConfigError` is added to `error.rs` with `ParseError` and `IoError` variants
  - [x]Callers in `discovery.rs` and `mcp.rs` propagate or handle the error
  - [x]A malformed `rust-doctor.toml` produces a clear error message with the file path and TOML parse error
  - [x]A missing `rust-doctor.toml` still returns `Ok(None)` silently
  - [x]A test verifies that malformed TOML returns `Err`, not `None`

---

### EP-004: Testing Improvements (P1)

Closes testing gaps identified by the audit.

#### US-012: Migrate all tests to `tempfile::TempDir`

- **Description:** As a maintainer, I want all tests to use `tempfile::TempDir` instead of fixed-name directories so that tests are isolated and clean up on panic.
- **Priority:** P1
- **Size:** S (2 points)
- **Blocked by:** none
- **Acceptance Criteria:**
  - [x]No test in the codebase uses `std::env::temp_dir().join("rust-doctor-test-*")`
  - [x]All tests creating temporary directories use `tempfile::tempdir()` or `tempfile::TempDir`
  - [x]Tests in `src/discovery.rs`, `src/config.rs`, `src/suppression.rs`, and `src/rules/mod.rs` are migrated
  - [x]All migrated tests pass
  - [x]No manual `remove_dir_all` cleanup calls remain in test code

#### US-013: Add MCP tool handler end-to-end tests

- **Description:** As a maintainer, I want end-to-end tests for the MCP `scan` and `score` tools so that regressions in the scan-to-output path are caught.
- **Priority:** P1
- **Size:** M (3 points)
- **Blocked by:** US-004 (spawn_blocking)
- **Acceptance Criteria:**
  - [x]A test calls the `scan` tool handler on a known temp Rust project and verifies the `ScanOutput` structure
  - [x]A test calls the `score` tool handler and verifies the `ScoreOutput` structure
  - [x]A test verifies that scanning a project with known issues returns expected diagnostics
  - [x]A test verifies that scanning a non-existent directory returns `McpError::invalid_params`
  - [x]Tests are in `src/mcp.rs` or `tests/integration.rs`

#### US-014: Add missing snapshot test coverage

- **Description:** As a maintainer, I want snapshot tests for all `ScoreLabel` variants and edge cases so that serialization regressions are caught.
- **Priority:** P1
- **Size:** XS (1 point)
- **Blocked by:** none
- **Acceptance Criteria:**
  - [x]`tests/snapshots.rs` includes a test for `ScoreLabel::Critical` serialization
  - [x]The snapshot file `tests/snapshots/snapshots__severity_critical.snap` is created and committed
  - [x]All three `ScoreLabel` variants are snapshot-tested

#### US-015: Fix no-op availability tests

- **Description:** As a maintainer, I want tool availability tests to either assert useful behavior or be removed so that the test suite provides real signal.
- **Priority:** P1
- **Size:** XS (1 point)
- **Blocked by:** none
- **Acceptance Criteria:**
  - [x]`test_clippy_is_available` in `src/clippy.rs` asserts `assert!(is_clippy_available())` (clippy is required for development)
  - [x]`test_cargo_audit_is_available` and `test_machete_is_available` are marked `#[ignore]` with a comment explaining they depend on optional external tools
  - [x]No test in the codebase calls a function and ignores the result

---

### EP-005: DX & Documentation (P1)

Improves developer experience for library consumers and CI users.

#### US-016: Gate MCP dependencies behind Cargo feature flag

- **Description:** As a library consumer, I want to use `rust_doctor` without pulling in `tokio`, `rmcp`, and `schemars` so that my build is faster and lighter.
- **Priority:** P1
- **Size:** L (5 points)
- **Blocked by:** US-004 (spawn_blocking), US-005..US-008 (MCP changes)
- **Acceptance Criteria:**
  - [x]`Cargo.toml` defines `[features] default = ["mcp"]` and `mcp = ["dep:rmcp", "dep:tokio", "dep:schemars"]`
  - [x]`rmcp`, `tokio`, and `schemars` are marked `optional = true`
  - [x]`src/mcp.rs` module is gated with `#[cfg(feature = "mcp")]`
  - [x]`src/lib.rs` conditionally exports `pub mod mcp` only when `feature = "mcp"` is enabled
  - [x]`src/main.rs` conditionally compiles the `--mcp` CLI flag
  - [x]`cargo build --no-default-features` compiles successfully without tokio/rmcp/schemars
  - [x]`cargo build` (with default features) compiles and works identically to before
  - [x]CI tests both `--no-default-features` and default feature builds
  - [x]`cargo publish --dry-run` succeeds

#### US-017: Add crate-level documentation and public API docs

- **Description:** As a library consumer, I want rustdoc documentation on all public modules and key types so that I can use the library without reading source code.
- **Priority:** P1
- **Size:** M (3 points)
- **Blocked by:** none
- **Acceptance Criteria:**
  - [x]`src/lib.rs` has a `//!` module-level doc comment with crate description and usage example
  - [x]Each public module (`cli`, `config`, `diagnostics`, `discovery`, `error`, `output`, `scan`) has a `//!` doc comment
  - [x]The `ScanResult`, `Diagnostic`, `Severity`, `Category`, `ScoreLabel` types have `///` doc comments
  - [x]The `scan_project` and `bootstrap_project` functions have `///` doc comments with `# Errors` sections
  - [x]Remove `#[expect(clippy::missing_errors_doc)]` and `#[expect(clippy::missing_panics_doc)]` from `lib.rs` (replacing with actual docs)
  - [x]`cargo doc --no-deps` generates without warnings

---

### EP-006: Strategic Refactors (P2)

Architectural improvements that reduce maintenance burden for future development.

#### US-018: Co-locate rule metadata with rule implementations

- **Description:** As a maintainer, I want rule metadata (name, category, severity, description, fix hint) to live alongside the rule implementation so that adding a new rule requires changes in only one place.
- **Priority:** P2
- **Size:** L (5 points)
- **Blocked by:** none
- **Acceptance Criteria:**
  - [x]The `CustomRule` trait gains methods: `category()`, `severity()`, `description()`, `fix_hint()`
  - [x]All 18 rule implementations provide these methods
  - [x]`RULE_DOCS` in `mcp.rs` is derived from the rule registry at startup (not a parallel static slice)
  - [x]`CUSTOM_RULE_NAMES` in `scan.rs` is derived from the rule registry (not a parallel static slice)
  - [x]The test `test_rule_docs_covers_all_custom_rules` passes against the new dynamic derivation
  - [x]Adding a new rule requires changes ONLY in `src/rules/{category}.rs` and the `all_rules()` function
  - [x]The `RuleDoc` struct in `mcp.rs` is removed (replaced by trait method calls)

#### US-019: Cache file contents between analysis passes

- **Description:** As a user scanning a large project, I want source files to be read from disk only once so that scans are faster.
- **Priority:** P2
- **Size:** M (3 points)
- **Blocked by:** none
- **Acceptance Criteria:**
  - [x]The rule engine pass stores file contents in a `HashMap<PathBuf, String>` (or similar)
  - [x]`apply_inline_suppressions` accepts a reference to the file cache instead of re-reading files
  - [x]No `.rs` file in the scan scope is read from disk more than once per scan
  - [x]The file cache is scoped to the scan lifetime (dropped after `scan_project` returns)
  - [x]Existing tests pass without modification
  - [x]A comment documents the cache's memory impact: "holds all scanned .rs content in memory"

#### US-020: Cache tool availability with `OnceLock`

- **Description:** As a user scanning a workspace, I want tool availability to be checked once per process so that redundant subprocess calls are eliminated.
- **Priority:** P2
- **Size:** S (2 points)
- **Blocked by:** none
- **Acceptance Criteria:**
  - [x]`is_clippy_available()` uses `static AVAILABLE: OnceLock<bool>` to cache the result
  - [x]`is_cargo_audit_available()` uses `static AVAILABLE: OnceLock<bool>` to cache the result
  - [x]`is_machete_available()` uses `static AVAILABLE: OnceLock<bool>` to cache the result
  - [x]For a 5-member workspace, only 3 subprocess calls are made for availability (not 15)
  - [x]Existing tests pass (OnceLock is transparent to callers)

---

## Functional Requirements

| ID | Requirement | Story |
|----|-------------|-------|
| FR-01 | The crate must reject `unsafe` code at compile time | US-001 |
| FR-02 | CI must run `cargo audit` and fail on known CVEs | US-002 |
| FR-03 | CI clippy must use pedantic + nursery lint groups | US-002 |
| FR-04 | Files >10 MB must be skipped during scan with a warning | US-003 |
| FR-05 | MCP scan/score handlers must not block tokio worker threads | US-004 |
| FR-06 | MCP directory parameter must be restricted to paths under `$HOME` | US-005 |
| FR-07 | MCP mode must default to offline for cargo audit | US-006 |
| FR-08 | MCP error responses must not contain internal filesystem paths | US-007 |
| FR-09 | A `--no-project-config` flag must bypass `rust-doctor.toml` loading | US-008 |
| FR-10 | Glob patterns in config must be capped at 100 patterns, 256 chars each | US-008 |
| FR-11 | MCP server entry point must not panic — return proper errors | US-009 |
| FR-12 | Workspace and diff errors must use typed enums, not String | US-010 |
| FR-13 | Malformed `rust-doctor.toml` must produce a clear error | US-011 |
| FR-14 | MCP dependencies must be gated behind an optional `mcp` feature | US-016 |
| FR-15 | All public modules and key types must have rustdoc documentation | US-017 |
| FR-16 | Rule metadata must be co-located with rule implementation | US-018 |
| FR-17 | Source files must be read at most once per scan | US-019 |
| FR-18 | Tool availability must be checked at most once per process | US-020 |

## Non-Functional Requirements

| Category | Requirement | Metric |
|----------|-------------|--------|
| Performance | MCP handlers must not block tokio workers | 0 ms of tokio worker blocking during scan |
| Performance | Tool availability check overhead per scan | <=100 ms (down from 100-300 ms per member) |
| Security | MCP directory restriction | 100% of paths outside $HOME rejected |
| Security | No internal path leakage in MCP errors | 0 raw filesystem paths in McpError messages |
| Build | CLI-only build time reduction | No tokio/rmcp/schemars with `--no-default-features` |
| Build | Default build remains backward-compatible | `cargo install rust-doctor` works identically |
| Testing | All temp directories use tempfile | 0 fixed-name temp dirs in test code |
| Testing | MCP e2e coverage | >=2 integration tests for scan/score handlers |
| Documentation | Public API doc coverage | All pub modules and key types have `///` docs |

## Edge Cases & Error States

| # | Edge Case | Handling | Story |
|---|-----------|----------|-------|
| 1 | `.rs` file exactly at 10 MB boundary | Include it (cap is strictly >) | US-003 |
| 2 | `$HOME` env var not set | Skip directory validation with stderr warning | US-005 |
| 3 | MCP directory is a symlink to outside `$HOME` | `canonicalize()` resolves symlink BEFORE `$HOME` check — rejected | US-005 |
| 4 | `cargo audit` not installed on CI runner | CI step installs it; scan pass skips with "skipped" note | US-002 |
| 5 | `rust-doctor.toml` is valid TOML but wrong schema | Return error with helpful message about expected fields | US-011 |
| 6 | Workspace with 100+ members and `OnceLock` caching | Availability checked once; all 100 members reuse cached result | US-020 |
| 7 | `spawn_blocking` task panics during scan | Caught by `JoinError`, returned as `McpError::internal_error` | US-004 |
| 8 | Glob pattern with malicious regex backtracking | `globset` uses finite automaton (no backtracking), but length cap prevents excessive compilation | US-008 |
| 9 | Unicode control characters in MCP directory path | `canonicalize` may fail — caught by error handling | US-005 |
| 10 | `--no-project-config` with `--mcp` flag | Both flags work together — MCP ignores project config | US-008 |

## Risks & Mitigations

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| `#![forbid(unsafe_code)]` breaks a dependency's build | Low | Medium | `forbid` only applies to the crate root, not dependencies |
| Feature flag `mcp` introduces conditional compilation bugs | Medium | High | CI tests both `--no-default-features` and default builds |
| `$HOME` restriction too aggressive for legitimate use cases | Low | Low | Skip validation if `$HOME` unset; document in MCP tool description |
| Co-locating rule metadata breaks MCP tool list format | Low | Medium | Run `/meta-code` self-scan after US-018 to verify output |
| Removing `#[expect(clippy::missing_errors_doc)]` surfaces many warnings | High | Low | Address in US-017 by adding actual docs; use `#[expect]` per-item for stragglers |

## Non-Goals

- **Streamable HTTP transport for MCP** — stdio is correct for local developer tools; HTTP deferred to post-1.0
- **Workspace split into multiple crates** — project is small (~10K LOC); single crate with feature flags is sufficient
- **Property-based testing with `proptest`** — valuable but not in scope for this audit remediation
- **`cargo-deny` integration** — useful for license/ban policies but not an audit finding
- **Benchmarks with `criterion`** — deferred until performance becomes a user-reported issue
- **Refactoring diagnostic inline construction** — BL-7 in audit backlog; lower priority than metadata co-location

## Files NOT to Modify

- `.meta/best-practices-report.md` — audit report is read-only reference
- `tasks/prd-rust-doctor.md` — original product PRD, separate scope
- `tasks/prd-rust-doctor-status.json` — original product status tracker
- `tests/snapshots/*.snap` — snapshots should be updated via `cargo insta review`, not manually edited
- `Cargo.lock` — will change as a side effect of `Cargo.toml` changes; do not edit directly
- `.github/workflows/release.yml` — release pipeline is out of scope

## Technical Considerations

- **Q: Should `ScanResult` gain `JsonSchema` to eliminate the `ScanOutput` duplication?** The audit noted this as an architectural smell (AM-5 in the quality audit). If `ScanResult` derives `JsonSchema`, the `ScanOutput` wrapper in `mcp.rs` can be eliminated. However, `ScanResult` uses `Duration` (not `JsonSchema`-able), so the `elapsed_secs: f64` mapping in `ScanOutput` is intentional. Decision: keep the separation but document why.

- **Q: Should the `CustomRule` trait methods for metadata use associated constants or methods?** Associated constants (`const NAME: &str`) are more ergonomic but prevent trait object usage (`dyn CustomRule`). Methods (`fn name(&self) -> &str`) work with `Box<dyn CustomRule>`. Decision: use methods, consistent with existing `fn name(&self)`.

- **Q: Should `--no-project-config` be the default in MCP mode?** The audit noted that a malicious project config can disable security rules. Making it the default in MCP mode is more secure but changes current behavior. Decision: default to false for backward compatibility, but document the flag prominently.

## Success Metrics

| Metric | Baseline | Target | Timeframe |
|--------|----------|--------|-----------|
| Best practices audit score | 62/100 | 85+/100 | After EP-001..EP-005 complete |
| Security findings (HIGH+MEDIUM) | 1 HIGH + 4 MEDIUM | 0 | After EP-001 + EP-002 |
| MCP async blocking time | 5-30s per scan | 0ms (offloaded to blocking pool) | After US-004 |
| CLI-only build crate count | ~170 crates | ~50 crates (without tokio/rmcp) | After US-016 |
| Public API doc coverage | 0% | 100% of pub modules + key types | After US-017 |
| Test temp dir hygiene | 6+ fixed-name dirs | 0 fixed-name dirs | After US-012 |

## Open Questions

1. Should `US-016` (feature flag) pin the default to `mcp` or leave it off? Current decision: `default = ["mcp"]` for backward compatibility. Revisit if crates.io analytics show most users are CLI-only.
2. Should the `$HOME` restriction in `US-005` be configurable via an env var (e.g., `RUST_DOCTOR_ALLOWED_ROOTS`)? Current decision: no, YAGNI for v0.x.
3. Should `US-018` (rule metadata co-location) also move the rule tests into the same file as the rule implementation? Current decision: out of scope for this story — address in a follow-up if needed.

[/PRD]
