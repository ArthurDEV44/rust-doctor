use crate::diagnostics::{Category, Diagnostic, Severity};
use crate::rules::CustomRule;
use std::path::Path;
use syn::spanned::Spanned;
use syn::visit::Visit;

// ─── Rule 1: excessive-clone ────────────────────────────────────────────────

/// Flags `.clone()` calls as a review prompt.
/// Without type information, this rule cannot distinguish Copy-type clones
/// from necessary non-Copy clones. It serves as a heuristic — use
/// `clippy::clone_on_copy` (in the clippy pass) for precise Copy-type detection.
/// Suppress with `// rust-doctor-disable-next-line excessive-clone` for reviewed clones.
pub struct ExcessiveClone;

impl CustomRule for ExcessiveClone {
    fn name(&self) -> &'static str {
        "excessive-clone"
    }
    fn category(&self) -> Category {
        Category::Performance
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn check_file(&self, syntax: &syn::File, path: &Path) -> Vec<Diagnostic> {
        let mut visitor = CloneVisitor {
            path,
            diagnostics: Vec::new(),
        };
        visitor.visit_file(syntax);
        visitor.diagnostics
    }
}

struct CloneVisitor<'a> {
    path: &'a Path,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for CloneVisitor<'_> {
    fn visit_expr_method_call(&mut self, i: &'ast syn::ExprMethodCall) {
        if i.method == "clone" && i.args.is_empty() {
            let span = i.method.span();
            self.diagnostics.push(Diagnostic {
                file_path: self.path.to_path_buf(),
                rule: "excessive-clone".to_string(),
                category: Category::Performance,
                severity: Severity::Warning,
                message: "Potentially unnecessary .clone() call".to_string(),
                help: Some(
                    "If the type implements Copy, remove .clone(). Otherwise, consider borrowing or using Cow<T>".to_string(),
                ),
                line: Some(span.start().line as u32),
                column: Some(span.start().column as u32 + 1),
            });
        }
        syn::visit::visit_expr_method_call(self, i);
    }
}

// ─── Rule 2: string-from-literal ────────────────────────────────────────────

/// Flags `String::from("literal")` and `"literal".to_string()` patterns.
pub struct StringFromLiteral;

impl CustomRule for StringFromLiteral {
    fn name(&self) -> &'static str {
        "string-from-literal"
    }
    fn category(&self) -> Category {
        Category::Performance
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn check_file(&self, syntax: &syn::File, path: &Path) -> Vec<Diagnostic> {
        let mut visitor = StringLiteralVisitor {
            path,
            diagnostics: Vec::new(),
        };
        visitor.visit_file(syntax);
        visitor.diagnostics
    }
}

struct StringLiteralVisitor<'a> {
    path: &'a Path,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for StringLiteralVisitor<'_> {
    fn visit_expr_method_call(&mut self, i: &'ast syn::ExprMethodCall) {
        // "literal".to_string()
        if i.method == "to_string"
            && i.args.is_empty()
            && matches!(i.receiver.as_ref(), syn::Expr::Lit(lit) if matches!(lit.lit, syn::Lit::Str(_)))
        {
            let span = i.method.span();
            self.diagnostics.push(Diagnostic {
                file_path: self.path.to_path_buf(),
                rule: "string-from-literal".to_string(),
                category: Category::Performance,
                severity: Severity::Warning,
                message: r#""literal".to_string() creates a heap allocation"#.to_string(),
                help: Some(
                    "Use &str directly when possible, or use a const/static for reuse".to_string(),
                ),
                line: Some(span.start().line as u32),
                column: Some(span.start().column as u32 + 1),
            });
        }
        syn::visit::visit_expr_method_call(self, i);
    }

    fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
        // String::from("literal")
        if let syn::Expr::Path(func_path) = i.func.as_ref() {
            let segments: Vec<String> = func_path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();
            if segments == ["String", "from"]
                && i.args.len() == 1
                && matches!(&i.args[0], syn::Expr::Lit(lit) if matches!(lit.lit, syn::Lit::Str(_)))
            {
                let span = func_path.path.span();
                self.diagnostics.push(Diagnostic {
                    file_path: self.path.to_path_buf(),
                    rule: "string-from-literal".to_string(),
                    category: Category::Performance,
                    severity: Severity::Warning,
                    message: r#"String::from("literal") creates a heap allocation"#.to_string(),
                    help: Some(
                        "Use &str directly when possible, or use a const/static for reuse"
                            .to_string(),
                    ),
                    line: Some(span.start().line as u32),
                    column: Some(span.start().column as u32 + 1),
                });
            }
        }
        syn::visit::visit_expr_call(self, i);
    }
}

// ─── Rule 3: collect-then-iterate ───────────────────────────────────────────

/// Flags `.collect::<Vec<_>>()` immediately followed by `.iter()` or `.into_iter()`.
pub struct CollectThenIterate;

impl CustomRule for CollectThenIterate {
    fn name(&self) -> &'static str {
        "collect-then-iterate"
    }
    fn category(&self) -> Category {
        Category::Performance
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn check_file(&self, syntax: &syn::File, path: &Path) -> Vec<Diagnostic> {
        let mut visitor = CollectIterVisitor {
            path,
            diagnostics: Vec::new(),
        };
        visitor.visit_file(syntax);
        visitor.diagnostics
    }
}

struct CollectIterVisitor<'a> {
    path: &'a Path,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for CollectIterVisitor<'_> {
    fn visit_expr_method_call(&mut self, i: &'ast syn::ExprMethodCall) {
        // Check if this is .iter() or .into_iter() called on a .collect() result
        let method_name = i.method.to_string();
        if (method_name == "iter" || method_name == "into_iter")
            && i.args.is_empty()
            && is_collect_call(&i.receiver)
        {
            let span = i.method.span();
            self.diagnostics.push(Diagnostic {
                file_path: self.path.to_path_buf(),
                rule: "collect-then-iterate".to_string(),
                category: Category::Performance,
                severity: Severity::Warning,
                message: ".collect() immediately followed by .iter() — unnecessary allocation"
                    .to_string(),
                help: Some(
                    "Remove the .collect() call and continue chaining iterator adaptors directly"
                        .to_string(),
                ),
                line: Some(span.start().line as u32),
                column: Some(span.start().column as u32 + 1),
            });
        }
        syn::visit::visit_expr_method_call(self, i);
    }
}

fn is_collect_call(expr: &syn::Expr) -> bool {
    if let syn::Expr::MethodCall(call) = expr {
        call.method == "collect"
    } else {
        false
    }
}

// ─── Rule 4: large-enum-variant ─────────────────────────────────────────────

/// Flags enums where the largest variant has >3x more fields than the smallest.
pub struct LargeEnumVariant;

impl CustomRule for LargeEnumVariant {
    fn name(&self) -> &'static str {
        "large-enum-variant"
    }
    fn category(&self) -> Category {
        Category::Performance
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn check_file(&self, syntax: &syn::File, path: &Path) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for item in &syntax.items {
            if let syn::Item::Enum(e) = item {
                let field_counts: Vec<usize> = e
                    .variants
                    .iter()
                    .map(|v| match &v.fields {
                        syn::Fields::Named(f) => f.named.len(),
                        syn::Fields::Unnamed(f) => f.unnamed.len(),
                        syn::Fields::Unit => 0,
                    })
                    .collect();

                if field_counts.len() < 2 {
                    continue;
                }

                let min = field_counts.iter().copied().min().unwrap_or(0);
                let max = field_counts.iter().copied().max().unwrap_or(0);

                // Only flag if the largest variant has >3x the fields of the smallest non-zero,
                // or if the largest has >5 fields and the smallest is 0
                let threshold_exceeded = if min > 0 { max > min * 3 } else { max > 5 };

                if threshold_exceeded {
                    let span = e.ident.span();
                    diagnostics.push(Diagnostic {
                        file_path: path.to_path_buf(),
                        rule: "large-enum-variant".to_string(),
                        category: Category::Performance,
                        severity: Severity::Warning,
                        message: format!(
                            "Enum `{}` has variant size disparity (min {} fields, max {} fields)",
                            e.ident, min, max
                        ),
                        help: Some(
                            "Consider boxing the large variant's fields to reduce enum size"
                                .to_string(),
                        ),
                        line: Some(span.start().line as u32),
                        column: Some(span.start().column as u32 + 1),
                    });
                }
            }
        }

        diagnostics
    }
}

// ─── Rule 5: unnecessary-allocation ─────────────────────────────────────────

/// Flags `Vec::new()` inside loop bodies without pre-allocation hint.
pub struct UnnecessaryAllocation;

impl CustomRule for UnnecessaryAllocation {
    fn name(&self) -> &'static str {
        "unnecessary-allocation"
    }
    fn category(&self) -> Category {
        Category::Performance
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn check_file(&self, syntax: &syn::File, path: &Path) -> Vec<Diagnostic> {
        let mut visitor = AllocVisitor {
            path,
            diagnostics: Vec::new(),
            in_loop: false,
        };
        visitor.visit_file(syntax);
        visitor.diagnostics
    }
}

struct AllocVisitor<'a> {
    path: &'a Path,
    diagnostics: Vec<Diagnostic>,
    in_loop: bool,
}

impl<'ast> Visit<'ast> for AllocVisitor<'_> {
    fn visit_expr_for_loop(&mut self, i: &'ast syn::ExprForLoop) {
        let was_in_loop = self.in_loop;
        self.in_loop = true;
        syn::visit::visit_expr_for_loop(self, i);
        self.in_loop = was_in_loop;
    }

    fn visit_expr_while(&mut self, i: &'ast syn::ExprWhile) {
        let was_in_loop = self.in_loop;
        self.in_loop = true;
        syn::visit::visit_expr_while(self, i);
        self.in_loop = was_in_loop;
    }

    fn visit_expr_loop(&mut self, i: &'ast syn::ExprLoop) {
        let was_in_loop = self.in_loop;
        self.in_loop = true;
        syn::visit::visit_expr_loop(self, i);
        self.in_loop = was_in_loop;
    }

    fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
        if self.in_loop
            && let syn::Expr::Path(func_path) = i.func.as_ref()
        {
            let segments: Vec<String> = func_path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();
            if (segments == ["Vec", "new"] || segments == ["String", "new"]) && i.args.is_empty() {
                let span = func_path.path.span();
                self.diagnostics.push(Diagnostic {
                    file_path: self.path.to_path_buf(),
                    rule: "unnecessary-allocation".to_string(),
                    category: Category::Performance,
                    severity: Severity::Warning,
                    message: format!(
                        "{}::new() inside a loop — allocates on every iteration",
                        segments.join("::")
                    ),
                    help: Some(
                        "Move the allocation outside the loop and use .clear() to reuse, or use Vec::with_capacity()"
                            .to_string(),
                    ),
                    line: Some(span.start().line as u32),
                    column: Some(span.start().column as u32 + 1),
                });
            }
        }
        syn::visit::visit_expr_call(self, i);
    }
}

// ─── Convenience ────────────────────────────────────────────────────────────

/// Returns all performance rules.
pub fn all_rules() -> Vec<Box<dyn CustomRule>> {
    vec![
        Box::new(ExcessiveClone),
        Box::new(StringFromLiteral),
        Box::new(CollectThenIterate),
        Box::new(LargeEnumVariant),
        Box::new(UnnecessaryAllocation),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(rule: &dyn CustomRule, code: &str) -> Vec<Diagnostic> {
        let syntax = syn::parse_file(code).expect("test code should parse");
        rule.check_file(&syntax, Path::new("test.rs"))
    }

    // --- excessive-clone ---

    #[test]
    fn test_clone_detected() {
        let diags = check(
            &ExcessiveClone,
            r#"
            fn main() {
                let x = vec![1, 2, 3];
                let y = x.clone();
            }
            "#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "excessive-clone");
    }

    #[test]
    fn test_clone_no_false_positive_on_other_methods() {
        let diags = check(
            &ExcessiveClone,
            r#"
            fn main() {
                let x = vec![1, 2, 3];
                let y = x.len();
            }
            "#,
        );
        assert!(diags.is_empty());
    }

    // --- string-from-literal ---

    #[test]
    fn test_string_from_literal_detected() {
        let diags = check(
            &StringFromLiteral,
            r#"
            fn main() {
                let s = String::from("hello");
            }
            "#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "string-from-literal");
    }

    #[test]
    fn test_to_string_on_literal_detected() {
        let diags = check(
            &StringFromLiteral,
            r#"
            fn main() {
                let s = "hello".to_string();
            }
            "#,
        );
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_string_from_variable_not_flagged() {
        let diags = check(
            &StringFromLiteral,
            r#"
            fn main() {
                let x = "hello";
                let s = String::from(x);
            }
            "#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_to_string_on_variable_not_flagged() {
        let diags = check(
            &StringFromLiteral,
            r#"
            fn main() {
                let x = 42;
                let s = x.to_string();
            }
            "#,
        );
        assert!(diags.is_empty());
    }

    // --- collect-then-iterate ---

    #[test]
    fn test_collect_then_iter_detected() {
        let diags = check(
            &CollectThenIterate,
            r#"
            fn main() {
                let v: Vec<i32> = (0..10).collect::<Vec<_>>().iter().map(|x| x + 1).collect();
            }
            "#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "collect-then-iterate");
    }

    #[test]
    fn test_collect_without_iter_not_flagged() {
        let diags = check(
            &CollectThenIterate,
            r#"
            fn main() {
                let v: Vec<i32> = (0..10).collect();
            }
            "#,
        );
        assert!(diags.is_empty());
    }

    // --- large-enum-variant ---

    #[test]
    fn test_large_enum_variant_detected() {
        let diags = check(
            &LargeEnumVariant,
            r#"
            enum Message {
                Quit,
                Data {
                    a: i32, b: i32, c: i32, d: i32,
                    e: i32, f: i32, g: i32, h: i32,
                },
            }
            "#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "large-enum-variant");
    }

    #[test]
    fn test_balanced_enum_not_flagged() {
        let diags = check(
            &LargeEnumVariant,
            r#"
            enum Color {
                Red(u8),
                Green(u8),
                Blue(u8),
            }
            "#,
        );
        assert!(diags.is_empty());
    }

    // --- unnecessary-allocation ---

    #[test]
    fn test_vec_new_in_loop_detected() {
        let diags = check(
            &UnnecessaryAllocation,
            r#"
            fn main() {
                for _ in 0..10 {
                    let v = Vec::new();
                }
            }
            "#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "unnecessary-allocation");
    }

    #[test]
    fn test_vec_new_outside_loop_not_flagged() {
        let diags = check(
            &UnnecessaryAllocation,
            r#"
            fn main() {
                let v = Vec::new();
            }
            "#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_string_new_in_while_loop_detected() {
        let diags = check(
            &UnnecessaryAllocation,
            r#"
            fn main() {
                while true {
                    let s = String::new();
                }
            }
            "#,
        );
        assert_eq!(diags.len(), 1);
    }

    // --- all_rules ---

    #[test]
    fn test_all_rules_returns_5() {
        assert_eq!(all_rules().len(), 5);
    }
}
