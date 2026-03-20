use crate::diagnostics::{ScanResult, Severity};
use owo_colors::{OwoColorize, Stream};
use std::collections::HashMap;
use unicode_width::UnicodeWidthStr;

use super::score::{SCORE_GOOD_THRESHOLD, SCORE_OK_THRESHOLD};

const SCORE_BAR_WIDTH: usize = 40;

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
    let skipped_text = if result.skipped_passes.is_empty() {
        None
    } else {
        Some(format!(
            "⊘ {} pass(es) skipped — run: rust-doctor --install-deps",
            result.skipped_passes.len()
        ))
    };

    // Calculate box width from content display widths
    let mut widths = vec![
        7_usize, // face lines "│ X X │"
        score_text.width(),
        bar.plain.width(),
        dim_text.width(),
        stats.width(),
    ];
    if let Some(ref text) = skipped_text {
        widths.push(text.width());
    }
    let max_width = widths.into_iter().max().unwrap_or(40).max(40);
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
    println!("{}", pad_line(&colored_score, score_text.width()));
    println!("{}", empty_line());

    // Bar
    println!("{}", pad_line(&bar.colored, bar.plain.width()));
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
    println!("{}", pad_line(&colored_dim, dim_text.width()));
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
    println!("{}", pad_line(&colored_stats, stats.width()));

    // Skipped passes hint
    if let Some(ref text) = skipped_text {
        println!("{}", empty_line());
        let colored_skip = format!(
            "{} {} pass(es) skipped — run: {}",
            "⊘".if_supports_color(Stream::Stdout, |t| t.yellow()),
            result.skipped_passes.len(),
            "rust-doctor --install-deps".if_supports_color(Stream::Stdout, |t| t.bold()),
        );
        println!("{}", pad_line(&colored_skip, text.width()));
    }

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
struct DiagGroup<'a> {
    rule: &'a str,
    severity: Severity,
    message: &'a str,
    help: Option<&'a str>,
    count: usize,
    occurrences: Vec<DiagOccurrence<'a>>,
}

struct DiagOccurrence<'a> {
    file_path: std::borrow::Cow<'a, str>,
    line: Option<u32>,
    column: Option<u32>,
}

/// Print diagnostics grouped by rule, errors first.
fn print_diagnostics(diagnostics: &[crate::diagnostics::Diagnostic], verbose: bool) {
    // Group by rule — borrow from diagnostics to avoid cloning strings
    let mut groups: HashMap<&str, DiagGroup<'_>> = HashMap::new();
    for d in diagnostics {
        let entry = groups.entry(&d.rule).or_insert_with(|| DiagGroup {
            rule: &d.rule,
            severity: d.severity,
            message: &d.message,
            help: d.help.as_deref(),
            count: 0,
            occurrences: vec![],
        });
        entry.count += 1;
        entry.occurrences.push(DiagOccurrence {
            file_path: d.file_path.to_string_lossy(),
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
            .then(a.rule.cmp(b.rule))
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
                let location: std::borrow::Cow<'_, str> = match (occ.line, occ.column) {
                    (Some(l), Some(c)) => format!("{}:{}:{}", occ.file_path, l, c).into(),
                    (Some(l), None) => format!("{}:{}", occ.file_path, l).into(),
                    _ => std::borrow::Cow::Borrowed(occ.file_path.as_ref()),
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
