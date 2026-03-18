//! End-to-end integration tests for the scan pipeline.
//!
//! These tests create temporary Rust projects with known violations
//! and verify that `scan_project` detects them correctly.

use rust_doctor::cli::FailOn;
use rust_doctor::config::ResolvedConfig;
use rust_doctor::diagnostics::Severity;
use rust_doctor::discovery::discover_project;
use rust_doctor::scan::scan_project;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Create a minimal Cargo.toml for a temporary test project.
fn write_cargo_toml(dir: &Path, name: &str) {
    let toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"
"#
    );
    fs::write(dir.join("Cargo.toml"), toml).unwrap();
}

/// Create a `ResolvedConfig` that enables custom rules but skips external tools.
fn fast_config() -> ResolvedConfig {
    ResolvedConfig {
        verbose: false,
        diff: None,
        fail_on: FailOn::None,
        ignore_rules: vec![],
        ignore_files: vec![],
        lint: true, // enable custom rules (+ clippy, but clippy won't affect assertions)
        dependencies: false, // skip audit + machete
        rules_config: std::collections::HashMap::new(),
        enable_rules: vec![],
        score_fail_below: None,
    }
}

#[test]
fn test_scan_detects_unwrap_in_production() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    write_cargo_toml(dir, "test-unwrap");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("src/lib.rs"),
        r#"
pub fn risky() -> String {
    std::env::var("HOME").unwrap()
}
"#,
    )
    .unwrap();

    let cargo_toml = dir.join("Cargo.toml");
    let project_info = discover_project(&cargo_toml, true).unwrap();
    let config = fast_config();

    let result = scan_project(&project_info, &config, true, &[], true).unwrap();

    let unwrap_diags: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule == "unwrap-in-production")
        .collect();

    assert!(
        !unwrap_diags.is_empty(),
        "Expected unwrap-in-production diagnostic but got none. All diags: {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| &d.rule)
            .collect::<Vec<_>>()
    );
    assert_eq!(unwrap_diags[0].severity, Severity::Warning);
}

#[test]
fn test_scan_detects_hardcoded_secret() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    write_cargo_toml(dir, "test-secret");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("src/lib.rs"),
        r#"
pub fn connect() {
    let api_key = "sk-1234567890abcdef1234567890abcdef";
    println!("{}", api_key);
}
"#,
    )
    .unwrap();

    let cargo_toml = dir.join("Cargo.toml");
    let project_info = discover_project(&cargo_toml, true).unwrap();
    let config = fast_config();

    let result = scan_project(&project_info, &config, true, &[], true).unwrap();

    let secret_diags: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule == "hardcoded-secrets")
        .collect();

    assert!(
        !secret_diags.is_empty(),
        "Expected hardcoded-secrets diagnostic but got none. All diags: {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| &d.rule)
            .collect::<Vec<_>>()
    );
    assert_eq!(secret_diags[0].severity, Severity::Error);
}

#[test]
fn test_scan_clean_project_has_no_custom_rule_violations() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    write_cargo_toml(dir, "test-clean");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("src/lib.rs"),
        r"
/// Add two numbers.
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
",
    )
    .unwrap();

    let cargo_toml = dir.join("Cargo.toml");
    let project_info = discover_project(&cargo_toml, true).unwrap();
    let config = fast_config();

    let result = scan_project(&project_info, &config, true, &[], true).unwrap();

    // Filter out clippy lints and informational pass diagnostics — only check custom rules
    let custom_diags: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| {
            !d.rule.starts_with("clippy::") && d.rule != "skipped-pass" && d.rule != "missing-msrv"
        })
        .collect();

    assert!(
        custom_diags.is_empty(),
        "Clean project should have no custom rule violations but got: {:?}",
        custom_diags.iter().map(|d| &d.rule).collect::<Vec<_>>()
    );
}

#[test]
fn test_scan_respects_ignored_rules() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    write_cargo_toml(dir, "test-ignore");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("src/lib.rs"),
        r#"
pub fn risky() -> String {
    std::env::var("HOME").unwrap()
}
"#,
    )
    .unwrap();

    let cargo_toml = dir.join("Cargo.toml");
    let project_info = discover_project(&cargo_toml, true).unwrap();
    let mut config = fast_config();
    config.ignore_rules = vec!["unwrap-in-production".to_string()];

    let result = scan_project(&project_info, &config, true, &[], true).unwrap();

    let unwrap_diags: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule == "unwrap-in-production")
        .collect();

    assert!(
        unwrap_diags.is_empty(),
        "unwrap-in-production should be ignored but found {unwrap_diags:?}"
    );
}
