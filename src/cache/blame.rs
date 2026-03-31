use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::snapshot::BlameLine;

use super::storage::CACHE_DIR;

const BLAME_CACHE_FILE: &str = "blame_cache.bin";

/// Content-addressed blame cache: blob OID → blame lines.
/// Lives in `.repository-analysis/blame_cache.bin`, independent of the snapshot cache.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BlameCache {
    pub entries: HashMap<String, Vec<BlameLine>>,
}

impl BlameCache {
    /// Remove entries whose blob OID is not in the current file tree.
    pub fn prune(&mut self, current_blob_oids: &HashSet<String>) {
        self.entries
            .retain(|oid, _| current_blob_oids.contains(oid));
    }
}

pub fn load(repo_path: &Path) -> Result<BlameCache> {
    let cache_file = repo_path.join(CACHE_DIR).join(BLAME_CACHE_FILE);
    if !cache_file.exists() {
        return Ok(BlameCache::default());
    }
    let data = std::fs::read(&cache_file)?;
    match bincode::deserialize(&data) {
        Ok(cache) => Ok(cache),
        Err(_) => {
            let _ = std::fs::remove_file(&cache_file);
            Ok(BlameCache::default())
        }
    }
}

pub fn save(cache: &BlameCache, repo_path: &Path) -> Result<()> {
    let cache_dir = repo_path.join(CACHE_DIR);
    std::fs::create_dir_all(&cache_dir)?;
    let data = bincode::serialize(cache)?;
    std::fs::write(cache_dir.join(BLAME_CACHE_FILE), data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::TempDir;

    fn make_blame_line() -> BlameLine {
        BlameLine {
            author_id: 0,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn blame_cache_save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let mut cache = BlameCache::default();
        cache
            .entries
            .insert("a".repeat(40), vec![make_blame_line()]);
        save(&cache, dir.path()).unwrap();

        let loaded = load(dir.path()).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert!(loaded.entries.contains_key(&"a".repeat(40)));
    }

    #[test]
    fn blame_cache_load_missing_returns_empty() {
        let dir = TempDir::new().unwrap();
        let loaded = load(dir.path()).unwrap();
        assert!(loaded.entries.is_empty());
    }

    #[test]
    fn blame_cache_prune_removes_stale_entries() {
        let mut cache = BlameCache::default();
        cache
            .entries
            .insert("keep".repeat(10), vec![make_blame_line()]);
        cache
            .entries
            .insert("gone".repeat(10), vec![make_blame_line()]);

        let current_oids: HashSet<String> = vec!["keep".repeat(10)].into_iter().collect();
        cache.prune(&current_oids);

        assert_eq!(cache.entries.len(), 1);
        assert!(cache.entries.contains_key(&"keep".repeat(10)));
    }
}
