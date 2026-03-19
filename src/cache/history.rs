use anyhow::Result;
use std::io::{BufRead, Write};
use std::path::Path;

use crate::cache::storage::CACHE_DIR;
use crate::scorer::HistoryEntry;

const HISTORY_FILE: &str = "trends.json";
const BAK_FILE: &str = "trends.json.bak";

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

/// Load history, detecting total corruption (file exists and is non-empty but
/// produced zero valid entries). Returns the entries and an optional warning
/// string. When corruption is detected, calls archive_and_replace() and returns
/// the warning message so the caller can emit it.
pub fn load_history_checked(repo_path: &Path) -> Result<(Vec<HistoryEntry>, Option<String>)> {
    let path = repo_path.join(CACHE_DIR).join(HISTORY_FILE);
    if !path.exists() {
        return Ok((Vec::new(), None));
    }

    let metadata = std::fs::metadata(&path)?;
    let file_is_nonempty = metadata.len() > 0;

    let entries = load_history(repo_path)?;

    if file_is_nonempty && entries.is_empty() {
        let warning = archive_and_replace(repo_path)?;
        return Ok((Vec::new(), Some(warning)));
    }

    Ok((entries, None))
}

/// Rename trends.json to trends.json.bak (overwriting any prior .bak) and
/// create a fresh empty trends.json. Returns a warning string that the caller
/// should emit via println!/eprintln!.
pub fn archive_and_replace(repo_path: &Path) -> Result<String> {
    let cache_dir = repo_path.join(CACHE_DIR);
    let trends_path = cache_dir.join(HISTORY_FILE);
    let bak_path = cache_dir.join(BAK_FILE);

    std::fs::rename(&trends_path, &bak_path)?;
    std::fs::File::create(&trends_path)?;

    Ok("Warning: trends.json could not be read. The corrupt file has been archived to trends.json.bak and a fresh history has been started.".to_string())
}

pub fn append_if_new_head(entry: &HistoryEntry, repo_path: &Path) -> Result<()> {
    let path = repo_path.join(CACHE_DIR).join(HISTORY_FILE);

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

    #[test]
    fn archive_and_replace_moves_corrupt_file_to_bak_and_creates_empty() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(CACHE_DIR);
        std::fs::create_dir_all(&cache_dir).unwrap();

        let corrupt_content = "{ NOT VALID JSON @@@@";
        let trends_path = cache_dir.join(HISTORY_FILE);
        let bak_path = cache_dir.join(format!("{}.bak", HISTORY_FILE));
        std::fs::write(&trends_path, corrupt_content).unwrap();

        let warning = archive_and_replace(dir.path()).unwrap();

        // bak file contains the original corrupt content
        assert!(bak_path.exists(), "trends.json.bak should exist");
        let bak_content = std::fs::read_to_string(&bak_path).unwrap();
        assert_eq!(
            bak_content, corrupt_content,
            "bak should preserve corrupt content"
        );

        // trends.json exists and is empty
        assert!(trends_path.exists(), "trends.json should be recreated");
        let new_content = std::fs::read_to_string(&trends_path).unwrap();
        assert!(new_content.is_empty(), "new trends.json should be empty");

        // warning message contains the required phrase
        assert!(
            warning.contains("trends.json could not be read"),
            "warning should contain 'trends.json could not be read', got: {warning}"
        );
    }

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
            branch: String::new(),
            schema_version: 1,
            source: None,
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
    fn append_if_new_head_records_each_run() {
        // Each call to append_if_new_head records a new entry, even for the same SHA.
        // This preserves a complete run history so users can track repeated analyses.
        let dir = TempDir::new().unwrap();
        let entry = make_entry("abc123", 72);
        append_if_new_head(&entry, dir.path()).unwrap();
        append_if_new_head(&entry, dir.path()).unwrap();

        let history = load_history(dir.path()).unwrap();
        assert_eq!(history.len(), 2);
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
