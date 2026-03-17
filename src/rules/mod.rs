pub mod async_rules;
pub mod error_handling;
pub mod framework;
pub mod performance;
pub mod security;

use crate::diagnostics::{Category, Diagnostic, Severity};
use crate::scanner::{self, AnalysisPass};
use globset::GlobSet;
use rayon::prelude::*;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

// ─── Shared helpers for test-code detection ─────────────────────────────────

/// Check if an attribute list contains `#[test]`.
pub(crate) fn has_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("test"))
}

/// Check if an attribute list contains `#[cfg(test)]`.
pub(crate) fn has_cfg_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }
        attr.parse_args::<syn::Ident>()
            .is_ok_and(|ident| ident == "test")
    })
}

/// Trait for custom AST-based rules that clippy doesn't cover.
///
/// Rules must be `Send + Sync` for parallel file processing.
#[allow(dead_code)] // methods used by implementors in sub-modules
pub trait CustomRule: Send + Sync {
    /// Unique rule identifier (e.g. "unwrap-in-production").
    fn name(&self) -> &str;

    /// Category this rule belongs to.
    fn category(&self) -> Category;

    /// Default severity for findings from this rule.
    fn severity(&self) -> Severity;

    /// Check a parsed Rust file and return diagnostics.
    fn check_file(&self, syntax: &syn::File, path: &Path) -> Vec<Diagnostic>;

    /// Helper to construct a `Diagnostic` using this rule's metadata.
    fn diagnostic(
        &self,
        path: &Path,
        message: String,
        help: Option<String>,
        line: Option<u32>,
        column: Option<u32>,
    ) -> Diagnostic {
        Diagnostic {
            file_path: path.to_path_buf(),
            rule: self.name().to_string(),
            category: self.category(),
            severity: self.severity(),
            message,
            help,
            line,
            column,
        }
    }
}

/// The rule engine: runs custom rules against all `.rs` files in parallel.
pub struct RuleEngine {
    rules: Vec<Box<dyn CustomRule>>,
}

impl RuleEngine {
    /// Create a new rule engine with the given rules.
    pub fn new(rules: Vec<Box<dyn CustomRule>>) -> Self {
        Self { rules }
    }

    /// Scan all `.rs` files under `project_root/src/`, skipping files
    /// matching the ignore patterns. Returns collected diagnostics.
    pub fn scan(
        &self,
        project_root: &Path,
        ignore_files: &[String],
    ) -> Vec<Diagnostic> {
        if self.rules.is_empty() {
            return vec![];
        }

        // Collect .rs files
        let src_dir = project_root.join("src");
        if !src_dir.is_dir() {
            return vec![];
        }

        let files = scanner::collect_rs_files(&src_dir);
        if files.is_empty() {
            return vec![];
        }

        // Build ignore glob set
        let ignore_set = build_ignore_set(ignore_files);

        // Process files in parallel with rayon
        let diagnostics: Vec<Diagnostic> = files
            .par_iter()
            .flat_map(|file_path| {
                // Make path relative to project root for matching and display
                let rel_path = file_path.strip_prefix(project_root).unwrap_or(file_path);

                // Check ignore patterns
                if let Ok(ref set) = ignore_set
                    && set.is_match(rel_path)
                {
                    return vec![];
                }

                // Read and parse file
                let content = match std::fs::read_to_string(file_path) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Warning: could not read '{}': {e}", file_path.display());
                        return vec![];
                    }
                };

                let syntax = match syn::parse_file(&content) {
                    Ok(ast) => ast,
                    Err(e) => {
                        eprintln!("Warning: parse error in '{}': {e}", rel_path.display());
                        return vec![];
                    }
                };

                // Run all rules on this file, catching panics
                self.rules
                    .iter()
                    .flat_map(|rule| run_rule_safely(rule.as_ref(), &syntax, rel_path))
                    .collect::<Vec<_>>()
            })
            .collect();

        diagnostics
    }
}

/// Run a single rule with panic isolation.
fn run_rule_safely(rule: &dyn CustomRule, syntax: &syn::File, path: &Path) -> Vec<Diagnostic> {
    let result = panic::catch_unwind(AssertUnwindSafe(|| rule.check_file(syntax, path)));

    match result {
        Ok(diagnostics) => diagnostics,
        Err(payload) => {
            let msg = if let Some(s) = payload.downcast_ref::<&'static str>() {
                (*s).to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "<non-string panic>".to_string()
            };
            eprintln!(
                "Warning: rule '{}' panicked on '{}': {msg}",
                rule.name(),
                path.display()
            );
            vec![]
        }
    }
}


fn build_ignore_set(patterns: &[String]) -> Result<GlobSet, globset::Error> {
    scanner::build_glob_set(patterns)
}

/// Analysis pass that wraps the rule engine for the scan orchestrator.
pub struct RuleEnginePass {
    engine: RuleEngine,
    ignore_files: Vec<String>,
}

impl RuleEnginePass {
    pub fn new(rules: Vec<Box<dyn CustomRule>>, ignore_files: Vec<String>) -> Self {
        Self {
            engine: RuleEngine::new(rules),
            ignore_files,
        }
    }
}

impl AnalysisPass for RuleEnginePass {
    fn name(&self) -> &'static str {
        "custom rules"
    }

    fn run(&self, project_root: &Path) -> Result<Vec<Diagnostic>, crate::error::PassError> {
        Ok(self.engine.scan(project_root, &self.ignore_files))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    // --- Test rule implementations ---

    struct CountFnRule;

    impl CustomRule for CountFnRule {
        fn name(&self) -> &str {
            "count-fn"
        }
        fn category(&self) -> Category {
            Category::Architecture
        }
        fn severity(&self) -> Severity {
            Severity::Warning
        }
        fn check_file(&self, syntax: &syn::File, path: &Path) -> Vec<Diagnostic> {
            let fn_count = syntax
                .items
                .iter()
                .filter(|item| matches!(item, syn::Item::Fn(_)))
                .count();
            if fn_count > 10 {
                vec![Diagnostic {
                    file_path: path.to_path_buf(),
                    rule: self.name().to_string(),
                    category: self.category(),
                    severity: self.severity(),
                    message: format!("File has {fn_count} functions (threshold: 10)"),
                    help: None,
                    line: None,
                    column: None,
                }]
            } else {
                vec![]
            }
        }
    }

    struct PanickingRule;

    impl CustomRule for PanickingRule {
        fn name(&self) -> &str {
            "panicking-rule"
        }
        fn category(&self) -> Category {
            Category::Correctness
        }
        fn severity(&self) -> Severity {
            Severity::Error
        }
        fn check_file(&self, _syntax: &syn::File, _path: &Path) -> Vec<Diagnostic> {
            panic!("intentional test panic");
        }
    }

    struct AlwaysWarnsRule;

    impl CustomRule for AlwaysWarnsRule {
        fn name(&self) -> &str {
            "always-warns"
        }
        fn category(&self) -> Category {
            Category::Style
        }
        fn severity(&self) -> Severity {
            Severity::Warning
        }
        fn check_file(&self, _syntax: &syn::File, path: &Path) -> Vec<Diagnostic> {
            vec![Diagnostic {
                file_path: path.to_path_buf(),
                rule: self.name().to_string(),
                category: self.category(),
                severity: self.severity(),
                message: "Test warning".to_string(),
                help: None,
                line: None,
                column: None,
            }]
        }
    }

    // --- Tests ---

    fn make_temp_project(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rust-doctor-test-{name}"));
        let src_dir = dir.join("src");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&src_dir).unwrap();
        for (filename, content) in files {
            let path = src_dir.join(filename);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            let mut f = std::fs::File::create(&path).unwrap();
            write!(f, "{content}").unwrap();
        }
        dir
    }

    #[test]
    fn test_rule_engine_with_no_rules() {
        let engine = RuleEngine::new(vec![]);
        let dir = make_temp_project("no-rules", &[("main.rs", "fn main() {}")]);
        let diags = engine.scan(&dir, &[]);
        assert!(diags.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_rule_engine_no_src_dir() {
        let dir = std::env::temp_dir().join("rust-doctor-test-no-src");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let engine = RuleEngine::new(vec![Box::new(AlwaysWarnsRule)]);
        let diags = engine.scan(&dir, &[]);
        assert!(diags.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_rule_engine_runs_rules_on_files() {
        let dir = make_temp_project("runs-rules", &[("main.rs", "fn main() {}")]);
        let engine = RuleEngine::new(vec![Box::new(AlwaysWarnsRule)]);
        let diags = engine.scan(&dir, &[]);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "always-warns");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_rule_engine_multiple_files() {
        let dir = make_temp_project(
            "multi-files",
            &[
                ("main.rs", "fn main() {}"),
                ("lib.rs", "pub fn hello() {}"),
                ("utils.rs", "pub fn util() {}"),
            ],
        );
        let engine = RuleEngine::new(vec![Box::new(AlwaysWarnsRule)]);
        let diags = engine.scan(&dir, &[]);
        assert_eq!(diags.len(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_rule_engine_catches_panics() {
        let dir = make_temp_project("panic-catch", &[("main.rs", "fn main() {}")]);
        let engine = RuleEngine::new(vec![Box::new(PanickingRule), Box::new(AlwaysWarnsRule)]);
        let diags = engine.scan(&dir, &[]);
        // PanickingRule panicked and was caught; AlwaysWarnsRule still ran
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "always-warns");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_rule_engine_handles_parse_errors() {
        let dir = make_temp_project("parse-error", &[("main.rs", "this is not valid rust {{{{")]);
        let engine = RuleEngine::new(vec![Box::new(AlwaysWarnsRule)]);
        let diags = engine.scan(&dir, &[]);
        // File couldn't be parsed, so no diagnostics
        assert!(diags.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_rule_engine_skips_ignored_files() {
        let dir = make_temp_project(
            "ignore-files",
            &[
                ("main.rs", "fn main() {}"),
                ("generated/output.rs", "pub fn gen() {}"),
            ],
        );
        let engine = RuleEngine::new(vec![Box::new(AlwaysWarnsRule)]);
        let ignore = vec!["src/generated/**".to_string()];
        let diags = engine.scan(&dir, &ignore);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].file_path.to_string_lossy().contains("main.rs"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_rule_engine_on_self() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let engine = RuleEngine::new(vec![Box::new(CountFnRule)]);
        let _diags = engine.scan(manifest_dir, &[]);
        // CountFnRule only fires if a file has >10 functions, so may or may not find issues
    }

    #[test]
    fn test_collect_rs_files() {
        let dir = make_temp_project(
            "collect-rs",
            &[("main.rs", ""), ("lib.rs", ""), ("sub/mod.rs", "")],
        );
        let files = scanner::collect_rs_files(&dir.join("src"));
        assert_eq!(files.len(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_rule_engine_pass() {
        let dir = make_temp_project("pass-test", &[("main.rs", "fn main() {}")]);
        let pass = RuleEnginePass::new(vec![Box::new(AlwaysWarnsRule)], vec![]);
        let result = pass.run(&dir);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
