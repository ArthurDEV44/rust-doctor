mod score;
mod terminal;

pub use score::calculate_score;
pub use terminal::render_terminal;

use crate::diagnostics::ScanResult;
use owo_colors::{OwoColorize, Stream};

/// Render `--score` mode: bare integer to stdout.
pub fn render_score(result: &ScanResult) {
    if result.source_file_count == 0 {
        eprintln!(
            "{}",
            "No Rust source files found".if_supports_color(Stream::Stderr, |t| t.yellow())
        );
    }
    if !result.skipped_passes.is_empty() {
        eprintln!(
            "Warning: {} pass(es) skipped (missing tools) — score may be incomplete. \
             Run: rust-doctor --install-deps",
            result.skipped_passes.len()
        );
    }
    println!("{}", result.score);
}

/// Render `--json` mode: full ScanResult as JSON to stdout.
///
/// Each diagnostic from a syn-only custom rule is tagged with `"heuristic": true`
/// (US-013) so consumers can calibrate confidence vs type-aware clippy lints.
/// The flag is omitted for non-heuristic findings, keeping the output
/// backward-compatible (existing consumers ignore the optional field).
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn render_json(result: &ScanResult) -> Result<(), serde_json::Error> {
    let mut value = serde_json::to_value(result)?;
    annotate_heuristic_diagnostics(&mut value);
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

/// Add `"heuristic": true` to each diagnostic produced by a syn-only rule.
fn annotate_heuristic_diagnostics(value: &mut serde_json::Value) {
    let Some(diagnostics) = value
        .get_mut("diagnostics")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    for diag in diagnostics {
        let is_heuristic = diag
            .get("rule")
            .and_then(serde_json::Value::as_str)
            .is_some_and(crate::rules::is_heuristic_rule);
        if is_heuristic && let Some(obj) = diag.as_object_mut() {
            obj.insert("heuristic".to_string(), serde_json::Value::Bool(true));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{Category, Diagnostic, ScoreLabel, Severity};
    use std::path::PathBuf;

    fn make_diag(rule: &str, severity: Severity) -> Diagnostic {
        make_diag_with_category(rule, severity, Category::ErrorHandling)
    }

    fn make_diag_with_category(rule: &str, severity: Severity, category: Category) -> Diagnostic {
        Diagnostic {
            file_path: PathBuf::from("src/main.rs"),
            rule: rule.to_string(),
            category,
            severity,
            message: format!("Issue: {rule}"),
            help: None,
            line: Some(1),
            column: None,
            fix: None,
        }
    }

    // --- heuristic JSON annotation (US-013) ---

    #[test]
    fn test_annotate_heuristic_diagnostics() {
        let mut value = serde_json::json!({
            "diagnostics": [
                { "rule": "unwrap-in-production" },
                { "rule": "clippy::unwrap_used" },
            ]
        });
        annotate_heuristic_diagnostics(&mut value);
        let diags = value["diagnostics"].as_array().unwrap();
        assert_eq!(diags[0]["heuristic"], serde_json::Value::Bool(true));
        assert!(
            diags[1].get("heuristic").is_none(),
            "clippy lints must not be tagged heuristic"
        );
    }

    #[test]
    fn test_render_json_round_trip_tags_heuristic() {
        // Serialize a REAL ScanResult (not a hand-rolled json! literal): if the
        // `diagnostics` field is ever renamed, `annotate_heuristic_diagnostics`
        // would silently no-op and drop the flag — this test fails instead,
        // guarding the JSON backward-compat contract (US-013).
        use crate::diagnostics::DimensionScores;
        let result = ScanResult {
            diagnostics: vec![
                make_diag("unwrap-in-production", Severity::Warning),
                make_diag("clippy::unwrap_used", Severity::Warning),
            ],
            score: 90,
            score_label: ScoreLabel::Great,
            dimension_scores: DimensionScores {
                security: 100,
                reliability: 90,
                maintainability: 100,
                performance: 100,
                dependencies: 100,
            },
            source_file_count: 1,
            elapsed: std::time::Duration::from_millis(1),
            skipped_passes: vec![],
            error_count: 0,
            warning_count: 2,
            info_count: 0,
            pass_timings: vec![],
        };

        let mut value = serde_json::to_value(&result).unwrap();
        annotate_heuristic_diagnostics(&mut value);
        let diags = value["diagnostics"].as_array().unwrap();

        let heuristic = diags
            .iter()
            .find(|d| d["rule"] == "unwrap-in-production")
            .expect("heuristic diagnostic present");
        assert_eq!(heuristic["heuristic"], serde_json::Value::Bool(true));

        let clippy = diags
            .iter()
            .find(|d| d["rule"] == "clippy::unwrap_used")
            .expect("clippy diagnostic present");
        assert!(
            clippy.get("heuristic").is_none(),
            "clippy lints must not be tagged heuristic"
        );
    }

    // --- Score calculation tests ---

    #[test]
    fn test_perfect_score() {
        let (score, label, dims) = calculate_score(&[]);
        assert_eq!(score, 100);
        assert_eq!(label, ScoreLabel::Great);
        assert_eq!(dims.security, 100);
        assert_eq!(dims.reliability, 100);
        assert_eq!(dims.maintainability, 100);
        assert_eq!(dims.performance, 100);
        assert_eq!(dims.dependencies, 100);
    }

    #[test]
    fn test_score_with_errors_in_reliability() {
        let diags = vec![
            make_diag("rule1", Severity::Error),
            make_diag("rule2", Severity::Error),
        ];
        let (score, label, dims) = calculate_score(&diags);
        assert_eq!(dims.reliability, 97);
        assert_eq!(dims.security, 100);
        assert_eq!(score, 99);
        assert_eq!(label, ScoreLabel::Great);
    }

    #[test]
    fn test_score_with_warnings_in_reliability() {
        let diags = vec![
            make_diag("w1", Severity::Warning),
            make_diag("w2", Severity::Warning),
            make_diag("w3", Severity::Warning),
            make_diag("w4", Severity::Warning),
        ];
        let (score, label, dims) = calculate_score(&diags);
        assert_eq!(dims.reliability, 97);
        assert_eq!(score, 99);
        assert_eq!(label, ScoreLabel::Great);
    }

    #[test]
    fn test_score_duplicate_rules_counted_once() {
        let diags = vec![
            make_diag("rule1", Severity::Error),
            make_diag("rule1", Severity::Error),
            make_diag("rule1", Severity::Error),
            make_diag("rule1", Severity::Error),
            make_diag("rule1", Severity::Error),
        ];
        let (score, _, dims) = calculate_score(&diags);
        assert_eq!(dims.reliability, 99);
        assert_eq!(score, 100);
    }

    #[test]
    fn test_score_mixed_single_dimension() {
        let mut diags = Vec::new();
        for i in 0..10 {
            diags.push(make_diag(&format!("err{i}"), Severity::Error));
        }
        for i in 0..20 {
            diags.push(make_diag(&format!("warn{i}"), Severity::Warning));
        }
        let (score, label, dims) = calculate_score(&diags);
        assert_eq!(dims.reliability, 70);
        assert_eq!(score, 93);
        assert_eq!(label, ScoreLabel::Great);
    }

    #[test]
    fn test_dimension_clamped_to_zero() {
        let mut diags = Vec::new();
        for i in 0..100 {
            diags.push(make_diag(&format!("err{i}"), Severity::Error));
        }
        let (score, label, dims) = calculate_score(&diags);
        assert_eq!(dims.reliability, 0);
        assert_eq!(score, 77);
        assert_eq!(label, ScoreLabel::Great);
    }

    #[test]
    fn test_all_dimensions_severely_degraded() {
        let mut diags = Vec::new();
        for i in 0..100 {
            diags.push(make_diag_with_category(
                &format!("sec{i}"),
                Severity::Error,
                Category::Security,
            ));
            diags.push(make_diag_with_category(
                &format!("err{i}"),
                Severity::Error,
                Category::ErrorHandling,
            ));
            diags.push(make_diag_with_category(
                &format!("arch{i}"),
                Severity::Error,
                Category::Architecture,
            ));
            diags.push(make_diag_with_category(
                &format!("perf{i}"),
                Severity::Error,
                Category::Performance,
            ));
            diags.push(make_diag_with_category(
                &format!("dep{i}"),
                Severity::Error,
                Category::Dependencies,
            ));
        }
        let (score, label, dims) = calculate_score(&diags);
        assert_eq!(dims.security, 0);
        assert_eq!(dims.reliability, 0);
        assert_eq!(dims.maintainability, 0);
        assert_eq!(dims.performance, 0);
        assert_eq!(dims.dependencies, 0);
        assert_eq!(score, 0);
        assert_eq!(label, ScoreLabel::Critical);
    }

    #[test]
    fn test_score_label_thresholds() {
        use score::score_label;
        assert_eq!(score_label(100), ScoreLabel::Great);
        assert_eq!(score_label(75), ScoreLabel::Great);
        assert_eq!(score_label(74), ScoreLabel::NeedsWork);
        assert_eq!(score_label(50), ScoreLabel::NeedsWork);
        assert_eq!(score_label(49), ScoreLabel::Critical);
        assert_eq!(score_label(0), ScoreLabel::Critical);
    }

    #[test]
    fn test_security_category_only_affects_security_dimension() {
        let diags = vec![
            make_diag_with_category("sec1", Severity::Error, Category::Security),
            make_diag_with_category("sec2", Severity::Error, Category::Security),
        ];
        let (_, _, dims) = calculate_score(&diags);
        assert_eq!(dims.security, 97);
        assert_eq!(dims.reliability, 100);
        assert_eq!(dims.maintainability, 100);
        assert_eq!(dims.performance, 100);
        assert_eq!(dims.dependencies, 100);
    }

    #[test]
    fn test_overall_is_weighted_average() {
        let diags = vec![
            make_diag_with_category("sec1", Severity::Error, Category::Security),
            make_diag_with_category("sec2", Severity::Error, Category::Security),
        ];
        let (score, _, _) = calculate_score(&diags);
        assert_eq!(score, 99);
    }

    #[test]
    fn test_empty_diagnostics_all_dimensions_100() {
        let (score, label, dims) = calculate_score(&[]);
        assert_eq!(score, 100);
        assert_eq!(label, ScoreLabel::Great);
        assert_eq!(dims.security, 100);
        assert_eq!(dims.reliability, 100);
        assert_eq!(dims.maintainability, 100);
        assert_eq!(dims.performance, 100);
        assert_eq!(dims.dependencies, 100);
    }

    #[test]
    fn test_multiple_dimensions_affected() {
        let diags = vec![
            make_diag_with_category("sec1", Severity::Error, Category::Security),
            make_diag_with_category("perf1", Severity::Warning, Category::Performance),
            make_diag_with_category("style1", Severity::Info, Category::Style),
        ];
        let (_, _, dims) = calculate_score(&diags);
        assert_eq!(dims.security, 99);
        assert_eq!(dims.performance, 99);
        assert_eq!(dims.maintainability, 100);
        assert_eq!(dims.reliability, 100);
        assert_eq!(dims.dependencies, 100);
    }

    #[test]
    fn test_dependencies_category_maps_to_dependencies_dimension() {
        let diags = vec![
            make_diag_with_category("dep1", Severity::Warning, Category::Dependencies),
            make_diag_with_category("cargo1", Severity::Warning, Category::Cargo),
        ];
        let (_, _, dims) = calculate_score(&diags);
        assert_eq!(dims.dependencies, 99);
        assert_eq!(dims.security, 100);
    }
}
