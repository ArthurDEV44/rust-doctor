//! Incremental scan cache for custom AST rules.
//!
//! Avoids re-running custom rules on files that haven't changed since the last
//! scan. The cache is stored as `.rust-doctor-cache.json` in the project root.
//! If the cache is missing, corrupt, or from a different config, a full scan is
//! performed and the cache is rebuilt.

use crate::diagnostics::Diagnostic;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Current cache format version. Bump this when the on-disk format changes.
const CACHE_VERSION: u32 = 1;

/// Name of the cache file written to the project root.
const CACHE_FILENAME: &str = ".rust-doctor-cache.json";

/// A cached entry for a single source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileEntry {
    /// Hash of the file content at the time of the last scan.
    hash: String,
    /// Diagnostics produced by custom rules for this file.
    diagnostics: Vec<Diagnostic>,
}

/// Incremental scan cache.
///
/// Tracks per-file content hashes and their associated custom-rule diagnostics
/// so unchanged files can skip re-analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanCache {
    /// Format version — must match [`CACHE_VERSION`] for the cache to be valid.
    version: u32,
    /// Hash of the resolved config (ignore_rules, ignore_files, enable_rules).
    /// When the config changes, the entire cache is invalidated.
    config_hash: String,
    /// Per-file cache entries keyed by relative path.
    files: HashMap<PathBuf, FileEntry>,
}

impl ScanCache {
    /// Create a new, empty cache for the given config hash.
    pub fn new(config_hash: String) -> Self {
        Self {
            version: CACHE_VERSION,
            config_hash,
            files: HashMap::new(),
        }
    }

    /// Load the cache from `.rust-doctor-cache.json` in `project_root`.
    ///
    /// Returns `None` if the file is missing, unreadable, malformed, has a
    /// wrong version, or a different config hash.
    pub fn load(project_root: &Path, config_hash: &str) -> Option<Self> {
        let cache_path = project_root.join(CACHE_FILENAME);
        let content = std::fs::read_to_string(&cache_path).ok()?;
        let cache: Self = serde_json::from_str(&content).ok()?;

        if cache.version != CACHE_VERSION {
            return None;
        }
        if cache.config_hash != config_hash {
            return None;
        }

        Some(cache)
    }

    /// Save the cache to `.rust-doctor-cache.json` in `project_root`.
    ///
    /// Errors are silently ignored — the cache is a best-effort optimisation.
    pub fn save(&self, project_root: &Path) {
        let cache_path = project_root.join(CACHE_FILENAME);
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&cache_path, json);
        }
    }

    /// Check whether the cached entry for `path` is still fresh (i.e. the
    /// content hash matches). Returns the computed hash for reuse by callers.
    pub fn is_fresh_with_hash(&self, path: &Path, content: &str) -> (bool, String) {
        let hash = hash_content(content);
        let fresh = self.files.get(path).is_some_and(|entry| entry.hash == hash);
        (fresh, hash)
    }

    /// Return cached diagnostics for `path` if the entry exists.
    ///
    /// Callers should verify freshness with [`is_fresh_with_hash`](Self::is_fresh_with_hash) first.
    pub fn get_cached_diagnostics(&self, path: &Path) -> Option<&[Diagnostic]> {
        self.files
            .get(path)
            .map(|entry| entry.diagnostics.as_slice())
    }

    /// Insert or update the cache entry for `path` with a pre-computed hash.
    pub fn update_with_hash(&mut self, path: &Path, hash: String, diagnostics: Vec<Diagnostic>) {
        self.files
            .insert(path.to_path_buf(), FileEntry { hash, diagnostics });
    }
}

/// Compute a hex-encoded hash of `content` using the standard library's
/// `DefaultHasher` (SipHash). This is not cryptographic, but perfectly adequate
/// for change detection.
pub fn hash_content(content: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Compute a config hash from the resolved config fields that affect custom
/// rule results. When any of these change, the entire cache is invalidated.
/// Compute a hash of the scan configuration and active rule set.
///
/// The hash includes ignore/enable lists AND the names of all active rules,
/// so adding or removing a rule invalidates the cache automatically.
pub fn compute_config_hash(
    ignore_rules: &[String],
    ignore_files: &[String],
    enable_rules: &[String],
    active_rule_names: &[&str],
) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    ignore_rules.hash(&mut hasher);
    ignore_files.hash(&mut hasher);
    enable_rules.hash(&mut hasher);
    active_rule_names.hash(&mut hasher);
    // Include the rust-doctor version so binary upgrades invalidate the cache
    env!("CARGO_PKG_VERSION").hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{Category, Severity};
    use std::path::PathBuf;

    fn sample_diagnostic(file: &str, rule: &str) -> Diagnostic {
        Diagnostic {
            file_path: PathBuf::from(file),
            rule: rule.to_string(),
            category: Category::ErrorHandling,
            severity: Severity::Warning,
            message: "test message".to_string(),
            help: None,
            line: Some(1),
            column: None,
            fix: None,
        }
    }

    // ── Test 1: Load/save roundtrip ─────────────────────────────────────

    #[test]
    fn test_load_save_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let config_hash = compute_config_hash(&[], &[], &[], &[]);

        let mut cache = ScanCache::new(config_hash.clone());
        let diag = sample_diagnostic("src/main.rs", "unwrap-in-production");
        cache.update_with_hash(
            Path::new("src/main.rs"),
            hash_content("fn main() {}"),
            vec![diag],
        );

        cache.save(dir.path());

        let loaded = ScanCache::load(dir.path(), &config_hash);
        assert!(loaded.is_some(), "cache should load successfully");
        let loaded = loaded.unwrap();

        assert_eq!(loaded.version, CACHE_VERSION);
        assert_eq!(loaded.config_hash, config_hash);
        assert!(loaded.files.contains_key(Path::new("src/main.rs")));

        let entry = loaded.files.get(Path::new("src/main.rs")).unwrap();
        assert_eq!(entry.diagnostics.len(), 1);
        assert_eq!(entry.diagnostics[0].rule, "unwrap-in-production");
    }

    // ── Test 2: Fresh file returns cached diagnostics ───────────────────

    #[test]
    fn test_fresh_file_returns_cached_diagnostics() {
        let content = "fn main() { println!(\"hello\"); }";
        let config_hash = compute_config_hash(&[], &[], &[], &[]);
        let mut cache = ScanCache::new(config_hash);

        let diag = sample_diagnostic("src/main.rs", "test-rule");
        cache.update_with_hash(Path::new("src/main.rs"), hash_content(content), vec![diag]);

        assert!(
            cache
                .is_fresh_with_hash(Path::new("src/main.rs"), content)
                .0
        );

        let cached = cache.get_cached_diagnostics(Path::new("src/main.rs"));
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 1);
        assert_eq!(cached.unwrap()[0].rule, "test-rule");
    }

    // ── Test 3: Modified file is detected as stale ──────────────────────

    #[test]
    fn test_modified_file_is_stale() {
        let original = "fn main() {}";
        let modified = "fn main() { todo!() }";
        let config_hash = compute_config_hash(&[], &[], &[], &[]);
        let mut cache = ScanCache::new(config_hash);

        cache.update_with_hash(
            Path::new("src/main.rs"),
            hash_content(original),
            vec![sample_diagnostic("src/main.rs", "r1")],
        );

        assert!(
            cache
                .is_fresh_with_hash(Path::new("src/main.rs"), original)
                .0
        );
        assert!(
            !cache
                .is_fresh_with_hash(Path::new("src/main.rs"), modified)
                .0
        );
    }

    // ── Test 4: Config change invalidates entire cache ──────────────────

    #[test]
    fn test_config_change_invalidates_cache() {
        let dir = tempfile::tempdir().unwrap();
        let hash_v1 = compute_config_hash(&["rule-a".to_string()], &[], &[], &[]);
        let hash_v2 = compute_config_hash(&["rule-b".to_string()], &[], &[], &[]);

        let mut cache = ScanCache::new(hash_v1.clone());
        cache.update_with_hash(
            Path::new("src/main.rs"),
            hash_content("fn main() {}"),
            vec![sample_diagnostic("src/main.rs", "r1")],
        );
        cache.save(dir.path());

        // Loading with the original hash works
        assert!(ScanCache::load(dir.path(), &hash_v1).is_some());

        // Loading with a different config hash returns None
        assert!(ScanCache::load(dir.path(), &hash_v2).is_none());
    }

    // ── Test 5: Missing cache file returns None ─────────────────────────

    #[test]
    fn test_missing_cache_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let config_hash = compute_config_hash(&[], &[], &[], &[]);
        let loaded = ScanCache::load(dir.path(), &config_hash);
        assert!(loaded.is_none());
    }

    // ── Test 6: Corrupt cache file returns None ─────────────────────────

    #[test]
    fn test_corrupt_cache_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join(CACHE_FILENAME);
        std::fs::write(&cache_path, "not valid json {{").unwrap();

        let config_hash = compute_config_hash(&[], &[], &[], &[]);
        let loaded = ScanCache::load(dir.path(), &config_hash);
        assert!(loaded.is_none());
    }

    // ── Test 7: Wrong version returns None ──────────────────────────────

    #[test]
    fn test_wrong_version_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let config_hash = compute_config_hash(&[], &[], &[], &[]);

        // Manually write a cache with wrong version
        let bad = serde_json::json!({
            "version": 999,
            "config_hash": config_hash,
            "files": {}
        });
        let cache_path = dir.path().join(CACHE_FILENAME);
        std::fs::write(&cache_path, serde_json::to_string(&bad).unwrap()).unwrap();

        let loaded = ScanCache::load(dir.path(), &config_hash);
        assert!(loaded.is_none());
    }

    // ── Test 8: hash_content is deterministic ───────────────────────────

    #[test]
    fn test_hash_content_deterministic() {
        let content = "fn main() { let x = 42; }";
        let h1 = hash_content(content);
        let h2 = hash_content(content);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16); // 64-bit hash => 16 hex chars
    }

    // ── Test 9: hash_content differs for different content ──────────────

    #[test]
    fn test_hash_content_differs() {
        let h1 = hash_content("fn main() {}");
        let h2 = hash_content("fn main() { todo!() }");
        assert_ne!(h1, h2);
    }

    // ── Test 10: get_cached_diagnostics for unknown path ────────────────

    #[test]
    fn test_get_cached_diagnostics_unknown_path() {
        let config_hash = compute_config_hash(&[], &[], &[], &[]);
        let cache = ScanCache::new(config_hash);
        assert!(
            cache
                .get_cached_diagnostics(Path::new("nonexistent.rs"))
                .is_none()
        );
    }

    // ── Test 11: update overwrites previous entry ───────────────────────

    #[test]
    fn test_update_overwrites_previous_entry() {
        let config_hash = compute_config_hash(&[], &[], &[], &[]);
        let mut cache = ScanCache::new(config_hash);

        cache.update_with_hash(
            Path::new("src/lib.rs"),
            hash_content("v1"),
            vec![sample_diagnostic("src/lib.rs", "rule-a")],
        );
        assert_eq!(
            cache
                .get_cached_diagnostics(Path::new("src/lib.rs"))
                .unwrap()
                .len(),
            1
        );

        cache.update_with_hash(
            Path::new("src/lib.rs"),
            hash_content("v2"),
            vec![
                sample_diagnostic("src/lib.rs", "rule-b"),
                sample_diagnostic("src/lib.rs", "rule-c"),
            ],
        );
        let diags = cache
            .get_cached_diagnostics(Path::new("src/lib.rs"))
            .unwrap();
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].rule, "rule-b");
    }
}
