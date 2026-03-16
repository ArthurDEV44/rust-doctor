# rust-doctor

A unified code health tool for Rust — scan, score, and fix your codebase.

rust-doctor scans Rust projects for security, performance, correctness, architecture, and dependency issues, producing a 0–100 health score with actionable diagnostics.

## Features

- **700+ clippy lints** with severity overrides and category mapping
- **12 custom AST rules** via syn: error handling, performance, security anti-patterns
- **Async anti-pattern detection**: blocking calls in async, block_on in async context
- **Framework-specific rules**: tokio, axum, actix-web
- **Dependency auditing**: CVE detection via cargo-audit, unused deps via cargo-machete
- **Health score**: 0–100 with ASCII doctor face output
- **Diff mode**: scan only changed files for fast CI feedback
- **Workspace support**: scan all crates or select specific members
- **Inline suppression**: `// rust-doctor-disable-next-line <rule>`
- **Multiple output modes**: terminal, `--json`, `--score`
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
curl -fsSL https://github.com/anthropics/rust-doctor/releases/latest/download/install.sh | bash
```

### PowerShell installer (Windows)

```powershell
irm https://github.com/anthropics/rust-doctor/releases/latest/download/install.ps1 | iex
```

### GitHub Releases

Download pre-built binaries from [GitHub Releases](https://github.com/anthropics/rust-doctor/releases).

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
```

## GitHub Actions

```yaml
- uses: anthropics/rust-doctor@v1
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
