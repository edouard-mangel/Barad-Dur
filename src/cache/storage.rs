use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::snapshot::RepoSnapshot;

pub const CACHE_DIR: &str = ".repository-analysis";
const CACHE_FILE: &str = "snapshot.bin";
const FINGERPRINT_FILE: &str = "exclude.fingerprint";

/// Bumped whenever `RepoSnapshot`'s serialized shape changes. Bincode is
/// positional: a mid-struct field addition can garbage-parse instead of
/// failing, silently serving stale data. The explicit version makes
/// invalidation deterministic. History: 1 = post-M1 shape (coupling_findings);
/// 2 = M7 (class_records); 3 = bincode 2 wire format (varint).
const CACHE_VERSION: u32 = 3;

/// Wire format for all cache files. Bincode 2's standard config (varint
/// lengths) — not compatible with the bincode 1 fixint files of versions ≤ 2.
pub(super) fn wire_config() -> bincode::config::Configuration {
    bincode::config::standard()
}

/// Save a snapshot to the cache directory.
pub fn save(snapshot: &RepoSnapshot, repo_path: &Path) -> Result<()> {
    let cache_dir = repo_path.join(CACHE_DIR);
    fs::create_dir_all(&cache_dir)?;
    let data = bincode::serde::encode_to_vec((CACHE_VERSION, snapshot), wire_config())?;
    fs::write(cache_dir.join(CACHE_FILE), data)?;
    ensure_gitignore(repo_path)?;
    Ok(())
}

/// Record the exclusion fingerprint that produced the cached snapshot, so a later
/// run can detect when exclusion inputs changed even though HEAD did not.
pub fn save_exclude_fingerprint(repo_path: &Path, fingerprint: u64) -> Result<()> {
    let cache_dir = repo_path.join(CACHE_DIR);
    fs::create_dir_all(&cache_dir)?;
    fs::write(cache_dir.join(FINGERPRINT_FILE), fingerprint.to_string())?;
    Ok(())
}

/// Whether the recorded exclusion fingerprint equals `fingerprint`. A missing or
/// unreadable fingerprint counts as a mismatch, forcing a fresh collection.
pub fn exclude_fingerprint_matches(repo_path: &Path, fingerprint: u64) -> bool {
    fs::read_to_string(repo_path.join(CACHE_DIR).join(FINGERPRINT_FILE))
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .is_some_and(|stored| stored == fingerprint)
}

/// Load a snapshot from the cache directory. Returns None if no cache exists.
/// Silently deletes corrupt or outdated caches.
pub fn load(repo_path: &Path) -> Result<Option<RepoSnapshot>> {
    let cache_file = repo_path.join(CACHE_DIR).join(CACHE_FILE);
    if !cache_file.exists() {
        return Ok(None);
    }
    let data = fs::read(&cache_file)?;
    match bincode::serde::decode_from_slice::<(u32, RepoSnapshot), _>(&data, wire_config()) {
        Ok(((CACHE_VERSION, snapshot), _)) => Ok(Some(snapshot)),
        // Wrong version or corrupt — delete and re-collect.
        Ok(_) | Err(_) => {
            let _ = fs::remove_file(&cache_file);
            Ok(None)
        }
    }
}

/// Ensure .repository-analysis/ is in .gitignore.
fn ensure_gitignore(repo_path: &Path) -> Result<()> {
    let gitignore_path = repo_path.join(".gitignore");
    let entry = ".repository-analysis/";

    if gitignore_path.exists() {
        let content = fs::read_to_string(&gitignore_path)?;
        if content.lines().any(|line| line.trim() == entry) {
            return Ok(());
        }
        let mut file = fs::OpenOptions::new().append(true).open(&gitignore_path)?;
        if content.ends_with('\n') || content.is_empty() {
            writeln!(file, "{}", entry)?;
        } else {
            writeln!(file, "\n{}", entry)?;
        }
    } else {
        fs::write(&gitignore_path, format!("{}\n", entry))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::TimeWindow;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_test_snapshot() -> RepoSnapshot {
        RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test-repo".to_string(),
            "main".to_string(),
            TimeWindow::default(),
        )
    }

    #[test]
    fn save_creates_cache_file() {
        let dir = TempDir::new().unwrap();
        let snapshot = make_test_snapshot();
        save(&snapshot, dir.path()).unwrap();

        let cache_file = dir.path().join(CACHE_DIR).join(CACHE_FILE);
        assert!(cache_file.exists());
    }

    #[test]
    fn exclude_fingerprint_roundtrip_and_mismatch() {
        let dir = TempDir::new().unwrap();
        // No fingerprint recorded yet → mismatch (forces re-collection).
        assert!(!exclude_fingerprint_matches(dir.path(), 42));
        save_exclude_fingerprint(dir.path(), 42).unwrap();
        assert!(exclude_fingerprint_matches(dir.path(), 42));
        // A different fingerprint (changed exclusions) → mismatch.
        assert!(!exclude_fingerprint_matches(dir.path(), 99));
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let snapshot = make_test_snapshot();
        save(&snapshot, dir.path()).unwrap();

        let loaded = load(dir.path()).unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.name, "test-repo");
        assert_eq!(loaded.default_branch, "main");
    }

    #[test]
    fn load_nonexistent_returns_none() {
        let dir = TempDir::new().unwrap();
        let loaded = load(dir.path()).unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn load_corrupt_file_returns_none_and_deletes() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(CACHE_DIR);
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(cache_dir.join(CACHE_FILE), b"corrupt data").unwrap();

        let loaded = load(dir.path()).unwrap();
        assert!(loaded.is_none());
        // File should be deleted
        assert!(!cache_dir.join(CACHE_FILE).exists());
    }

    #[test]
    fn ensure_gitignore_adds_entry() {
        let dir = TempDir::new().unwrap();
        ensure_gitignore(dir.path()).unwrap();

        let content = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(content.contains(".repository-analysis/"));
    }

    #[test]
    fn entry_is_on_own_line_when_gitignore_has_no_trailing_newline() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitignore"), "*.log").unwrap();
        ensure_gitignore(dir.path()).unwrap();
        let content = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(
            content.lines().any(|l| l.trim() == ".repository-analysis/"),
            ".repository-analysis/ must be on its own line, got: {:?}",
            content
        );
    }

    #[test]
    fn entry_does_not_add_blank_line_when_gitignore_ends_with_newline() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitignore"), "*.log\n").unwrap();
        ensure_gitignore(dir.path()).unwrap();
        let content = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(content, "*.log\n.repository-analysis/\n");
    }

    #[test]
    fn ensure_gitignore_does_not_duplicate() {
        let dir = TempDir::new().unwrap();
        ensure_gitignore(dir.path()).unwrap();
        ensure_gitignore(dir.path()).unwrap();

        let content = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        let count = content
            .lines()
            .filter(|l| l.trim() == ".repository-analysis/")
            .count();
        assert_eq!(count, 1, "Should not duplicate .repository-analysis/ entry");
    }

    #[test]
    fn snapshot_roundtrips_coupling_findings() {
        use crate::snapshot::{CouplingFinding, CouplingKind};
        let dir = TempDir::new().unwrap();
        let mut snapshot = make_test_snapshot();
        snapshot.coupling_findings.push(CouplingFinding {
            path: PathBuf::from("src/lib.rs"),
            line: Some(42),
            kind: CouplingKind::Common,
            evidence: "static mut CACHE: usize = 0;".into(),
        });
        save(&snapshot, dir.path()).unwrap();
        let loaded = load(dir.path()).unwrap().unwrap();
        assert_eq!(loaded.coupling_findings.len(), 1);
        assert_eq!(loaded.coupling_findings[0].kind, CouplingKind::Common);
        assert_eq!(loaded.coupling_findings[0].line, Some(42));
        assert_eq!(
            loaded.coupling_findings[0].evidence,
            "static mut CACHE: usize = 0;"
        );
    }

    #[test]
    fn load_rejects_mismatched_cache_version() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(CACHE_DIR);
        fs::create_dir_all(&cache_dir).unwrap();
        let stale =
            bincode::serde::encode_to_vec(&(0u32, make_test_snapshot()), wire_config()).unwrap();
        fs::write(cache_dir.join(CACHE_FILE), stale).unwrap();
        assert!(load(dir.path()).unwrap().is_none());
        assert!(
            !cache_dir.join(CACHE_FILE).exists(),
            "stale-version cache must be deleted like a corrupt one"
        );
    }

    #[test]
    fn load_rejects_unversioned_legacy_cache() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(CACHE_DIR);
        fs::create_dir_all(&cache_dir).unwrap();
        let legacy = bincode::serde::encode_to_vec(make_test_snapshot(), wire_config()).unwrap();
        fs::write(cache_dir.join(CACHE_FILE), legacy).unwrap();
        assert!(load(dir.path()).unwrap().is_none());
    }
}
