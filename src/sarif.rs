//! SARIF 2.1.0 output for CI/CD integration (GitHub Code Scanning, GitLab SAST).
//!
//! Produces a valid SARIF 2.1.0 JSON file from a `ScanResult`.
//! See <https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html>

use crate::diagnostics::{Diagnostic, ScanResult, Severity};
use serde::Serialize;

const SARIF_SCHEMA: &str =
    "https://schemastore.azurewebsites.net/schemas/json/sarif-2.1.0-rtm.5.json";
const SARIF_VERSION: &str = "2.1.0";
const TOOL_NAME: &str = "rust-doctor";

// ---------------------------------------------------------------------------
// SARIF types (minimal subset for GitHub Code Scanning compatibility)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifLog {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: &'static str,
    runs: Vec<Run>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Run {
    tool: Tool,
    results: Vec<Result_>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Tool {
    driver: ToolComponent,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolComponent {
    name: &'static str,
    version: String,
    information_uri: &'static str,
    rules: Vec<ReportingDescriptor>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportingDescriptor {
    id: String,
    short_description: Message,
    #[serde(skip_serializing_if = "Option::is_none")]
    help_uri: Option<String>,
    default_configuration: DefaultConfiguration,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DefaultConfiguration {
    level: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Result_ {
    rule_id: String,
    level: &'static str,
    message: Message,
    locations: Vec<Location>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Message {
    text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Location {
    physical_location: PhysicalLocation,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicalLocation {
    artifact_location: ArtifactLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<Region>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactLocation {
    uri: String,
    uri_base_id: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Region {
    start_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_column: Option<u32>,
}

// ---------------------------------------------------------------------------
// Conversion
// ---------------------------------------------------------------------------

fn severity_to_sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
    }
}

fn build_rules(diagnostics: &[Diagnostic]) -> Vec<ReportingDescriptor> {
    let mut seen = std::collections::HashSet::new();
    let mut rules = Vec::new();

    for d in diagnostics {
        if seen.insert(d.rule.clone()) {
            rules.push(ReportingDescriptor {
                id: d.rule.clone(),
                short_description: Message {
                    text: d.message.clone(),
                },
                help_uri: d.help.clone(),
                default_configuration: DefaultConfiguration {
                    level: severity_to_sarif_level(d.severity),
                },
            });
        }
    }

    rules
}

fn diagnostic_to_result(d: &Diagnostic) -> Result_ {
    let region = d.line.map(|line| Region {
        start_line: line,
        start_column: d.column,
    });

    Result_ {
        rule_id: d.rule.clone(),
        level: severity_to_sarif_level(d.severity),
        message: Message {
            text: if let Some(ref help) = d.help {
                format!("{} — {help}", d.message)
            } else {
                d.message.clone()
            },
        },
        locations: vec![Location {
            physical_location: PhysicalLocation {
                artifact_location: ArtifactLocation {
                    uri: d.file_path.to_string_lossy().into_owned(),
                    uri_base_id: "%SRCROOT%",
                },
                region,
            },
        }],
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Convert a `ScanResult` into a SARIF 2.1.0 JSON string.
///
/// # Errors
///
/// Returns an error if JSON serialization fails.
pub fn render_sarif(scan_result: &ScanResult) -> Result<String, serde_json::Error> {
    let rules = build_rules(&scan_result.diagnostics);
    let results: Vec<Result_> = scan_result
        .diagnostics
        .iter()
        .map(diagnostic_to_result)
        .collect();

    let log = SarifLog {
        schema: SARIF_SCHEMA,
        version: SARIF_VERSION,
        runs: vec![Run {
            tool: Tool {
                driver: ToolComponent {
                    name: TOOL_NAME,
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    information_uri: "https://github.com/ArthurDEV44/rust-doctor",
                    rules,
                },
            },
            results,
        }],
    };

    serde_json::to_string_pretty(&log)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{Category, ScoreLabel};
    use std::path::PathBuf;
    use std::time::Duration;

    fn make_scan_result(diagnostics: Vec<Diagnostic>) -> ScanResult {
        let error_count = diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count();
        let warning_count = diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count();
        let info_count = diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Info)
            .count();

        ScanResult {
            diagnostics,
            score: 85,
            score_label: ScoreLabel::Great,
            dimension_scores: crate::diagnostics::DimensionScores {
                security: 100,
                reliability: 100,
                maintainability: 100,
                performance: 100,
                dependencies: 100,
            },
            source_file_count: 10,
            elapsed: Duration::from_millis(100),
            skipped_passes: vec![],
            error_count,
            warning_count,
            info_count,
        }
    }

    #[test]
    fn test_empty_scan_produces_valid_sarif() {
        let result = make_scan_result(vec![]);
        let sarif = render_sarif(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&sarif).unwrap();
        assert_eq!(parsed["version"], "2.1.0");
        assert_eq!(parsed["runs"][0]["tool"]["driver"]["name"], "rust-doctor");
        assert!(parsed["runs"][0]["results"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_diagnostics_map_to_sarif_results() {
        let diags = vec![
            Diagnostic {
                file_path: PathBuf::from("src/main.rs"),
                rule: "unwrap-in-production".to_string(),
                category: Category::ErrorHandling,
                severity: Severity::Warning,
                message: "Use of .unwrap() in production code".to_string(),
                help: Some("Use ? operator instead".to_string()),
                line: Some(42),
                column: Some(10),
            },
            Diagnostic {
                file_path: PathBuf::from("src/lib.rs"),
                rule: "hardcoded-secrets".to_string(),
                category: Category::Security,
                severity: Severity::Error,
                message: "Hardcoded secret detected".to_string(),
                help: None,
                line: Some(7),
                column: None,
            },
        ];
        let result = make_scan_result(diags);
        let sarif = render_sarif(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&sarif).unwrap();

        let results = parsed["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["ruleId"], "unwrap-in-production");
        assert_eq!(results[0]["level"], "warning");
        assert_eq!(
            results[0]["locations"][0]["physicalLocation"]["region"]["startLine"],
            42
        );
        assert_eq!(results[1]["ruleId"], "hardcoded-secrets");
        assert_eq!(results[1]["level"], "error");
    }

    #[test]
    fn test_severity_mapping() {
        assert_eq!(severity_to_sarif_level(Severity::Error), "error");
        assert_eq!(severity_to_sarif_level(Severity::Warning), "warning");
        assert_eq!(severity_to_sarif_level(Severity::Info), "note");
    }

    #[test]
    fn test_rules_are_deduplicated() {
        let diags = vec![
            Diagnostic {
                file_path: PathBuf::from("a.rs"),
                rule: "same-rule".to_string(),
                category: Category::Style,
                severity: Severity::Warning,
                message: "msg1".to_string(),
                help: None,
                line: Some(1),
                column: None,
            },
            Diagnostic {
                file_path: PathBuf::from("b.rs"),
                rule: "same-rule".to_string(),
                category: Category::Style,
                severity: Severity::Warning,
                message: "msg2".to_string(),
                help: None,
                line: Some(5),
                column: None,
            },
        ];
        let result = make_scan_result(diags);
        let sarif = render_sarif(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&sarif).unwrap();

        let rules = parsed["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .unwrap();
        assert_eq!(rules.len(), 1, "duplicate rules should be deduplicated");
        assert_eq!(
            parsed["runs"][0]["results"].as_array().unwrap().len(),
            2,
            "all results should be present"
        );
    }
}
