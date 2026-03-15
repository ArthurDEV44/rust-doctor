use cargo_metadata::{DependencyKind, MetadataCommand, TargetKind};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Detected framework or runtime in the project's dependencies.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Framework {
    Tokio,
    AsyncStd,
    Smol,
    Axum,
    ActixWeb,
    Rocket,
    Warp,
    Diesel,
    Sqlx,
    SeaOrm,
    Tonic,
    WasmBindgen,
    WebSys,
    Embassy,
    CortexM,
}

impl std::fmt::Display for Framework {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tokio => write!(f, "tokio"),
            Self::AsyncStd => write!(f, "async-std"),
            Self::Smol => write!(f, "smol"),
            Self::Axum => write!(f, "axum"),
            Self::ActixWeb => write!(f, "actix-web"),
            Self::Rocket => write!(f, "rocket"),
            Self::Warp => write!(f, "warp"),
            Self::Diesel => write!(f, "diesel"),
            Self::Sqlx => write!(f, "sqlx"),
            Self::SeaOrm => write!(f, "sea-orm"),
            Self::Tonic => write!(f, "tonic"),
            Self::WasmBindgen => write!(f, "wasm-bindgen"),
            Self::WebSys => write!(f, "web-sys"),
            Self::Embassy => write!(f, "embassy"),
            Self::CortexM => write!(f, "cortex-m"),
        }
    }
}

/// Maps crate dependency names to Framework variants.
/// For prefix-based matching (embassy-*), see `detect_frameworks`.
const FRAMEWORK_MAP: &[(&str, Framework)] = &[
    ("tokio", Framework::Tokio),
    ("async-std", Framework::AsyncStd),
    ("smol", Framework::Smol),
    ("axum", Framework::Axum),
    ("actix-web", Framework::ActixWeb),
    ("rocket", Framework::Rocket),
    ("warp", Framework::Warp),
    ("diesel", Framework::Diesel),
    ("sqlx", Framework::Sqlx),
    ("sea-orm", Framework::SeaOrm),
    ("tonic", Framework::Tonic),
    ("wasm-bindgen", Framework::WasmBindgen),
    ("web-sys", Framework::WebSys),
    ("cortex-m", Framework::CortexM),
];

/// Discovered project information from cargo metadata.
#[derive(Debug)]
pub struct ProjectInfo {
    /// Absolute path to the workspace or project root.
    pub root_dir: PathBuf,
    /// Primary package name (first workspace member, or the single package).
    pub name: String,
    /// Primary package version.
    pub version: String,
    /// Rust edition of the primary package.
    pub edition: String,
    /// Detected frameworks/runtimes from dependencies.
    pub frameworks: Vec<Framework>,
    /// Whether this is a Cargo workspace (>1 member).
    pub is_workspace: bool,
    /// Number of workspace members.
    pub member_count: usize,
    /// Whether the primary package has a build script (build.rs).
    pub has_build_script: bool,
    /// The `rust-version` (MSRV) field, if specified.
    pub rust_version: Option<String>,
    /// Whether the project declares `#![no_std]`.
    pub is_no_std: bool,
}

/// Run cargo metadata and discover project characteristics.
///
/// `manifest_path` should point to the Cargo.toml file.
/// If `offline` is true, passes `--offline` to cargo to prevent network access.
/// Returns `Ok(ProjectInfo)` on success, or an error if cargo metadata fails.
pub fn discover_project(manifest_path: &Path, offline: bool) -> Result<ProjectInfo, String> {
    let mut cmd = MetadataCommand::new();
    cmd.manifest_path(manifest_path).no_deps();
    if offline {
        cmd.other_options(["--offline".to_string()]);
    }
    let metadata = cmd
        .exec()
        .map_err(|e| format!("cargo metadata failed: {e}"))?;

    let workspace_root = PathBuf::from(metadata.workspace_root.as_std_path());
    let members = metadata.workspace_packages();
    let member_count = members.len();
    let is_workspace = member_count > 1;

    // Use first workspace member as "primary" package
    let primary = members.first().ok_or("No packages found in workspace")?;

    let name = primary.name.clone();
    let version = primary.version.to_string();
    let edition = primary.edition.as_str().to_string();
    let rust_version = primary.rust_version.as_ref().map(|v| v.to_string());

    // Detect build script
    let has_build_script = primary
        .targets
        .iter()
        .any(|t| t.kind.contains(&TargetKind::CustomBuild));

    // Collect all dependency names across all workspace members
    let all_dep_names: HashSet<&str> = members
        .iter()
        .flat_map(|pkg| {
            pkg.dependencies
                .iter()
                .filter(|d| d.kind == DependencyKind::Normal)
                .map(|d| d.name.as_str())
        })
        .collect();

    let frameworks = detect_frameworks(&all_dep_names);

    // Detect #![no_std] from primary package's lib.rs or main.rs
    let is_no_std = detect_no_std(primary);

    Ok(ProjectInfo {
        root_dir: workspace_root,
        name,
        version,
        edition,
        frameworks,
        is_workspace,
        member_count,
        has_build_script,
        rust_version,
        is_no_std,
    })
}

/// Detect frameworks from dependency names.
fn detect_frameworks(dep_names: &HashSet<&str>) -> Vec<Framework> {
    let mut frameworks: Vec<Framework> = FRAMEWORK_MAP
        .iter()
        .filter(|(crate_name, _)| dep_names.contains(crate_name))
        .map(|(_, framework)| framework.clone())
        .collect();

    // Prefix-based detection for embassy-* crates
    if dep_names.iter().any(|name| name.starts_with("embassy-"))
        && !frameworks.contains(&Framework::Embassy)
    {
        frameworks.push(Framework::Embassy);
    }

    frameworks
}

/// Detect `#![no_std]` by scanning the primary source file's first 10 lines.
fn detect_no_std(pkg: &cargo_metadata::Package) -> bool {
    // Find lib or bin target's source path
    let src_path = pkg
        .targets
        .iter()
        .find(|t| {
            t.kind.contains(&TargetKind::Lib)
                || t.kind.contains(&TargetKind::RLib)
                || t.kind.contains(&TargetKind::Bin)
        })
        .map(|t| t.src_path.as_std_path());

    match src_path {
        Some(path) => file_declares_no_std(path),
        None => false,
    }
}

/// Returns `true` if the file declares `#![no_std]` in its first 10 lines.
fn file_declares_no_std(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let reader = BufReader::new(file);

    for line in reader.lines().take(10) {
        let Ok(line) = line else {
            break;
        };
        let trimmed = line.trim();
        // Check for #![no_std], tolerating internal whitespace like #![ no_std ]
        if trimmed
            .strip_prefix("#![")
            .and_then(|s| s.strip_suffix(']'))
            .is_some_and(|inner| inner.trim() == "no_std")
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_detect_frameworks_tokio() {
        let deps: HashSet<&str> = ["tokio", "serde"].into_iter().collect();
        let frameworks = detect_frameworks(&deps);
        assert!(frameworks.contains(&Framework::Tokio));
        assert!(!frameworks.contains(&Framework::Axum));
    }

    #[test]
    fn test_detect_frameworks_web_stack() {
        let deps: HashSet<&str> = ["tokio", "axum", "sqlx", "serde"].into_iter().collect();
        let frameworks = detect_frameworks(&deps);
        assert!(frameworks.contains(&Framework::Tokio));
        assert!(frameworks.contains(&Framework::Axum));
        assert!(frameworks.contains(&Framework::Sqlx));
    }

    #[test]
    fn test_detect_frameworks_embassy_prefix() {
        let deps: HashSet<&str> = ["embassy-executor", "embassy-time"].into_iter().collect();
        let frameworks = detect_frameworks(&deps);
        assert!(frameworks.contains(&Framework::Embassy));
    }

    #[test]
    fn test_detect_frameworks_cortex_m() {
        let deps: HashSet<&str> = ["cortex-m", "cortex-m-rt"].into_iter().collect();
        let frameworks = detect_frameworks(&deps);
        assert!(frameworks.contains(&Framework::CortexM));
    }

    #[test]
    fn test_detect_frameworks_empty() {
        let deps: HashSet<&str> = HashSet::new();
        let frameworks = detect_frameworks(&deps);
        assert!(frameworks.is_empty());
    }

    #[test]
    fn test_detect_frameworks_no_match() {
        let deps: HashSet<&str> = ["serde", "rand", "log"].into_iter().collect();
        let frameworks = detect_frameworks(&deps);
        assert!(frameworks.is_empty());
    }

    #[test]
    fn test_file_declares_no_std_true() {
        let dir = std::env::temp_dir().join("rust-doctor-test-no-std");
        let _ = std::fs::create_dir_all(&dir);
        let file_path = dir.join("lib.rs");
        let mut f = File::create(&file_path).unwrap();
        writeln!(f, "#![no_std]").unwrap();
        writeln!(f, "pub fn hello() {{}}").unwrap();
        drop(f);

        assert!(file_declares_no_std(&file_path));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_file_declares_no_std_false() {
        let dir = std::env::temp_dir().join("rust-doctor-test-std");
        let _ = std::fs::create_dir_all(&dir);
        let file_path = dir.join("lib.rs");
        let mut f = File::create(&file_path).unwrap();
        writeln!(f, "use std::io;").unwrap();
        writeln!(f, "pub fn hello() {{}}").unwrap();
        drop(f);

        assert!(!file_declares_no_std(&file_path));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_file_declares_no_std_with_comments() {
        let dir = std::env::temp_dir().join("rust-doctor-test-no-std-comments");
        let _ = std::fs::create_dir_all(&dir);
        let file_path = dir.join("lib.rs");
        let mut f = File::create(&file_path).unwrap();
        writeln!(f, "// Copyright 2026").unwrap();
        writeln!(f, "//! Crate documentation").unwrap();
        writeln!(f, "#![no_std]").unwrap();
        writeln!(f, "pub fn hello() {{}}").unwrap();
        drop(f);

        assert!(file_declares_no_std(&file_path));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_file_declares_no_std_beyond_line_10() {
        let dir = std::env::temp_dir().join("rust-doctor-test-no-std-late");
        let _ = std::fs::create_dir_all(&dir);
        let file_path = dir.join("lib.rs");
        let mut f = File::create(&file_path).unwrap();
        for i in 1..=11 {
            writeln!(f, "// Line {i}").unwrap();
        }
        writeln!(f, "#![no_std]").unwrap();
        drop(f);

        // no_std is on line 12, beyond the 10-line scan window
        assert!(!file_declares_no_std(&file_path));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_file_declares_no_std_nonexistent() {
        assert!(!file_declares_no_std(Path::new("/nonexistent/lib.rs")));
    }

    #[test]
    fn test_framework_display() {
        assert_eq!(Framework::Tokio.to_string(), "tokio");
        assert_eq!(Framework::ActixWeb.to_string(), "actix-web");
        assert_eq!(Framework::SeaOrm.to_string(), "sea-orm");
        assert_eq!(Framework::WasmBindgen.to_string(), "wasm-bindgen");
    }

    #[test]
    fn test_file_declares_no_std_with_internal_spaces() {
        let dir = std::env::temp_dir().join("rust-doctor-test-no-std-spaces");
        let _ = std::fs::create_dir_all(&dir);
        let file_path = dir.join("lib.rs");
        let mut f = File::create(&file_path).unwrap();
        writeln!(f, "#![ no_std ]").unwrap();
        drop(f);

        assert!(file_declares_no_std(&file_path));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_discover_project_on_self() {
        // Run discovery on rust-doctor itself
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let info = discover_project(&manifest, false).unwrap();

        assert_eq!(info.name, "rust-doctor");
        assert_eq!(info.version, "0.1.0");
        assert_eq!(info.edition, "2024");
        assert!(!info.is_workspace);
        assert_eq!(info.member_count, 1);
        assert!(!info.has_build_script);
        assert!(!info.is_no_std);
        assert!(info.frameworks.is_empty());
    }

    #[test]
    fn test_discover_project_bad_path() {
        let result = discover_project(Path::new("/nonexistent/Cargo.toml"), false);
        assert!(result.is_err());
    }
}
