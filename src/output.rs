use crate::diagnostics::{Diagnostic, ScanResult, ScoreLabel, Severity};
use owo_colors::{OwoColorize, Stream};
use std::collections::{HashMap, HashSet};

// --- Score constants ---

const ERROR_RULE_PENALTY: f64 = 1.5;
const WARNING_RULE_PENALTY: f64 = 0.75;
const INFO_RULE_PENALTY: f64 = 0.25;
const SCORE_GOOD_THRESHOLD: u32 = 75;
const SCORE_OK_THRESHOLD: u32 = 50;
const SCORE_BAR_WIDTH: usize = 40;

// --- Score calculation ---

/// Calculate health score from diagnostics.
/// Score = 100 - (unique_error_rules × 1.5) - (unique_warning_rules × 0.75), clamped 0–100.
/// Returns (score, label).
pub fn calculate_score(diagnostics: &[Diagnostic]) -> (u32, ScoreLabel) {
    let mut error_rules = HashSet::new();
    let mut warning_rules = HashSet::new();
    let mut info_rules = HashSet::new();

    for d in diagnostics {
        match d.severity {
            Severity::Error => {
                error_rules.insert(d.rule.as_str());
            }
            Severity::Warning => {
                warning_rules.insert(d.rule.as_str());
            }
            Severity::Info => {
                info_rules.insert(d.rule.as_str());
            }
        }
    }

    let penalty = (info_rules.len() as f64).mul_add(
        INFO_RULE_PENALTY,
        (error_rules.len() as f64).mul_add(
            ERROR_RULE_PENALTY,
            warning_rules.len() as f64 * WARNING_RULE_PENALTY,
        ),
    );

    let score = (100.0 - penalty).round().clamp(0.0, 100.0) as u32;
    let label = score_label(score);

    (score, label)
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

    // Calculate box width
    let content_lines = [
        format!("│ {eyes} │"),
        format!("│ {mouth} │"),
        String::new(), // blank
        score_text.clone(),
        String::new(), // blank
        bar.plain.clone(),
        String::new(), // blank
        stats.clone(),
    ];
    let max_width = content_lines
        .iter()
        .map(|l| l.chars().count())
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

        let count_suffix = if group.count > 1 {
            format!(
                " {}",
                format!("({})", group.count).if_supports_color(Stream::Stderr, |t| t.dimmed())
            )
        } else {
            String::new()
        };

        eprintln!("  {symbol} {}{count_suffix}", group.message);

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
        Diagnostic {
            file_path: PathBuf::from("src/main.rs"),
            rule: rule.to_string(),
            category: Category::ErrorHandling,
            severity,
            message: format!("Issue: {rule}"),
            help: None,
            line: Some(1),
            column: None,
        }
    }

    // --- Score calculation tests ---

    #[test]
    fn test_perfect_score() {
        let (score, label) = calculate_score(&[]);
        assert_eq!(score, 100);
        assert_eq!(label, ScoreLabel::Great);
    }

    #[test]
    fn test_score_with_errors() {
        // 2 unique error rules: 100 - (2 × 1.5) = 97
        let diags = vec![
            make_diag("rule1", Severity::Error),
            make_diag("rule2", Severity::Error),
        ];
        let (score, label) = calculate_score(&diags);
        assert_eq!(score, 97);
        assert_eq!(label, ScoreLabel::Great);
    }

    #[test]
    fn test_score_with_warnings() {
        // 4 unique warning rules: 100 - (4 × 0.75) = 97
        let diags = vec![
            make_diag("w1", Severity::Warning),
            make_diag("w2", Severity::Warning),
            make_diag("w3", Severity::Warning),
            make_diag("w4", Severity::Warning),
        ];
        let (score, label) = calculate_score(&diags);
        assert_eq!(score, 97);
        assert_eq!(label, ScoreLabel::Great);
    }

    #[test]
    fn test_score_duplicate_rules_counted_once() {
        // Same rule appearing 5 times = 1 unique error rule: 100 - (1 × 1.5) = 98.5 → rounds to 99
        let diags = vec![
            make_diag("rule1", Severity::Error),
            make_diag("rule1", Severity::Error),
            make_diag("rule1", Severity::Error),
            make_diag("rule1", Severity::Error),
            make_diag("rule1", Severity::Error),
        ];
        let (score, _) = calculate_score(&diags);
        assert_eq!(score, 99);
    }

    #[test]
    fn test_score_mixed() {
        // 10 error rules + 20 warning rules: 100 - (10×1.5) - (20×0.75) = 100 - 15 - 15 = 70
        let mut diags = Vec::new();
        for i in 0..10 {
            diags.push(make_diag(&format!("err{i}"), Severity::Error));
        }
        for i in 0..20 {
            diags.push(make_diag(&format!("warn{i}"), Severity::Warning));
        }
        let (score, label) = calculate_score(&diags);
        assert_eq!(score, 70);
        assert_eq!(label, ScoreLabel::NeedsWork);
    }

    #[test]
    fn test_score_clamped_to_zero() {
        // 100 error rules: 100 - (100×1.5) = 100 - 150 = -50 → clamped to 0
        let mut diags = Vec::new();
        for i in 0..100 {
            diags.push(make_diag(&format!("err{i}"), Severity::Error));
        }
        let (score, label) = calculate_score(&diags);
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
}
