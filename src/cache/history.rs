use anyhow::Result;
use std::io::{BufRead, Write};
use std::path::Path;

use crate::cache::storage::CACHE_DIR;
use crate::scorer::{HistoryEntry, HISTORY_SCHEMA_VERSION};

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
        let warning = archive_and_replace(repo_path, CORRUPT_REASON)?;
        return Ok((Vec::new(), Some(warning)));
    }

    // Scores from an older formula are a stale computation, not history:
    // `backfill` recomputes the whole series from git. Keeping them would
    // make the next run report a formula change as a code change.
    if entries
        .iter()
        .any(|entry| entry.schema_version < HISTORY_SCHEMA_VERSION)
    {
        let warning = archive_and_replace(repo_path, STALE_SCORING_REASON)?;
        return Ok((Vec::new(), Some(warning)));
    }

    Ok((entries, None))
}

const CORRUPT_REASON: &str = "trends.json could not be read";
const STALE_SCORING_REASON: &str =
    "trends.json holds scores from an older scoring formula; run `barad-dur backfill` to \
     regenerate the series";

/// Rename trends.json to trends.json.bak (overwriting any prior .bak) and
/// create a fresh empty trends.json. Returns a warning string that the caller
/// should emit via println!/eprintln!.
///
/// `reason` names why the file was set aside — it is the part of the warning
/// that tells the user whether anything is wrong or the history simply needs
/// regenerating. Nothing is deleted: the previous file remains as .bak.
pub fn archive_and_replace(repo_path: &Path, reason: &str) -> Result<String> {
    let cache_dir = repo_path.join(CACHE_DIR);
    let trends_path = cache_dir.join(HISTORY_FILE);
    let bak_path = cache_dir.join(BAK_FILE);

    std::fs::rename(&trends_path, &bak_path)?;
    std::fs::File::create(&trends_path)?;

    Ok(format!(
        "Warning: {reason}. The previous file has been archived to \
         trends.json.bak and a fresh history has been started."
    ))
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

        let warning = archive_and_replace(dir.path(), CORRUPT_REASON).unwrap();

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
                ..Default::default()
            },
            branch: String::new(),
            schema_version: HISTORY_SCHEMA_VERSION,
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

    // --- load_history_checked ---

    #[test]
    fn load_history_checked_no_file_returns_empty_no_warning() {
        let dir = TempDir::new().unwrap();
        let (entries, warning) = load_history_checked(dir.path()).unwrap();
        assert!(entries.is_empty(), "no file → no entries");
        assert!(warning.is_none(), "no file → no warning");
    }

    #[test]
    fn load_history_checked_empty_file_returns_empty_no_warning() {
        // A zero-byte trends.json is valid (just no entries yet); must not trigger archiving.
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(CACHE_DIR);
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(cache_dir.join(HISTORY_FILE), "").unwrap();

        let (entries, warning) = load_history_checked(dir.path()).unwrap();
        assert!(entries.is_empty(), "empty file → no entries");
        assert!(warning.is_none(), "empty file is valid → no warning");
    }

    #[test]
    fn load_history_checked_valid_entries_returns_them_no_warning() {
        let dir = TempDir::new().unwrap();
        append_if_new_head(&make_entry("aaa", 80), dir.path()).unwrap();
        append_if_new_head(&make_entry("bbb", 90), dir.path()).unwrap();

        let (entries, warning) = load_history_checked(dir.path()).unwrap();
        assert_eq!(entries.len(), 2, "valid file → both entries returned");
        assert!(warning.is_none(), "valid file → no warning");
    }

    #[test]
    fn load_history_checked_corrupt_file_triggers_archive_and_returns_warning() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(CACHE_DIR);
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(cache_dir.join(HISTORY_FILE), "NOT VALID JSON\n").unwrap();

        let (entries, warning) = load_history_checked(dir.path()).unwrap();
        assert!(entries.is_empty(), "corrupt file → no entries");
        let w = warning.expect("corrupt file → warning should be Some");
        assert!(
            w.contains("trends.json could not be read"),
            "warning should name the file, got: {w}"
        );
        assert!(
            cache_dir.join(BAK_FILE).exists(),
            "archive_and_replace should have created .bak"
        );
    }

    #[test]
    fn history_from_an_older_scoring_version_is_archived_not_trusted() {
        // Entries are a cache of what the *current* formula says about past
        // commits, not a record of what was observed. After a scoring
        // change they are a stale computation, and mixing them with fresh
        // ones would report a formula change as if the code had moved.
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(CACHE_DIR);
        std::fs::create_dir_all(&cache_dir).unwrap();
        let stale = serde_json::json!({
            "timestamp": "2026-01-01T00:00:00Z",
            "head": "abc123",
            "overall_score": 80,
            "category_scores": {},
            "metrics": {},
            "counts": {"commits": 1, "files": 2, "authors": 3},
            "branch": "main",
            "schema_version": HISTORY_SCHEMA_VERSION - 1,
        });
        std::fs::write(cache_dir.join(HISTORY_FILE), format!("{stale}\n")).unwrap();

        let (entries, warning) = load_history_checked(dir.path()).unwrap();
        assert!(entries.is_empty(), "stale-formula entries must not load");
        let w = warning.expect("a version bump must warn");
        assert!(
            w.contains("backfill"),
            "the warning must name the command that regenerates history, got: {w}"
        );
        assert!(
            cache_dir.join(BAK_FILE).exists(),
            "the old history must be archived, not destroyed"
        );
    }

    #[test]
    fn history_at_the_current_scoring_version_loads_untouched() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(CACHE_DIR);
        std::fs::create_dir_all(&cache_dir).unwrap();
        append_if_new_head(&make_entry("abc123", 80), dir.path()).unwrap();

        let (entries, warning) = load_history_checked(dir.path()).unwrap();
        assert_eq!(entries.len(), 1, "current-version history must survive");
        assert!(warning.is_none(), "no warning for current-version history");
        assert!(!cache_dir.join(BAK_FILE).exists(), "nothing to archive");
    }

    #[test]
    fn old_trends_json_without_coupling_counts_still_loads() {
        // Serialize an entry built from HistoryCounts::default() minus the new
        // fields by writing a raw JSON string with only commits/files/authors,
        // deserialize, and assert the Option fields default to None.
        let raw = r#"[{"timestamp":"2026-01-01T00:00:00Z","head":"abc","overall_score":80,
        "category_scores":{},"metrics":{},"counts":{"commits":1,"files":2,"authors":3},
        "branch":"main","schema_version":1}]"#;
        let entries: Vec<HistoryEntry> = serde_json::from_str(raw).unwrap();
        assert_eq!(entries[0].counts.content_coupling, None);
        assert_eq!(entries[0].counts.commits, 1);
    }
}
