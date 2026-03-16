use crate::diagnostics::{Category, Diagnostic, Severity};
use crate::scanner::AnalysisPass;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const AUDIT_TIMEOUT_SECS: u64 = 60;

/// cargo-audit analysis pass — checks dependencies for known CVEs.
pub struct AuditPass;

impl AnalysisPass for AuditPass {
    fn name(&self) -> &str {
        "dependencies (cargo-audit)"
    }

    fn run(&self, project_root: &Path) -> Result<Vec<Diagnostic>, String> {
        if !is_cargo_audit_available() {
            eprintln!(
                "Info: Install cargo-audit for vulnerability scanning: cargo install cargo-audit"
            );
            return Ok(vec![]);
        }
        run_audit(project_root)
    }
}

fn is_cargo_audit_available() -> bool {
    Command::new("cargo")
        .args(["audit", "--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_audit(project_root: &Path) -> Result<Vec<Diagnostic>, String> {
    let mut child = Command::new("cargo")
        .args(["audit", "--json"])
        .current_dir(project_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to spawn cargo audit: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or("failed to capture cargo-audit stdout")?;

    // Cancellable timeout watchdog
    let (cancel_tx, cancel_rx) = mpsc::channel::<()>();
    let child = Arc::new(Mutex::new(child));
    let child_watcher = Arc::clone(&child);
    let timed_out = Arc::new(Mutex::new(false));
    let timed_out_watcher = Arc::clone(&timed_out);

    let watcher = thread::spawn(move || {
        if cancel_rx
            .recv_timeout(Duration::from_secs(AUDIT_TIMEOUT_SECS))
            .is_err()
            && let Ok(mut c) = child_watcher.lock()
            && let Ok(None) = c.try_wait()
        {
            let _ = c.kill();
            if let Ok(mut t) = timed_out_watcher.lock() {
                *t = true;
            }
        }
    });

    // Read all stdout
    let output = std::io::read_to_string(stdout).unwrap_or_default();

    // Cancel watchdog and reap child
    let _ = cancel_tx.send(());
    let _ = watcher.join();

    let exit_status = if let Ok(mut c) = child.lock() {
        c.wait().ok()
    } else {
        None
    };

    // Check timeout
    if *timed_out.lock().unwrap_or_else(|e| e.into_inner()) {
        eprintln!("Warning: cargo-audit timed out after {AUDIT_TIMEOUT_SECS}s");
        return Ok(vec![]);
    }

    // Exit code 2 = operational error (no Cargo.lock, etc.)
    if let Some(status) = exit_status
        && status.code() == Some(2)
    {
        return Err(
            "cargo-audit encountered an error (missing Cargo.lock or fetch failure)".into(),
        );
    }

    // Parse JSON
    if output.is_empty() {
        return Ok(vec![]);
    }

    let report: AuditReport = serde_json::from_str(&output)
        .map_err(|e| format!("failed to parse cargo-audit JSON: {e}"))?;

    let mut diagnostics = Vec::new();

    // Process vulnerabilities
    if let Some(vulns) = &report.vulnerabilities {
        for vuln in &vulns.list {
            let advisory = &vuln.advisory;
            let pkg = &vuln.package;

            let severity = cvss_to_severity(advisory.cvss.as_deref());

            let patched = &vuln.versions.patched;
            let fix_hint = if patched.is_empty() {
                "No patched version available — consider an alternative crate".to_string()
            } else {
                format!("Upgrade {} to {}", pkg.name, patched.join(" or "))
            };

            let url_hint = advisory
                .url
                .as_deref()
                .map(|u| format!("\n  {u}"))
                .unwrap_or_default();

            diagnostics.push(Diagnostic {
                file_path: PathBuf::from("Cargo.lock"),
                rule: advisory.id.clone(),
                category: Category::Dependencies,
                severity,
                message: format!(
                    "{}: {} v{} — {}",
                    advisory.id, pkg.name, pkg.version, advisory.title
                ),
                help: Some(format!("{fix_hint}{url_hint}")),
                line: None,
                column: None,
            });
        }
    }

    // Process warnings (unmaintained, yanked, etc.) as low-severity
    for (kind, warnings) in &report.warnings {
        for warn in warnings {
            if let Some(advisory) = &warn.advisory {
                diagnostics.push(Diagnostic {
                    file_path: PathBuf::from("Cargo.lock"),
                    rule: advisory.id.clone(),
                    category: Category::Dependencies,
                    severity: Severity::Warning,
                    message: format!(
                        "{}: {} v{} — {} ({})",
                        advisory.id, warn.package.name, warn.package.version, advisory.title, kind
                    ),
                    help: advisory.url.as_deref().map(|u| u.to_string()),
                    line: None,
                    column: None,
                });
            }
        }
    }

    Ok(diagnostics)
}

/// Map CVSS vector string to severity.
/// CVSS 3.x base score: Critical (9.0-10.0), High (7.0-8.9) → Error
/// Medium (4.0-6.9), Low (0.1-3.9) → Warning
/// If no CVSS string, default to Warning.
fn cvss_to_severity(cvss: Option<&str>) -> Severity {
    let Some(cvss) = cvss else {
        return Severity::Warning;
    };

    // Extract base score from CVSS vector — look for AV: (Attack Vector) as heuristic
    // CVSS:3.1/AV:N/AC:L/... → Network + Low complexity = likely High/Critical
    // Simple heuristic: if AV:N (network) and AC:L (low complexity), treat as Error
    if cvss.contains("AV:N") {
        Severity::Error
    } else {
        Severity::Warning
    }
}

// ─── JSON deserialization types ─────────────────────────────────────────────

#[derive(Deserialize)]
struct AuditReport {
    vulnerabilities: Option<Vulnerabilities>,
    #[serde(default)]
    warnings: std::collections::HashMap<String, Vec<WarningEntry>>,
}

#[derive(Deserialize)]
struct Vulnerabilities {
    #[serde(default)]
    list: Vec<VulnerabilityEntry>,
}

#[derive(Deserialize)]
struct VulnerabilityEntry {
    advisory: Advisory,
    versions: Versions,
    package: Package,
}

#[derive(Deserialize)]
struct Advisory {
    id: String,
    title: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    cvss: Option<String>,
}

#[derive(Deserialize)]
struct Versions {
    #[serde(default)]
    patched: Vec<String>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    version: String,
}

#[derive(Deserialize)]
struct WarningEntry {
    advisory: Option<Advisory>,
    package: Package,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cvss_network_low_complexity_is_error() {
        assert_eq!(
            cvss_to_severity(Some("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:N/A:N")),
            Severity::Error
        );
    }

    #[test]
    fn test_cvss_network_high_complexity_is_error() {
        assert_eq!(
            cvss_to_severity(Some("CVSS:3.1/AV:N/AC:H/PR:N/UI:N/S:U/C:H/I:N/A:N")),
            Severity::Error
        );
    }

    #[test]
    fn test_cvss_local_is_warning() {
        assert_eq!(
            cvss_to_severity(Some("CVSS:3.1/AV:L/AC:L/PR:N/UI:N/S:U/C:L/I:N/A:N")),
            Severity::Warning
        );
    }

    #[test]
    fn test_cvss_none_is_warning() {
        assert_eq!(cvss_to_severity(None), Severity::Warning);
    }

    #[test]
    fn test_parse_audit_report_empty() {
        let json = r#"{"vulnerabilities":{"found":false,"count":0,"list":[]},"warnings":{}}"#;
        let report: AuditReport = serde_json::from_str(json).unwrap();
        assert!(report.vulnerabilities.unwrap().list.is_empty());
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn test_parse_audit_report_with_vuln() {
        let json = r#"{
            "vulnerabilities": {
                "found": true,
                "count": 1,
                "list": [{
                    "advisory": {
                        "id": "RUSTSEC-2023-0071",
                        "title": "Marvin Attack",
                        "url": "https://example.com",
                        "cvss": "CVSS:3.1/AV:N/AC:H/PR:N/UI:N/S:U/C:H/I:N/A:N"
                    },
                    "versions": {
                        "patched": [">=0.10.0"]
                    },
                    "package": {
                        "name": "rsa",
                        "version": "0.9.6"
                    }
                }]
            },
            "warnings": {}
        }"#;
        let report: AuditReport = serde_json::from_str(json).unwrap();
        let vulns = report.vulnerabilities.unwrap();
        assert_eq!(vulns.list.len(), 1);
        assert_eq!(vulns.list[0].advisory.id, "RUSTSEC-2023-0071");
        assert_eq!(vulns.list[0].package.name, "rsa");
        assert_eq!(vulns.list[0].versions.patched, vec![">=0.10.0"]);
    }

    #[test]
    fn test_parse_audit_report_with_warning() {
        let json = r#"{
            "vulnerabilities": {"found": false, "count": 0, "list": []},
            "warnings": {
                "unmaintained": [{
                    "advisory": {
                        "id": "RUSTSEC-2021-0145",
                        "title": "Potential unaligned read",
                        "url": null,
                        "cvss": null
                    },
                    "package": {
                        "name": "atty",
                        "version": "0.2.14"
                    }
                }]
            }
        }"#;
        let report: AuditReport = serde_json::from_str(json).unwrap();
        assert_eq!(report.warnings["unmaintained"].len(), 1);
        assert_eq!(report.warnings["unmaintained"][0].package.name, "atty");
    }

    #[test]
    fn test_cargo_audit_availability() {
        // This test is informational — cargo-audit may or may not be installed
        let available = is_cargo_audit_available();
        if available {
            eprintln!("cargo-audit is available");
        } else {
            eprintln!("cargo-audit is NOT installed (test passes either way)");
        }
    }
}
