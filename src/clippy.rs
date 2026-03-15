use crate::diagnostics::{Category, Diagnostic, Severity};
use crate::scanner::AnalysisPass;
use cargo_metadata::Message;
use cargo_metadata::diagnostic::DiagnosticLevel;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Timeout for clippy subprocess in seconds.
const CLIPPY_TIMEOUT_SECS: u64 = 120;

/// Clippy analysis pass — runs `cargo clippy --message-format=json` and
/// converts the output to rust-doctor diagnostics.
pub struct ClippyPass;

impl AnalysisPass for ClippyPass {
    fn name(&self) -> &str {
        "clippy"
    }

    fn run(&self, project_root: &Path) -> Result<Vec<Diagnostic>, String> {
        // Check if clippy is installed
        if !is_clippy_available() {
            return Err("clippy not found — install with: rustup component add clippy".to_string());
        }

        run_clippy(project_root)
    }
}

/// Check if `cargo clippy` is available.
fn is_clippy_available() -> bool {
    Command::new("cargo")
        .args(["clippy", "--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run cargo clippy and parse JSON output into diagnostics.
fn run_clippy(project_root: &Path) -> Result<Vec<Diagnostic>, String> {
    let manifest_path = project_root.join("Cargo.toml");

    let mut child = Command::new("cargo")
        .args([
            "clippy",
            "--message-format=json",
            "--all-targets",
            "--manifest-path",
        ])
        .arg(&manifest_path)
        .args([
            "--",
            "-W",
            "clippy::all",
            "-W",
            "clippy::pedantic",
            "-W",
            "clippy::nursery",
            "-W",
            "clippy::cargo",
        ])
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
    let timed_out = Arc::new(Mutex::new(false));
    let timed_out_watcher = Arc::clone(&timed_out);

    let watcher = thread::spawn(move || {
        // Wait for either cancellation or timeout
        if cancel_rx
            .recv_timeout(Duration::from_secs(CLIPPY_TIMEOUT_SECS))
            .is_err()
        {
            // Timeout expired (or sender dropped without sending)
            if let Ok(mut c) = child_watcher.lock()
                && let Ok(None) = c.try_wait()
            {
                let _ = c.kill();
                if let Ok(mut t) = timed_out_watcher.lock() {
                    *t = true;
                }
            }
        }
    });

    // Parse JSON messages from clippy stdout
    let reader = BufReader::new(stdout);
    let mut diagnostics = Vec::new();
    let mut build_succeeded = true;

    for message in Message::parse_stream(reader) {
        let message = match message {
            Ok(m) => m,
            Err(_) => continue,
        };

        match message {
            Message::CompilerMessage(compiler_msg) => {
                let diag = &compiler_msg.message;

                // Filter: only process error and warning level messages
                let severity = match diag.level {
                    DiagnosticLevel::Error | DiagnosticLevel::Ice => Severity::Error,
                    DiagnosticLevel::Warning => Severity::Warning,
                    _ => continue,
                };

                // Extract code (lint name)
                let rule = match &diag.code {
                    Some(code) => code.code.clone(),
                    None => {
                        if severity == Severity::Error {
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

                let (file_path, line, column) = match primary_span {
                    Some(span) => (
                        PathBuf::from(&span.file_name),
                        Some(span.line_start as u32),
                        Some(span.column_start as u32),
                    ),
                    None => (PathBuf::from("<unknown>"), None, None),
                };

                // Map category from lint name
                let category = map_lint_category(&rule);

                // Extract help: prefer rendered field, fall back to children
                let help = diag.rendered.clone().or_else(|| {
                    diag.children
                        .iter()
                        .find(|c| c.level == DiagnosticLevel::Help)
                        .map(|c| c.message.clone())
                });

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
    if *timed_out.lock().unwrap_or_else(|e| e.into_inner()) {
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
        let stderr_output = std::io::read_to_string(stderr).unwrap_or_default();
        if !stderr_output.is_empty() {
            let first_error = stderr_output
                .lines()
                .find(|l| l.starts_with("error"))
                .unwrap_or("project failed to compile");

            diagnostics.push(Diagnostic {
                file_path: PathBuf::from("Cargo.toml"),
                rule: "compiler-error".to_string(),
                category: Category::Correctness,
                severity: Severity::Error,
                message: first_error.to_string(),
                help: Some(stderr_output),
                line: None,
                column: None,
            });
        }
    }

    Ok(diagnostics)
}

/// Map a clippy lint name to a rust-doctor diagnostic category.
fn map_lint_category(lint: &str) -> Category {
    let name = lint.strip_prefix("clippy::").unwrap_or(lint);

    match name {
        // Error Handling
        "unwrap_used" | "expect_used" | "panic" | "todo" | "unimplemented" | "unreachable"
        | "unwrap_in_result" | "panic_in_result_fn" | "indexing_slicing" | "exit"
        | "result_unit_err" | "option_if_let_else" => Category::ErrorHandling,

        // Performance
        "clone_on_copy"
        | "redundant_clone"
        | "needless_collect"
        | "large_enum_variant"
        | "box_collection"
        | "inefficient_to_string"
        | "unnecessary_to_owned"
        | "large_stack_arrays"
        | "large_futures"
        | "too_many_arguments"
        | "unnecessary_wraps"
        | "useless_vec"
        | "manual_memcpy"
        | "naive_bytecount"
        | "bytes_count_to_usize"
        | "iter_with_drain"
        | "extend_with_drain"
        | "flat_map_option"
        | "map_flatten"
        | "manual_retain"
        | "or_fun_call"
        | "single_char_pattern"
        | "format_collect"
        | "trivially_copy_pass_by_ref" => Category::Performance,

        // Security
        "transmute_ptr_to_ref"
        | "cast_ptr_alignment"
        | "fn_to_numeric_cast"
        | "string_lit_as_bytes" => Category::Security,

        // Correctness
        "wrong_self_convention"
        | "not_unsafe_ptr_arg_deref"
        | "cast_possible_truncation"
        | "cast_sign_loss"
        | "cast_possible_wrap"
        | "cast_lossless"
        | "float_cmp"
        | "float_equality_without_abs"
        | "eq_op"
        | "erasing_op"
        | "bad_bit_mask"
        | "nonsensical_open_options"
        | "suspicious_assignment_formatting"
        | "suspicious_else_formatting"
        | "mistyped_literal_suffixes"
        | "match_overlapping_arm"
        | "invalid_regex" => Category::Correctness,

        // Cargo
        "multiple_crate_versions"
        | "wildcard_dependencies"
        | "negative_feature_names"
        | "redundant_feature_names"
        | "cargo_common_metadata" => Category::Cargo,

        // Async
        "async_yields_async" | "unused_async" => Category::Async,

        // Architecture
        "module_inception"
        | "too_many_lines"
        | "cognitive_complexity"
        | "type_complexity"
        | "struct_excessive_bools"
        | "fn_params_excessive_bools" => Category::Architecture,

        // Compiler errors
        "compiler-error" | "compiler-ice" => Category::Correctness,

        // Default: fall back to Style
        _ => Category::Style,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            map_lint_category("clippy::redundant_clone"),
            Category::Performance
        );
    }

    #[test]
    fn test_map_security() {
        assert_eq!(
            map_lint_category("clippy::transmute_ptr_to_ref"),
            Category::Security
        );
    }

    #[test]
    fn test_map_correctness() {
        assert_eq!(
            map_lint_category("clippy::float_cmp"),
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
    fn test_map_architecture() {
        assert_eq!(
            map_lint_category("clippy::cognitive_complexity"),
            Category::Architecture
        );
    }

    #[test]
    fn test_map_unknown_falls_to_style() {
        assert_eq!(
            map_lint_category("clippy::some_unknown_lint"),
            Category::Style
        );
        assert_eq!(map_lint_category("unknown_lint"), Category::Style);
    }

    #[test]
    fn test_map_without_prefix() {
        assert_eq!(map_lint_category("unwrap_used"), Category::ErrorHandling);
        assert_eq!(map_lint_category("clone_on_copy"), Category::Performance);
    }

    #[test]
    fn test_clippy_is_available() {
        assert!(is_clippy_available());
    }

    #[test]
    fn test_run_clippy_on_self() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let result = run_clippy(manifest_dir);
        assert!(result.is_ok(), "clippy failed: {:?}", result.err());
    }
}
