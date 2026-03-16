use std::path::PathBuf;

/// Top-level error for the rust-doctor scan pipeline.
#[derive(thiserror::Error, Debug)]
pub enum ScanError {
    #[error(transparent)]
    Discovery(#[from] DiscoveryError),

    #[error("workspace resolution failed: {0}")]
    Workspace(String),

    #[error("diff resolution failed: {0}")]
    Diff(String),
}

/// Errors from project discovery via `cargo metadata`.
#[derive(thiserror::Error, Debug)]
pub enum DiscoveryError {
    #[error("cargo metadata failed: {source}")]
    CargoMetadata {
        #[source]
        source: cargo_metadata::Error,
    },

    #[error("no packages found in workspace")]
    NoPackages,
}

/// Errors from individual analysis passes.
#[derive(thiserror::Error, Debug)]
pub enum PassError {
    #[error("{pass}: {message}")]
    Failed { pass: String, message: String },

    #[error("{pass}: analysis pass panicked")]
    Panicked { pass: String },
}

/// Errors from the MCP server tool handlers.
#[derive(thiserror::Error, Debug)]
pub enum McpToolError {
    #[error("invalid directory '{path}': {source}")]
    InvalidDirectory {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("no Cargo.toml found in '{}'", path.display())]
    NoCargo { path: PathBuf },

    #[error(transparent)]
    Discovery(#[from] DiscoveryError),

    #[error(transparent)]
    Scan(#[from] ScanError),

    #[error("missing required argument: {0}")]
    MissingArgument(&'static str),
}
