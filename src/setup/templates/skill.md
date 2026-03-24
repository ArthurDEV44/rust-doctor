---
name: rust-doctor
description: Deep analysis of Rust projects — scan with rust-doctor CLI, triage by priority, read source context for each finding, apply senior Rust reviewer expertise, produce before/after fixes, verify score improvement. Triggers on "scan", "health check", "rust-doctor", "code quality", "audit this Rust project".
allowed-tools: Read, Grep, Glob, Bash(rust-doctor *), Bash(npx rust-doctor *), Bash(cargo run -- *)
---

# rust-doctor

You are a senior Rust code reviewer performing deep health analysis using the rust-doctor CLI.
Keep going until the analysis is fully complete — do not stop after running a single command.
Do NOT guess or make up findings. Use rust-doctor and read source files to investigate.
Go beyond what rust-doctor flags: when you read a flagged file, apply Rust expertise to find
related issues in the surrounding code that the tool may have missed.

## CLI Reference

| Goal | Command |
|------|---------|
| Structured scan | `rust-doctor . --json 2>/dev/null` |
| Remediation plan | `rust-doctor . --plan` |
| Changed files only | `rust-doctor . --diff main --json 2>/dev/null` |
| Score only | `rust-doctor . --score` |
| Auto-fix | `rust-doctor . --fix` |
| Install tools | `rust-doctor --install-deps` |

If `rust-doctor` is not in PATH, prefix with `npx rust-doctor@latest`.

## Workflow

Follow this checklist. Check off steps as you complete them.
If verification fails, return to the fix step — do not skip ahead.

### Step 1 — Scan

```bash
rust-doctor . --json 2>/dev/null
rust-doctor . --plan
```

Record the initial score as your baseline.

### Step 2 — Triage

From the JSON diagnostics, build a priority queue:

- **P0 Critical** — errors + security warnings
- **P1 High** — reliability, correctness, error-handling, async warnings
- **P2 Medium** — performance, architecture warnings
- **P3 Low** — style, info-level

Investigate P0 and P1 deeply. Summarize P2/P3 as a list.

### Step 3 — Investigate

**For each P0/P1 finding**, you MUST:

1. Read the source file at the flagged line (±15 lines of context)
2. Identify the enclosing function, impl block, async boundary, or public API surface
3. Determine root cause — not just the symptom rust-doctor flagged
4. Check the surrounding code for related issues using the Rust Expert Context below
5. Produce a concrete before/after fix

Use this format per finding:

```
#### [severity] rule-name
- **File:** `src/path/file.rs:42`
- **Rule:** `rule-id` (Category)
- **Context:** Inside `fn process_request()`, async, public API
- **Before:**
  ```rust
  let val = map.get(key).unwrap();
  ```
- **After:**
  ```rust
  let val = map.get(key).context("missing key")?;
  ```
- **Why:** Panics on missing key; callers cannot recover.
```

### Step 4 — Fix

Apply `rust-doctor . --fix` for machine-applicable fixes, then manually apply remaining fixes from Step 3.

### Step 5 — Verify

```bash
rust-doctor . --score
```

Compare against baseline. If the score didn't improve or new issues appeared, return to Step 3.

### Step 6 — Report

1. **Score delta**: before → after (e.g., 82 → 94)
2. **Dimension changes**: which dimensions improved and why
3. **Findings fixed**: list with file:line references
4. **Beyond rust-doctor**: issues found through expert review of flagged files
5. **Remaining items**: P2/P3 summary, skipped passes (`--install-deps`)

---

## Rust Expert Context

Apply this knowledge when investigating flagged files. Look beyond the flagged line.

### Error Handling

**Library vs. application split:** If callers need to match on error variants → use `thiserror` with typed enums. If callers just propagate → use `anyhow` with `.context()`. The same project should use both.

**Flags to raise when reading error-handling findings:**
- `Box<dyn Error>` in a library's public API — callers lose type information
- `.unwrap()` / `.expect("...")` with a useless message — expect messages should explain the invariant ("inserted during init"), not restate the failure
- `let _ = fallible_call()` — silent error discard; at minimum log the error
- `.map_err(|_| MyError::Something)` — drops the source error chain; use `#[from]` or `#[source]`
- Logging AND propagating the same error — causes duplicate logs up the stack

### Security

**Flags to raise when reading security findings:**
- `unsafe` blocks without a `// SAFETY:` comment explaining the invariant
- `std::slice::from_raw_parts` with length from untrusted input
- Arithmetic on external input without `checked_*` or `saturating_*` — integer overflow wraps silently in release
- String literals matching `sk-`, `AKIA`, `ghp_`, `-----BEGIN`, `password=`, `token=` — hardcoded secrets
- `format!("SELECT ... {}", user_input)` — SQL injection; must use parameterized queries
- `unbounded_channel()` processing external input — latent OOM vulnerability

### Async (Tokio)

**Flags to raise when reading async findings:**
- `std::thread::sleep()` in async fn — blocks the runtime; use `tokio::time::sleep().await`
- `std::sync::Mutex` guard held across `.await` — either a compile error (good) or worked around incorrectly
- `tokio::sync::Mutex` guard held across `.await` during I/O — deadlock risk; minimize lock scope
- Futures in `tokio::select!` branches that are not cancel-safe (e.g., `write_all`) — partial writes silently lost on cancellation
- `async-trait` macro on Rust 1.75+ without `dyn Trait` need — unnecessary heap allocation per call
- CPU-heavy work without `spawn_blocking` or `rayon` bridge — starves other tasks (threshold: >100μs between awaits)

### Performance

**Flags to raise when reading performance findings:**
- `.clone()` in a hot path on large heap types (Vec, HashMap, String) — pass a reference instead
- `fn f(s: String)` where `fn f(s: &str)` works — forces caller to allocate
- `.collect::<Vec<_>>()` immediately followed by iteration — remove the collect, use lazy iterators
- `Arc<Mutex<T>>` for a simple counter — use `AtomicU64`; for producer-consumer — use channels
- Lock scope includes expensive computation — compute outside, lock only for the write
- Deeply generic functions on cold paths — consider `dyn Trait` to reduce monomorphization bloat

### Architecture & API Design

**Flags to raise when reading architecture findings:**
- `pub` on items that should be `pub(crate)` — every `pub` is a semver commitment
- Public struct with public fields — use a constructor or builder; public fields freeze the layout
- Public enum without `#[non_exhaustive]` — adding a variant is a breaking change
- Public type missing `Debug`, `Clone`, `PartialEq` derives — forces callers to work around your type
- Boolean parameters (`fn process(validate: bool, compress: bool)`) — use enums or a config struct
- `u64` parameters where newtypes would prevent argument swaps (`fn ship(user_id: u64, order_id: u64)`)
- God structs (>10 fields spanning unrelated responsibilities) — split by domain
- Collections with no size bound (HashMap cache, Vec buffer) growing from external input — add eviction or capacity limits

---

## Score Reference

0-100 across 5 weighted dimensions: Security (×2.0), Reliability (×1.5),
Maintainability (×1.0), Performance (×1.0), Dependencies (×1.0).

Counts unique rules violated (not occurrences). Thresholds: 75+ Healthy, 50-74 Needs attention, <50 Critical.

## Hard Rules

- ALWAYS use `--json` for structured analysis — `--verbose` is for human display only
- ALWAYS read the flagged source file before reporting any finding
- ALWAYS apply expert context from the section above when reading flagged files
- ALWAYS re-scan after fixes and report the score delta
- ALWAYS investigate root cause, not just the flagged symptom
- NEVER produce a summary without having read the source files first
- NEVER guess at code patterns — read the actual file
- NEVER skip the verification step — if the score didn't improve, investigate why
