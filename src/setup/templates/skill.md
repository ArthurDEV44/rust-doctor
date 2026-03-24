---
name: rust-doctor
description: Deep analysis of Rust projects — scan, triage findings, read source context, and produce actionable fixes with before/after code for each issue
---

# rust-doctor

A unified code health tool for Rust. Scans for security, performance, correctness,
architecture, and dependency issues, producing a 0-100 health score with actionable diagnostics.

## Prerequisites

```bash
rust-doctor --version
# If not installed: cargo install rust-doctor
# Or without install: npx rust-doctor@latest --version
```

## CLI Reference

| Goal | Command |
|------|---------|
| Structured scan (for analysis) | `rust-doctor . --json 2>/dev/null` |
| Prioritized remediation plan | `rust-doctor . --plan` |
| Scan only changed files | `rust-doctor . --diff main --json 2>/dev/null` |
| Score only (bare integer) | `rust-doctor . --score` |
| Apply auto-fixes | `rust-doctor . --fix` |
| Install missing tools | `rust-doctor --install-deps` |
| Verbose terminal output | `rust-doctor . --verbose` |

## Deep Analysis Workflow

When asked to scan, check health, or fix a Rust project, follow this **three-pass pipeline**.
Do NOT skip passes or produce a summary without reading source files.

### Pass 1 — Scan & Capture

Run rust-doctor with `--json` to get structured, iterable findings:

```bash
rust-doctor . --json 2>/dev/null
```

The JSON output contains every diagnostic with full metadata:

```json
{
  "diagnostics": [{
    "file_path": "src/main.rs",
    "rule": "unwrap-in-production",
    "category": "error-handling",
    "severity": "warning",
    "message": "Use of .unwrap() in production code",
    "help": "Use ? operator or handle the error explicitly",
    "line": 42,
    "column": 10,
    "fix": { "old_text": ".unwrap()", "new_text": "?", "line": 42 }
  }],
  "score": 87,
  "score_label": "Great",
  "dimension_scores": {
    "security": 100, "reliability": 85, "maintainability": 92,
    "performance": 88, "dependencies": 95
  },
  "error_count": 2, "warning_count": 14, "info_count": 3,
  "source_file_count": 42, "skipped_passes": []
}
```

Also run `--plan` to get rust-doctor's own prioritization:

```bash
rust-doctor . --plan
```

This outputs a P0-P3 prioritized remediation plan grouped by rule.

### Pass 2 — Triage

From the JSON output, build a priority queue:

1. **P0 Critical** — All errors + security warnings
2. **P1 High** — Reliability, correctness, error-handling warnings
3. **P2 Medium** — Performance, architecture warnings
4. **P3 Low** — Style, info-level findings

Focus investigation on P0 and P1 first. Report P2/P3 as a summary list.

### Pass 3 — Investigate & Fix

**For each P0/P1 finding**, you MUST:

1. **Read the source file** at the flagged line (±15 lines of context)
2. **Identify the enclosing context** — function name, impl block, async context, public API boundary
3. **Produce a concrete fix** with before/after code
4. **Apply the fix** or explain why manual intervention is needed

Use this output format for each finding:

```
#### [severity] rule-name
- **File:** `src/path/file.rs:42`
- **Rule:** `rule-id` (Category — SEVERITY)
- **Context:** Called inside `fn process_request()` in async context
- **Before:**
  ```rust
  let config = serde_json::from_str(&raw).unwrap();
  ```
- **After:**
  ```rust
  let config = serde_json::from_str(&raw)
      .context("failed to parse config")?;
  ```
- **Why:** .unwrap() panics on invalid input; callers cannot recover from the error.
```

### Post-Fix Verification

After applying fixes, re-run rust-doctor to verify improvement:

```bash
rust-doctor . --score
```

Report the before/after score delta.

## Score Interpretation

0-100 score across 5 weighted dimensions:

| Dimension       | Weight | What it measures                                |
|-----------------|--------|-------------------------------------------------|
| Security        | ×2.0   | Vulnerabilities, unsafe code, hardcoded secrets |
| Reliability     | ×1.5   | Error handling, panics, correctness issues      |
| Maintainability | ×1.0   | Complexity, style, architecture patterns        |
| Performance     | ×1.0   | Allocations, clones, blocking calls in async    |
| Dependencies    | ×1.0   | Outdated deps, unused deps, supply chain risks  |

Counts **unique rules violated** (not occurrences). Fixing one rule category
improves the score regardless of how many files were affected.

**Thresholds:** 75-100 Healthy | 50-74 Needs attention | 0-49 Critical

## Hard Rules

- ALWAYS use `--json` for analysis — never rely on `--verbose` text alone
- ALWAYS read the flagged source file before reporting a finding
- ALWAYS use `--plan` to get rust-doctor's own prioritization
- ALWAYS re-scan after fixes to verify and report the score delta
- For each P0/P1 finding, show the specific code and a concrete before/after fix

## DO NOT

- Produce a summary table of findings without reading the source files first
- Suggest generic Rust advice without showing the specific call site from the codebase
- Fix code without understanding the enclosing context (function, trait impl, async boundary)
- Skip the re-scan verification step after applying fixes
- Run only `--verbose` without `--json` — verbose output cannot be iterated programmatically

## Limitations

- **Read-only by default** — does not modify files unless `--fix` is explicitly passed
- **External tools optional** — some passes require cargo-audit, cargo-deny, etc.
  Install with `--install-deps`. Missing tools are skipped with an info diagnostic
- **Rust only** — scans `.rs` source files only
- **Rule explanations** — detailed rule docs available via MCP mode (`rust-doctor --mcp`)
