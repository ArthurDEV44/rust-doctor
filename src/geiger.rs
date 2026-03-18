//! cargo-geiger integration — unsafe code budget across the dependency tree.

use crate::diagnostics::{Category, Diagnostic, Severity};
use crate::process;
use crate::scanner::AnalysisPass;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const GEIGER_TIMEOUT_SECS: u64 = 120;
const MAX_OUTPUT_BYTES: u64 = 10 * 1024 * 1024; // 10 MB

/// cargo-geiger analysis pass — audits unsafe code usage across the dependency tree.
pub struct GeigerPass;

impl AnalysisPass for GeigerPass {
    fn name(&self) -> &'static str {
        "unsafe audit (cargo-geiger)"
    }

    fn run(&self, project_root: &Path) -> Result<Vec<Diagnostic>, crate::error::PassError> {
        if !is_geiger_available() {
            eprintln!(
                "Info: Install cargo-geiger for unsafe code auditing: cargo install cargo-geiger"
            );
            return Err(crate::error::PassError::Skipped {
                pass: self.name().to_string(),
                reason: "cargo-geiger is not installed — unsafe dependency auditing disabled. \
                         Install with: cargo install cargo-geiger"
                    .to_string(),
            });
        }
        run_geiger(project_root).map_err(|message| crate::error::PassError::Failed {
            pass: "unsafe audit (cargo-geiger)".to_string(),
            message,
        })
    }
}

/// Check if `cargo geiger` is available. Result is cached for the process lifetime.
fn is_geiger_available() -> bool {
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        Command::new("cargo")
            .args(["geiger", "--version"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

fn run_geiger(project_root: &Path) -> Result<Vec<Diagnostic>, String> {
    let child = Command::new("cargo")
        .args(["geiger", "--output-format", "json", "--quiet"])
        .current_dir(project_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to spawn cargo geiger: {e}"))?;

    let output = process::run_with_timeout(child, GEIGER_TIMEOUT_SECS, MAX_OUTPUT_BYTES)?;

    if output.timed_out {
        return Err(format!(
            "cargo geiger timed out after {GEIGER_TIMEOUT_SECS}s"
        ));
    }

    parse_geiger_output(&output.stdout)
}

#[derive(Deserialize)]
struct GeigerOutput {
    #[serde(default)]
    packages: Vec<GeigerPackage>,
}

#[derive(Deserialize)]
struct GeigerPackage {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    unsafety: GeigerUnsafety,
}

#[derive(Deserialize, Default)]
struct GeigerUnsafety {
    #[serde(default)]
    used: GeigerCounts,
    #[serde(default)]
    unused: GeigerCounts,
}

#[derive(Deserialize, Default)]
struct GeigerCounts {
    #[serde(default)]
    functions: CountPair,
    #[serde(default)]
    exprs: CountPair,
}

#[derive(Deserialize, Default)]
struct CountPair {
    #[serde(default)]
    safe: u64,
    #[serde(rename = "unsafe", default)]
    unsafe_: u64,
}

fn parse_geiger_output(json_str: &str) -> Result<Vec<Diagnostic>, String> {
    let output: GeigerOutput = serde_json::from_str(json_str)
        .map_err(|e| format!("failed to parse geiger output: {e}"))?;

    let mut diagnostics = Vec::new();

    for pkg in &output.packages {
        let unsafe_fns = pkg.unsafety.used.functions.unsafe_;
        let unsafe_exprs = pkg.unsafety.used.exprs.unsafe_;
        let total_unsafe = unsafe_fns + unsafe_exprs;

        if total_unsafe == 0 {
            continue;
        }

        let severity = if total_unsafe > 50 {
            Severity::Warning
        } else {
            Severity::Info
        };

        diagnostics.push(Diagnostic {
            file_path: PathBuf::from("Cargo.toml"),
            rule: "unsafe-dependency".to_string(),
            category: Category::Security,
            severity,
            message: format!(
                "Dependency `{}@{}` uses unsafe: {} functions, {} expressions",
                pkg.name, pkg.version, unsafe_fns, unsafe_exprs
            ),
            help: Some(format!(
                "Review unsafe usage in `{}` or consider alternatives",
                pkg.name
            )),
            line: None,
            column: None,
            fix: None,
        });
    }

    Ok(diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_output() {
        let json = r#"{"packages": []}"#;
        let diags = parse_geiger_output(json).unwrap();
        assert!(diags.is_empty());
    }

    #[test]
    fn test_parse_package_with_unsafe() {
        let json = r#"{
            "packages": [{
                "name": "some-crate",
                "version": "1.0.0",
                "unsafety": {
                    "used": {
                        "functions": {"safe": 10, "unsafe": 3},
                        "exprs": {"safe": 100, "unsafe": 20}
                    },
                    "unused": {
                        "functions": {"safe": 0, "unsafe": 0},
                        "exprs": {"safe": 0, "unsafe": 0}
                    }
                }
            }]
        }"#;
        let diags = parse_geiger_output(json).unwrap();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("some-crate@1.0.0"));
        assert!(diags[0].message.contains("3 functions"));
        assert!(diags[0].message.contains("20 expressions"));
    }

    #[test]
    fn test_safe_package_no_diagnostic() {
        let json = r#"{
            "packages": [{
                "name": "safe-crate",
                "version": "0.1.0",
                "unsafety": {
                    "used": {
                        "functions": {"safe": 50, "unsafe": 0},
                        "exprs": {"safe": 200, "unsafe": 0}
                    },
                    "unused": {
                        "functions": {"safe": 0, "unsafe": 0},
                        "exprs": {"safe": 0, "unsafe": 0}
                    }
                }
            }]
        }"#;
        let diags = parse_geiger_output(json).unwrap();
        assert!(diags.is_empty());
    }

    #[test]
    fn test_high_unsafe_count_is_warning() {
        let json = r#"{
            "packages": [{
                "name": "risky-crate",
                "version": "2.0.0",
                "unsafety": {
                    "used": {
                        "functions": {"safe": 5, "unsafe": 30},
                        "exprs": {"safe": 10, "unsafe": 25}
                    },
                    "unused": {
                        "functions": {"safe": 0, "unsafe": 0},
                        "exprs": {"safe": 0, "unsafe": 0}
                    }
                }
            }]
        }"#;
        let diags = parse_geiger_output(json).unwrap();
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Warning);
    }
}
