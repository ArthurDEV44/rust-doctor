#![warn(clippy::pedantic)]
// Allow these pedantic lints project-wide — they conflict with our design choices
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::used_underscore_binding)]

// Public API modules
pub mod cli;
pub mod config;
pub mod diagnostics;
pub mod discovery;
pub mod error;
pub mod mcp;
pub mod output;
pub mod scan;

// Internal implementation modules
pub(crate) mod audit;
pub(crate) mod clippy;
pub(crate) mod diff;
pub(crate) mod machete;
pub(crate) mod process;
pub(crate) mod rules;
pub(crate) mod scanner;
pub(crate) mod suppression;
pub(crate) mod workspace;
