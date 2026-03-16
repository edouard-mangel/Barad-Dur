use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::snapshot::RepoSnapshot;

pub const CACHE_DIR: &str = ".repository-analysis";
const CACHE_FILE: &str = "snapshot.bin";

/// Save a snapshot to the cache directory.
pub fn save(snapshot: &RepoSnapshot, repo_path: &Path) -> Result<()> {
    let cache_dir = repo_path.join(CACHE_DIR);
    fs::create_dir_all(&cache_dir)?;
    let data = bincode::serialize(snapshot)?;
    fs::write(cache_dir.join(CACHE_FILE), data)?;
    ensure_gitignore(repo_path)?;
    Ok(())
}

/// Load a snapshot from the cache directory. Returns None if no cache exists.
/// Silently deletes corrupt caches.
pub fn load(repo_path: &Path) -> Result<Option<RepoSnapshot>> {
    let cache_file = repo_path.join(CACHE_DIR).join(CACHE_FILE);
    if !cache_file.exists() {
        return Ok(None);
    }
    let data = fs::read(&cache_file)?;
    match bincode::deserialize(&data) {
        Ok(snapshot) => Ok(Some(snapshot)),
        Err(_) => {
            // Corrupt cache — delete and return None
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
        writeln!(file, "{}", entry)?;
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
    fn ensure_gitignore_does_not_duplicate() {
        let dir = TempDir::new().unwrap();
        ensure_gitignore(dir.path()).unwrap();
        ensure_gitignore(dir.path()).unwrap();

        let content = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        let count = content.lines().filter(|l| l.trim() == ".repository-analysis/").count();
        assert_eq!(count, 1, "Should not duplicate .repository-analysis/ entry");
    }
}
