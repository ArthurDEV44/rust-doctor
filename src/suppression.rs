use crate::diagnostics::Diagnostic;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const DISABLE_NEXT_LINE: &str = "rust-doctor-disable-next-line";
const DISABLE_LINE: &str = "rust-doctor-disable-line";

/// A suppression directive found in source code.
#[derive(Debug)]
struct Suppression {
    /// The line this suppression applies to (1-based).
    target_line: u32,
    /// Rule name to suppress, or None for all rules.
    rule: Option<String>,
}

/// Apply inline suppression comments to filter diagnostics.
///
/// Reads source files referenced by diagnostics, finds `rust-doctor-disable-*`
/// comments, and removes matching diagnostics. Returns the filtered list and
/// the count of suppressed diagnostics.
pub fn apply_inline_suppressions(
    diagnostics: Vec<Diagnostic>,
    project_root: &Path,
) -> (Vec<Diagnostic>, usize) {
    if diagnostics.is_empty() {
        return (diagnostics, 0);
    }

    // Collect unique file paths that have diagnostics with line numbers
    let files_to_check: Vec<PathBuf> = diagnostics
        .iter()
        .filter(|d| d.line.is_some())
        .map(|d| d.file_path.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Parse suppression comments from each file
    let mut suppressions: HashMap<PathBuf, Vec<Suppression>> = HashMap::new();
    for file_path in &files_to_check {
        // Try absolute path first, then relative to project root
        let abs_path = if file_path.is_absolute() {
            file_path.clone()
        } else {
            project_root.join(file_path)
        };

        if let Ok(content) = fs::read_to_string(&abs_path) {
            let file_suppressions = parse_suppressions(&content);
            if !file_suppressions.is_empty() {
                suppressions.insert(file_path.clone(), file_suppressions);
            }
        }
    }

    if suppressions.is_empty() {
        return (diagnostics, 0);
    }

    // Filter diagnostics
    let original_count = diagnostics.len();
    let filtered: Vec<Diagnostic> = diagnostics
        .into_iter()
        .filter(|d| !is_suppressed(d, &suppressions))
        .collect();
    let suppressed_count = original_count - filtered.len();

    (filtered, suppressed_count)
}

/// Parse suppression comments from file content.
fn parse_suppressions(content: &str) -> Vec<Suppression> {
    let mut suppressions = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = (line_idx + 1) as u32;
        let trimmed = line.trim();

        // Check for // rust-doctor-disable-next-line [rule]
        if let Some(rest) = extract_comment_directive(trimmed, DISABLE_NEXT_LINE) {
            let rule = parse_rule_name(rest);
            suppressions.push(Suppression {
                target_line: line_num + 1, // applies to the NEXT line
                rule,
            });
        }

        // Check for // rust-doctor-disable-line [rule]
        if let Some(rest) = extract_comment_directive(trimmed, DISABLE_LINE) {
            let rule = parse_rule_name(rest);
            suppressions.push(Suppression {
                target_line: line_num, // applies to THIS line
                rule,
            });
        }

        // Also check for inline comments at end of line: `code // rust-doctor-disable-line`
        if !trimmed.starts_with("//")
            && let Some(comment_start) = line.find("//")
        {
            let comment = line[comment_start + 2..].trim();
            if let Some(rest) = comment.strip_prefix(DISABLE_LINE) {
                let rule = parse_rule_name(rest);
                suppressions.push(Suppression {
                    target_line: line_num,
                    rule,
                });
            }
        }
    }

    suppressions
}

/// Extract the rest of a comment after a directive prefix.
fn extract_comment_directive<'a>(line: &'a str, directive: &str) -> Option<&'a str> {
    // Match: // directive [rest]
    let stripped = line.strip_prefix("//")?;
    let stripped = stripped.trim_start();
    stripped.strip_prefix(directive).map(|rest| rest.trim())
}

/// Parse an optional rule name from the rest of a directive.
fn parse_rule_name(rest: &str) -> Option<String> {
    let name = rest.trim();
    if name.is_empty() {
        None // No rule = suppress all
    } else {
        Some(name.to_string())
    }
}

/// Check if a diagnostic is suppressed by any suppression in its file.
fn is_suppressed(diag: &Diagnostic, suppressions: &HashMap<PathBuf, Vec<Suppression>>) -> bool {
    let Some(line) = diag.line else {
        return false; // Diagnostics without line numbers can't be suppressed inline
    };

    let Some(file_suppressions) = suppressions.get(&diag.file_path) else {
        return false;
    };

    file_suppressions.iter().any(|s| {
        s.target_line == line && (s.rule.is_none() || s.rule.as_deref() == Some(diag.rule.as_str()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{Category, Severity};

    fn make_diag(file: &str, rule: &str, line: u32) -> Diagnostic {
        Diagnostic {
            file_path: PathBuf::from(file),
            rule: rule.to_string(),
            category: Category::Style,
            severity: Severity::Warning,
            message: "test".to_string(),
            help: None,
            line: Some(line),
            column: None,
        }
    }

    // --- parse_suppressions ---

    #[test]
    fn test_parse_disable_next_line_with_rule() {
        let content =
            "// rust-doctor-disable-next-line unwrap-in-production\nlet x = foo.unwrap();\n";
        let supps = parse_suppressions(content);
        assert_eq!(supps.len(), 1);
        assert_eq!(supps[0].target_line, 2);
        assert_eq!(supps[0].rule, Some("unwrap-in-production".to_string()));
    }

    #[test]
    fn test_parse_disable_next_line_no_rule() {
        let content = "// rust-doctor-disable-next-line\nlet x = foo.unwrap();\n";
        let supps = parse_suppressions(content);
        assert_eq!(supps.len(), 1);
        assert_eq!(supps[0].target_line, 2);
        assert_eq!(supps[0].rule, None);
    }

    #[test]
    fn test_parse_disable_line() {
        let content = "let x = foo.unwrap(); // rust-doctor-disable-line\n";
        let supps = parse_suppressions(content);
        assert_eq!(supps.len(), 1);
        assert_eq!(supps[0].target_line, 1);
        assert_eq!(supps[0].rule, None);
    }

    #[test]
    fn test_parse_disable_line_with_rule() {
        let content = "let x = foo.unwrap(); // rust-doctor-disable-line unwrap-in-production\n";
        let supps = parse_suppressions(content);
        assert_eq!(supps.len(), 1);
        assert_eq!(supps[0].rule, Some("unwrap-in-production".to_string()));
    }

    #[test]
    fn test_parse_standalone_disable_line_comment() {
        let content = "// rust-doctor-disable-line some-rule\n";
        let supps = parse_suppressions(content);
        assert_eq!(supps.len(), 1);
        assert_eq!(supps[0].target_line, 1);
        assert_eq!(supps[0].rule, Some("some-rule".to_string()));
    }

    #[test]
    fn test_parse_no_suppressions() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let supps = parse_suppressions(content);
        assert!(supps.is_empty());
    }

    #[test]
    fn test_parse_multiple_suppressions() {
        let content = "// rust-doctor-disable-next-line rule-a\nline1\n// rust-doctor-disable-next-line rule-b\nline2\n";
        let supps = parse_suppressions(content);
        assert_eq!(supps.len(), 2);
    }

    // --- is_suppressed ---

    #[test]
    fn test_suppressed_by_specific_rule() {
        let diag = make_diag("test.rs", "unwrap-in-production", 5);
        let mut suppressions = HashMap::new();
        suppressions.insert(
            PathBuf::from("test.rs"),
            vec![Suppression {
                target_line: 5,
                rule: Some("unwrap-in-production".to_string()),
            }],
        );
        assert!(is_suppressed(&diag, &suppressions));
    }

    #[test]
    fn test_suppressed_by_wildcard() {
        let diag = make_diag("test.rs", "any-rule", 5);
        let mut suppressions = HashMap::new();
        suppressions.insert(
            PathBuf::from("test.rs"),
            vec![Suppression {
                target_line: 5,
                rule: None,
            }],
        );
        assert!(is_suppressed(&diag, &suppressions));
    }

    #[test]
    fn test_not_suppressed_wrong_rule() {
        let diag = make_diag("test.rs", "rule-a", 5);
        let mut suppressions = HashMap::new();
        suppressions.insert(
            PathBuf::from("test.rs"),
            vec![Suppression {
                target_line: 5,
                rule: Some("rule-b".to_string()),
            }],
        );
        assert!(!is_suppressed(&diag, &suppressions));
    }

    #[test]
    fn test_not_suppressed_wrong_line() {
        let diag = make_diag("test.rs", "rule-a", 5);
        let mut suppressions = HashMap::new();
        suppressions.insert(
            PathBuf::from("test.rs"),
            vec![Suppression {
                target_line: 10,
                rule: Some("rule-a".to_string()),
            }],
        );
        assert!(!is_suppressed(&diag, &suppressions));
    }

    #[test]
    fn test_not_suppressed_no_line_number() {
        let mut diag = make_diag("test.rs", "rule-a", 5);
        diag.line = None;
        let mut suppressions = HashMap::new();
        suppressions.insert(
            PathBuf::from("test.rs"),
            vec![Suppression {
                target_line: 5,
                rule: None,
            }],
        );
        assert!(!is_suppressed(&diag, &suppressions));
    }

    // --- apply_inline_suppressions with real files ---

    #[test]
    fn test_apply_with_temp_file() {
        let dir = std::env::temp_dir().join("rust-doctor-test-suppression");
        let _ = std::fs::create_dir_all(&dir);
        let file_path = dir.join("test.rs");
        std::fs::write(
            &file_path,
            "// rust-doctor-disable-next-line test-rule\nlet x = 1;\nlet y = 2;\n",
        )
        .unwrap();

        let diags = vec![
            Diagnostic {
                file_path: file_path.clone(),
                rule: "test-rule".to_string(),
                category: Category::Style,
                severity: Severity::Warning,
                message: "test".to_string(),
                help: None,
                line: Some(2),
                column: None,
            },
            Diagnostic {
                file_path: file_path.clone(),
                rule: "other-rule".to_string(),
                category: Category::Style,
                severity: Severity::Warning,
                message: "test".to_string(),
                help: None,
                line: Some(3),
                column: None,
            },
        ];

        let (filtered, suppressed) = apply_inline_suppressions(diags, &dir);
        assert_eq!(suppressed, 1);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].rule, "other-rule");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
