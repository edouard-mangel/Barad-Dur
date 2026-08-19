use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::Path;

use crate::cache::history::archive_corrupt_file;
use crate::cache::storage::CACHE_DIR;

const ENTITY_HISTORY_FILE: &str = "entity_trends.json";
const BAK_FILE: &str = "entity_trends.json.bak";
const CORRUPT_REASON: &str = "entity_trends.json could not be read";
const SCHEMA_VERSION: u32 = 1;

/// One backfill sample's per-entity data: a bounded (top-N hotspots,
/// qualifying coupling pairs) slice of complexity/churn/coupling-degree,
/// keyed by path string (files) or sorted pair key (coupling pairs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityTrendEntry {
    pub timestamp: DateTime<Utc>,
    pub head: String,
    /// Recorded for a future branch-aware filtering feature. Unlike
    /// `compute_trend` in `src/trend.rs`, which filters aggregate-score
    /// history to same-branch entries before computing a trend,
    /// `attach_entity_trends` currently reads all history regardless of
    /// branch — this is a deliberate scope decision, not a bug.
    pub branch: String,
    /// path → cyclomatic_complexity, top-N hotspots only.
    pub complexity: HashMap<String, u32>,
    /// path → churn_count, same key set as `complexity`.
    pub churn: HashMap<String, u32>,
    /// "{path_a}|{path_b}" (sorted) → co_changes, qualifying smell pairs only.
    pub coupling_degree: HashMap<String, usize>,
    /// v1 is currently the only schema version and there is only one
    /// producer, so this is not validated on read (see `load_entity_history`).
    /// Revisit if/when the schema changes.
    pub schema_version: u32,
}

pub fn load_entity_history(repo_path: &Path) -> Result<Vec<EntityTrendEntry>> {
    let path = repo_path.join(CACHE_DIR).join(ENTITY_HISTORY_FILE);
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
        // `schema_version` is deserialized but not validated here — fine
        // while there's only one schema version (see `SCHEMA_VERSION`) and
        // one producer of this file.
        if let Ok(entry) = serde_json::from_str::<EntityTrendEntry>(&line) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

/// Load entity history, detecting total corruption the same way
/// `cache::history::load_history_checked` does (file exists and is
/// non-empty but produced zero valid entries).
pub fn load_entity_history_checked(
    repo_path: &Path,
) -> Result<(Vec<EntityTrendEntry>, Option<String>)> {
    let path = repo_path.join(CACHE_DIR).join(ENTITY_HISTORY_FILE);
    if !path.exists() {
        return Ok((Vec::new(), None));
    }

    let metadata = std::fs::metadata(&path)?;
    let file_is_nonempty = metadata.len() > 0;

    let entries = load_entity_history(repo_path)?;

    if file_is_nonempty && entries.is_empty() {
        let warning =
            archive_corrupt_file(repo_path, ENTITY_HISTORY_FILE, BAK_FILE, CORRUPT_REASON)?;
        return Ok((Vec::new(), Some(warning)));
    }

    Ok((entries, None))
}

pub fn append_entity_entry(entry: &EntityTrendEntry, repo_path: &Path) -> Result<()> {
    let path = repo_path.join(CACHE_DIR).join(ENTITY_HISTORY_FILE);

    std::fs::create_dir_all(repo_path.join(CACHE_DIR))?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    let json = serde_json::to_string(entry)?;
    writeln!(file, "{}", json)?;
    Ok(())
}

/// Deterministic key for an unordered file pair — the same pair always
/// produces the same key regardless of argument order (Decision 4).
pub(crate) fn entity_pair_key(a: &str, b: &str) -> String {
    if a <= b {
        format!("{a}|{b}")
    } else {
        format!("{b}|{a}")
    }
}

/// The top `top_n` hotspots by `hotspot_score`, descending. Returns all of
/// them when there are fewer than `top_n`.
fn select_top_hotspots(
    hotspots: &[crate::scorer::HotspotFile],
    top_n: usize,
) -> Vec<&crate::scorer::HotspotFile> {
    let mut sorted: Vec<&crate::scorer::HotspotFile> = hotspots.iter().collect();
    sorted.sort_by(|a, b| {
        b.hotspot_score
            .partial_cmp(&a.hotspot_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    sorted.truncate(top_n);
    sorted
}

/// Build one backfill sample's `EntityTrendEntry` from the snapshot (for
/// coupling-degree, via the shared `qualifying_smell_pairs` predicate) and
/// the already-computed hotspot list (for complexity/churn), bounded to the
/// top `top_n` hotspots by score.
pub fn build_entity_trend_entry(
    snapshot: &crate::snapshot::RepoSnapshot,
    hotspots: &[crate::scorer::HotspotFile],
    coupling: &crate::config::CouplingThresholds,
    top_n: usize,
    head: &str,
    timestamp: DateTime<Utc>,
    branch: &str,
) -> EntityTrendEntry {
    let top = select_top_hotspots(hotspots, top_n);

    let mut complexity = HashMap::new();
    let mut churn = HashMap::new();
    for h in top {
        complexity.insert(h.path.clone(), h.cyclomatic_complexity);
        churn.insert(h.path.clone(), h.churn_count as u32);
    }

    let mut coupling_degree = HashMap::new();
    for (a, b, co_changes) in crate::metrics::coupling::qualifying_smell_pairs(snapshot, coupling) {
        let key = entity_pair_key(&a.display().to_string(), &b.display().to_string());
        coupling_degree.insert(key, co_changes);
    }

    EntityTrendEntry {
        timestamp,
        head: head.to_string(),
        branch: branch.to_string(),
        complexity,
        churn,
        coupling_degree,
        schema_version: SCHEMA_VERSION,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_entry(head: &str) -> EntityTrendEntry {
        let mut complexity = HashMap::new();
        complexity.insert("src/big.rs".to_string(), 42);
        let mut churn = HashMap::new();
        churn.insert("src/big.rs".to_string(), 7);
        let mut coupling_degree = HashMap::new();
        coupling_degree.insert("src/a.rs|src/b.rs".to_string(), 3);
        EntityTrendEntry {
            timestamp: Utc::now(),
            head: head.to_string(),
            branch: "main".to_string(),
            complexity,
            churn,
            coupling_degree,
            schema_version: SCHEMA_VERSION,
        }
    }

    #[test]
    fn append_then_load_round_trips_byte_identical_data() {
        let dir = TempDir::new().unwrap();
        let entry = make_entry("abc123");
        append_entity_entry(&entry, dir.path()).unwrap();

        let loaded = load_entity_history(dir.path()).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].head, "abc123");
        assert_eq!(loaded[0].complexity, entry.complexity);
        assert_eq!(loaded[0].churn, entry.churn);
        assert_eq!(loaded[0].coupling_degree, entry.coupling_degree);
    }

    #[test]
    fn append_records_multiple_samples() {
        let dir = TempDir::new().unwrap();
        append_entity_entry(&make_entry("aaa"), dir.path()).unwrap();
        append_entity_entry(&make_entry("bbb"), dir.path()).unwrap();

        let loaded = load_entity_history(dir.path()).unwrap();
        assert_eq!(loaded.len(), 2);
    }

    #[test]
    fn load_entity_history_no_file_returns_empty() {
        let dir = TempDir::new().unwrap();
        assert!(load_entity_history(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn load_entity_history_checked_no_file_returns_empty_no_warning() {
        let dir = TempDir::new().unwrap();
        let (entries, warning) = load_entity_history_checked(dir.path()).unwrap();
        assert!(entries.is_empty());
        assert!(warning.is_none());
    }

    #[test]
    fn load_entity_history_checked_corrupt_file_triggers_archive_and_returns_warning() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(CACHE_DIR);
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(cache_dir.join(ENTITY_HISTORY_FILE), "NOT VALID JSON\n").unwrap();

        let (entries, warning) = load_entity_history_checked(dir.path()).unwrap();
        assert!(entries.is_empty());
        let w = warning.expect("corrupt file must produce a warning");
        assert!(
            w.contains("entity_trends.json"),
            "warning should name the file, got: {w}"
        );
        assert!(
            cache_dir.join(BAK_FILE).exists(),
            "corrupt file must be archived to .bak"
        );
    }

    #[test]
    fn load_entity_history_checked_empty_file_is_valid_not_corrupt() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(CACHE_DIR);
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(cache_dir.join(ENTITY_HISTORY_FILE), "").unwrap();

        let (entries, warning) = load_entity_history_checked(dir.path()).unwrap();
        assert!(entries.is_empty());
        assert!(warning.is_none(), "a zero-byte file is not corruption");
    }

    use crate::config::CouplingThresholds;
    use crate::scorer::HotspotFile;
    use crate::snapshot::{FileEntry, RepoSnapshot, TimeWindow};
    use std::path::PathBuf;

    fn hotspot(path: &str, score: f64, complexity: u32, churn: usize) -> HotspotFile {
        HotspotFile {
            path: path.to_string(),
            role: crate::metrics::file_role::FileRole::Source,
            churn_count: churn,
            bug_commit_count: 0,
            loc: 100,
            total_lines: 100,
            cyclomatic_complexity: complexity,
            public_methods: 0,
            properties: 0,
            hotspot_score: score,
            coupling_trend: None,
            content_findings: 0,
            common_findings: 0,
            control_findings: 0,
            inheritance_findings: 0,
            churn_timeline: vec![],
            complexity_trend: None,
            churn_trend: None,
        }
    }

    #[test]
    fn build_entity_trend_entry_selects_top_n_hotspots_by_score() {
        let snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let hotspots = vec![
            hotspot("low.rs", 10.0, 5, 1),
            hotspot("high.rs", 90.0, 50, 20),
            hotspot("mid.rs", 50.0, 20, 5),
        ];
        let entry = build_entity_trend_entry(
            &snapshot,
            &hotspots,
            &CouplingThresholds::default(),
            2, // top_n
            "abc123",
            Utc::now(),
            "main",
        );
        assert_eq!(entry.complexity.len(), 2, "only top 2 by hotspot_score");
        assert_eq!(entry.complexity.get("high.rs"), Some(&50));
        assert_eq!(entry.complexity.get("mid.rs"), Some(&20));
        assert!(!entry.complexity.contains_key("low.rs"));
        assert_eq!(entry.churn.get("high.rs"), Some(&20));
    }

    #[test]
    fn build_entity_trend_entry_returns_all_hotspots_when_fewer_than_top_n() {
        let snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let hotspots = vec![hotspot("only.rs", 10.0, 5, 1)];
        let entry = build_entity_trend_entry(
            &snapshot,
            &hotspots,
            &CouplingThresholds::default(),
            20,
            "abc123",
            Utc::now(),
            "main",
        );
        assert_eq!(entry.complexity.len(), 1);
    }

    #[test]
    fn build_entity_trend_entry_includes_qualifying_coupling_pairs() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        // Two files in different top-level components, co-changing on every
        // commit each was touched in — clears both the cross-boundary and
        // ratio-threshold parts of `qualifying_smell_pairs`' predicate.
        snapshot.files = vec![
            FileEntry {
                path: "src/a.rs".into(),
                size_bytes: 1,
                is_binary: false,
                depth: 2,
                blob_oid: String::new(),
            },
            FileEntry {
                path: "tests/b.rs".into(),
                size_bytes: 1,
                is_binary: false,
                depth: 2,
                blob_oid: String::new(),
            },
        ];
        snapshot.commits_by_file.insert(
            "src/a.rs".into(),
            (0..5).map(crate::snapshot::CommitId).collect(),
        );
        snapshot.commits_by_file.insert(
            "tests/b.rs".into(),
            (0..5).map(crate::snapshot::CommitId).collect(),
        );
        snapshot.file_change_pairs = vec![(
            PathBuf::from("src/a.rs"),
            PathBuf::from("tests/b.rs"),
            5, // co_changes == min_commits → ratio 1.0, clears any threshold
        )];

        let entry = build_entity_trend_entry(
            &snapshot,
            &[],
            &CouplingThresholds::default(),
            20,
            "abc123",
            Utc::now(),
            "main",
        );
        assert_eq!(entry.coupling_degree.len(), 1);
        assert_eq!(
            entry.coupling_degree.get("src/a.rs|tests/b.rs"),
            Some(&5),
            "pair key must be lexicographically sorted"
        );
    }

    #[test]
    fn entity_pair_key_sorts_lexicographically_regardless_of_argument_order() {
        assert_eq!(entity_pair_key("b.rs", "a.rs"), "a.rs|b.rs");
        assert_eq!(entity_pair_key("a.rs", "b.rs"), "a.rs|b.rs");
    }
}
