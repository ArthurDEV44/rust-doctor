use crate::diagnostics::{Category, Diagnostic, Severity};
use crate::discovery::Framework;
use crate::rules::CustomRule;
use std::path::Path;
use syn::spanned::Spanned;
use syn::visit::Visit;

// ─── Rule 1: tokio-main-missing ─────────────────────────────────────────────

pub struct TokioMainMissing;

impl CustomRule for TokioMainMissing {
    fn name(&self) -> &'static str {
        "tokio-main-missing"
    }
    fn category(&self) -> Category {
        Category::Framework
    }
    fn severity(&self) -> Severity {
        Severity::Error
    }
    fn description(&self) -> &'static str {
        "Flags `async fn main()` without `#[tokio::main]` (or equivalent runtime attribute). Without it, the async runtime is not initialized and the program won't compile or will panic."
    }
    fn fix_hint(&self) -> &'static str {
        "Add `#[tokio::main]` above `async fn main()`."
    }
    fn check_file(&self, syntax: &syn::File, path: &Path) -> Vec<Diagnostic> {
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        if filename != "main.rs" {
            return vec![];
        }

        let mut diagnostics = Vec::new();
        for item in &syntax.items {
            if let syn::Item::Fn(func) = item
                && func.sig.ident == "main"
                && func.sig.asyncness.is_some()
            {
                let has_runtime_attr = func.attrs.iter().any(|attr| {
                    let path_str = attr
                        .path()
                        .segments
                        .iter()
                        .map(|s| s.ident.to_string())
                        .collect::<Vec<_>>()
                        .join("::");
                    path_str == "tokio::main"
                        || path_str == "async_std::main"
                        || path_str == "actix_web::main"
                        || path_str == "rocket::main"
                        || path_str == "main"
                });

                if !has_runtime_attr {
                    let span = func.sig.ident.span();
                    diagnostics.push(Diagnostic {
                        file_path: path.to_path_buf(),
                        rule: "tokio-main-missing".to_string(),
                        category: Category::Framework,
                        severity: Severity::Error,
                        message: "`async fn main()` without runtime macro attribute".to_string(),
                        help: Some(
                            "Add `#[tokio::main]` or `#[async_std::main]` attribute to main"
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

// ─── Rule 2: tokio-spawn-without-move ───────────────────────────────────────

pub struct TokioSpawnWithoutMove;

impl CustomRule for TokioSpawnWithoutMove {
    fn name(&self) -> &'static str {
        "tokio-spawn-without-move"
    }
    fn category(&self) -> Category {
        Category::Framework
    }
    fn severity(&self) -> Severity {
        Severity::Error
    }
    fn description(&self) -> &'static str {
        "Flags `tokio::spawn(async { ... })` without the `move` keyword. Without `move`, the spawned task borrows from the enclosing scope, which often fails to compile due to lifetime requirements ('static bound on spawn)."
    }
    fn fix_hint(&self) -> &'static str {
        "Use `tokio::spawn(async move { ... })`."
    }
    fn check_file(&self, syntax: &syn::File, path: &Path) -> Vec<Diagnostic> {
        let mut visitor = SpawnVisitor {
            path,
            diagnostics: Vec::new(),
        };
        visitor.visit_file(syntax);
        visitor.diagnostics
    }
}

struct SpawnVisitor<'a> {
    path: &'a Path,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for SpawnVisitor<'_> {
    fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
        if let syn::Expr::Path(func_path) = i.func.as_ref() {
            let segments: Vec<String> = func_path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();
            let seg_strs: Vec<&str> = segments.iter().map(std::string::String::as_str).collect();

            if seg_strs.ends_with(&["spawn"])
                && (seg_strs.len() == 1 || seg_strs.contains(&"tokio"))
                && !i.args.is_empty()
                && let Some(first_arg) = i.args.first()
                && is_non_move_async_block(first_arg)
            {
                let span = func_path.path.span();
                self.diagnostics.push(Diagnostic {
                    file_path: self.path.to_path_buf(),
                    rule: "tokio-spawn-without-move".to_string(),
                    category: Category::Framework,
                    severity: Severity::Error,
                    message: "tokio::spawn with non-move async block may capture references"
                        .to_string(),
                    help: Some(
                        "Use `tokio::spawn(async move { ... })` to transfer ownership".to_string(),
                    ),
                    line: Some(span.start().line as u32),
                    column: Some(span.start().column as u32 + 1),
                });
            }
        }
        syn::visit::visit_expr_call(self, i);
    }
}

const fn is_non_move_async_block(expr: &syn::Expr) -> bool {
    matches!(expr, syn::Expr::Async(ab) if ab.capture.is_none())
}

// ─── Rule 3: axum-handler-not-async ─────────────────────────────────────────

pub struct AxumHandlerNotAsync;

const AXUM_EXTRACTOR_TYPES: &[&str] = &[
    "Json",
    "Path",
    "Query",
    "State",
    "Extension",
    "Form",
    "Header",
    "TypedHeader",
];

impl CustomRule for AxumHandlerNotAsync {
    fn name(&self) -> &'static str {
        "axum-handler-not-async"
    }
    fn category(&self) -> Category {
        Category::Framework
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn description(&self) -> &'static str {
        "Flags non-async handler functions in axum. Web framework handlers run on the async runtime and must not block."
    }
    fn fix_hint(&self) -> &'static str {
        "Make the handler `async fn` and use async I/O operations."
    }
    fn check_file(&self, syntax: &syn::File, path: &Path) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for item in &syntax.items {
            if let syn::Item::Fn(func) = item
                && func.sig.asyncness.is_none()
                && has_axum_extractor_params(&func.sig)
            {
                let span = func.sig.ident.span();
                diagnostics.push(Diagnostic {
                    file_path: path.to_path_buf(),
                    rule: "axum-handler-not-async".to_string(),
                    category: Category::Framework,
                    severity: Severity::Warning,
                    message: format!(
                        "Function `{}` uses axum extractors but is not async",
                        func.sig.ident
                    ),
                    help: Some(
                        "Axum handlers should be `async fn` to work with the router".to_string(),
                    ),
                    line: Some(span.start().line as u32),
                    column: Some(span.start().column as u32 + 1),
                });
            }
        }
        diagnostics
    }
}

fn has_axum_extractor_params(sig: &syn::Signature) -> bool {
    sig.inputs.iter().any(|arg| {
        if let syn::FnArg::Typed(pat_type) = arg {
            type_contains_axum_extractor(&pat_type.ty)
        } else {
            false
        }
    })
}

fn type_contains_axum_extractor(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty
        && let Some(seg) = type_path.path.segments.last()
    {
        let name = seg.ident.to_string();
        return AXUM_EXTRACTOR_TYPES.contains(&name.as_str());
    }
    false
}

// ─── Rule 4: actix-blocking-handler ─────────────────────────────────────────

pub struct ActixBlockingHandler;

const ACTIX_EXTRACTOR_PREFIXES: &[&str] = &["web"];

impl CustomRule for ActixBlockingHandler {
    fn name(&self) -> &'static str {
        "actix-blocking-handler"
    }
    fn category(&self) -> Category {
        Category::Framework
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn description(&self) -> &'static str {
        "Flags blocking calls (`std::thread::sleep`, `std::fs::*`, `std::net::*`) in actix-web handler functions. Web framework handlers run on the async runtime and must not block."
    }
    fn fix_hint(&self) -> &'static str {
        "Use async equivalents (`tokio::time::sleep`, `tokio::fs::*`, `tokio::net::*`) or wrap blocking code in `actix_web::web::block()`."
    }
    fn check_file(&self, syntax: &syn::File, path: &Path) -> Vec<Diagnostic> {
        let mut visitor = ActixVisitor {
            path,
            diagnostics: Vec::new(),
            in_handler: false,
        };
        visitor.visit_file(syntax);
        visitor.diagnostics
    }
}

struct ActixVisitor<'a> {
    path: &'a Path,
    diagnostics: Vec<Diagnostic>,
    in_handler: bool,
}

/// Blocking call patterns detected in actix-web handlers.
/// Each entry is (path segments to match, help message).
const ACTIX_BLOCKING_CALLS: &[(&[&str], &str)] = &[
    (
        &["std", "thread", "sleep"],
        "Use `tokio::time::sleep` or wrap in `actix_web::web::block()`",
    ),
    (
        &["std", "fs", "read_to_string"],
        "Use `tokio::fs::read_to_string` or wrap in `web::block()`",
    ),
    (
        &["std", "fs", "write"],
        "Use `tokio::fs::write` or wrap in `web::block()`",
    ),
    (
        &["std", "fs", "read"],
        "Use `tokio::fs::read` or wrap in `web::block()`",
    ),
    (
        &["std", "fs", "create_dir_all"],
        "Use `tokio::fs::create_dir_all` or wrap in `web::block()`",
    ),
    (
        &["std", "fs", "remove_file"],
        "Use `tokio::fs::remove_file` or wrap in `web::block()`",
    ),
    (
        &["std", "fs", "File", "open"],
        "Use `tokio::fs::File::open` or wrap in `web::block()`",
    ),
    (
        &["std", "fs", "File", "create"],
        "Use `tokio::fs::File::create` or wrap in `web::block()`",
    ),
    (
        &["std", "net", "TcpStream", "connect"],
        "Use `tokio::net::TcpStream::connect` or wrap in `web::block()`",
    ),
    (
        &["std", "net", "TcpListener", "bind"],
        "Use `tokio::net::TcpListener::bind` or wrap in `web::block()`",
    ),
];

/// Short patterns (last 2 segments) for common blocking calls.
const ACTIX_BLOCKING_SHORT: &[(&str, &str, &str)] = &[(
    "thread",
    "sleep",
    "Use `tokio::time::sleep` or wrap in `actix_web::web::block()`",
)];

impl<'ast> Visit<'ast> for ActixVisitor<'_> {
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        let was_in_handler = self.in_handler;
        if i.sig.asyncness.is_some() && has_actix_extractor_params(&i.sig) {
            self.in_handler = true;
        }
        syn::visit::visit_item_fn(self, i);
        self.in_handler = was_in_handler;
    }

    fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
        if self.in_handler
            && let syn::Expr::Path(func_path) = i.func.as_ref()
        {
            let segments: Vec<String> = func_path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();
            let seg_strs: Vec<&str> = segments.iter().map(std::string::String::as_str).collect();

            // Check full path patterns
            let mut matched = false;
            for (pattern, help) in ACTIX_BLOCKING_CALLS {
                if super::segments_match(&seg_strs, pattern) {
                    let span = func_path.path.span();
                    self.diagnostics.push(Diagnostic {
                        file_path: self.path.to_path_buf(),
                        rule: "actix-blocking-handler".to_string(),
                        category: Category::Framework,
                        severity: Severity::Warning,
                        message: format!(
                            "Blocking call `{}` in actix-web handler",
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

            // Check short patterns (last 2 segments) if no full match
            if !matched && segments.len() >= 2 {
                let last_two = &segments[segments.len() - 2..];
                for (a, b, help) in ACTIX_BLOCKING_SHORT {
                    if last_two[0] == *a && last_two[1] == *b {
                        let span = func_path.path.span();
                        self.diagnostics.push(Diagnostic {
                            file_path: self.path.to_path_buf(),
                            rule: "actix-blocking-handler".to_string(),
                            category: Category::Framework,
                            severity: Severity::Warning,
                            message: format!(
                                "Blocking call `{}` in actix-web handler",
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
}

fn has_actix_extractor_params(sig: &syn::Signature) -> bool {
    sig.inputs.iter().any(|arg| {
        if let syn::FnArg::Typed(pat_type) = arg
            && let syn::Type::Path(type_path) = pat_type.ty.as_ref()
            && let Some(first_seg) = type_path.path.segments.first()
        {
            ACTIX_EXTRACTOR_PREFIXES.contains(&first_seg.ident.to_string().as_str())
        } else {
            false
        }
    })
}

// ─── Convenience ────────────────────────────────────────────────────────────

/// Return all framework rules regardless of detected frameworks.
/// Used for rule documentation and metadata enumeration.
pub fn all_rules() -> Vec<Box<dyn CustomRule>> {
    vec![
        Box::new(TokioMainMissing),
        Box::new(TokioSpawnWithoutMove),
        Box::new(AxumHandlerNotAsync),
        Box::new(ActixBlockingHandler),
    ]
}

pub fn rules_for_frameworks(frameworks: &[Framework]) -> Vec<Box<dyn CustomRule>> {
    let mut rules: Vec<Box<dyn CustomRule>> = Vec::new();

    let has_tokio = frameworks.contains(&Framework::Tokio);
    let has_axum = frameworks.contains(&Framework::Axum);
    let has_actix = frameworks.contains(&Framework::ActixWeb);
    let has_async_runtime = has_tokio
        || frameworks.contains(&Framework::AsyncStd)
        || frameworks.contains(&Framework::Smol);

    if has_async_runtime {
        rules.push(Box::new(TokioMainMissing));
    }
    if has_tokio {
        rules.push(Box::new(TokioSpawnWithoutMove));
    }
    if has_axum {
        rules.push(Box::new(AxumHandlerNotAsync));
    }
    if has_actix {
        rules.push(Box::new(ActixBlockingHandler));
    }

    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(rule: &dyn CustomRule, code: &str, path: &str) -> Vec<Diagnostic> {
        let syntax = syn::parse_file(code).expect("test code should parse");
        rule.check_file(&syntax, Path::new(path))
    }

    #[test]
    fn test_async_main_without_tokio_attr() {
        let diags = check(&TokioMainMissing, "async fn main() {}", "src/main.rs");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "tokio-main-missing");
    }

    #[test]
    fn test_async_main_with_tokio_attr() {
        let diags = check(
            &TokioMainMissing,
            "#[tokio::main]\nasync fn main() {}",
            "src/main.rs",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_sync_main_not_flagged() {
        let diags = check(&TokioMainMissing, "fn main() {}", "src/main.rs");
        assert!(diags.is_empty());
    }

    #[test]
    fn test_not_main_rs_skipped() {
        let diags = check(&TokioMainMissing, "async fn main() {}", "src/lib.rs");
        assert!(diags.is_empty());
    }

    #[test]
    fn test_spawn_without_move_detected() {
        let diags = check(
            &TokioSpawnWithoutMove,
            "fn start() { tokio::spawn(async { println!(\"work\"); }); }",
            "test.rs",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "tokio-spawn-without-move");
    }

    #[test]
    fn test_spawn_with_move_not_flagged() {
        let diags = check(
            &TokioSpawnWithoutMove,
            "fn start() { tokio::spawn(async move { println!(\"work\"); }); }",
            "test.rs",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_axum_handler_not_async_detected() {
        let diags = check(
            &AxumHandlerNotAsync,
            "fn get_user(Path(id): Path<u32>) -> Json<User> { todo!() }",
            "test.rs",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "axum-handler-not-async");
    }

    #[test]
    fn test_axum_async_handler_not_flagged() {
        let diags = check(
            &AxumHandlerNotAsync,
            "async fn get_user(Path(id): Path<u32>) -> Json<User> { todo!() }",
            "test.rs",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_non_axum_fn_not_flagged() {
        let diags = check(
            &AxumHandlerNotAsync,
            "fn helper(x: i32) -> i32 { x + 1 }",
            "test.rs",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_actix_blocking_in_handler() {
        let diags = check(
            &ActixBlockingHandler,
            r#"
            async fn index(info: web::Json<Info>) -> impl Responder {
                std::thread::sleep(std::time::Duration::from_secs(1));
                "ok"
            }
            "#,
            "test.rs",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "actix-blocking-handler");
    }

    #[test]
    fn test_actix_blocking_fs_in_handler() {
        let diags = check(
            &ActixBlockingHandler,
            r#"
            async fn index(info: web::Json<Info>) -> impl Responder {
                let data = std::fs::read_to_string("config.toml");
                "ok"
            }
            "#,
            "test.rs",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "actix-blocking-handler");
        assert!(diags[0].message.contains("std::fs::read_to_string"));
    }

    #[test]
    fn test_actix_blocking_net_in_handler() {
        let diags = check(
            &ActixBlockingHandler,
            r#"
            async fn index(info: web::Json<Info>) -> impl Responder {
                let stream = std::net::TcpStream::connect("127.0.0.1:8080");
                "ok"
            }
            "#,
            "test.rs",
        );
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("std::net::TcpStream::connect"));
    }

    #[test]
    fn test_actix_non_handler_not_flagged() {
        let diags = check(
            &ActixBlockingHandler,
            "async fn bg() { std::thread::sleep(std::time::Duration::from_secs(1)); }",
            "test.rs",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn test_no_frameworks_no_rules() {
        assert!(rules_for_frameworks(&[]).is_empty());
    }

    #[test]
    fn test_tokio_gets_main_and_spawn_rules() {
        let rules = rules_for_frameworks(&[Framework::Tokio]);
        let names: Vec<&str> = rules.iter().map(|r| r.name()).collect();
        assert!(names.contains(&"tokio-main-missing"));
        assert!(names.contains(&"tokio-spawn-without-move"));
    }

    #[test]
    fn test_axum_gets_handler_rule() {
        let rules = rules_for_frameworks(&[Framework::Axum]);
        assert!(rules.iter().any(|r| r.name() == "axum-handler-not-async"));
    }

    #[test]
    fn test_actix_gets_blocking_rule() {
        let rules = rules_for_frameworks(&[Framework::ActixWeb]);
        assert!(rules.iter().any(|r| r.name() == "actix-blocking-handler"));
    }
}
