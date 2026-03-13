use anyhow::Result;
use std::io::{BufRead, Write};
use std::path::Path;

use crate::cache::storage::CACHE_DIR;
use crate::scorer::HistoryEntry;

const HISTORY_FILE: &str = "history.json";

pub fn load_history(repo_path: &Path) -> Result<Vec<HistoryEntry>> {
    let path = repo_path.join(CACHE_DIR).join(HISTORY_FILE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = std::fs::File::open(&path)?;
    let reader = std::io::BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<HistoryEntry>(&line) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

pub fn append_if_new_head(entry: &HistoryEntry, repo_path: &Path) -> Result<()> {
    let path = repo_path.join(CACHE_DIR).join(HISTORY_FILE);

    // Check last entry's HEAD
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        if let Some(last_line) = content.lines().rev().find(|l| !l.trim().is_empty()) {
            if let Ok(last) = serde_json::from_str::<HistoryEntry>(last_line) {
                if last.head == entry.head {
                    return Ok(()); // Duplicate HEAD — skip
                }
            }
        }
    }

    std::fs::create_dir_all(repo_path.join(CACHE_DIR))?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    let json = serde_json::to_string(entry)?;
    writeln!(file, "{}", json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    use crate::scorer::HistoryCounts;

    fn make_entry(head: &str, score: u32) -> HistoryEntry {
        HistoryEntry {
            timestamp: chrono::Utc::now(),
            head: head.to_string(),
            overall_score: score,
            categories: HashMap::new(),
            metrics: HashMap::new(),
            counts: HistoryCounts {
                commits: 10,
                files: 50,
                authors: 3,
            },
        }
    }

    #[test]
    fn append_if_new_head_writes_entry() {
        let dir = TempDir::new().unwrap();
        let entry = make_entry("abc123", 72);
        append_if_new_head(&entry, dir.path()).unwrap();

        let history = load_history(dir.path()).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].head, "abc123");
    }

    #[test]
    fn append_if_new_head_skips_duplicate() {
        let dir = TempDir::new().unwrap();
        let entry = make_entry("abc123", 72);
        append_if_new_head(&entry, dir.path()).unwrap();
        append_if_new_head(&entry, dir.path()).unwrap();

        let history = load_history(dir.path()).unwrap();
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn append_different_heads() {
        let dir = TempDir::new().unwrap();
        append_if_new_head(&make_entry("aaa", 70), dir.path()).unwrap();
        append_if_new_head(&make_entry("bbb", 75), dir.path()).unwrap();

        let history = load_history(dir.path()).unwrap();
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn load_history_empty_file() {
        let dir = TempDir::new().unwrap();
        let history = load_history(dir.path()).unwrap();
        assert!(history.is_empty());
    }
}
