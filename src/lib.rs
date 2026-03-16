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

pub mod audit;
pub mod cli;
pub mod clippy;
pub mod config;
pub mod diagnostics;
pub mod error;
pub mod diff;
pub mod discovery;
pub mod machete;
pub mod mcp;
pub mod output;
pub mod rules;
pub mod scan;
pub mod scanner;
pub mod suppression;
pub mod workspace;
