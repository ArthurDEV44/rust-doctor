[PRD]

## Changelog

| Version | Date       | Author | Changes                    |
|---------|------------|--------|----------------------------|
| 0.1     | 2026-03-15 | Arthur | Initial PRD — full scope   |

# PRD: rust-doctor

## Problem Statement

Rust developers lack a unified code health tool. Today, assessing a Rust codebase requires running 5+ separate tools independently (clippy, cargo-audit, cargo-machete, cargo-geiger, manual code review), then mentally aggregating results with no common severity model, no single score, and no opinionated guidance on what matters most. This fragmentation means most teams only run clippy in CI and miss entire categories of issues — security vulnerabilities in dependencies go undetected, async anti-patterns cause production deadlocks, excessive `.clone()` calls degrade performance silently, and `unsafe` blocks accumulate without review.

**Why now:** The Rust ecosystem has matured to the point where individual analysis tools are excellent (clippy has 700+ lints, cargo-audit covers RustSec, cargo-machete catches unused deps), but no one has aggregated them into a single developer-friendly experience with a health score. React has react-doctor; Rust has nothing equivalent. The gap is confirmed by research — no competing tool exists as of March 2026.

## Overview

rust-doctor is a Rust CLI tool that scans Rust codebases for security, performance, correctness, architecture, and dependency issues, producing a 0–100 health score with actionable diagnostics. It aggregates cargo clippy, cargo-audit, cargo-machete, and custom syn-based AST rules into a single scan with unified severity levels and a polished terminal output featuring an ASCII doctor face.

Written in Rust (dogfooding), distributed via crates.io and pre-built binaries via cargo-dist, it supports Cargo workspaces, diff mode (scan only changed files), framework-specific rules (tokio, axum, actix-web), and GitHub Actions integration.

## Goals

| Goal | Month-1 Target | Month-6 Target |
|------|----------------|----------------|
| Rule coverage | 30+ rules across 6 categories | 60+ rules across 9 categories |
| Scan speed | <10s for a 50-crate workspace | <5s for a 50-crate workspace |
| Adoption | Published on crates.io, 50+ installs | 500+ installs, 100+ GitHub stars |
| CI integration | GitHub Actions composite action | GitHub Actions + GitLab CI template |
| Ecosystem detection | tokio, axum, actix-web | +rocket, warp, diesel, sqlx, sea-orm, tonic, embedded |

## Target Users

### Persona 1: Solo Rust Developer
- **Role:** Individual developer maintaining 1-5 Rust crates
- **Pain points:** Runs `cargo clippy` but misses dependency vulnerabilities, performance anti-patterns, and unsafe accumulation. No single view of codebase health.
- **Current tools:** cargo clippy, occasional cargo-audit
- **Behavior:** Wants a single command that tells them "your code is healthy" or "fix these 3 things"

### Persona 2: Rust Team Lead
- **Role:** Engineering lead managing a Rust monorepo with 10-50 crates
- **Pain points:** No way to enforce code quality standards across the workspace. Different developers have different clippy configurations. No objective health metric for code reviews.
- **Current tools:** clippy in CI, manual code review
- **Behavior:** Wants a CI-integrated health score that blocks PRs below a threshold and posts results as PR comments

### Persona 3: AI Coding Agent
- **Role:** Claude Code, Cursor, Codex running in automated mode
- **Pain points:** Needs a single command to verify code quality after generating Rust code. Cannot run 5 separate tools and aggregate results.
- **Current tools:** `cargo check && cargo clippy`
- **Behavior:** Runs `rust-doctor . --score` to get a numeric health score, uses `--fail-on error` to gate commits

## Research Findings

### Competitive Landscape
- **No direct competitor exists** — there is no unified Rust code health tool with a score
- **cargo-deny** is the closest (advisory + license + ban + duplicate checks) but has no health score, no custom rules, and no performance/async analysis
- **clippy-sarif** converts clippy output to SARIF but doesn't aggregate other tools
- **dylint** enables custom project-scoped lints but requires nightly and is developer-facing, not a health tool

### Technical Landscape
- **cargo clippy --message-format=json** provides stable, well-structured diagnostic output with lint name, severity, file spans, and suggested fixes
- **cargo_metadata** crate provides typed Rust structs for workspace/dependency analysis
- **syn 2.x** with `features = ["full"]` is the standard for Rust AST parsing — used by all proc-macro crates
- **cargo-machete** is preferred over cargo-udeps: faster (no compilation), no nightly required, used by Apache DataFusion in CI
- **cargo-audit --json** outputs structured vulnerability data from RustSec
- **clap 4 derive API** is the gold standard for Rust CLIs
- **owo-colors 4** is preferred over colored: zero-allocation, no_std, respects NO_COLOR
- **indicatif 0.18** for progress/spinners, integrates with tracing
- **cargo-dist** for automated cross-platform binary releases

### Common Rust Anti-Patterns (rule candidates)
- **Error handling:** `.unwrap()` / `.expect()` in production, `panic!()` in library code, `Box<dyn Error>` in public library APIs
- **Performance:** excessive `.clone()`, `String` where `&str` suffices, `.collect()` then `.iter()`, `HashMap` for small sets
- **Async:** `std::thread::sleep` in async, `Mutex` held across `.await`, `block_on` inside async
- **Safety:** `unsafe` without documented invariants, transmute abuse
- **Architecture:** god modules (>500 lines), too many dependencies, circular module references

### Key Risks
- **cargo clippy may fail** on projects that don't compile — must handle gracefully
- **External tool availability** — cargo-audit, cargo-machete may not be installed — must detect and skip gracefully
- **Large workspaces** — scanning 100+ crates could be slow — need parallel execution
- **no_std projects** — many rules don't apply — need framework detection to skip irrelevant rules

*Sources: RustSec Advisory Database, Cargo Book (cargo-metadata docs), rustc JSON output docs, dylint GitHub, cargo-semver-checks crates.io, GeekWala security blog (Feb 2026), Rust Anti-Patterns Book, Async Rust Pitfalls guide (reintech.io Feb 2026)*

## Assumptions & Constraints

### Assumptions
- Users have `rustup` and a recent stable Rust toolchain installed (1.75+)
- `cargo clippy` is available via the toolchain (installed by default with rustup)
- Projects being scanned have a valid `Cargo.toml` at the root or workspace root
- External tools (cargo-audit, cargo-machete) are optional — rust-doctor degrades gracefully if they're missing

### Constraints
- Must be written in Rust (dogfooding principle)
- Must work on stable Rust — no nightly requirement for rust-doctor itself
- Must work on Linux, macOS, and Windows
- Must complete a scan of a single crate in <5 seconds (excluding compilation time)
- Must not modify any source files (read-only analysis) — unlike react-doctor which temporarily rewrites disable comments
- External tool spawning must be parallel where possible

## Quality Gates

Every story must pass:
```bash
cargo check && cargo clippy -- -D warnings && cargo test && cargo fmt --check
```

## Epics & User Stories

### EP-001: Core Infrastructure (P0)

Foundation: CLI skeleton, project discovery, configuration, scan orchestrator, scoring, and terminal output.

---

#### US-001: Initialize Cargo project with clap CLI

**As a** Rust developer, **I want to** run `rust-doctor [directory]` from the terminal **so that** I can scan my project with a single command.

**Acceptance Criteria:**
- [x] Cargo binary crate initialized with `name = "rust-doctor"`, edition 2024
- [x] clap 4 derive API parses: positional `directory` (default `.`), `--verbose`, `--score`, `--json`, `--diff [base]`, `--fail-on <error|warning|none>`, `--offline`, `-y/--yes`, `--project <names>`, `-v/--version`, `-h/--help`
- [x] Running `rust-doctor --version` prints version from Cargo.toml
- [x] Running `rust-doctor --help` prints formatted help text
- [x] Running `rust-doctor` with no args defaults to scanning current directory
- [x] Error: running in a directory with no Cargo.toml prints clear error message and exits with code 1
- [x] CI environment auto-detection: skip interactive prompts when `CI`, `CLAUDECODE`, `CURSOR_AGENT`, `CODEX_CI` env vars are set

**Priority:** P0 | **Size:** S (2 pts) | **Blocked by:** —

---

#### US-002: Project discovery via cargo_metadata

**As a** rust-doctor scanner, **I want to** auto-detect project characteristics **so that** I can enable/disable rules per ecosystem.

**Acceptance Criteria:**
- [x] Run `cargo metadata --format-version 1 --no-deps` and parse output via `cargo_metadata` crate
- [x] Extract: workspace root, workspace members, Rust edition, package name, package version
- [x] Detect frameworks/runtimes from dependencies: tokio, axum, actix-web, rocket, warp, diesel, sqlx, sea-orm, tonic, wasm-bindgen, web-sys, embassy-*, cortex-m
- [x] Detect `#![no_std]` by scanning lib.rs/main.rs first 10 lines
- [x] Store discovery results in a `ProjectInfo` struct: `root_dir`, `name`, `edition`, `frameworks: Vec<Framework>`, `is_workspace`, `member_count`, `has_build_script`, `rust_version`
- [x] Error: if `cargo metadata` fails (e.g., broken Cargo.toml), print diagnostic and exit gracefully with partial results

**Priority:** P0 | **Size:** M (3 pts) | **Blocked by:** US-001

---

#### US-003: Configuration system

**As a** developer, **I want to** configure which rules to ignore and which files to skip **so that** rust-doctor works for my project's specific needs.

**Acceptance Criteria:**
- [x] Load config from `rust-doctor.toml` in project root (first priority)
- [x] Fall back to `[package.metadata.rust-doctor]` in Cargo.toml
- [x] Config shape: `ignore.rules: Vec<String>`, `ignore.files: Vec<String>` (glob patterns), `lint: bool`, `dependencies: bool`, `verbose: bool`, `diff: Option<String>`, `fail_on: String`
- [x] CLI flags override config file values
- [x] Invalid config file prints warning with specific parse error and continues with defaults
- [x] Error: config references a non-existent rule name → print warning listing valid rule names

**Priority:** P0 | **Size:** S (2 pts) | **Blocked by:** US-001

---

#### US-004: Scan orchestrator and diagnostic types

**As a** rust-doctor core, **I want to** orchestrate multiple analysis passes in parallel and merge results **so that** scanning is fast and comprehensive.

**Acceptance Criteria:**
- [x] Define `Diagnostic` struct: `file_path`, `rule`, `category`, `severity` (Error/Warning), `message`, `help`, `line`, `column`
- [x] Define `ScanResult` struct: `diagnostics: Vec<Diagnostic>`, `project: ProjectInfo`, `elapsed: Duration`, `source_file_count: usize`
- [x] Scan orchestrator runs analysis passes in parallel using `std::thread` or `rayon`
- [x] Analysis passes (pluggable): clippy pass, custom rules pass, dependency pass — each returns `Vec<Diagnostic>`
- [x] Combined diagnostics are filtered by config `ignore.rules` and `ignore.files`
- [x] Spinner displayed during scan using `indicatif` (suppressed when `--score` or `--json` flag)
- [x] Error: if all analysis passes fail, print "No analysis could be completed" with individual pass errors

**Priority:** P0 | **Size:** M (3 pts) | **Blocked by:** US-002, US-003

---

#### US-005: Score calculation and terminal output

**As a** developer, **I want to** see a health score and formatted diagnostics in my terminal **so that** I can quickly assess my codebase's health.

**Acceptance Criteria:**
- [x] Score formula: `100 - (unique_error_rules × 1.5) - (unique_warning_rules × 0.75)`, clamped to 0–100
- [x] Score is calculated per unique rule violated (not per occurrence), matching react-doctor's approach
- [x] ASCII framed box output with doctor face: happy (score >= 75), neutral (50-74), sad (<50)
- [x] Box displays: score with label ("Great"/"Needs work"/"Critical"), progress bar, error count, warning count, files scanned, scan duration
- [x] `--score` flag: print bare integer to stdout only (for CI piping)
- [x] `--json` flag: print full `ScanResult` as JSON to stdout
- [x] `--verbose` flag: show file:line details per diagnostic
- [x] Diagnostics grouped by severity (errors first, then warnings), each showing rule name, message, occurrence count
- [x] `--fail-on error`: exit code 1 if any errors found. `--fail-on warning`: exit code 1 if any errors or warnings
- [x] Colors respect `NO_COLOR` env var via owo-colors
- [x] Error: score calculation with zero files scanned prints "No Rust source files found" instead of showing 100/100

**Priority:** P0 | **Size:** M (3 pts) | **Blocked by:** US-004

---

### EP-002: Clippy Analysis Pass (P0)

Integrate cargo clippy as the primary linting backend.

---

#### US-006: Run clippy and parse JSON output into diagnostics

**As a** rust-doctor scanner, **I want to** run cargo clippy and convert its output to rust-doctor diagnostics **so that** all 700+ clippy lints are available.

**Acceptance Criteria:**
- [x] Spawn `cargo clippy --message-format=json --all-targets --all-features -- -W clippy::all -W clippy::pedantic -W clippy::nursery -W clippy::cargo` as subprocess
- [x] Parse each JSON line with `reason: "compiler-message"`, extract `message.code.code`, `message.level`, `message.message`, `message.spans[0]` (file, line, column)
- [x] Map clippy lint names to rust-doctor categories: `clippy::unwrap_used` → Error Handling, `clippy::clone_on_copy` → Performance, etc.
- [x] Map clippy severity: `"warning"` → Warning, `"error"` → Error. Filter out `"note"` and `"help"` level messages
- [x] Capture clippy's `rendered` field for verbose output
- [x] Timeout: kill clippy process after 120 seconds, report partial results
- [x] Error: if clippy is not installed, print "clippy not found — install with: rustup component add clippy" and skip this pass (do not fail entire scan)
- [x] Error: if project doesn't compile, capture compiler errors and report them as diagnostics with severity Error

**Priority:** P0 | **Size:** M (3 pts) | **Blocked by:** US-004

---

#### US-007: Clippy lint category mapping and allow-directive handling

**As a** developer, **I want** rust-doctor to see the real state of my code, not what clippy allows have hidden **so that** the health score reflects actual code quality.

**Acceptance Criteria:**
- [x] Before running clippy, generate a temporary clippy.toml or pass `-W` flags that re-enable commonly suppressed lints
- [x] Map 50+ most impactful clippy lints to rust-doctor categories with assigned severities (see mapping table in Technical Considerations)
- [x] Lints not in the mapping table inherit clippy's default severity
- [x] `rust-doctor-disable-next-line <rule>` comments in source code are NOT processed during clippy pass (they're handled in the diagnostic filter step, US-018)
- [x] Error: invalid lint name in mapping table logs a warning at startup

**Priority:** P0 | **Size:** S (2 pts) | **Blocked by:** US-006

---

### EP-003: Custom AST Rules via syn (P0)

Rules that go beyond clippy: patterns specific to rust-doctor's opinionated analysis.

---

#### US-008: syn-based rule engine with file visitor

**As a** rust-doctor developer, **I want** a rule engine that walks Rust AST via syn **so that** I can implement custom rules clippy doesn't cover.

**Acceptance Criteria:**
- [x] Define `CustomRule` trait: `fn name(&self) -> &str`, `fn category(&self) -> Category`, `fn severity(&self) -> Severity`, `fn check_file(&self, syntax: &syn::File, path: &Path) -> Vec<Diagnostic>`
- [x] Rule registry: `Vec<Box<dyn CustomRule>>` populated at startup based on detected frameworks
- [x] For each `.rs` file in the project's `src/` directory: read file, parse with `syn::parse_file()`, run all registered rules
- [x] Skip files matching `ignore.files` config patterns
- [x] Parse errors are non-fatal: log warning and skip the file
- [x] Files are processed in parallel using rayon
- [x] Error: if a custom rule panics, catch it, log the rule name and file path, and continue with remaining rules

**Priority:** P0 | **Size:** M (3 pts) | **Blocked by:** US-004

---

#### US-009: Error handling rules

**As a** developer, **I want** rust-doctor to catch error handling anti-patterns **so that** my code handles failures gracefully.

**Rules implemented:**
- [x] `unwrap-in-production`: Flag `.unwrap()` and `.expect()` calls outside of `#[test]` and `#[cfg(test)]` modules — severity: Warning
- [x] `panic-in-library`: Flag `panic!()`, `todo!()`, `unimplemented!()` in library crates (not binary crates) — severity: Error
- [x] `box-dyn-error-in-public-api`: Flag public functions returning `Box<dyn Error>` or `Box<dyn std::error::Error>` — severity: Warning
- [x] `result-unit-error`: Flag `Result<T, ()>` in public APIs — severity: Warning
- [x] Each rule produces diagnostics with actionable help text (e.g., "Use `?` with `anyhow::Result` or define a custom error type with `thiserror`")
- [x] Rules correctly skip `#[test]` functions and `#[cfg(test)]` modules
- [x] Error: false positive on `.unwrap()` called on `Option<Infallible>` or after `.is_some()` check — accepted as known limitation, documentable via inline suppression

**Priority:** P0 | **Size:** M (3 pts) | **Blocked by:** US-008

---

#### US-010: Performance rules

**As a** developer, **I want** rust-doctor to catch performance anti-patterns **so that** I avoid unnecessary allocations and copies.

**Rules implemented:**
- [ ] `excessive-clone`: Flag `.clone()` on types that implement `Copy` — severity: Warning
- [ ] `string-from-literal`: Flag `String::from("literal")` or `"literal".to_string()` where `&str` could be used (in function args accepting `impl AsRef<str>`) — severity: Warning
- [ ] `collect-then-iterate`: Flag `.collect::<Vec<_>>()` immediately followed by `.iter()` or `.into_iter()` — severity: Warning
- [ ] `large-enum-variant`: Flag enums where the largest variant is >3x the size of the smallest (heuristic: count fields) — severity: Warning
- [ ] `unnecessary-allocation`: Flag `Vec::new()` in loops without pre-allocation hint — severity: Warning
- [ ] Each rule includes help text with the recommended fix pattern
- [ ] Error: rule triggers on code inside a macro expansion — accepted as known limitation

**Priority:** P0 | **Size:** M (3 pts) | **Blocked by:** US-008

---

#### US-011: Security rules

**As a** developer, **I want** rust-doctor to catch security issues in my code **so that** I don't ship vulnerable software.

**Rules implemented:**
- [ ] `hardcoded-secrets`: Flag string literals assigned to variables matching `/(?i)(api_?key|secret|token|password|credential|auth_?token)/` with value length >8 — severity: Error
- [ ] `unsafe-block-audit`: Count and report all `unsafe` blocks with file:line, categorize by type (raw pointer deref, FFI call, mutable static access, union field access) — severity: Warning
- [ ] `sql-injection-risk`: Flag string formatting (`format!()`) used inside `.query()`, `.execute()`, or similar database method calls (sqlx, diesel, sea-orm patterns) — severity: Error
- [ ] `hardcoded-secrets` has an allowlist of non-secret suffixes: `_url`, `_path`, `_name`, `_type`, `_label`, `_mode`, `_format`, `_version`, `_prefix`, `_suffix`
- [ ] `unsafe-block-audit` respects `#![forbid(unsafe_code)]` — if present, report zero unsafe blocks instead of scanning
- [ ] Error: `hardcoded-secrets` false positive on test fixtures — accepted, suppressible via `// rust-doctor-disable-next-line`

**Priority:** P0 | **Size:** M (3 pts) | **Blocked by:** US-008

---

### EP-004: Dependency Analysis (P1)

Assess dependency health via external tools.

---

#### US-012: cargo-audit integration for CVE detection

**As a** developer, **I want** rust-doctor to check my dependencies for known vulnerabilities **so that** I don't ship code with known CVEs.

**Acceptance Criteria:**
- [ ] Detect if `cargo-audit` is installed via `which` crate
- [ ] If installed: spawn `cargo audit --json` and parse output
- [ ] Map each advisory to a rust-doctor diagnostic: advisory ID, affected crate, severity (CVSS-based: critical/high → Error, medium/low → Warning), description
- [ ] Include help text with advisory URL and fix suggestion ("upgrade {crate} to {patched_version}")
- [ ] If not installed: print info message "Install cargo-audit for vulnerability scanning: cargo install cargo-audit" and skip this pass (zero diagnostics, no score penalty)
- [ ] Timeout: 60 seconds for cargo-audit subprocess
- [ ] Error: cargo-audit exits with non-zero for reasons other than advisories (e.g., no Cargo.lock) — log warning, skip pass

**Priority:** P1 | **Size:** S (2 pts) | **Blocked by:** US-004

---

#### US-013: cargo-machete integration for unused dependencies

**As a** developer, **I want** rust-doctor to detect unused dependencies **so that** I can keep my dependency tree lean.

**Acceptance Criteria:**
- [ ] Detect if `cargo-machete` is installed via `which` crate
- [ ] If installed: spawn `cargo machete --with-metadata` and parse stdout
- [ ] Map each unused dependency to a diagnostic: crate name, Cargo.toml file path, severity Warning
- [ ] Help text: "Remove `{crate}` from [dependencies] in {Cargo.toml path}"
- [ ] If not installed: print info message and skip pass (zero diagnostics, no score penalty)
- [ ] Timeout: 30 seconds
- [ ] Error: cargo-machete false positive on crates used only via proc-macro — accepted as known limitation, documented

**Priority:** P1 | **Size:** S (2 pts) | **Blocked by:** US-004

---

### EP-005: Async & Framework Rules (P1)

Rules specific to async Rust and popular frameworks.

---

#### US-014: Async anti-pattern rules

**As a** developer using async Rust, **I want** rust-doctor to catch async anti-patterns **so that** I avoid deadlocks and executor starvation.

**Acceptance Criteria:**
- [ ] Rules only activate when tokio, async-std, or smol detected in dependencies
- [ ] `blocking-in-async`: Flag `std::thread::sleep`, `std::fs::*` (read, write, etc.), `std::net::*` calls inside `async fn` bodies — severity: Error
- [ ] `mutex-across-await`: Flag `std::sync::Mutex` or `std::sync::RwLock` lock guard held across an `.await` point — severity: Error
- [ ] `block-on-in-async`: Flag `futures::executor::block_on` or `tokio::runtime::Handle::block_on` inside async context — severity: Error
- [ ] Help text for each rule suggests the correct alternative (e.g., "Use `tokio::time::sleep` instead of `std::thread::sleep`")
- [ ] Rules skip detection inside `spawn_blocking` closures (correct usage)
- [ ] Error: complex control flow (e.g., await in a match arm after lock) may cause false negatives — accepted

**Priority:** P1 | **Size:** M (3 pts) | **Blocked by:** US-008

---

#### US-015: Framework-specific rules

**As a** developer using axum/actix-web/rocket, **I want** framework-specific checks **so that** I follow each framework's best practices.

**Acceptance Criteria:**
- [ ] Rules activate conditionally based on `ProjectInfo.frameworks`
- [ ] **axum:** `axum-handler-not-async` — Flag handler functions passed to Router that are not async — severity: Warning
- [ ] **axum:** `axum-state-clone` — Flag `State<T>` where T doesn't implement Clone — severity: Error
- [ ] **actix-web:** `actix-blocking-handler` — Flag blocking operations in actix handler functions — severity: Warning
- [ ] **tokio:** `tokio-main-missing` — Flag `async fn main()` without `#[tokio::main]` attribute — severity: Error
- [ ] **tokio:** `tokio-spawn-without-move` — Flag `tokio::spawn` with closures capturing references (needs `move`) — severity: Error
- [ ] Each rule includes framework-specific help text with code example
- [ ] Error: framework not detected but user has the dependency under a feature flag — may miss rules; accepted limitation

**Priority:** P1 | **Size:** M (3 pts) | **Blocked by:** US-008, US-002

---

### EP-006: Advanced Modes (P1)

Diff mode, workspace support, and inline suppression.

---

#### US-016: Diff mode — scan only changed files

**As a** developer on a feature branch, **I want to** scan only the files I changed **so that** I get fast feedback on my work.

**Acceptance Criteria:**
- [ ] `--diff` flag with optional base branch argument (default: auto-detect via `git merge-base HEAD main` then `master`)
- [ ] Use `git diff --name-only {base}...HEAD` to get list of changed `.rs` files
- [ ] Filter all analysis passes to only process changed files
- [ ] Clippy pass: use `--` with file list or filter JSON output post-hoc
- [ ] Dependency analysis is skipped in diff mode (requires full project context)
- [ ] Score is calculated only from diagnostics in changed files
- [ ] Output shows "Diff mode: scanning N changed files vs {base}" header
- [ ] Error: not a git repository → print "Diff mode requires a git repository" and fall back to full scan
- [ ] Error: base branch doesn't exist → print error with suggestion to specify base with `--diff <branch>`

**Priority:** P1 | **Size:** M (3 pts) | **Blocked by:** US-004

---

#### US-017: Cargo workspace multi-crate scanning

**As a** developer with a Cargo workspace, **I want to** scan all crates or select specific ones **so that** I get a workspace-wide health view.

**Acceptance Criteria:**
- [ ] Auto-detect workspace from `cargo metadata` → `workspace_members` with >1 member
- [ ] Default behavior: scan all workspace members
- [ ] `--project <name1,name2>` flag: scan only specified workspace members
- [ ] `-y` flag: skip interactive member selection prompt
- [ ] Interactive mode (no `-y`, terminal is TTY): prompt user to select which members to scan
- [ ] Each member scanned independently, results merged into single output
- [ ] Score is calculated from combined diagnostics across all scanned members
- [ ] Output shows per-member diagnostic count breakdown in summary
- [ ] Error: `--project` specifies non-existent member → print available members and exit

**Priority:** P1 | **Size:** M (3 pts) | **Blocked by:** US-002, US-004

---

#### US-018: Inline suppression and diagnostic filtering

**As a** developer, **I want to** suppress specific warnings with inline comments **so that** I can acknowledge known issues without losing the overall score benefit.

**Acceptance Criteria:**
- [ ] `// rust-doctor-disable-next-line <rule-name>` suppresses the diagnostic on the next line
- [ ] `// rust-doctor-disable-line` suppresses all diagnostics on the current line
- [ ] `// rust-doctor-disable-next-line` (no rule name) suppresses ALL diagnostics on the next line
- [ ] Suppression parsing happens post-analysis: read source files, find suppression comments, filter matching diagnostics
- [ ] Config `ignore.rules` filters diagnostics by rule name globally
- [ ] Config `ignore.files` filters diagnostics by file path glob pattern
- [ ] Filtered diagnostics are excluded from score calculation
- [ ] `--verbose` mode shows count of suppressed diagnostics at the end
- [ ] Error: suppression comment references non-existent rule → print warning with valid rule names

**Priority:** P1 | **Size:** S (2 pts) | **Blocked by:** US-004

---

### EP-007: CI/CD & Distribution (P2)

GitHub Actions integration and binary distribution.

---

#### US-019: GitHub Actions composite action

**As a** team lead, **I want to** add rust-doctor to my CI pipeline **so that** PRs get a health score comment automatically.

**Acceptance Criteria:**
- [ ] `action.yml` at repo root defines a composite GitHub Action
- [ ] Inputs: `directory` (default `.`), `fail-on` (default `none`), `token` (for PR comments), `diff` (default `true`)
- [ ] Action installs rust-doctor via `cargo-binstall` (pre-built binary, fast) or falls back to `cargo install`
- [ ] Runs `rust-doctor {directory} --json --diff --fail-on {fail-on}` and captures output
- [ ] If `token` provided and running on a PR: post/update a PR comment with formatted score, error/warning counts, and top 5 diagnostics
- [ ] Outputs: `score` (integer), `errors` (integer), `warnings` (integer)
- [ ] Exit code matches `--fail-on` behavior
- [ ] Error: token not provided on PR → skip comment posting, still output score

**Priority:** P2 | **Size:** M (3 pts) | **Blocked by:** US-005, US-016

---

#### US-020: crates.io publishing and cargo-dist binary distribution

**As a** developer, **I want to** install rust-doctor via `cargo install` or download a pre-built binary **so that** setup is fast.

**Acceptance Criteria:**
- [ ] Cargo.toml has all required crates.io metadata: description, license (MIT OR Apache-2.0), repository, readme, keywords, categories
- [ ] `cargo publish --dry-run` succeeds with no errors
- [ ] cargo-dist initialized: `.github/workflows/release.yml` generated for tag-triggered releases
- [ ] Release workflow builds binaries for: x86_64-linux, aarch64-linux, x86_64-macos, aarch64-macos, x86_64-windows
- [ ] Release artifacts include shell installer script and powershell installer
- [ ] `cargo binstall rust-doctor` works (downloads pre-built binary from GitHub Releases)
- [ ] README.md documents installation methods: `cargo install`, `cargo binstall`, GitHub Releases, GitHub Actions
- [ ] Error: `cargo publish` fails due to missing field → CI catches this in dry-run step

**Priority:** P2 | **Size:** S (2 pts) | **Blocked by:** US-001

---

## Functional Requirements

| ID | Requirement | Stories |
|----|-------------|---------|
| FR-01 | Scan a Rust project and produce a 0-100 health score | US-004, US-005 |
| FR-02 | Run cargo clippy and convert output to unified diagnostics | US-006, US-007 |
| FR-03 | Detect custom anti-patterns via syn AST analysis | US-008, US-009, US-010, US-011 |
| FR-04 | Check dependencies for CVEs via cargo-audit | US-012 |
| FR-05 | Detect unused dependencies via cargo-machete | US-013 |
| FR-06 | Auto-detect project ecosystem and enable/disable rules accordingly | US-002, US-015 |
| FR-07 | Support configuration via TOML file and Cargo.toml metadata | US-003 |
| FR-08 | Support inline diagnostic suppression | US-018 |
| FR-09 | Scan only changed files in diff mode | US-016 |
| FR-10 | Scan all crates in a Cargo workspace | US-017 |
| FR-11 | Output results as JSON for machine consumption | US-005 |
| FR-12 | Integrate with GitHub Actions for automated PR scanning | US-019 |
| FR-13 | Distribute pre-built binaries for major platforms | US-020 |

## Non-Functional Requirements

| ID | Requirement | Metric |
|----|-------------|--------|
| NFR-01 | Single-crate scan completes in <5 seconds (excluding compilation) | p95 < 5000ms |
| NFR-02 | 50-crate workspace scan completes in <30 seconds | p95 < 30000ms |
| NFR-03 | Binary size <20MB (release build, stripped) | measured with `ls -la` |
| NFR-04 | Memory usage <200MB for a 100-crate workspace | measured with `/usr/bin/time -v` |
| NFR-05 | Works on stable Rust 1.75+ (no nightly required) | tested in CI |
| NFR-06 | Cross-platform: Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows (x86_64) | CI matrix |
| NFR-07 | Zero false positives on the top 100 crates.io crates for security rules | tested via integration test suite |
| NFR-08 | Graceful degradation: missing external tools reduce coverage, not crash | tested with tool removal |

## Edge Cases & Error States

| # | Category | Scenario | Handling |
|---|----------|----------|----------|
| 1 | Empty project | Directory has Cargo.toml but no .rs files | Print "No Rust source files found", score 100, exit 0 |
| 2 | Build failure | Project doesn't compile (rustc errors) | Capture compiler errors as diagnostics, skip clippy lint pass, run syn rules on raw AST |
| 3 | Missing tools | cargo-audit/cargo-machete not installed | Print install suggestion, skip that pass, no score penalty |
| 4 | Large workspace | 100+ crates | Parallel scanning with rayon, progress bar per crate |
| 5 | no_std project | Embedded/WASM with no standard library | Disable async rules, adjust performance rules (no String/Vec checks) |
| 6 | Git not available | --diff mode in non-git directory | Fall back to full scan with warning |
| 7 | Timeout | clippy takes >120s (huge project) | Kill process, report partial results, print timeout warning |
| 8 | Permission denied | Cannot read source files | Skip file, log warning, continue scan |
| 9 | Concurrent runs | Two rust-doctor instances on same project | No shared state — safe to run concurrently |
| 10 | Interrupted scan | User presses Ctrl+C | Clean up any temp files, exit cleanly |

## Risks & Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| clippy JSON format changes between versions | Low | High | Pin minimum clippy version, test against nightly in CI, version-check before parsing |
| False positives erode user trust | Medium | High | Conservative default rules, easy suppression via inline comments, track false positive reports |
| cargo-machete false positives on proc-macro crates | Medium | Medium | Document known limitation, suggest suppression for proc-macro crates |
| Large project scan timeout in CI | Medium | Medium | Configurable timeout, diff mode as default in CI, parallel execution |
| syn can't parse all valid Rust (edition differences) | Low | Medium | Catch parse errors gracefully, skip file, report as warning |
| Name collision on crates.io | Low | High | Check name availability before first publish — "rust-doctor" is not taken as of research date |

## Non-Goals

- **Auto-fixing code** — rust-doctor diagnoses, it does not modify source files (v1 scope)
- **Custom user-defined rules** — no plugin/extension system in v1 (use dylint for that)
- **IDE integration** — no LSP server or editor plugin in v1 (clippy integration handles this)
- **Historical tracking** — no score history, trends, or dashboards in v1
- **Remote/cloud scoring API** — all scoring is local (unlike react-doctor which posts to a remote API)
- **Rust compiler error explanations** — rust-doctor is not a compiler; it won't explain E0308
- **Code formatting** — not a rustfmt replacement; formatting is explicitly out of scope
- **Coverage analysis** — not a test coverage tool; use tarpaulin or llvm-cov for that

## Technical Considerations

These are questions and patterns for engineering, not mandates:

1. **Should clippy be run with `--all-features` or feature-aware?** Running `--all-features` catches more code but may fail if features are mutually exclusive. Consider `--all-targets` without `--all-features` as safer default.

2. **How to handle edition 2024 vs 2021 differences in syn parsing?** `syn::parse_file` handles both, but some syntax (e.g., `gen` keyword in 2024) may need version-aware parsing.

3. **Should the scan orchestrator use `std::thread`, `rayon`, or `tokio`?** Since we're spawning subprocesses (clippy, audit, machete) and doing CPU-bound AST walks, `rayon` for file parallelism + `std::process::Command` for subprocesses seems simplest. No need for async runtime.

4. **Clippy lint → category mapping table** (initial 50 lints):
   - Error Handling: `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `unreachable`
   - Performance: `clone_on_copy`, `redundant_clone`, `needless_collect`, `large_enum_variant`, `box_collection`, `inefficient_to_string`
   - Correctness: `wrong_self_convention`, `not_unsafe_ptr_arg_deref`, `cast_possible_truncation`
   - Cargo: `multiple_crate_versions`, `wildcard_dependencies`, `negative_feature_names`
   - Style (Warning): `needless_return`, `redundant_closure`, `manual_map`

5. **How to neutralize `#[allow(clippy::*)]` attributes?** Unlike react-doctor's approach of rewriting source files, we can pass `-W clippy::all -W clippy::pedantic` on the command line to override file-level `allow` attributes. This avoids modifying source files entirely.

6. **Binary size optimization:** Consider `[profile.release]` with `strip = true`, `lto = true`, `codegen-units = 1`, `opt-level = "z"` if binary size exceeds 20MB.

## Success Metrics

| Metric | Baseline | Target | Timeframe |
|--------|----------|--------|-----------|
| crates.io installs | 0 | 500 | 6 months |
| GitHub stars | 0 | 100 | 6 months |
| Rule count | 0 | 30+ custom + 700 clippy | 1 month |
| Scan speed (single crate) | N/A | p95 < 5s | 1 month |
| False positive rate | N/A | <5% on top 100 crates | 3 months |
| CI adoption | 0 | 20 repos using GH Action | 6 months |

## Open Questions

1. Should rust-doctor have a companion website (like www.react.doctor) for score sharing and leaderboards?
2. Should there be a `--fix` mode that opens an AI coding agent (similar to react-doctor's Ami integration)?
3. Should the health score be customizable (weights per category) or fixed?
4. Should rust-doctor support SARIF output for integration with GitHub Advanced Security?
5. Is "rust-doctor" available as a crate name? Needs verification before first publish.

[/PRD]
