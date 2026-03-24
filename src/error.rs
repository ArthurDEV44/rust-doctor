use std::path::PathBuf;

/// Top-level error for the rust-doctor scan pipeline.
#[derive(thiserror::Error, Debug)]
pub enum ScanError {
    #[error(transparent)]
    Discovery(#[from] DiscoveryError),

    #[error("workspace resolution failed: {0}")]
    Workspace(#[from] WorkspaceError),

    #[error("diff resolution failed: {0}")]
    Diff(#[from] DiffError),
}

/// Errors from workspace member resolution.
#[derive(thiserror::Error, Debug)]
pub enum WorkspaceError {
    #[error("unknown workspace member '{name}'. Available members: {available}")]
    UnknownMember { name: String, available: String },

    #[error("workspace has no members")]
    NoMembers,
}

/// Errors from diff mode resolution.
#[derive(thiserror::Error, Debug)]
pub enum DiffError {
    #[error("invalid ref '{name}': {reason}")]
    InvalidRef { name: String, reason: String },

    #[error("git is not available or directory is not a git repository")]
    GitNotFound,

    #[error("failed to find merge base: {0}")]
    MergeBaseFailed(String),

    #[error("{0}")]
    Other(String),
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

    #[error("{pass}: skipped ({reason})")]
    Skipped { pass: String, reason: String },
}

/// Errors from project bootstrapping (shared between CLI and MCP).
#[derive(thiserror::Error, Debug)]
pub enum BootstrapError {
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
}

/// Errors from the interactive setup wizard.
#[derive(thiserror::Error, Debug)]
pub enum SetupError {
    #[error(transparent)]
    Prompt(#[from] dialoguer::Error),

    #[error("{0}")]
    NotInteractive(String),
}

/// Errors from loading the config file (`rust-doctor.toml`).
#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("failed to read config file '{}': {source}", path.display())]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse config file '{}': {source}", path.display())]
    Parse {
        path: std::path::PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("failed to parse [package.metadata.rust-doctor] in Cargo.toml: {0}")]
    MetadataParse(#[from] serde_json::Error),
}
