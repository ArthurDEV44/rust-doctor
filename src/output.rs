use crate::diagnostics::{Category, Diagnostic, DimensionScores, ScanResult, ScoreLabel, Severity};
use owo_colors::{OwoColorize, Stream};
use std::collections::{HashMap, HashSet};

// --- Score constants ---

const ERROR_RULE_PENALTY: f64 = 1.5;
const WARNING_RULE_PENALTY: f64 = 0.75;
const INFO_RULE_PENALTY: f64 = 0.25;
const SCORE_GOOD_THRESHOLD: u32 = 75;
const SCORE_OK_THRESHOLD: u32 = 50;
const SCORE_BAR_WIDTH: usize = 40;

// --- Dimension weights ---

const WEIGHT_SECURITY: f64 = 2.0;
const WEIGHT_RELIABILITY: f64 = 1.5;
const WEIGHT_MAINTAINABILITY: f64 = 1.0;
const WEIGHT_PERFORMANCE: f64 = 1.0;
const WEIGHT_DEPENDENCIES: f64 = 1.0;

// --- Score calculation ---

/// Determine which scoring dimension a category belongs to.
const fn category_dimension(category: &Category) -> Dimension {
    match category {
        Category::Security => Dimension::Security,
        // Async and Framework map to Reliability as they typically involve correctness.
        Category::Correctness | Category::ErrorHandling | Category::Async | Category::Framework => {
            Dimension::Reliability
        }
        Category::Architecture | Category::Style => Dimension::Maintainability,
        Category::Performance => Dimension::Performance,
        Category::Cargo | Category::Dependencies => Dimension::Dependencies,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Dimension {
    Security,
    Reliability,
    Maintainability,
    Performance,
    Dependencies,
}

/// Compute the score for a single dimension from its unique rules by severity.
fn dimension_score(error_count: usize, warning_count: usize, info_count: usize) -> u32 {
    let penalty = (info_count as f64).mul_add(
        INFO_RULE_PENALTY,
        (error_count as f64).mul_add(
            ERROR_RULE_PENALTY,
            warning_count as f64 * WARNING_RULE_PENALTY,
        ),
    );
    (100.0 - penalty).round().clamp(0.0, 100.0) as u32
}

/// Calculate health score from diagnostics using weighted dimension scoring.
///
/// Each diagnostic is assigned to a dimension based on its category.
/// Each dimension is scored independently: `100 - (unique_rules × severity_penalty)`.
/// The overall score is the weighted average of all dimension scores.
///
/// Returns `(score, label, dimension_scores)`.
pub fn calculate_score(diagnostics: &[Diagnostic]) -> (u32, ScoreLabel, DimensionScores) {
    // Collect unique rules per (dimension, severity).
    let mut dim_errors: HashMap<Dimension, HashSet<&str>> = HashMap::new();
    let mut dim_warnings: HashMap<Dimension, HashSet<&str>> = HashMap::new();
    let mut dim_infos: HashMap<Dimension, HashSet<&str>> = HashMap::new();

    for d in diagnostics {
        let dim = category_dimension(&d.category);
        match d.severity {
            Severity::Error => {
                dim_errors.entry(dim).or_default().insert(d.rule.as_str());
            }
            Severity::Warning => {
                dim_warnings.entry(dim).or_default().insert(d.rule.as_str());
            }
            Severity::Info => {
                dim_infos.entry(dim).or_default().insert(d.rule.as_str());
            }
        }
    }

    let score_for = |dim: Dimension| -> u32 {
        dimension_score(
            dim_errors.get(&dim).map_or(0, HashSet::len),
            dim_warnings.get(&dim).map_or(0, HashSet::len),
            dim_infos.get(&dim).map_or(0, HashSet::len),
        )
    };

    let security = score_for(Dimension::Security);
    let reliability = score_for(Dimension::Reliability);
    let maintainability = score_for(Dimension::Maintainability);
    let performance = score_for(Dimension::Performance);
    let dependencies = score_for(Dimension::Dependencies);

    let dimensions = DimensionScores {
        security,
        reliability,
        maintainability,
        performance,
        dependencies,
    };

    // Weighted average
    let total_weight = WEIGHT_SECURITY
        + WEIGHT_RELIABILITY
        + WEIGHT_MAINTAINABILITY
        + WEIGHT_PERFORMANCE
        + WEIGHT_DEPENDENCIES;
    let weighted_sum = f64::from(security).mul_add(
        WEIGHT_SECURITY,
        f64::from(reliability).mul_add(
            WEIGHT_RELIABILITY,
            f64::from(maintainability).mul_add(
                WEIGHT_MAINTAINABILITY,
                f64::from(performance).mul_add(
                    WEIGHT_PERFORMANCE,
                    f64::from(dependencies) * WEIGHT_DEPENDENCIES,
                ),
            ),
        ),
    );
    let score = (weighted_sum / total_weight).round().clamp(0.0, 100.0) as u32;
    let label = score_label(score);

    (score, label, dimensions)
}

const fn score_label(score: u32) -> ScoreLabel {
    if score >= SCORE_GOOD_THRESHOLD {
        ScoreLabel::Great
    } else if score >= SCORE_OK_THRESHOLD {
        ScoreLabel::NeedsWork
    } else {
        ScoreLabel::Critical
    }
}

// --- Terminal output ---

/// Render full scan results to stdout/stderr.
pub fn render_terminal(result: &ScanResult, verbose: bool) {
    // Handle zero files — still show diagnostics (e.g., audit/machete findings)
    if result.source_file_count == 0 && result.diagnostics.is_empty() {
        eprintln!(
            "{}",
            "No Rust source files found".if_supports_color(Stream::Stderr, |t| t.yellow())
        );
        return;
    }

    // Print diagnostics grouped by severity
    if !result.diagnostics.is_empty() {
        print_diagnostics(&result.diagnostics, verbose);
        eprintln!();
    }

    // Print score box
    print_score_box(result);
}

/// Print the ASCII doctor box with score.
fn print_score_box(result: &ScanResult) {
    let score = result.score;
    let label = &result.score_label;

    // Doctor face
    let (eyes, mouth) = if score >= SCORE_GOOD_THRESHOLD {
        ("◠ ◠", " ▽ ")
    } else if score >= SCORE_OK_THRESHOLD {
        ("• •", " ─ ")
    } else {
        ("x x", " △ ")
    };

    // Build content lines
    let score_text = format!("{score} / 100  {label}");
    let bar = build_score_bar(score);
    let ds = &result.dimension_scores;
    let dim_text = format!(
        "Security: {}  Reliability: {}  Maintainability: {}  Performance: {}  Dependencies: {}",
        ds.security, ds.reliability, ds.maintainability, ds.performance, ds.dependencies
    );
    let info_part = if result.info_count > 0 {
        format!("  ℹ {} info(s)", result.info_count)
    } else {
        String::new()
    };
    let stats = format!(
        "{} {} error(s)  {} {} warning(s){info_part}  {} files  {:.1}s",
        if result.error_count > 0 { "✗" } else { "✓" },
        result.error_count,
        if result.warning_count > 0 {
            "⚠"
        } else {
            "✓"
        },
        result.warning_count,
        result.source_file_count,
        result.elapsed.as_secs_f64(),
    );

    // Calculate box width from content widths (avoid cloning strings)
    let max_width = [
        7, // face lines "│ X X │"
        score_text.chars().count(),
        bar.plain.chars().count(),
        dim_text.chars().count(),
        stats.chars().count(),
    ]
    .into_iter()
    .max()
    .unwrap_or(40)
    .max(40);
    let inner_width = max_width + 2; // padding

    // Print box
    let dim =
        |s: &str| -> String { format!("{}", s.if_supports_color(Stream::Stdout, |t| t.dimmed())) };

    let top = format!(
        "  {}{}{}",
        dim("┌"),
        dim(&"─".repeat(inner_width)),
        dim("┐"),
    );
    let bottom = format!(
        "  {}{}{}",
        dim("└"),
        dim(&"─".repeat(inner_width)),
        dim("┘"),
    );

    let pad_line = |content: &str, plain_len: usize| -> String {
        let padding = inner_width.saturating_sub(plain_len + 2);
        format!(
            "  {} {} {}{}",
            dim("│"),
            content,
            " ".repeat(padding),
            dim("│")
        )
    };
    let empty_line =
        || -> String { format!("  {} {}{}", dim("│"), " ".repeat(inner_width - 2), dim("│")) };

    println!("{top}");

    // Doctor face (colored by score)
    println!("{}", pad_line("┌─────┐", 7));
    println!(
        "{}",
        pad_line(&format!("│ {} │", colorize_by_score(eyes, score)), 7)
    );
    println!(
        "{}",
        pad_line(&format!("│ {} │", colorize_by_score(mouth, score)), 7)
    );
    println!("{}", pad_line("└─────┘", 7));

    // Brand
    println!(
        "{}",
        pad_line(
            &format!(
                "{}",
                "rust-doctor".if_supports_color(Stream::Stdout, |t| t.bold())
            ),
            11,
        )
    );
    println!("{}", empty_line());

    // Score
    let colored_score = colorize_by_score(&score_text, score);
    println!("{}", pad_line(&colored_score, score_text.len()));
    println!("{}", empty_line());

    // Bar
    println!("{}", pad_line(&bar.colored, bar.plain.chars().count()));
    println!("{}", empty_line());

    // Dimension scores
    let colored_dim = format!(
        "{}: {}  {}: {}  {}: {}  {}: {}  {}: {}",
        "Security".if_supports_color(Stream::Stdout, |t| t.dimmed()),
        colorize_by_score(&ds.security.to_string(), ds.security),
        "Reliability".if_supports_color(Stream::Stdout, |t| t.dimmed()),
        colorize_by_score(&ds.reliability.to_string(), ds.reliability),
        "Maintainability".if_supports_color(Stream::Stdout, |t| t.dimmed()),
        colorize_by_score(&ds.maintainability.to_string(), ds.maintainability),
        "Performance".if_supports_color(Stream::Stdout, |t| t.dimmed()),
        colorize_by_score(&ds.performance.to_string(), ds.performance),
        "Dependencies".if_supports_color(Stream::Stdout, |t| t.dimmed()),
        colorize_by_score(&ds.dependencies.to_string(), ds.dependencies),
    );
    println!("{}", pad_line(&colored_dim, dim_text.len()));
    println!("{}", empty_line());

    // Stats
    let colored_info_part = if result.info_count > 0 {
        format!(
            "  {} {} info(s)",
            "ℹ".if_supports_color(Stream::Stdout, |t| t.cyan()),
            result.info_count
        )
    } else {
        String::new()
    };
    let colored_stats = format!(
        "{} {} error(s)  {} {} warning(s){colored_info_part}  {} files  {:.1}s",
        colorize_by_score(
            if result.error_count > 0 { "✗" } else { "✓" },
            if result.error_count > 0 { 0 } else { 100 }
        ),
        result.error_count,
        colorize_by_score(
            if result.warning_count > 0 {
                "⚠"
            } else {
                "✓"
            },
            if result.warning_count > 0 { 49 } else { 100 }
        ),
        result.warning_count,
        result.source_file_count,
        result.elapsed.as_secs_f64(),
    );
    println!("{}", pad_line(&colored_stats, stats.len()));

    println!("{bottom}");
}

struct ScoreBar {
    plain: String,
    colored: String,
}

fn build_score_bar(score: u32) -> ScoreBar {
    let filled = ((f64::from(score) / 100.0) * SCORE_BAR_WIDTH as f64).round() as usize;
    let empty = SCORE_BAR_WIDTH - filled;

    let filled_str = "█".repeat(filled);
    let empty_str = "░".repeat(empty);

    let plain = format!("{filled_str}{empty_str}");
    let dimmed_empty = empty_str.if_supports_color(Stream::Stdout, |t| t.dimmed());
    let colored = format!("{}{}", colorize_by_score(&filled_str, score), dimmed_empty,);

    ScoreBar { plain, colored }
}

fn colorize_by_score(text: &str, score: u32) -> String {
    if score >= SCORE_GOOD_THRESHOLD {
        format!("{}", text.if_supports_color(Stream::Stdout, |t| t.green()))
    } else if score >= SCORE_OK_THRESHOLD {
        format!("{}", text.if_supports_color(Stream::Stdout, |t| t.yellow()))
    } else {
        format!("{}", text.if_supports_color(Stream::Stdout, |t| t.red()))
    }
}

/// A grouped diagnostic: a rule with its occurrence count and representative info.
struct DiagGroup {
    rule: String,
    severity: Severity,
    message: String,
    help: Option<String>,
    count: usize,
    occurrences: Vec<DiagOccurrence>,
}

struct DiagOccurrence {
    file_path: String,
    line: Option<u32>,
    column: Option<u32>,
}

/// Print diagnostics grouped by rule, errors first.
fn print_diagnostics(diagnostics: &[Diagnostic], verbose: bool) {
    // Group by rule
    let mut groups: HashMap<String, DiagGroup> = HashMap::new();
    for d in diagnostics {
        let entry = groups.entry(d.rule.clone()).or_insert_with(|| DiagGroup {
            rule: d.rule.clone(),
            severity: d.severity,
            message: d.message.clone(),
            help: d.help.clone(),
            count: 0,
            occurrences: vec![],
        });
        entry.count += 1;
        entry.occurrences.push(DiagOccurrence {
            file_path: d.file_path.to_string_lossy().to_string(),
            line: d.line,
            column: d.column,
        });
    }

    // Sort: errors first, then warnings, then info
    let mut sorted: Vec<_> = groups.into_values().collect();
    sorted.sort_by(|a, b| {
        let severity_ord = |s: &Severity| match s {
            Severity::Error => 0,
            Severity::Warning => 1,
            Severity::Info => 2,
        };
        severity_ord(&a.severity)
            .cmp(&severity_ord(&b.severity))
            .then(a.rule.cmp(&b.rule))
    });

    for group in &sorted {
        let symbol = match group.severity {
            Severity::Error => format!("{}", "✗".if_supports_color(Stream::Stderr, |t| t.red())),
            Severity::Warning => {
                format!("{}", "⚠".if_supports_color(Stream::Stderr, |t| t.yellow()))
            }
            Severity::Info => {
                format!("{}", "ℹ".if_supports_color(Stream::Stderr, |t| t.cyan()))
            }
        };

        eprint!("  {symbol} {}", group.message);
        if group.count > 1 {
            eprint!(
                " {}",
                format!("({})", group.count).if_supports_color(Stream::Stderr, |t| t.dimmed())
            );
        }
        eprintln!();

        if let Some(ref help) = group.help {
            eprintln!(
                "    {}",
                help.if_supports_color(Stream::Stderr, |t| t.dimmed())
            );
        }

        if verbose {
            for occ in &group.occurrences {
                let location = match (occ.line, occ.column) {
                    (Some(l), Some(c)) => format!("{}:{}:{}", occ.file_path, l, c),
                    (Some(l), None) => format!("{}:{}", occ.file_path, l),
                    _ => occ.file_path.clone(),
                };
                eprintln!(
                    "    {}",
                    location.if_supports_color(Stream::Stderr, |t| t.dimmed())
                );
            }
        }

        eprintln!();
    }
}

/// Render `--score` mode: bare integer to stdout.
pub fn render_score(result: &ScanResult) {
    if result.source_file_count == 0 {
        eprintln!(
            "{}",
            "No Rust source files found".if_supports_color(Stream::Stderr, |t| t.yellow())
        );
    }
    println!("{}", result.score);
}

/// Render `--json` mode: full ScanResult as JSON to stdout.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn render_json(result: &ScanResult) -> Result<(), serde_json::Error> {
    let json = serde_json::to_string_pretty(result)?;
    println!("{json}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Category;
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
        // 2 unique error rules in Reliability dimension: dim = 100 - (2 × 1.5) = 97
        // Overall = (100×2.0 + 97×1.5 + 100×1.0 + 100×1.0 + 100×1.0) / 6.5
        //         = (200 + 145.5 + 300) / 6.5 = 645.5 / 6.5 ≈ 99
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
        // 4 unique warning rules in Reliability: dim = 100 - (4 × 0.75) = 97
        // Overall weighted average ≈ 99
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
        // Same rule appearing 5 times = 1 unique error rule in Reliability: dim = 99
        // Overall weighted average rounds to 100
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
        // 10 error + 20 warning rules all in Reliability: dim = 100 - 15 - 15 = 70
        // Overall = (200 + 105 + 300) / 6.5 ≈ 93
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
        // 100 error rules in Reliability: dim = 0
        // Overall = (200 + 0 + 300) / 6.5 ≈ 77
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
        // 100 error rules in every dimension → all dims = 0 → overall = 0
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
        assert_eq!(score_label(100), ScoreLabel::Great);
        assert_eq!(score_label(75), ScoreLabel::Great);
        assert_eq!(score_label(74), ScoreLabel::NeedsWork);
        assert_eq!(score_label(50), ScoreLabel::NeedsWork);
        assert_eq!(score_label(49), ScoreLabel::Critical);
        assert_eq!(score_label(0), ScoreLabel::Critical);
    }

    #[test]
    fn test_score_bar_full() {
        let bar = build_score_bar(100);
        assert_eq!(bar.plain.chars().count(), SCORE_BAR_WIDTH);
        assert!(bar.plain.contains('█'));
        assert!(!bar.plain.contains('░'));
    }

    #[test]
    fn test_score_bar_empty() {
        let bar = build_score_bar(0);
        assert_eq!(bar.plain.chars().count(), SCORE_BAR_WIDTH);
        assert!(!bar.plain.contains('█'));
        assert!(bar.plain.contains('░'));
    }

    #[test]
    fn test_score_bar_half() {
        let bar = build_score_bar(50);
        assert_eq!(bar.plain.chars().count(), SCORE_BAR_WIDTH);
        let filled: usize = bar.plain.chars().filter(|&c| c == '█').count();
        let empty: usize = bar.plain.chars().filter(|&c| c == '░').count();
        assert_eq!(filled, 20);
        assert_eq!(empty, 20);
    }

    // --- Dimension scoring tests ---

    #[test]
    fn test_security_category_only_affects_security_dimension() {
        let diags = vec![
            make_diag_with_category("sec1", Severity::Error, Category::Security),
            make_diag_with_category("sec2", Severity::Error, Category::Security),
        ];
        let (_, _, dims) = calculate_score(&diags);
        // Security: 100 - (2 × 1.5) = 97
        assert_eq!(dims.security, 97);
        // All other dimensions unaffected
        assert_eq!(dims.reliability, 100);
        assert_eq!(dims.maintainability, 100);
        assert_eq!(dims.performance, 100);
        assert_eq!(dims.dependencies, 100);
    }

    #[test]
    fn test_overall_is_weighted_average() {
        // Security = 97 (2 error rules), all others = 100
        // Overall = (97×2.0 + 100×1.5 + 100×1.0 + 100×1.0 + 100×1.0) / 6.5
        //         = (194 + 150 + 100 + 100 + 100) / 6.5
        //         = 644 / 6.5 ≈ 99.08 → 99
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
        // Security: 100 - 1.5 = 98.5 → 99
        assert_eq!(dims.security, 99);
        // Performance: 100 - 0.75 = 99.25 → 99
        assert_eq!(dims.performance, 99);
        // Maintainability: 100 - 0.25 = 99.75 → 100
        assert_eq!(dims.maintainability, 100);
        // Reliability: untouched
        assert_eq!(dims.reliability, 100);
        // Dependencies: untouched
        assert_eq!(dims.dependencies, 100);
    }

    #[test]
    fn test_dependencies_category_maps_to_dependencies_dimension() {
        let diags = vec![
            make_diag_with_category("dep1", Severity::Warning, Category::Dependencies),
            make_diag_with_category("cargo1", Severity::Warning, Category::Cargo),
        ];
        let (_, _, dims) = calculate_score(&diags);
        // Dependencies dim: 100 - (2 × 0.75) = 98.5 → 99
        assert_eq!(dims.dependencies, 99);
        assert_eq!(dims.security, 100);
    }
}
