//! # rust-doctor
//!
//! A unified code health tool for Rust — scan, score, and fix your codebase.
//!
//! rust-doctor analyzes Rust projects for security, performance, correctness,
//! architecture, and dependency issues, producing a 0–100 health score with
//! actionable diagnostics.
//!
//! ## Quick start (library usage)
//!
//! ```rust,no_run
//! use std::path::Path;
//!
//! // Discover the project
//! let (dir, info, config) = rust_doctor::discovery::bootstrap_project(
//!     Path::new("."), false,
//! ).unwrap();
//!
//! // Resolve config with defaults
//! let resolved = rust_doctor::config::resolve_config_defaults(config.as_ref());
//!
//! // Run the scan
//! let result = rust_doctor::scan::scan_project(&info, &resolved, false, &[], true).unwrap();
//! println!("Score: {}/100 ({})", result.score, result.score_label);
//! ```

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
// Expect these pedantic lints project-wide — they conflict with our design choices.
// Using #[expect] so the compiler warns if any suppression becomes dead.
#![expect(
    clippy::module_name_repetitions,
    reason = "module prefixes on types are intentional"
)]
#![expect(
    clippy::must_use_candidate,
    reason = "not all public fns need #[must_use]"
)]
#![expect(
    clippy::missing_errors_doc,
    reason = "# Errors docs added to key functions; remaining deferred until v1.0"
)]
#![expect(
    clippy::doc_markdown,
    reason = "markdown linting too strict for rule names"
)]
#![expect(
    clippy::struct_excessive_bools,
    reason = "visitor state requires bool fields"
)]
#![expect(
    clippy::cast_possible_truncation,
    reason = "line/column casts are safe within u32 range"
)]
#![expect(
    clippy::cast_precision_loss,
    reason = "score penalty math is fine with f64"
)]
#![expect(
    clippy::items_after_statements,
    reason = "inline test helpers after setup"
)]
#![expect(
    clippy::too_many_lines,
    reason = "some analysis functions are inherently long"
)]
#![expect(clippy::cast_sign_loss, reason = "score clamped to 0-100 before cast")]
#![expect(
    clippy::used_underscore_binding,
    reason = "underscore prefixes used in destructuring"
)]

/// Command-line argument parsing and flag definitions.
pub mod cli;
/// Configuration loading, merging, and validation.
pub mod config;
/// External tool dependency checking and installation.
pub mod deps;
/// Core diagnostic types: `Diagnostic`, `Severity`, `Category`, `ScanResult`.
pub mod diagnostics;
/// Project discovery via `cargo metadata` and framework detection.
pub mod discovery;
/// Error types for the scan pipeline, bootstrapping, and MCP.
pub mod error;
/// Auto-fix application for machine-applicable diagnostic fixes.
pub mod fixer;
/// MCP (Model Context Protocol) server for AI tool integration.
#[cfg(feature = "mcp")]
pub mod mcp;
/// Terminal, JSON, and score output rendering.
pub mod output;
/// Remediation plan generator from scan diagnostics.
pub mod plan;
/// SARIF 2.1.0 output for CI/CD integration.
pub mod sarif;
/// Top-level scan pipeline that orchestrates all analysis passes.
pub mod scan;
/// Interactive setup wizard for AI agent integration.
pub mod setup;

// Internal implementation modules
pub(crate) mod cache;
pub(crate) mod diff;
pub(crate) mod passes;
pub(crate) mod process;
pub(crate) mod scanner;
pub(crate) mod suppression;
pub(crate) mod workspace;

// Re-export pass modules at crate root so existing `use crate::audit` etc. still work.
pub(crate) use passes::quality::coverage;
pub(crate) use passes::quality::machete;
pub(crate) use passes::quality::msrv;
pub(crate) use passes::quality::semver_checks;
pub(crate) use passes::security::audit;
pub(crate) use passes::security::deny;
pub(crate) use passes::security::geiger;
pub(crate) use passes::static_analysis::clippy;
pub(crate) use passes::static_analysis::rules;
