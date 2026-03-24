---
name: rust-doctor
description: Scan Rust projects for security, performance, correctness, architecture, and dependency issues — producing a 0-100 health score with actionable diagnostics
---

# rust-doctor

A unified code health tool for Rust. Scans for security, performance, correctness,
architecture, and dependency issues, producing a 0-100 health score with actionable diagnostics.

## Prerequisites

Verify rust-doctor is installed:

```bash
rust-doctor --version
```

If not installed:

```bash
# Via cargo
cargo install rust-doctor

# Or via npx (no install needed)
npx rust-doctor@latest --version
```

## Commands

### 1. Full Scan (primary command)

```bash
# Scan current directory with detailed diagnostics
rust-doctor . --verbose

# JSON output (for programmatic processing)
rust-doctor . --json

# Scan only files changed vs a branch
rust-doctor . --diff main

# Scan with a prioritized remediation plan
rust-doctor . --diff main --plan

# Scan specific workspace members
rust-doctor . --project core,api

# SARIF output (GitHub Code Scanning, GitLab SAST)
rust-doctor . --sarif
```

### 2. Score Only

```bash
# Print bare integer score (0-100) to stdout — ideal for CI piping
rust-doctor . --score
```

### 3. Auto-Fix

```bash
# Apply machine-applicable fixes (modifies source files)
rust-doctor . --fix
```

### 4. Install External Tools

```bash
# Check and install missing analysis tools (cargo-audit, cargo-deny, etc.)
rust-doctor --install-deps
```

## Score Interpretation

The health score is 0-100, computed across 5 weighted dimensions:

| Dimension       | Weight | What it measures                                  |
|-----------------|--------|---------------------------------------------------|
| Security        | ×2.0   | Vulnerabilities, unsafe code, hardcoded secrets   |
| Reliability     | ×1.5   | Error handling, panics, correctness issues        |
| Maintainability | ×1.0   | Complexity, style, architecture patterns          |
| Performance     | ×1.0   | Allocations, clones, blocking calls in async      |
| Dependencies    | ×1.0   | Outdated deps, unused deps, supply chain risks    |

The score counts **unique rules violated** (not occurrences), so fixing one rule category
improves the score regardless of how many files were affected.

**Thresholds:**

- **75-100** Healthy — minor improvements possible
- **50-74** Needs attention — several issues to address
- **0-49** Critical — significant problems detected

## Recommended Workflow

When asked to check code health or diagnose Rust issues:

1. **Scan**: `rust-doctor . --verbose` for full diagnostics with file:line details
2. **Review**: Check the score box and dimension breakdown in the output
3. **Plan**: `rust-doctor . --diff main --plan` for a prioritized remediation plan
4. **Fix**: `rust-doctor . --fix` to apply machine-applicable fixes automatically
5. **Verify**: Re-scan to confirm improvements

For CI integration:

```bash
# Fail CI if score drops below 70
rust-doctor . --score | xargs -I {} test {} -ge 70

# Or use the built-in fail-on flag
rust-doctor . --fail-on warning
```

## Limitations

- **Read-only by default** — does not modify files unless `--fix` is explicitly passed
- **External tools optional** — some passes (cargo-audit, cargo-deny, cargo-geiger) require
  external tools; install with `--install-deps`. Missing tools are skipped with an info diagnostic
- **Rust only** — scans `.rs` source files; does not analyze other languages
- **Rule explanations** — detailed rule documentation is available via MCP mode
  (`rust-doctor --mcp`), not via CLI flags
