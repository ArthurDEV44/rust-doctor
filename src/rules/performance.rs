use crate::diagnostics::{Category, Diagnostic, Severity};
use crate::rules::{CustomRule, has_cfg_test, has_test_attr};
use std::path::Path;
use syn::spanned::Spanned;
use syn::visit::Visit;

// ─── Rule 1: excessive-clone ────────────────────────────────────────────────

/// Flags excessive `.clone()` calls as a review prompt.
/// Without type information, this rule cannot distinguish Copy-type clones
/// from necessary non-Copy clones. It uses heuristics to reduce false positives:
///
/// - Ignores clones inside test functions and `#[cfg(test)]` modules
/// - Only reports when a file has 3+ clone calls (isolated clones are usually intentional)
/// - Uses `clippy::clone_on_copy` (in the clippy pass) for precise Copy-type detection
///
/// Suppress with `// rust-doctor-disable-next-line excessive-clone` for reviewed clones.
pub struct ExcessiveClone;

/// Minimum number of clone calls in a file before reporting.
const CLONE_THRESHOLD: usize = 3;

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
    fn description(&self) -> &'static str {
        "Flags `.clone()` calls that may indicate unnecessary heap allocations. Each clone copies the entire value, which is expensive for `String`, `Vec`, and other heap-allocated types."
    }
    fn fix_hint(&self) -> &'static str {
        "Use references (`&T`) or `Cow<T>` instead of cloning. Consider restructuring ownership to avoid the clone."
    }
    fn check_file(&self, syntax: &syn::File, path: &Path) -> Vec<Diagnostic> {
        let mut visitor = CloneVisitor {
            path,
            diagnostics: Vec::new(),
            in_test: false,
            loop_depth: 0,
        };
        visitor.visit_file(syntax);
        // Only report if the file has enough clones to suggest a pattern issue
        if visitor.diagnostics.len() >= CLONE_THRESHOLD {
            visitor.diagnostics
        } else {
            Vec::new()
        }
    }
}

struct CloneVisitor<'a> {
    path: &'a Path,
    diagnostics: Vec<Diagnostic>,
    in_test: bool,
    /// Nesting depth of loops (for/while/loop). Clones inside loops are hot-path.
    loop_depth: u32,
}

impl<'ast> Visit<'ast> for CloneVisitor<'_> {
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        let was_in_test = self.in_test;
        if has_test_attr(&i.attrs) {
            self.in_test = true;
        }
        syn::visit::visit_item_fn(self, i);
        self.in_test = was_in_test;
    }

    fn visit_item_mod(&mut self, i: &'ast syn::ItemMod) {
        if has_cfg_test(&i.attrs) {
            return; // Skip entire #[cfg(test)] modules
        }
        syn::visit::visit_item_mod(self, i);
    }

    fn visit_expr_for_loop(&mut self, i: &'ast syn::ExprForLoop) {
        self.loop_depth += 1;
        syn::visit::visit_expr_for_loop(self, i);
        self.loop_depth -= 1;
    }

    fn visit_expr_while(&mut self, i: &'ast syn::ExprWhile) {
        self.loop_depth += 1;
        syn::visit::visit_expr_while(self, i);
        self.loop_depth -= 1;
    }

    fn visit_expr_loop(&mut self, i: &'ast syn::ExprLoop) {
        self.loop_depth += 1;
        syn::visit::visit_expr_loop(self, i);
        self.loop_depth -= 1;
    }

    fn visit_expr_method_call(&mut self, i: &'ast syn::ExprMethodCall) {
        if !self.in_test && i.method == "clone" && i.args.is_empty() {
            let span = i.method.span();
            let in_loop = self.loop_depth > 0;
            let (severity, message) = if in_loop {
                (
                    Severity::Warning,
                    "`.clone()` inside a loop — may cause repeated heap allocations".to_string(),
                )
            } else {
                (
                    Severity::Info,
                    "`.clone()` in non-loop context (cold path)".to_string(),
                )
            };
            self.diagnostics.push(Diagnostic {
                file_path: self.path.to_path_buf(),
                rule: "excessive-clone".to_string(),
                category: Category::Performance,
                severity,
                message,
                help: Some(
                    "If the type implements Copy, remove .clone(). Otherwise, consider borrowing or using Cow<T>".to_string(),
                ),
                line: Some(span.start().line as u32),
                column: Some(span.start().column as u32 + 1),
                fix: None,
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
        Severity::Info
    }
    fn default_enabled(&self) -> bool {
        false
    }
    fn description(&self) -> &'static str {
        "Flags `String::from(\"literal\")` and `\"literal\".to_string()`. These allocate on the heap when a `&str` reference might suffice."
    }
    fn fix_hint(&self) -> &'static str {
        "Use `&str` for function parameters and constants. Owned `String` is correct for struct fields, HashMap keys, error messages, and APIs requiring ownership."
    }
    fn check_file(&self, syntax: &syn::File, path: &Path) -> Vec<Diagnostic> {
        let mut visitor = StringLiteralVisitor {
            path,
            diagnostics: Vec::new(),
            in_test: false,
        };
        visitor.visit_file(syntax);
        visitor.diagnostics
    }
}

struct StringLiteralVisitor<'a> {
    path: &'a Path,
    diagnostics: Vec<Diagnostic>,
    in_test: bool,
}

impl<'ast> Visit<'ast> for StringLiteralVisitor<'_> {
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        let was_in_test = self.in_test;
        if has_test_attr(&i.attrs) {
            self.in_test = true;
        }
        syn::visit::visit_item_fn(self, i);
        self.in_test = was_in_test;
    }

    fn visit_item_mod(&mut self, i: &'ast syn::ItemMod) {
        if has_cfg_test(&i.attrs) {
            return; // Skip entire #[cfg(test)] modules
        }
        syn::visit::visit_item_mod(self, i);
    }

    fn visit_expr_method_call(&mut self, i: &'ast syn::ExprMethodCall) {
        // "literal".to_string()
        if !self.in_test
            && i.method == "to_string"
            && i.args.is_empty()
            && matches!(i.receiver.as_ref(), syn::Expr::Lit(lit) if matches!(lit.lit, syn::Lit::Str(_)))
        {
            let span = i.method.span();
            self.diagnostics.push(Diagnostic {
                file_path: self.path.to_path_buf(),
                rule: "string-from-literal".to_string(),
                category: Category::Performance,
                severity: Severity::Info,
                message:
                    r#""literal".to_string() allocates — consider &str if ownership is not needed"#
                        .to_string(),
                help: Some(
                    "Acceptable for struct fields, HashMap keys, and APIs requiring String. \
                     Use &str for function parameters and constants."
                        .to_string(),
                ),
                line: Some(span.start().line as u32),
                column: Some(span.start().column as u32 + 1),
                fix: None,
            });
        }
        syn::visit::visit_expr_method_call(self, i);
    }

    fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
        // String::from("literal")
        if !self.in_test
            && let syn::Expr::Path(func_path) = i.func.as_ref()
        {
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
                    severity: Severity::Info,
                    message: r#"String::from("literal") allocates — consider &str if ownership is not needed"#.to_string(),
                    help: Some(
                        "Acceptable for struct fields, HashMap keys, and APIs requiring String. \
                         Use &str for function parameters and constants."
                            .to_string(),
                    ),
                    line: Some(span.start().line as u32),
                    column: Some(span.start().column as u32 + 1),
                    fix: None,
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
    fn description(&self) -> &'static str {
        "Flags `.collect::<Vec<_>>()` immediately followed by `.iter()`. This allocates a temporary vector unnecessarily since the original iterator could be used directly."
    }
    fn fix_hint(&self) -> &'static str {
        "Remove the `.collect()` and chain the iterator operations directly."
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
                fix: None,
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
    fn description(&self) -> &'static str {
        "Flags enums where variants have significantly different sizes (>3x field count disparity). The enum's size equals its largest variant, wasting memory for smaller variants."
    }
    fn fix_hint(&self) -> &'static str {
        "Box the large variant's data: `LargeVariant(Box<LargeData>)`."
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
                        fix: None,
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
    fn description(&self) -> &'static str {
        "Flags `Vec::new()` or `String::new()` inside loops. Each iteration allocates a new buffer, which is expensive."
    }
    fn fix_hint(&self) -> &'static str {
        "Move the allocation outside the loop and use `.clear()` to reuse it."
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
                    fix: None,
                });
            }
        }
        syn::visit::visit_expr_call(self, i);
    }
}

// ─── Rule 6: high-cyclomatic-complexity ─────────────────────────────────────

/// Flags functions with high cyclomatic complexity (> threshold).
/// Cyclomatic complexity measures the number of independent paths through a function.
/// High complexity makes code harder to test, understand, and maintain.
pub struct HighCyclomaticComplexity;

/// Default complexity threshold above which a function is flagged.
const COMPLEXITY_THRESHOLD: u32 = 15;

impl CustomRule for HighCyclomaticComplexity {
    fn name(&self) -> &'static str {
        "high-cyclomatic-complexity"
    }
    fn category(&self) -> Category {
        Category::Architecture
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn description(&self) -> &'static str {
        "Function has high cyclomatic complexity — consider refactoring into smaller functions"
    }
    fn fix_hint(&self) -> &'static str {
        "Extract complex branches into helper functions, use early returns, simplify match arms"
    }
    fn check_file(&self, syntax: &syn::File, path: &Path) -> Vec<Diagnostic> {
        let mut visitor = ComplexityVisitor {
            path,
            diagnostics: Vec::new(),
            in_test: false,
        };
        visitor.visit_file(syntax);
        visitor.diagnostics
    }
}

struct ComplexityVisitor<'a> {
    path: &'a Path,
    diagnostics: Vec<Diagnostic>,
    in_test: bool,
}

impl ComplexityVisitor<'_> {
    fn check_function_complexity(
        &mut self,
        fn_name: &str,
        block: &syn::Block,
        span: proc_macro2::Span,
    ) {
        if self.in_test {
            return;
        }
        let mut counter = ComplexityCounter { complexity: 1 };
        counter.visit_block(block);
        if counter.complexity > COMPLEXITY_THRESHOLD {
            self.diagnostics.push(Diagnostic {
                file_path: self.path.to_path_buf(),
                rule: "high-cyclomatic-complexity".to_string(),
                category: Category::Architecture,
                severity: Severity::Warning,
                message: format!(
                    "Function `{}` has cyclomatic complexity of {} (threshold: {})",
                    fn_name, counter.complexity, COMPLEXITY_THRESHOLD
                ),
                help: Some(
                    "Extract complex branches into helper functions, use early returns, simplify match arms"
                        .to_string(),
                ),
                line: Some(span.start().line as u32),
                column: Some(span.start().column as u32 + 1),
                fix: None,
            });
        }
    }
}

impl<'ast> Visit<'ast> for ComplexityVisitor<'_> {
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        let was_in_test = self.in_test;
        if has_test_attr(&i.attrs) {
            self.in_test = true;
        }
        let span = i.sig.ident.span();
        let fn_name = i.sig.ident.to_string();
        self.check_function_complexity(&fn_name, &i.block, span);
        // Continue visiting nested items (but not the block again — we already counted it)
        for item in &i.block.stmts {
            if let syn::Stmt::Item(item) = item {
                syn::visit::visit_item(self, item);
            }
        }
        self.in_test = was_in_test;
    }

    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        let was_in_test = self.in_test;
        if has_test_attr(&i.attrs) {
            self.in_test = true;
        }
        let span = i.sig.ident.span();
        let fn_name = i.sig.ident.to_string();
        self.check_function_complexity(&fn_name, &i.block, span);
        // Continue visiting nested items
        for item in &i.block.stmts {
            if let syn::Stmt::Item(item) = item {
                syn::visit::visit_item(self, item);
            }
        }
        self.in_test = was_in_test;
    }

    fn visit_item_mod(&mut self, i: &'ast syn::ItemMod) {
        if has_cfg_test(&i.attrs) {
            return;
        }
        syn::visit::visit_item_mod(self, i);
    }
}

/// Inner counter that walks a single function body and tallies complexity increments.
struct ComplexityCounter {
    complexity: u32,
}

impl<'ast> Visit<'ast> for ComplexityCounter {
    fn visit_expr_if(&mut self, i: &'ast syn::ExprIf) {
        self.complexity += 1;
        syn::visit::visit_expr_if(self, i);
    }

    fn visit_expr_match(&mut self, i: &'ast syn::ExprMatch) {
        // Each arm adds a path; subtract 1 because one arm is the base path
        let arms = i.arms.len() as u32;
        if arms > 1 {
            self.complexity += arms - 1;
        }
        syn::visit::visit_expr_match(self, i);
    }

    fn visit_expr_while(&mut self, i: &'ast syn::ExprWhile) {
        self.complexity += 1;
        syn::visit::visit_expr_while(self, i);
    }

    fn visit_expr_for_loop(&mut self, i: &'ast syn::ExprForLoop) {
        self.complexity += 1;
        syn::visit::visit_expr_for_loop(self, i);
    }

    fn visit_expr_loop(&mut self, i: &'ast syn::ExprLoop) {
        self.complexity += 1;
        syn::visit::visit_expr_loop(self, i);
    }

    fn visit_expr_binary(&mut self, i: &'ast syn::ExprBinary) {
        match i.op {
            syn::BinOp::And(_) | syn::BinOp::Or(_) => {
                self.complexity += 1;
            }
            _ => {}
        }
        syn::visit::visit_expr_binary(self, i);
    }

    fn visit_expr_try(&mut self, i: &'ast syn::ExprTry) {
        self.complexity += 1;
        syn::visit::visit_expr_try(self, i);
    }

    // Don't recurse into nested closures or async blocks — they are separate "functions"
    fn visit_expr_closure(&mut self, _i: &'ast syn::ExprClosure) {
        // Skip: closures have their own complexity
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
        Box::new(HighCyclomaticComplexity),
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
    fn test_clone_detected_above_threshold() {
        // 3+ clones in production code should trigger the rule
        let diags = check(
            &ExcessiveClone,
            r"
            fn main() {
                let x = vec![1, 2, 3];
                let a = x.clone();
                let b = x.clone();
                let c = x.clone();
            }
            ",
        );
        assert_eq!(diags.len(), 3);
        assert_eq!(diags[0].rule, "excessive-clone");
    }

    #[test]
    fn test_clone_below_threshold_not_reported() {
        // 1-2 clones are considered intentional and not reported
        let diags = check(
            &ExcessiveClone,
            r"
            fn main() {
                let x = vec![1, 2, 3];
                let y = x.clone();
            }
            ",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_clone_in_test_code_ignored() {
        let diags = check(
            &ExcessiveClone,
            r"
            #[test]
            fn test_something() {
                let x = vec![1, 2, 3];
                let a = x.clone();
                let b = x.clone();
                let c = x.clone();
                let d = x.clone();
            }
            ",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_clone_in_cfg_test_module_ignored() {
        let diags = check(
            &ExcessiveClone,
            r"
            #[cfg(test)]
            mod tests {
                fn helper() {
                    let x = vec![1];
                    let a = x.clone();
                    let b = x.clone();
                    let c = x.clone();
                }
            }
            ",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_clone_no_false_positive_on_other_methods() {
        let diags = check(
            &ExcessiveClone,
            r"
            fn main() {
                let x = vec![1, 2, 3];
                let y = x.len();
            }
            ",
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
            r"
            fn main() {
                let x = 42;
                let s = x.to_string();
            }
            ",
        );
        assert!(diags.is_empty());
    }

    // --- collect-then-iterate ---

    #[test]
    fn test_collect_then_iter_detected() {
        let diags = check(
            &CollectThenIterate,
            r"
            fn main() {
                let v: Vec<i32> = (0..10).collect::<Vec<_>>().iter().map(|x| x + 1).collect();
            }
            ",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "collect-then-iterate");
    }

    #[test]
    fn test_collect_without_iter_not_flagged() {
        let diags = check(
            &CollectThenIterate,
            r"
            fn main() {
                let v: Vec<i32> = (0..10).collect();
            }
            ",
        );
        assert!(diags.is_empty());
    }

    // --- large-enum-variant ---

    #[test]
    fn test_large_enum_variant_detected() {
        let diags = check(
            &LargeEnumVariant,
            r"
            enum Message {
                Quit,
                Data {
                    a: i32, b: i32, c: i32, d: i32,
                    e: i32, f: i32, g: i32, h: i32,
                },
            }
            ",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "large-enum-variant");
    }

    #[test]
    fn test_balanced_enum_not_flagged() {
        let diags = check(
            &LargeEnumVariant,
            r"
            enum Color {
                Red(u8),
                Green(u8),
                Blue(u8),
            }
            ",
        );
        assert!(diags.is_empty());
    }

    // --- unnecessary-allocation ---

    #[test]
    fn test_vec_new_in_loop_detected() {
        let diags = check(
            &UnnecessaryAllocation,
            r"
            fn main() {
                for _ in 0..10 {
                    let v = Vec::new();
                }
            }
            ",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "unnecessary-allocation");
    }

    #[test]
    fn test_vec_new_outside_loop_not_flagged() {
        let diags = check(
            &UnnecessaryAllocation,
            r"
            fn main() {
                let v = Vec::new();
            }
            ",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_string_new_in_while_loop_detected() {
        let diags = check(
            &UnnecessaryAllocation,
            r"
            fn main() {
                while true {
                    let s = String::new();
                }
            }
            ",
        );
        assert_eq!(diags.len(), 1);
    }

    // --- high-cyclomatic-complexity ---

    #[test]
    fn test_simple_function_no_complexity_diagnostic() {
        let diags = check(
            &HighCyclomaticComplexity,
            r"
            fn simple() -> i32 {
                let x = 1 + 2;
                x * 3
            }
            ",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_complex_function_triggers_diagnostic() {
        // Build a function with complexity > 15:
        // base=1, 10 ifs=+10, 1 while=+1, 1 for=+1, 1 loop=+1, 2 &&=+2 = 16
        let diags = check(
            &HighCyclomaticComplexity,
            r"
            fn very_complex(x: i32, y: bool, z: bool) {
                if x > 0 { }
                if x > 1 { }
                if x > 2 { }
                if x > 3 { }
                if x > 4 { }
                if x > 5 { }
                if x > 6 { }
                if x > 7 { }
                if x > 8 { }
                if x > 9 { }
                while x > 0 { }
                for _ in 0..10 { }
                loop { break; }
                if y && z { }
            }
            ",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "high-cyclomatic-complexity");
        assert!(diags[0].message.contains("very_complex"));
        assert!(diags[0].message.contains("16"));
    }

    #[test]
    fn test_complex_test_function_skipped() {
        let diags = check(
            &HighCyclomaticComplexity,
            r"
            #[test]
            fn test_complex(x: i32, y: bool, z: bool) {
                if x > 0 { }
                if x > 1 { }
                if x > 2 { }
                if x > 3 { }
                if x > 4 { }
                if x > 5 { }
                if x > 6 { }
                if x > 7 { }
                if x > 8 { }
                if x > 9 { }
                while x > 0 { }
                for _ in 0..10 { }
                loop { break; }
                if y && z { }
            }
            ",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_complexity_with_match_and_try() {
        // base=1, 8 ifs=+8, match with 5 arms=+4, 3 ?=+3 = 16
        let diags = check(
            &HighCyclomaticComplexity,
            r#"
            fn complex_match(x: i32) -> Result<(), Box<dyn std::error::Error>> {
                if x > 0 { }
                if x > 1 { }
                if x > 2 { }
                if x > 3 { }
                if x > 4 { }
                if x > 5 { }
                if x > 6 { }
                if x > 7 { }
                match x {
                    0 => {},
                    1 => {},
                    2 => {},
                    3 => {},
                    _ => {},
                }
                let _a = "foo".parse::<i32>()?;
                let _b = "bar".parse::<i32>()?;
                let _c = "baz".parse::<i32>()?;
                Ok(())
            }
            "#,
        );
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("16"));
    }

    // --- all_rules ---

    #[test]
    fn test_all_rules_returns_6() {
        assert_eq!(all_rules().len(), 6);
    }
}
