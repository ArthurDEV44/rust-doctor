use crate::{clippy, rules};

// ---------------------------------------------------------------------------
// Rule knowledge base (derived from trait implementations at runtime)
// ---------------------------------------------------------------------------

pub(super) struct RuleDoc {
    pub(super) name: &'static str,
    pub(super) category: String,
    pub(super) severity: String,
    pub(super) description: &'static str,
    pub(super) fix: &'static str,
}

/// Return cached rule docs. Computed once on first call since rules are static.
pub(super) fn rule_docs() -> &'static [RuleDoc] {
    static DOCS: std::sync::OnceLock<Vec<RuleDoc>> = std::sync::OnceLock::new();
    DOCS.get_or_init(|| {
        rules::all_custom_rules()
            .iter()
            .map(|rule| RuleDoc {
                name: rule.name(),
                category: rule.category().to_string(),
                severity: rule.severity().to_string(),
                description: rule.description(),
                fix: rule.fix_hint(),
            })
            .collect()
    })
}

/// Documented structural false-positive caveats for specific heuristic rules.
///
/// These syn-only rules are correct in spirit but, lacking type information,
/// have known blind spots worth surfacing so users calibrate confidence.
pub(super) fn rule_limitation(rule: &str) -> Option<&'static str> {
    match rule {
        "unwrap-in-production" => Some(
            "Matches `.unwrap()`/`.expect()` syntactically; it cannot tell a \
             provably-infallible unwrap from a risky one.",
        ),
        "large-enum-variant" => Some(
            "Counts a variant's fields, not its byte size; a few wide-type fields \
             can outweigh many small ones, and vice versa.",
        ),
        "blocking-in-async" => Some(
            "Flags known blocking calls by name inside async fns; it cannot follow \
             calls into other functions or resolve aliased imports.",
        ),
        "sql-injection-risk" => Some(
            "Flags string-built queries heuristically; it cannot confirm the \
             interpolated value is actually untrusted input.",
        ),
        _ => None,
    }
}

pub(super) fn get_rule_explanation(rule: &str) -> String {
    // Look up in the data-driven registry first
    let docs = rule_docs();
    if let Some(doc) = docs.iter().find(|d| d.name == rule) {
        let mut out = format!(
            "## {}\n\n**Category:** {} | **Severity:** {} | **Analysis:** Heuristic (syn AST only — no type info or macro expansion)\n\n{}\n\n**Fix:** {}",
            doc.name, doc.category, doc.severity, doc.description, doc.fix
        );
        if let Some(limitation) = rule_limitation(rule) {
            use std::fmt::Write;
            let _ = write!(out, "\n\n**Known limitation:** {limitation}");
        }
        return out;
    }

    // Fall back to clippy lint lookup
    let lint_name = rule.strip_prefix("clippy::").unwrap_or(rule);
    if clippy::known_lint_names().contains(&lint_name) {
        format!(
            "## {rule}\n\n**Analysis:** Type-aware (Clippy lint, resolved against the compiler).\n\nThis is a Clippy lint tracked by rust-doctor with custom severity/category mapping.\n\nSee full documentation: https://rust-lang.github.io/rust-clippy/master/index.html#{lint_name}"
        )
    } else {
        format!("Unknown rule: `{rule}`\n\nUse the `list_rules` tool to see all available rules.")
    }
}

pub(super) fn get_all_rules_listing() -> String {
    let mut text = String::from(
        "# rust-doctor Rules\n\n\
         ## Custom Rules (heuristic — AST-based via syn)\n\n\
         These rules analyze the syntax tree only: no type resolution, no macro \
         expansion. They are fast and offline, but report a *heuristic* signal — \
         calibrate confidence accordingly (rules marked ⚠ below have known blind \
         spots). Findings are tagged `heuristic` in JSON and SARIF output.\n\n",
    );

    use std::fmt::Write;
    let docs = rule_docs();
    let mut current_category = String::new();
    for doc in docs {
        if doc.category != current_category {
            if !current_category.is_empty() {
                text.push('\n');
            }
            let _ = writeln!(text, "### {}", doc.category);
            current_category.clone_from(&doc.category);
        }
        let caveat = if rule_limitation(doc.name).is_some() {
            " ⚠"
        } else {
            ""
        };
        let _ = writeln!(
            text,
            "- `{}` ({}){} — {}",
            doc.name,
            doc.severity.to_lowercase(),
            caveat,
            doc.description
                .split(". ")
                .next()
                .unwrap_or(doc.description)
        );
    }

    text.push_str("\n### Known heuristic limitations\n\n");
    for doc in docs {
        if let Some(limitation) = rule_limitation(doc.name) {
            let _ = writeln!(text, "- `{}` — {limitation}", doc.name);
        }
    }

    text.push_str("\n## Clippy Lints (55+ with category/severity overrides) — type-aware\n\n");
    text.push_str(
        "rust-doctor runs `cargo clippy` with pedantic, nursery, and cargo lint groups. \
         Unlike the custom rules above, clippy resolves types against the compiler, so \
         its findings are more authoritative.\n",
    );
    text.push_str("55+ lints have explicit category and severity overrides across:\n");
    text.push_str(
        "Error Handling, Performance, Security, Correctness, Architecture, Cargo, Async, Style\n",
    );
    text.push_str("\nUse `explain_rule` with a clippy lint name for details.\n");

    text.push_str("\n## External Tools\n\n");
    text.push_str("- **cargo-audit** — Vulnerability scanning for dependencies (install: `cargo install cargo-audit`)\n");
    text.push_str("- **cargo-machete** — Unused dependency detection (install: `cargo install cargo-machete`)\n");

    text
}
