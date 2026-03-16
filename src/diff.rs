use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Result of diff mode resolution.
pub struct DiffContext {
    /// Base branch or commit used for diff.
    pub base: String,
    /// Set of changed `.rs` file paths (relative to project root).
    pub changed_files: HashSet<PathBuf>,
}

/// Resolve the diff base and collect changed `.rs` files.
///
/// `base_hint` is the user-provided `--diff` value:
/// - `"auto"` → auto-detect via `git merge-base HEAD main` then `master`
/// - any other string → use as the base branch/commit directly
///
/// Returns `Ok(DiffContext)` on success, or `Err` with a user-facing message.
pub fn resolve_diff(project_root: &Path, base_hint: &str) -> Result<DiffContext, String> {
    // Check if we're in a git repo
    if !is_git_repo(project_root) {
        return Err("Diff mode requires a git repository — falling back to full scan".into());
    }

    let base = if base_hint == "auto" {
        auto_detect_base(project_root)?
    } else {
        // Verify the branch/commit exists
        verify_ref_exists(project_root, base_hint)?;
        base_hint.to_string()
    };

    let changed_files = get_changed_rs_files(project_root, &base)?;

    Ok(DiffContext {
        base,
        changed_files,
    })
}

/// Check if the directory is inside a git repository.
fn is_git_repo(dir: &Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Auto-detect the base branch by trying `main` then `master`.
fn auto_detect_base(dir: &Path) -> Result<String, String> {
    // Try merge-base with main
    for branch in ["main", "master"] {
        let output = Command::new("git")
            .args(["merge-base", "HEAD", branch])
            .current_dir(dir)
            .output();

        if let Ok(output) = output
            && output.status.success()
        {
            let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !commit.is_empty() {
                return Ok(commit);
            }
        }
    }

    // Fallback: use HEAD~1
    let output = Command::new("git")
        .args(["rev-parse", "HEAD~1"])
        .current_dir(dir)
        .output()
        .map_err(|e| format!("git rev-parse failed: {e}"))?;

    if output.status.success() {
        let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !commit.is_empty() {
            return Ok(commit);
        }
    }

    Err("Could not auto-detect base branch. Specify one with `--diff <branch>`".into())
}

/// Verify a git ref (branch, tag, commit) exists.
fn verify_ref_exists(dir: &Path, ref_name: &str) -> Result<(), String> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", ref_name])
        .current_dir(dir)
        .output()
        .map_err(|e| format!("git rev-parse failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Base ref '{ref_name}' not found. Try `--diff main` or `--diff HEAD~1`"
        ));
    }
    Ok(())
}

/// Get the list of changed `.rs` files between base and HEAD.
fn get_changed_rs_files(dir: &Path, base: &str) -> Result<HashSet<PathBuf>, String> {
    let output = Command::new("git")
        .args(["diff", "--name-only", &format!("{base}...HEAD")])
        .current_dir(dir)
        .output()
        .map_err(|e| format!("git diff failed: {e}"))?;

    if !output.status.success() {
        // Fallback: try without the triple-dot syntax (for single commits)
        let output = Command::new("git")
            .args(["diff", "--name-only", base, "HEAD"])
            .current_dir(dir)
            .output()
            .map_err(|e| format!("git diff failed: {e}"))?;

        if !output.status.success() {
            return Err(format!("git diff failed for base '{base}'"));
        }

        return Ok(parse_changed_rs_files(&output.stdout));
    }

    Ok(parse_changed_rs_files(&output.stdout))
}

fn parse_changed_rs_files(output: &[u8]) -> HashSet<PathBuf> {
    String::from_utf8_lossy(output)
        .lines()
        .filter(|line| line.ends_with(".rs"))
        .map(|line| PathBuf::from(line.trim()))
        .collect()
}

/// Filter diagnostics to only include those from changed files.
pub fn filter_to_changed_files(
    diagnostics: Vec<crate::diagnostics::Diagnostic>,
    changed_files: &HashSet<PathBuf>,
) -> Vec<crate::diagnostics::Diagnostic> {
    diagnostics
        .into_iter()
        .filter(|d| {
            // Match by exact path or by filename suffix
            changed_files.contains(&d.file_path)
                || changed_files
                    .iter()
                    .any(|cf| d.file_path.ends_with(cf) || cf.ends_with(&d.file_path))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_changed_rs_files() {
        let output = b"src/main.rs\nsrc/lib.rs\nREADME.md\ntests/test.rs\nCargo.toml\n";
        let files = parse_changed_rs_files(output);
        assert_eq!(files.len(), 3);
        assert!(files.contains(&PathBuf::from("src/main.rs")));
        assert!(files.contains(&PathBuf::from("src/lib.rs")));
        assert!(files.contains(&PathBuf::from("tests/test.rs")));
    }

    #[test]
    fn test_parse_empty_output() {
        let files = parse_changed_rs_files(b"");
        assert!(files.is_empty());
    }

    #[test]
    fn test_parse_no_rs_files() {
        let output = b"README.md\nCargo.toml\n.gitignore\n";
        let files = parse_changed_rs_files(output);
        assert!(files.is_empty());
    }

    #[test]
    fn test_filter_to_changed_files() {
        use crate::diagnostics::{Category, Diagnostic, Severity};

        let diags = vec![
            Diagnostic {
                file_path: PathBuf::from("src/main.rs"),
                rule: "test".into(),
                category: Category::Style,
                severity: Severity::Warning,
                message: "test".into(),
                help: None,
                line: Some(1),
                column: None,
            },
            Diagnostic {
                file_path: PathBuf::from("src/lib.rs"),
                rule: "test".into(),
                category: Category::Style,
                severity: Severity::Warning,
                message: "test".into(),
                help: None,
                line: Some(1),
                column: None,
            },
        ];

        let changed: HashSet<PathBuf> = ["src/main.rs"].iter().map(PathBuf::from).collect();
        let filtered = filter_to_changed_files(diags, &changed);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].file_path, PathBuf::from("src/main.rs"));
    }

    #[test]
    fn test_is_git_repo_on_self() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(is_git_repo(manifest_dir));
    }

    #[test]
    fn test_is_git_repo_on_tmp() {
        assert!(!is_git_repo(Path::new("/tmp")));
    }
}
