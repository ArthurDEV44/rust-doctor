use crate::diagnostics::{Category, Diagnostic, Severity};
use crate::rules::CustomRule;
use std::path::Path;
use syn::spanned::Spanned;
use syn::visit::Visit;

// ─── Rule 1: blocking-in-async ──────────────────────────────────────────────

/// Flags blocking calls inside `async fn` bodies:
/// - `std::thread::sleep`
/// - `std::fs::*` (read, write, etc.)
/// - `std::net::*` (TcpStream, UdpSocket, etc.)
pub struct BlockingInAsync;

/// Known blocking call patterns (path segments to match).
const BLOCKING_CALLS: &[(&[&str], &str)] = &[
    (
        &["std", "thread", "sleep"],
        "Use `tokio::time::sleep` or `async_std::task::sleep` instead",
    ),
    (
        &["std", "fs", "read"],
        "Use `tokio::fs::read` or `async_std::fs::read` instead",
    ),
    (
        &["std", "fs", "write"],
        "Use `tokio::fs::write` or `async_std::fs::write` instead",
    ),
    (
        &["std", "fs", "read_to_string"],
        "Use `tokio::fs::read_to_string` instead",
    ),
    (
        &["std", "fs", "read_dir"],
        "Use `tokio::fs::read_dir` instead",
    ),
    (
        &["std", "fs", "create_dir"],
        "Use `tokio::fs::create_dir` instead",
    ),
    (
        &["std", "fs", "create_dir_all"],
        "Use `tokio::fs::create_dir_all` instead",
    ),
    (
        &["std", "fs", "remove_file"],
        "Use `tokio::fs::remove_file` instead",
    ),
    (
        &["std", "fs", "remove_dir"],
        "Use `tokio::fs::remove_dir` instead",
    ),
    (&["std", "fs", "rename"], "Use `tokio::fs::rename` instead"),
    (&["std", "fs", "copy"], "Use `tokio::fs::copy` instead"),
    (
        &["std", "fs", "metadata"],
        "Use `tokio::fs::metadata` instead",
    ),
    (
        &["std", "fs", "File", "open"],
        "Use `tokio::fs::File::open` instead",
    ),
    (
        &["std", "fs", "File", "create"],
        "Use `tokio::fs::File::create` instead",
    ),
    (
        &["std", "net", "TcpStream", "connect"],
        "Use `tokio::net::TcpStream::connect` instead",
    ),
    (
        &["std", "net", "TcpListener", "bind"],
        "Use `tokio::net::TcpListener::bind` instead",
    ),
    (
        &["std", "net", "UdpSocket", "bind"],
        "Use `tokio::net::UdpSocket::bind` instead",
    ),
];

/// Shorter patterns — just the last 2 segments for common cases like `thread::sleep`.
const BLOCKING_SHORT: &[(&str, &str, &str)] = &[(
    "thread",
    "sleep",
    "Use `tokio::time::sleep` instead of `std::thread::sleep`",
)];

impl CustomRule for BlockingInAsync {
    fn name(&self) -> &str {
        "blocking-in-async"
    }
    fn category(&self) -> Category {
        Category::Async
    }
    fn severity(&self) -> Severity {
        Severity::Error
    }
    fn check_file(&self, syntax: &syn::File, path: &Path) -> Vec<Diagnostic> {
        let mut visitor = BlockingVisitor {
            path,
            diagnostics: Vec::new(),
            in_async: false,
            in_spawn_blocking: false,
        };
        visitor.visit_file(syntax);
        visitor.diagnostics
    }
}

struct BlockingVisitor<'a> {
    path: &'a Path,
    diagnostics: Vec<Diagnostic>,
    in_async: bool,
    in_spawn_blocking: bool,
}

impl<'ast> Visit<'ast> for BlockingVisitor<'_> {
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        let was_async = self.in_async;
        if i.sig.asyncness.is_some() {
            self.in_async = true;
        }
        syn::visit::visit_item_fn(self, i);
        self.in_async = was_async;
    }

    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        let was_async = self.in_async;
        if i.sig.asyncness.is_some() {
            self.in_async = true;
        }
        syn::visit::visit_impl_item_fn(self, i);
        self.in_async = was_async;
    }

    fn visit_expr_async(&mut self, i: &'ast syn::ExprAsync) {
        let was_async = self.in_async;
        self.in_async = true;
        syn::visit::visit_expr_async(self, i);
        self.in_async = was_async;
    }

    fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
        if self.in_async
            && !self.in_spawn_blocking
            && let syn::Expr::Path(func_path) = i.func.as_ref()
        {
            let segments: Vec<String> = func_path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();

            // Check spawn_blocking — mark so we skip inner closure
            let seg_strs: Vec<&str> = segments.iter().map(|s| s.as_str()).collect();
            if seg_strs.ends_with(&["spawn_blocking"]) {
                let was_spawn = self.in_spawn_blocking;
                self.in_spawn_blocking = true;
                syn::visit::visit_expr_call(self, i);
                self.in_spawn_blocking = was_spawn;
                return;
            }

            // Check full path patterns
            let mut matched = false;
            for (pattern, help) in BLOCKING_CALLS {
                if segments_match(&seg_strs, pattern) {
                    let span = func_path.path.span();
                    self.diagnostics.push(Diagnostic {
                        file_path: self.path.to_path_buf(),
                        rule: "blocking-in-async".to_string(),
                        category: Category::Async,
                        severity: Severity::Error,
                        message: format!(
                            "Blocking call `{}` inside async context",
                            segments.join("::")
                        ),
                        help: Some(help.to_string()),
                        line: Some(span.start().line as u32),
                        column: Some(span.start().column as u32 + 1),
                    });
                    matched = true;
                    break;
                }
            }

            // Check short patterns only if full pattern didn't match
            if !matched && segments.len() >= 2 {
                let last_two = &segments[segments.len() - 2..];
                for (a, b, help) in BLOCKING_SHORT {
                    if last_two[0] == *a && last_two[1] == *b {
                        let span = func_path.path.span();
                        self.diagnostics.push(Diagnostic {
                            file_path: self.path.to_path_buf(),
                            rule: "blocking-in-async".to_string(),
                            category: Category::Async,
                            severity: Severity::Error,
                            message: format!(
                                "Blocking call `{}` inside async context",
                                segments.join("::")
                            ),
                            help: Some(help.to_string()),
                            line: Some(span.start().line as u32),
                            column: Some(span.start().column as u32 + 1),
                        });
                    }
                }
            }
        }
        syn::visit::visit_expr_call(self, i);
    }

    fn visit_expr_method_call(&mut self, i: &'ast syn::ExprMethodCall) {
        // Check for .read() / .write() on std::fs::File-like types (heuristic)
        if self.in_async && !self.in_spawn_blocking {
            let method = i.method.to_string();
            if method == "read_to_string" || method == "read_to_end" {
                // These are blocking I/O methods when called on std::fs::File
                let span = i.method.span();
                self.diagnostics.push(Diagnostic {
                    file_path: self.path.to_path_buf(),
                    rule: "blocking-in-async".to_string(),
                    category: Category::Async,
                    severity: Severity::Error,
                    message: format!(
                        "Potentially blocking `.{method}()` call inside async context"
                    ),
                    help: Some("Use async I/O methods from tokio::io or async_std::io".to_string()),
                    line: Some(span.start().line as u32),
                    column: Some(span.start().column as u32 + 1),
                });
            }
        }
        syn::visit::visit_expr_method_call(self, i);
    }
}

fn segments_match(actual: &[&str], pattern: &[&str]) -> bool {
    if actual.len() < pattern.len() {
        return false;
    }
    actual.ends_with(pattern)
}

// ─── Rule 2: block-on-in-async ──────────────────────────────────────────────

/// Flags `block_on` calls inside async context.
pub struct BlockOnInAsync;

impl CustomRule for BlockOnInAsync {
    fn name(&self) -> &str {
        "block-on-in-async"
    }
    fn category(&self) -> Category {
        Category::Async
    }
    fn severity(&self) -> Severity {
        Severity::Error
    }
    fn check_file(&self, syntax: &syn::File, path: &Path) -> Vec<Diagnostic> {
        let mut visitor = BlockOnVisitor {
            path,
            diagnostics: Vec::new(),
            in_async: false,
        };
        visitor.visit_file(syntax);
        visitor.diagnostics
    }
}

struct BlockOnVisitor<'a> {
    path: &'a Path,
    diagnostics: Vec<Diagnostic>,
    in_async: bool,
}

impl<'ast> Visit<'ast> for BlockOnVisitor<'_> {
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        let was_async = self.in_async;
        if i.sig.asyncness.is_some() {
            self.in_async = true;
        }
        syn::visit::visit_item_fn(self, i);
        self.in_async = was_async;
    }

    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        let was_async = self.in_async;
        if i.sig.asyncness.is_some() {
            self.in_async = true;
        }
        syn::visit::visit_impl_item_fn(self, i);
        self.in_async = was_async;
    }

    fn visit_expr_async(&mut self, i: &'ast syn::ExprAsync) {
        let was_async = self.in_async;
        self.in_async = true;
        syn::visit::visit_expr_async(self, i);
        self.in_async = was_async;
    }

    fn visit_expr_method_call(&mut self, i: &'ast syn::ExprMethodCall) {
        if self.in_async && i.method == "block_on" {
            let span = i.method.span();
            self.diagnostics.push(Diagnostic {
                file_path: self.path.to_path_buf(),
                rule: "block-on-in-async".to_string(),
                category: Category::Async,
                severity: Severity::Error,
                message: ".block_on() inside async context causes executor deadlock".to_string(),
                help: Some("Use `.await` directly instead of `.block_on()`".to_string()),
                line: Some(span.start().line as u32),
                column: Some(span.start().column as u32 + 1),
            });
        }
        syn::visit::visit_expr_method_call(self, i);
    }

    fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
        if self.in_async
            && let syn::Expr::Path(func_path) = i.func.as_ref()
        {
            let segments: Vec<String> = func_path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();
            let seg_strs: Vec<&str> = segments.iter().map(|s| s.as_str()).collect();
            if seg_strs.ends_with(&["block_on"]) {
                let span = func_path.path.span();
                self.diagnostics.push(Diagnostic {
                    file_path: self.path.to_path_buf(),
                    rule: "block-on-in-async".to_string(),
                    category: Category::Async,
                    severity: Severity::Error,
                    message: "block_on() inside async context causes executor deadlock".to_string(),
                    help: Some("Use `.await` directly instead of `block_on()`".to_string()),
                    line: Some(span.start().line as u32),
                    column: Some(span.start().column as u32 + 1),
                });
            }
        }
        syn::visit::visit_expr_call(self, i);
    }
}

// ─── Convenience ────────────────────────────────────────────────────────────

/// Returns all async anti-pattern rules.
pub fn all_rules() -> Vec<Box<dyn CustomRule>> {
    vec![Box::new(BlockingInAsync), Box::new(BlockOnInAsync)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(rule: &dyn CustomRule, code: &str) -> Vec<Diagnostic> {
        let syntax = syn::parse_file(code).expect("test code should parse");
        rule.check_file(&syntax, Path::new("test.rs"))
    }

    // --- blocking-in-async ---

    #[test]
    fn test_thread_sleep_in_async_detected() {
        let diags = check(
            &BlockingInAsync,
            r#"
            async fn do_work() {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            "#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "blocking-in-async");
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(diags[0].message.contains("thread::sleep"));
    }

    #[test]
    fn test_thread_sleep_in_sync_fn_not_flagged() {
        let diags = check(
            &BlockingInAsync,
            r#"
            fn do_work() {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            "#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_short_thread_sleep_in_async() {
        let diags = check(
            &BlockingInAsync,
            r#"
            use std::thread;
            async fn do_work() {
                thread::sleep(std::time::Duration::from_secs(1));
            }
            "#,
        );
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_std_fs_in_async_detected() {
        let diags = check(
            &BlockingInAsync,
            r#"
            async fn read_file() {
                let data = std::fs::read_to_string("file.txt");
            }
            "#,
        );
        assert!(diags.len() >= 1);
        assert!(diags.iter().any(|d| d.rule == "blocking-in-async"));
    }

    #[test]
    fn test_spawn_blocking_not_flagged() {
        let diags = check(
            &BlockingInAsync,
            r#"
            async fn do_work() {
                tokio::task::spawn_blocking(|| {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                });
            }
            "#,
        );
        // thread::sleep inside spawn_blocking is correct usage
        assert!(diags.is_empty());
    }

    // --- block-on-in-async ---

    #[test]
    fn test_block_on_method_in_async_detected() {
        let diags = check(
            &BlockOnInAsync,
            r#"
            async fn do_work() {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async { 42 });
            }
            "#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "block-on-in-async");
        assert_eq!(diags[0].severity, Severity::Error);
    }

    #[test]
    fn test_block_on_call_in_async_detected() {
        let diags = check(
            &BlockOnInAsync,
            r#"
            async fn do_work() {
                futures::executor::block_on(async { 42 });
            }
            "#,
        );
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_block_on_in_sync_fn_not_flagged() {
        let diags = check(
            &BlockOnInAsync,
            r#"
            fn main() {
                futures::executor::block_on(async { 42 });
            }
            "#,
        );
        assert!(diags.is_empty());
    }

    // --- all_rules ---

    #[test]
    fn test_all_rules_returns_2() {
        assert_eq!(all_rules().len(), 2);
    }
}
