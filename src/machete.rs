use crate::diagnostics::{Category, Diagnostic, Severity};
use crate::scanner::AnalysisPass;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const MACHETE_TIMEOUT_SECS: u64 = 30;

/// cargo-machete analysis pass — detects unused dependencies.
pub struct MachetePass;

impl AnalysisPass for MachetePass {
    fn name(&self) -> &'static str {
        "dependencies (cargo-machete)"
    }

    fn run(&self, project_root: &Path) -> Result<Vec<Diagnostic>, crate::error::PassError> {
        if !is_machete_available() {
            eprintln!(
                "Info: Install cargo-machete for unused dependency detection: cargo install cargo-machete"
            );
            return Ok(vec![]);
        }
        run_machete(project_root).map_err(|message| crate::error::PassError::Failed {
            pass: "dependencies (cargo-machete)".to_string(),
            message,
        })
    }
}

fn is_machete_available() -> bool {
    Command::new("cargo")
        .args(["machete", "--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_machete(project_root: &Path) -> Result<Vec<Diagnostic>, String> {
    let mut child = Command::new("cargo")
        .args(["machete", "--with-metadata"])
        .current_dir(project_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to spawn cargo machete: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or("failed to capture cargo-machete stdout")?;

    // Cancellable timeout watchdog
    let (cancel_tx, cancel_rx) = mpsc::channel::<()>();
    let child = Arc::new(Mutex::new(child));
    let child_watcher = Arc::clone(&child);
    let timed_out = Arc::new(AtomicBool::new(false));
    let timed_out_watcher = Arc::clone(&timed_out);

    let watcher = thread::spawn(move || {
        if cancel_rx
            .recv_timeout(Duration::from_secs(MACHETE_TIMEOUT_SECS))
            .is_err()
            && let Ok(mut c) = child_watcher.lock()
            && let Ok(None) = c.try_wait()
        {
            let _ = c.kill();
            let _ = c.wait(); // Reap the child to avoid zombie process
            timed_out_watcher.store(true, Ordering::Relaxed);
        }
    });

    // Read stdout with a cap to prevent OOM from pathological output
    const MAX_OUTPUT_BYTES: u64 = 10 * 1024 * 1024; // 10 MB
    let mut output = String::new();
    {
        use std::io::Read;
        let _ = stdout.take(MAX_OUTPUT_BYTES).read_to_string(&mut output);
    }

    // Cancel watchdog and reap child
    let _ = cancel_tx.send(());
    let _ = watcher.join();

    let exit_status = if let Ok(mut c) = child.lock() {
        c.wait().ok()
    } else {
        None
    };

    // Check timeout
    if timed_out.load(Ordering::Relaxed) {
        eprintln!("Warning: cargo-machete timed out after {MACHETE_TIMEOUT_SECS}s");
        return Ok(vec![]);
    }

    // Exit code 2 = operational error
    if let Some(status) = exit_status
        && status.code() == Some(2)
    {
        return Err("cargo-machete encountered an error during analysis".into());
    }

    // Parse text output
    Ok(parse_machete_output(&output))
}

/// Parse cargo-machete text output into diagnostics.
///
/// Format:
/// ```text
/// package-name -- /path/to/Cargo.toml:
/// \tdep-name
/// \tanother-dep
/// ```
fn parse_machete_output(output: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut current_manifest: Option<PathBuf> = None;

    for line in output.lines() {
        // Package header: "package-name -- /path/to/Cargo.toml:"
        if line.contains(" -- ") && line.ends_with(':') {
            if let Some(path_part) = line.split(" -- ").nth(1) {
                let manifest = path_part.trim_end_matches(':').trim();
                current_manifest = Some(PathBuf::from(manifest));
            }
            continue;
        }

        // Dependency line: "\tdep-name"
        if line.starts_with('\t') {
            let dep_name = line.trim();
            if !dep_name.is_empty() {
                let file_path = current_manifest
                    .clone()
                    .unwrap_or_else(|| PathBuf::from("Cargo.toml"));

                diagnostics.push(Diagnostic {
                    file_path,
                    rule: "unused-dependency".to_string(),
                    category: Category::Dependencies,
                    severity: Severity::Warning,
                    message: format!("Unused dependency: `{dep_name}`"),
                    help: Some(format!(
                        "Remove `{dep_name}` from [dependencies] in Cargo.toml"
                    )),
                    line: None,
                    column: None,
                });
            }
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_package() {
        let output = r#"cargo-machete found the following unused dependencies in /tmp/project:

my-crate -- /tmp/project/Cargo.toml:
	serde
	tokio
"#;
        let diags = parse_machete_output(output);
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].rule, "unused-dependency");
        assert_eq!(diags[0].severity, Severity::Warning);
        assert!(diags[0].message.contains("serde"));
        assert!(diags[1].message.contains("tokio"));
        assert_eq!(diags[0].file_path, PathBuf::from("/tmp/project/Cargo.toml"));
    }

    #[test]
    fn test_parse_multiple_packages() {
        let output = r#"cargo-machete found the following unused dependencies in /tmp/workspace:

crate-a -- /tmp/workspace/crate-a/Cargo.toml:
	unused-dep

crate-b -- /tmp/workspace/crate-b/Cargo.toml:
	another-unused
	yet-another
"#;
        let diags = parse_machete_output(output);
        assert_eq!(diags.len(), 3);
        assert_eq!(
            diags[0].file_path,
            PathBuf::from("/tmp/workspace/crate-a/Cargo.toml")
        );
        assert_eq!(
            diags[1].file_path,
            PathBuf::from("/tmp/workspace/crate-b/Cargo.toml")
        );
        assert_eq!(
            diags[2].file_path,
            PathBuf::from("/tmp/workspace/crate-b/Cargo.toml")
        );
    }

    #[test]
    fn test_parse_no_unused() {
        let output =
            "cargo-machete didn't find any unused dependencies in /tmp/project. Good job!\n";
        let diags = parse_machete_output(output);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_parse_empty_output() {
        let diags = parse_machete_output("");
        assert!(diags.is_empty());
    }

    #[test]
    fn test_help_text() {
        let output = "pkg -- /tmp/Cargo.toml:\n\tserde\n";
        let diags = parse_machete_output(output);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].help.as_ref().unwrap().contains("Remove `serde`"));
    }

    #[test]
    fn test_machete_availability() {
        // Informational — may or may not be installed
        let available = is_machete_available();
        if available {
            eprintln!("cargo-machete is available");
        } else {
            eprintln!("cargo-machete is NOT installed (test passes either way)");
        }
    }
}
