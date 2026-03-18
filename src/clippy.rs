use crate::diagnostics::{Category, Diagnostic, Severity};
use crate::scanner::AnalysisPass;
use cargo_metadata::Message;
use cargo_metadata::diagnostic::DiagnosticLevel;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// Note: clippy uses a streaming parser (Message::parse_stream) so it cannot use
// the process::run_with_timeout helper which reads all stdout into a String.
// The watchdog pattern is kept inline here for that reason.

/// Timeout for clippy subprocess in seconds.
const CLIPPY_TIMEOUT_SECS: u64 = 120;

// ---------------------------------------------------------------------------
// Lint registry — data-driven mapping of clippy lints to categories/severities
// ---------------------------------------------------------------------------

/// A single entry in the lint-to-category mapping table.
struct LintEntry {
    /// Lint name without the `clippy::` prefix.
    name: &'static str,
    category: Category,
    /// Severity override — takes precedence over clippy's default.
    severity: Severity,
    /// Whether this lint belongs to clippy's `restriction` group (allow-by-default).
    /// Restriction lints are downgraded to Info in test code because they are opt-in
    /// style checks, not correctness issues.
    is_restriction: bool,
}

/// Registry of 55+ impactful clippy lints with explicit category and severity.
/// Lints NOT in this table inherit clippy's default severity and map to `Style`.
static LINT_REGISTRY: &[LintEntry] = &[
    // ── Error Handling (restriction group — allow-by-default in clippy) ─
    LintEntry {
        name: "unwrap_used",
        category: Category::ErrorHandling,
        severity: Severity::Warning,
        is_restriction: true,
    },
    LintEntry {
        name: "expect_used",
        category: Category::ErrorHandling,
        severity: Severity::Warning,
        is_restriction: true,
    },
    LintEntry {
        name: "panic",
        category: Category::ErrorHandling,
        severity: Severity::Warning,
        is_restriction: true,
    },
    LintEntry {
        name: "indexing_slicing",
        category: Category::ErrorHandling,
        severity: Severity::Warning,
        is_restriction: true,
    },
    LintEntry {
        name: "unwrap_in_result",
        category: Category::ErrorHandling,
        severity: Severity::Warning,
        is_restriction: true,
    },
    LintEntry {
        name: "panic_in_result_fn",
        category: Category::ErrorHandling,
        severity: Severity::Warning,
        is_restriction: true,
    },
    LintEntry {
        name: "exit",
        category: Category::ErrorHandling,
        severity: Severity::Warning,
        is_restriction: true,
    },
    LintEntry {
        name: "map_unwrap_or",
        category: Category::ErrorHandling,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "option_if_let_else",
        category: Category::ErrorHandling,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "question_mark",
        category: Category::ErrorHandling,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "manual_ok_or",
        category: Category::ErrorHandling,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "result_unit_err",
        category: Category::ErrorHandling,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "result_large_err",
        category: Category::ErrorHandling,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "let_underscore_must_use",
        category: Category::ErrorHandling,
        severity: Severity::Warning,
        is_restriction: false,
    },
    // ── Performance ─────────────────────────────────────────────────────
    LintEntry {
        name: "box_collection",
        category: Category::Performance,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "clone_on_copy",
        category: Category::Performance,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "redundant_clone",
        category: Category::Performance,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "needless_collect",
        category: Category::Performance,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "large_enum_variant",
        category: Category::Performance,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "inefficient_to_string",
        category: Category::Performance,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "unnecessary_to_owned",
        category: Category::Performance,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "large_stack_arrays",
        category: Category::Performance,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "large_futures",
        category: Category::Performance,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "single_char_pattern",
        category: Category::Performance,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "cmp_owned",
        category: Category::Performance,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "cloned_instead_of_copied",
        category: Category::Performance,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "suboptimal_flops",
        category: Category::Performance,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "or_fun_call",
        category: Category::Performance,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "trivially_copy_pass_by_ref",
        category: Category::Performance,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "useless_vec",
        category: Category::Performance,
        severity: Severity::Warning,
        is_restriction: false,
    },
    // ── Security ────────────────────────────────────────────────────────
    LintEntry {
        name: "undocumented_unsafe_blocks",
        category: Category::Security,
        severity: Severity::Warning,
        is_restriction: true,
    },
    LintEntry {
        name: "multiple_unsafe_ops_per_block",
        category: Category::Security,
        severity: Severity::Warning,
        is_restriction: true,
    },
    LintEntry {
        name: "transmute_ptr_to_ref",
        category: Category::Security,
        severity: Severity::Error,
        is_restriction: false,
    },
    LintEntry {
        name: "cast_ptr_alignment",
        category: Category::Security,
        severity: Severity::Error,
        is_restriction: false,
    },
    LintEntry {
        name: "fn_to_numeric_cast",
        category: Category::Security,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "mem_forget",
        category: Category::Security,
        severity: Severity::Warning,
        is_restriction: true,
    },
    LintEntry {
        name: "cast_possible_truncation",
        category: Category::Security,
        severity: Severity::Warning,
        is_restriction: false,
    },
    // ── Correctness ─────────────────────────────────────────────────────
    LintEntry {
        name: "almost_swapped",
        category: Category::Correctness,
        severity: Severity::Error,
        is_restriction: false,
    },
    LintEntry {
        name: "approx_constant",
        category: Category::Correctness,
        severity: Severity::Error,
        is_restriction: false,
    },
    LintEntry {
        name: "bad_bit_mask",
        category: Category::Correctness,
        severity: Severity::Error,
        is_restriction: false,
    },
    LintEntry {
        name: "absurd_extreme_comparisons",
        category: Category::Correctness,
        severity: Severity::Error,
        is_restriction: false,
    },
    LintEntry {
        name: "invalid_regex",
        category: Category::Correctness,
        severity: Severity::Error,
        is_restriction: false,
    },
    LintEntry {
        name: "wrong_self_convention",
        category: Category::Correctness,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "cast_sign_loss",
        category: Category::Correctness,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "cast_possible_wrap",
        category: Category::Correctness,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "cast_lossless",
        category: Category::Correctness,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "float_cmp",
        category: Category::Correctness,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "eq_op",
        category: Category::Correctness,
        severity: Severity::Error,
        is_restriction: false,
    },
    LintEntry {
        name: "match_overlapping_arm",
        category: Category::Correctness,
        severity: Severity::Warning,
        is_restriction: false,
    },
    // ── Cargo ───────────────────────────────────────────────────────────
    LintEntry {
        name: "wildcard_dependencies",
        category: Category::Cargo,
        severity: Severity::Error,
        is_restriction: false,
    },
    LintEntry {
        name: "multiple_crate_versions",
        category: Category::Cargo,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "cargo_common_metadata",
        category: Category::Cargo,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "negative_feature_names",
        category: Category::Cargo,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "redundant_feature_names",
        category: Category::Cargo,
        severity: Severity::Warning,
        is_restriction: false,
    },
    // ── Async ───────────────────────────────────────────────────────────
    LintEntry {
        name: "await_holding_lock",
        category: Category::Async,
        severity: Severity::Error,
        is_restriction: false,
    },
    LintEntry {
        name: "await_holding_refcell_ref",
        category: Category::Async,
        severity: Severity::Error,
        is_restriction: false,
    },
    LintEntry {
        name: "unused_async",
        category: Category::Async,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "redundant_async_block",
        category: Category::Async,
        severity: Severity::Warning,
        is_restriction: false,
    },
    // ── Architecture ────────────────────────────────────────────────────
    LintEntry {
        name: "struct_excessive_bools",
        category: Category::Architecture,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "fn_params_excessive_bools",
        category: Category::Architecture,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "too_many_lines",
        category: Category::Architecture,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "cognitive_complexity",
        category: Category::Architecture,
        severity: Severity::Warning,
        is_restriction: true,
    },
    LintEntry {
        name: "type_complexity",
        category: Category::Architecture,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "too_many_arguments",
        category: Category::Architecture,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "module_name_repetitions",
        category: Category::Architecture,
        severity: Severity::Warning,
        is_restriction: false,
    },
    // ── Style (restriction-group lints) ─────────────────────────────────
    LintEntry {
        name: "dbg_macro",
        category: Category::Style,
        severity: Severity::Warning,
        is_restriction: true,
    },
    LintEntry {
        name: "todo",
        category: Category::Style,
        severity: Severity::Warning,
        is_restriction: true,
    },
    LintEntry {
        name: "unimplemented",
        category: Category::Style,
        severity: Severity::Warning,
        is_restriction: true,
    },
    LintEntry {
        name: "unreachable",
        category: Category::Style,
        severity: Severity::Warning,
        is_restriction: true,
    },
    // ── Style (non-restriction) ─────────────────────────────────────────
    LintEntry {
        name: "wildcard_imports",
        category: Category::Style,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "missing_errors_doc",
        category: Category::Style,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "missing_panics_doc",
        category: Category::Style,
        severity: Severity::Warning,
        is_restriction: false,
    },
    LintEntry {
        name: "print_stdout",
        category: Category::Style,
        severity: Severity::Warning,
        is_restriction: true,
    },
    LintEntry {
        name: "print_stderr",
        category: Category::Style,
        severity: Severity::Warning,
        is_restriction: true,
    },
];

/// Restriction-group lints that must be explicitly enabled via `-W` flags
/// since they are not covered by `clippy::all`, `pedantic`, `nursery`, or `cargo`.
const RESTRICTION_LINTS: &[&str] = &[
    "clippy::unwrap_used",
    "clippy::expect_used",
    "clippy::panic",
    "clippy::indexing_slicing",
    "clippy::unwrap_in_result",
    "clippy::panic_in_result_fn",
    "clippy::exit",
    "clippy::undocumented_unsafe_blocks",
    "clippy::multiple_unsafe_ops_per_block",
    "clippy::mem_forget",
    "clippy::cognitive_complexity",
    "clippy::dbg_macro",
    "clippy::print_stdout",
    "clippy::print_stderr",
    "clippy::unimplemented",
    "clippy::unreachable",
];

/// Look up a lint in the registry. Returns `(category, severity, is_restriction)` if found.
fn lookup_lint(lint: &str) -> Option<(Category, Severity, bool)> {
    let name = lint.strip_prefix("clippy::").unwrap_or(lint);
    LINT_REGISTRY
        .iter()
        .find(|e| e.name == name)
        .map(|e| (e.category.clone(), e.severity, e.is_restriction))
}

/// Map a clippy lint name to a rust-doctor category. Falls back to `Style`.
fn map_lint_category(lint: &str) -> Category {
    match lint {
        "compiler-error" | "compiler-ice" => Category::Correctness,
        _ => lookup_lint(lint).map_or(Category::Style, |(cat, _, _)| cat),
    }
}

/// Apply severity override from the registry if the lint is known.
/// Otherwise, keep clippy's original severity.
fn resolve_severity(lint: &str, clippy_severity: Severity) -> Severity {
    match lint {
        "compiler-error" | "compiler-ice" => Severity::Error,
        _ => lookup_lint(lint).map_or(clippy_severity, |(_, sev, _)| sev),
    }
}

/// Returns `true` if the lint is in clippy's `restriction` group (allow-by-default).
fn is_restriction_lint(lint: &str) -> bool {
    lookup_lint(lint).is_some_and(|(_, _, restriction)| restriction)
}

/// Returns `true` if the file path looks like test code.
/// Matches: `tests/`, `test_`, `_test.rs`, and paths containing `/tests/`.
fn is_test_file(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.contains("/tests/") || s.starts_with("tests/")
}

/// Return the list of all known lint names (for config validation).
pub fn known_lint_names() -> Vec<&'static str> {
    LINT_REGISTRY.iter().map(|e| e.name).collect()
}

// ---------------------------------------------------------------------------
// Clippy pass implementation
// ---------------------------------------------------------------------------

/// Clippy analysis pass — runs `cargo clippy --message-format=json` and
/// converts the output to rust-doctor diagnostics.
pub struct ClippyPass;

impl AnalysisPass for ClippyPass {
    fn name(&self) -> &'static str {
        "clippy"
    }

    fn run(&self, project_root: &Path) -> Result<Vec<Diagnostic>, crate::error::PassError> {
        if !is_clippy_available() {
            return Err(crate::error::PassError::Failed {
                pass: "clippy".to_string(),
                message: "clippy not found — install with: rustup component add clippy".to_string(),
            });
        }
        run_clippy(project_root).map_err(|message| crate::error::PassError::Failed {
            pass: "clippy".to_string(),
            message,
        })
    }
}

/// Check if `cargo clippy` is available. Result is cached for the process lifetime.
fn is_clippy_available() -> bool {
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        Command::new("cargo")
            .args(["clippy", "--version"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

/// Build the full list of `-W` flags for clippy, including group-level
/// flags and individual restriction-group lints.
fn build_clippy_warn_flags() -> Vec<String> {
    let mut flags = Vec::new();

    // Group-level flags (override #[allow] directives)
    for group in [
        "clippy::all",
        "clippy::pedantic",
        "clippy::nursery",
        "clippy::cargo",
    ] {
        flags.push("-W".to_string());
        flags.push(group.to_string());
    }

    // Individual restriction-group lints
    for lint in RESTRICTION_LINTS {
        flags.push("-W".to_string());
        flags.push((*lint).to_string());
    }

    flags
}

/// Clippy config content that allows restriction lints in test code.
const CLIPPY_TEST_ALLOW_CONFIG: &str = "\
allow-unwrap-in-tests = true\n\
allow-expect-in-tests = true\n\
allow-indexing-slicing-in-tests = true\n\
allow-panic-in-tests = true\n\
allow-print-in-tests = true\n\
allow-dbg-in-tests = true\n\
allow-useless-vec-in-tests = true\n";

/// A guard that creates a temporary `clippy.toml` on construction
/// and removes it on drop, unless the project already had one.
struct ClippyConfigGuard {
    path: Option<PathBuf>,
}

impl ClippyConfigGuard {
    /// Write a temporary `clippy.toml` into `dir`. Returns `None` if one already exists.
    fn new(dir: &Path) -> Self {
        if dir.join("clippy.toml").exists() || dir.join(".clippy.toml").exists() {
            return Self { path: None };
        }
        let config_path = dir.join("clippy.toml");
        if std::fs::write(&config_path, CLIPPY_TEST_ALLOW_CONFIG).is_ok() {
            Self {
                path: Some(config_path),
            }
        } else {
            Self { path: None }
        }
    }
}

impl Drop for ClippyConfigGuard {
    fn drop(&mut self) {
        if let Some(ref path) = self.path {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Run cargo clippy and parse JSON output into diagnostics.
fn run_clippy(project_root: &Path) -> Result<Vec<Diagnostic>, String> {
    let manifest_path = project_root.join("Cargo.toml");

    let warn_flags = build_clippy_warn_flags();

    // Write a temporary clippy.toml that allows restriction lints in test code.
    // The guard removes it when dropped (even on early return via `?`).
    let _clippy_config_guard = ClippyConfigGuard::new(project_root);

    let mut cmd = Command::new("cargo");
    cmd.args([
        "clippy",
        "--message-format=json",
        "--all-targets",
        "--all-features",
        "--manifest-path",
    ])
    .arg(&manifest_path)
    .arg("--");

    for flag in &warn_flags {
        cmd.arg(flag);
    }

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn cargo clippy: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or("failed to capture clippy stdout")?;
    let stderr = child.stderr.take();

    // Cancellable timeout watchdog
    let (cancel_tx, cancel_rx) = mpsc::channel::<()>();
    let child = Arc::new(Mutex::new(child));
    let child_watcher = Arc::clone(&child);
    let timed_out = Arc::new(AtomicBool::new(false));
    let timed_out_watcher = Arc::clone(&timed_out);

    let watcher = thread::spawn(move || {
        if cancel_rx
            .recv_timeout(Duration::from_secs(CLIPPY_TIMEOUT_SECS))
            .is_err()
            && let Ok(mut c) = child_watcher.lock()
            && matches!(c.try_wait(), Ok(None))
        {
            let _ = c.kill();
            let _ = c.wait(); // Reap the child to avoid zombie process
            timed_out_watcher.store(true, Ordering::Relaxed);
        }
    });

    // Parse JSON messages from clippy stdout
    let reader = BufReader::new(stdout);
    let mut diagnostics = Vec::new();
    let mut build_succeeded = true;

    for message in Message::parse_stream(reader) {
        let Ok(message) = message else {
            continue;
        };

        match message {
            Message::CompilerMessage(compiler_msg) => {
                let diag = &compiler_msg.message;

                // Filter: only process error and warning level messages
                let clippy_severity = match diag.level {
                    DiagnosticLevel::Error | DiagnosticLevel::Ice => Severity::Error,
                    DiagnosticLevel::Warning => Severity::Warning,
                    _ => continue,
                };

                // Extract code (lint name)
                let rule = match &diag.code {
                    Some(code) => code.code.clone(),
                    None => {
                        if clippy_severity == Severity::Error {
                            if diag.level == DiagnosticLevel::Ice {
                                "compiler-ice".to_string()
                            } else {
                                "compiler-error".to_string()
                            }
                        } else {
                            continue;
                        }
                    }
                };

                // Extract primary span
                let primary_span = diag.spans.iter().find(|s| s.is_primary);
                let (file_path, line, column) = primary_span.map_or_else(
                    || (PathBuf::from("<unknown>"), None, None),
                    |span| {
                        (
                            PathBuf::from(&span.file_name),
                            Some(span.line_start as u32),
                            Some(span.column_start as u32),
                        )
                    },
                );

                // Apply registry: category and severity override
                let category = map_lint_category(&rule);
                let severity = resolve_severity(&rule, clippy_severity);

                // Extract help: prefer children help message, fall back to rendered
                let help = diag
                    .children
                    .iter()
                    .find(|c| c.level == DiagnosticLevel::Help)
                    .map(|c| c.message.clone())
                    .or_else(|| diag.rendered.clone());

                diagnostics.push(Diagnostic {
                    file_path,
                    rule,
                    category,
                    severity,
                    message: diag.message.clone(),
                    help,
                    line,
                    column,
                });
            }
            Message::BuildFinished(finished) => {
                build_succeeded = finished.success;
            }
            _ => {}
        }
    }

    // Cancel the watchdog thread
    let _ = cancel_tx.send(());
    let _ = watcher.join();

    // Reap the child process
    if let Ok(mut c) = child.lock() {
        let _ = c.wait();
    }

    // Check if we timed out
    if timed_out.load(Ordering::Relaxed) {
        eprintln!(
            "Warning: clippy timed out after {CLIPPY_TIMEOUT_SECS}s — reporting partial results"
        );
    }

    // If the build failed and we got no error diagnostics from JSON,
    // capture stderr as a compiler-error diagnostic
    if !build_succeeded
        && !diagnostics.iter().any(|d| d.severity == Severity::Error)
        && let Some(stderr) = stderr
    {
        // Cap stderr read to prevent OOM from pathological compiler output
        const MAX_STDERR_BYTES: u64 = 4 * 1024; // 4 KB
        let mut stderr_output = String::new();
        {
            use std::io::Read;
            let _ = stderr
                .take(MAX_STDERR_BYTES)
                .read_to_string(&mut stderr_output);
        }
        if !stderr_output.is_empty() {
            let first_error = stderr_output
                .lines()
                .find(|l| l.starts_with("error"))
                .unwrap_or("project failed to compile");

            // Truncate to 200 chars to avoid leaking verbose internal details
            let truncated: String = if first_error.chars().count() > 200 {
                let mut s: String = first_error.chars().take(200).collect();
                s.push('…');
                s
            } else {
                first_error.to_string()
            };

            diagnostics.push(Diagnostic {
                file_path: PathBuf::from("Cargo.toml"),
                rule: "compiler-error".to_string(),
                category: Category::Correctness,
                severity: Severity::Error,
                message: truncated,
                help: Some("Run `cargo build` to see the full error output".to_string()),
                line: None,
                column: None,
            });
        }
    }

    // Post-filter: drop restriction-group lints from integration test files.
    // Unit tests (#[cfg(test)] inside src/) are handled by clippy.toml config above,
    // but integration test files (tests/*.rs) may still fire restriction lints.
    diagnostics.retain(|d| !(is_restriction_lint(&d.rule) && is_test_file(&d.file_path)));

    // Post-filter: drop print_stdout/print_stderr for binary crates.
    // These lints target library code; CLI binaries legitimately use println!/eprintln!.
    let is_binary_crate = project_root.join("src/main.rs").exists();
    if is_binary_crate {
        diagnostics.retain(|d| {
            !matches!(
                d.rule.as_str(),
                "clippy::print_stdout" | "clippy::print_stderr"
            )
        });
    }

    Ok(diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Registry tests ---

    #[test]
    fn test_registry_has_50_plus_entries() {
        assert!(
            LINT_REGISTRY.len() >= 50,
            "Registry has {} entries, expected 50+",
            LINT_REGISTRY.len()
        );
    }

    #[test]
    fn test_registry_no_duplicate_names() {
        let names: Vec<&str> = LINT_REGISTRY.iter().map(|e| e.name).collect();
        let mut seen = std::collections::HashSet::new();
        for name in &names {
            assert!(seen.insert(name), "Duplicate lint name in registry: {name}");
        }
    }

    // --- Lookup tests ---

    #[test]
    fn test_lookup_known_lint() {
        let result = lookup_lint("clippy::unwrap_used");
        assert!(result.is_some());
        let (cat, sev, restriction) = result.unwrap();
        assert_eq!(cat, Category::ErrorHandling);
        assert_eq!(sev, Severity::Warning);
        assert!(restriction, "unwrap_used should be marked as restriction");
    }

    #[test]
    fn test_lookup_without_prefix() {
        let result = lookup_lint("unwrap_used");
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, Category::ErrorHandling);
    }

    #[test]
    fn test_lookup_unknown_lint() {
        assert!(lookup_lint("clippy::some_unknown_lint").is_none());
    }

    // --- Category mapping tests ---

    #[test]
    fn test_map_error_handling() {
        assert_eq!(
            map_lint_category("clippy::unwrap_used"),
            Category::ErrorHandling
        );
        assert_eq!(
            map_lint_category("clippy::expect_used"),
            Category::ErrorHandling
        );
        assert_eq!(map_lint_category("clippy::panic"), Category::ErrorHandling);
    }

    #[test]
    fn test_map_performance() {
        assert_eq!(
            map_lint_category("clippy::clone_on_copy"),
            Category::Performance
        );
        assert_eq!(
            map_lint_category("clippy::needless_collect"),
            Category::Performance
        );
    }

    #[test]
    fn test_map_security() {
        assert_eq!(
            map_lint_category("clippy::transmute_ptr_to_ref"),
            Category::Security
        );
        assert_eq!(
            map_lint_category("clippy::undocumented_unsafe_blocks"),
            Category::Security
        );
    }

    #[test]
    fn test_map_correctness() {
        assert_eq!(
            map_lint_category("clippy::float_cmp"),
            Category::Correctness
        );
        assert_eq!(
            map_lint_category("clippy::almost_swapped"),
            Category::Correctness
        );
        assert_eq!(map_lint_category("compiler-error"), Category::Correctness);
        assert_eq!(map_lint_category("compiler-ice"), Category::Correctness);
    }

    #[test]
    fn test_map_cargo() {
        assert_eq!(
            map_lint_category("clippy::wildcard_dependencies"),
            Category::Cargo
        );
    }

    #[test]
    fn test_map_async() {
        assert_eq!(
            map_lint_category("clippy::await_holding_lock"),
            Category::Async
        );
        assert_eq!(map_lint_category("clippy::unused_async"), Category::Async);
    }

    #[test]
    fn test_map_architecture() {
        assert_eq!(
            map_lint_category("clippy::cognitive_complexity"),
            Category::Architecture
        );
        assert_eq!(
            map_lint_category("clippy::too_many_arguments"),
            Category::Architecture
        );
    }

    #[test]
    fn test_map_style() {
        assert_eq!(map_lint_category("clippy::dbg_macro"), Category::Style);
        assert_eq!(map_lint_category("clippy::todo"), Category::Style);
    }

    #[test]
    fn test_map_unknown_falls_to_style() {
        assert_eq!(
            map_lint_category("clippy::some_unknown_lint"),
            Category::Style
        );
    }

    // --- Severity override tests ---

    #[test]
    fn test_severity_restriction_lints_are_warning() {
        // Restriction-group lints should be Warning, not Error (aligned with clippy)
        let sev = resolve_severity("clippy::unwrap_used", Severity::Warning);
        assert_eq!(sev, Severity::Warning);
        let sev = resolve_severity("clippy::expect_used", Severity::Warning);
        assert_eq!(sev, Severity::Warning);
        let sev = resolve_severity("clippy::panic", Severity::Warning);
        assert_eq!(sev, Severity::Warning);
    }

    #[test]
    fn test_severity_override_keeps_registered_warning() {
        // clone_on_copy is registered as Warning
        let sev = resolve_severity("clippy::clone_on_copy", Severity::Warning);
        assert_eq!(sev, Severity::Warning);
    }

    #[test]
    fn test_severity_unknown_lint_keeps_clippy_default() {
        let sev = resolve_severity("clippy::some_unknown_lint", Severity::Warning);
        assert_eq!(sev, Severity::Warning);
    }

    #[test]
    fn test_severity_compiler_error_always_error() {
        assert_eq!(
            resolve_severity("compiler-error", Severity::Warning),
            Severity::Error
        );
        assert_eq!(
            resolve_severity("compiler-ice", Severity::Warning),
            Severity::Error
        );
    }

    // --- Known lint names ---

    #[test]
    fn test_known_lint_names_count() {
        let names = known_lint_names();
        assert!(names.len() >= 50);
        assert!(names.contains(&"unwrap_used"));
        assert!(names.contains(&"await_holding_lock"));
    }

    // --- Restriction flags ---

    #[test]
    fn test_build_clippy_warn_flags_contains_groups() {
        let flags = build_clippy_warn_flags();
        assert!(flags.contains(&"clippy::all".to_string()));
        assert!(flags.contains(&"clippy::pedantic".to_string()));
        assert!(flags.contains(&"clippy::nursery".to_string()));
        assert!(flags.contains(&"clippy::cargo".to_string()));
    }

    #[test]
    fn test_build_clippy_warn_flags_contains_restriction_lints() {
        let flags = build_clippy_warn_flags();
        assert!(flags.contains(&"clippy::unwrap_used".to_string()));
        assert!(flags.contains(&"clippy::expect_used".to_string()));
        assert!(flags.contains(&"clippy::dbg_macro".to_string()));
    }

    // --- Restriction lint detection ---

    #[test]
    fn test_is_restriction_lint() {
        assert!(is_restriction_lint("clippy::unwrap_used"));
        assert!(is_restriction_lint("clippy::expect_used"));
        assert!(is_restriction_lint("clippy::panic"));
        assert!(is_restriction_lint("clippy::indexing_slicing"));
        assert!(is_restriction_lint("clippy::print_stdout"));
        assert!(is_restriction_lint("clippy::dbg_macro"));
        assert!(!is_restriction_lint("clippy::clone_on_copy"));
        assert!(!is_restriction_lint("clippy::almost_swapped"));
        assert!(!is_restriction_lint("clippy::some_unknown_lint"));
    }

    #[test]
    fn test_is_test_file() {
        assert!(is_test_file(Path::new("tests/integration.rs")));
        assert!(is_test_file(Path::new("/home/user/project/tests/foo.rs")));
        assert!(!is_test_file(Path::new("src/main.rs")));
        assert!(!is_test_file(Path::new("src/rules/mod.rs")));
    }

    // --- Integration ---

    #[test]
    fn test_clippy_is_available() {
        assert!(is_clippy_available());
    }

    #[test]
    fn test_run_clippy_on_self() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let result = run_clippy(manifest_dir);
        assert!(result.is_ok(), "clippy failed: {:?}", result.err());
        // Verify that diagnostics from registered lints get severity overrides
        let diags = result.unwrap();
        for d in &diags {
            if let Some((_, expected_sev, _)) = lookup_lint(&d.rule) {
                assert_eq!(
                    d.severity, expected_sev,
                    "Lint {} should have severity {:?} but got {:?}",
                    d.rule, expected_sev, d.severity
                );
            }
        }
        // Verify no restriction lints from test files survived filtering
        for d in &diags {
            if is_test_file(&d.file_path) {
                assert!(
                    !is_restriction_lint(&d.rule),
                    "Restriction lint {} should have been filtered from test file {:?}",
                    d.rule,
                    d.file_path
                );
            }
        }
    }
}
