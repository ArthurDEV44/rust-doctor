//! Generate and install the rust-doctor SKILL.md file.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The SKILL.md template bundled at compile time.
const SKILL_TEMPLATE: &str = include_str!("templates/skill.md");

/// Write the rust-doctor skill to the given skills directory.
///
/// Creates `<skills_dir>/rust-doctor/SKILL.md` and returns the full path.
pub fn write_skill(skills_dir: &Path) -> io::Result<PathBuf> {
    let target = skills_dir.join("rust-doctor");
    fs::create_dir_all(&target)?;

    let skill_path = target.join("SKILL.md");
    fs::write(&skill_path, SKILL_TEMPLATE)?;

    Ok(skill_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_is_not_empty() {
        assert!(!SKILL_TEMPLATE.is_empty());
        assert!(SKILL_TEMPLATE.contains("rust-doctor"));
    }

    #[test]
    fn writes_skill_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_skill(dir.path()).unwrap();

        assert!(path.exists());
        assert!(path.ends_with("rust-doctor/SKILL.md"));

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("name: rust-doctor"));
    }
}
