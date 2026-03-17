use schemars::JsonSchema;
use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;

/// Severity of a diagnostic finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
        }
    }
}

/// Category of a diagnostic rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    ErrorHandling,
    Performance,
    Security,
    Correctness,
    Architecture,
    Dependencies,
    Async,
    Framework,
    Cargo,
    Style,
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ErrorHandling => write!(f, "Error Handling"),
            Self::Performance => write!(f, "Performance"),
            Self::Security => write!(f, "Security"),
            Self::Correctness => write!(f, "Correctness"),
            Self::Architecture => write!(f, "Architecture"),
            Self::Dependencies => write!(f, "Dependencies"),
            Self::Async => write!(f, "Async"),
            Self::Framework => write!(f, "Framework"),
            Self::Cargo => write!(f, "Cargo"),
            Self::Style => write!(f, "Style"),
        }
    }
}

/// A single diagnostic finding from an analysis pass.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Diagnostic {
    /// Path to the source file (relative to project root).
    pub file_path: PathBuf,
    /// Rule identifier (e.g. "unwrap-in-production", "clippy::unwrap_used").
    pub rule: String,
    /// Category this rule belongs to.
    pub category: Category,
    /// Severity of the finding.
    pub severity: Severity,
    /// Human-readable description of the issue.
    pub message: String,
    /// Actionable suggestion for how to fix the issue.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// Line number (1-based) where the issue was found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Column number (1-based) where the issue was found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

/// Result of a complete scan across all analysis passes.
#[derive(Debug, Serialize)]
pub struct ScanResult {
    /// All diagnostics found (after filtering).
    pub diagnostics: Vec<Diagnostic>,
    /// Health score (0–100).
    pub score: u32,
    /// Score label (e.g. "Great", "Needs work", "Critical").
    pub score_label: &'static str,
    /// Number of source files scanned.
    pub source_file_count: usize,
    /// Total scan duration.
    #[serde(serialize_with = "serialize_duration")]
    pub elapsed: Duration,
    /// Names of analysis passes that were skipped or failed.
    pub skipped_passes: Vec<String>,
    /// Number of errors found.
    pub error_count: usize,
    /// Number of warnings found.
    pub warning_count: usize,
}

fn serialize_duration<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_f64(duration.as_secs_f64())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_display() {
        assert_eq!(Severity::Error.to_string(), "error");
        assert_eq!(Severity::Warning.to_string(), "warning");
    }

    #[test]
    fn test_category_display() {
        assert_eq!(Category::ErrorHandling.to_string(), "Error Handling");
        assert_eq!(Category::Performance.to_string(), "Performance");
        assert_eq!(Category::Security.to_string(), "Security");
    }

    #[test]
    fn test_diagnostic_serialize() {
        let diag = Diagnostic {
            file_path: PathBuf::from("src/main.rs"),
            rule: "unwrap-in-production".to_string(),
            category: Category::ErrorHandling,
            severity: Severity::Warning,
            message: "Use of .unwrap() in production code".to_string(),
            help: Some("Use ? operator or handle the error explicitly".to_string()),
            line: Some(42),
            column: Some(10),
        };
        let json = serde_json::to_value(&diag).unwrap();
        assert_eq!(json["rule"], "unwrap-in-production");
        assert_eq!(json["severity"], "warning");
        assert_eq!(json["category"], "error-handling");
        assert_eq!(json["line"], 42);
    }

    #[test]
    fn test_diagnostic_serialize_no_optionals() {
        let diag = Diagnostic {
            file_path: PathBuf::from("Cargo.toml"),
            rule: "unused-dependency".to_string(),
            category: Category::Dependencies,
            severity: Severity::Warning,
            message: "Unused dependency: serde".to_string(),
            help: None,
            line: None,
            column: None,
        };
        let json = serde_json::to_value(&diag).unwrap();
        assert!(json.get("help").is_none());
        assert!(json.get("line").is_none());
        assert!(json.get("column").is_none());
    }

    #[test]
    fn test_scan_result_serialize() {
        let result = ScanResult {
            diagnostics: vec![],
            score: 100,
            score_label: "Great",
            source_file_count: 10,
            elapsed: Duration::from_millis(1500),
            skipped_passes: vec![],
            error_count: 0,
            warning_count: 0,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["score"], 100);
        assert_eq!(json["score_label"], "Great");
        assert_eq!(json["source_file_count"], 10);
        assert_eq!(json["elapsed"], 1.5);
        assert_eq!(json["error_count"], 0);
    }
}
