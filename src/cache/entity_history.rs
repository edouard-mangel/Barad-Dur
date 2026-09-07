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
///
/// Ranked over the window backfill collects with, which is
/// `TimeWindow::full_history()` — not the window `analyze` displays hotspots
/// over (180 days by default). The two populations therefore differ, so a
/// currently-hot file can carry no direction at all. Tracked as deferred work
/// in the per-entity trend design; the fix is a scoring-semantics change that
/// also moves `gate`'s baseline, not a change to this function.
fn select_top_hotspots(
    hotspots: &[crate::scorer::HotspotFile],
    top_n: usize,
) -> Vec<&crate::scorer::HotspotFile> {
    // The sort is deliberate, not redundant. `build_hotspots` happens to
    // return its list already sorted by `hotspot_score`, but that is the
    // caller's current behaviour, not this function's contract: it selects
    // the top N by score from whatever it is given, and
    // `build_entity_trend_entry_selects_top_n_hotspots_by_score` passes an
    // unsorted list precisely to hold that line. Cost is one sort of the file
    // list per sample.
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

    // Bounded the same way the hotspot maps are. `qualifying_smell_pairs` is
    // not bounded by construction — pair count is O(n^2) in the limit — so
    // without a cap one sample's line grows with how coupled the repo is
    // (666 keys, 39 KB on a single line, measured on this repository). Keep
    // the most-coupled pairs, which are the ones a coupling trend is about.
    let mut ranked: Vec<(String, usize)> =
        crate::metrics::coupling::qualifying_smell_pairs(snapshot, coupling)
            .map(|(a, b, co_changes)| {
                (
                    entity_pair_key(&a.display().to_string(), &b.display().to_string()),
                    co_changes,
                )
            })
            .collect();
    // Ties broken on the key so the persisted set is deterministic run to run.
    ranked.sort_by(|(ka, ca), (kb, cb)| cb.cmp(ca).then_with(|| ka.cmp(kb)));
    ranked.truncate(top_n);
    let coupling_degree: HashMap<String, usize> = ranked.into_iter().collect();

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

/// Attach a `Growing`/`Shrinking`/`Stable` direction to every hotspot and
/// coupling pair that has at least 2 points of history — matched by path
/// (hotspots) or sorted pair key (coupling pairs). Entities absent from
/// `history` (never in a backfill sample's top-N, or no history exists yet)
/// are left as `None`, same "empty state" contract as `TrendSummary`.
pub fn attach_entity_trends(
    hotspots: &mut [crate::scorer::HotspotFile],
    pairs: &mut [crate::scorer::CouplingPair],
    history: &[EntityTrendEntry],
) {
    // `history`'s write order is not guaranteed chronological (see
    // `select_samples`'s `count >= len` shortcut and multi-run appends), so
    // sort a local copy by timestamp once, up front, and have every loop
    // below read from it — `compute_entity_trend` relies on `series.first()`
    // being the oldest point and `series.last()` being the newest.
    let mut ordered: Vec<&EntityTrendEntry> = history.iter().collect();
    ordered.sort_by_key(|e| e.timestamp);

    for h in hotspots.iter_mut() {
        let complexity_series: Vec<f64> = ordered
            .iter()
            .filter_map(|e| e.complexity.get(&h.path).copied())
            .map(|c| c as f64)
            .collect();
        if complexity_series.len() >= 2 {
            h.complexity_trend = Some(crate::trend::compute_entity_trend(&complexity_series));
        }

        // Cumulative: classify the per-period rate, not the running total.
        let churn_rates = crate::trend::period_rates(
            &ordered
                .iter()
                .filter_map(|e| e.churn.get(&h.path).copied())
                .map(|c| c as f64)
                .collect::<Vec<f64>>(),
        );
        if churn_rates.len() >= 2 {
            h.churn_trend = Some(crate::trend::compute_entity_trend(&churn_rates));
        }
    }

    for p in pairs.iter_mut() {
        let key = entity_pair_key(&p.file_a, &p.file_b);
        // Cumulative, like churn — see `period_rates`.
        let rates = crate::trend::period_rates(
            &ordered
                .iter()
                .filter_map(|e| e.coupling_degree.get(&key).copied())
                .map(|c| c as f64)
                .collect::<Vec<f64>>(),
        );
        if rates.len() >= 2 {
            p.coupling_trend = Some(crate::trend::compute_entity_trend(&rates));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a hotspot with only the fields these tests care about.
    fn trend_hotspot(path: &str) -> crate::scorer::HotspotFile {
        crate::scorer::HotspotFile {
            path: path.to_string(),
            role: crate::metrics::file_role::FileRole::Source,
            churn_count: 0,
            bug_commit_count: 0,
            loc: 100,
            total_lines: 100,
            cyclomatic_complexity: 10,
            public_methods: 0,
            properties: 0,
            hotspot_score: 50.0,
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

    /// One entry per sample, spaced a day apart, carrying a churn count for
    /// `path`. Churn is recorded as an all-time cumulative total, so these
    /// are totals, not per-period counts.
    fn churn_history(path: &str, totals: &[u32]) -> Vec<EntityTrendEntry> {
        use std::collections::HashMap;
        use EntityTrendEntry;
        let base = chrono::Utc::now() - chrono::Duration::days(totals.len() as i64);
        totals
            .iter()
            .enumerate()
            .map(|(i, total)| {
                let mut churn = HashMap::new();
                churn.insert(path.to_string(), *total);
                EntityTrendEntry {
                    timestamp: base + chrono::Duration::days(i as i64),
                    head: format!("sha{i}"),
                    branch: "main".into(),
                    complexity: HashMap::new(),
                    churn,
                    coupling_degree: HashMap::new(),
                    schema_version: 1,
                }
            })
            .collect()
    }

    #[test]
    fn attach_entity_trends_reports_steady_churn_rate_as_stable() {
        // Backfill records churn as an all-time total up to each sampled
        // commit, so the series only ever climbs. A file touched at a
        // perfectly steady rate yields totals 10 -> 20 -> 30; classifying
        // the totals directly gives (30-10)/10 = +200% and calls a file
        // whose habits never changed "Growing". The direction has to be
        // read from the per-period rate, not the running total.
        let mut hotspots = vec![trend_hotspot("src/steady.rs")];
        let mut pairs: Vec<crate::scorer::CouplingPair> = vec![];

        attach_entity_trends(
            &mut hotspots,
            &mut pairs,
            &churn_history("src/steady.rs", &[10, 20, 30]),
        );

        assert_eq!(
            hotspots[0].churn_trend,
            Some(crate::scorer::EntityTrendDirection::Stable),
            "a steady 10-commits-per-period rate is not a rising churn trend"
        );
    }

    #[test]
    fn attach_entity_trends_reports_decelerating_churn_as_shrinking() {
        // 10 commits in the first period, then only 2 in the second: the
        // file is calming down. `Shrinking` must be reachable at all — on a
        // monotonically climbing cumulative total it never is.
        let mut hotspots = vec![trend_hotspot("src/calming.rs")];
        let mut pairs: Vec<crate::scorer::CouplingPair> = vec![];

        attach_entity_trends(
            &mut hotspots,
            &mut pairs,
            &churn_history("src/calming.rs", &[10, 20, 22]),
        );

        assert_eq!(
            hotspots[0].churn_trend,
            Some(crate::scorer::EntityTrendDirection::Shrinking),
            "churn dropping from 10 to 2 per period is a shrinking churn trend"
        );
    }

    #[test]
    fn attach_entity_trends_sets_direction_on_matching_hotspot() {
        use crate::scorer::HotspotFile;
        use std::collections::HashMap;
        use EntityTrendEntry;

        let mut hotspots = vec![HotspotFile {
            path: "src/big.rs".to_string(),
            role: crate::metrics::file_role::FileRole::Source,
            churn_count: 10,
            bug_commit_count: 0,
            loc: 500,
            total_lines: 500,
            cyclomatic_complexity: 50,
            public_methods: 0,
            properties: 0,
            hotspot_score: 90.0,
            coupling_trend: None,
            content_findings: 0,
            common_findings: 0,
            control_findings: 0,
            inheritance_findings: 0,
            churn_timeline: vec![],
            complexity_trend: None,
            churn_trend: None,
        }];
        let mut pairs: Vec<crate::scorer::CouplingPair> = vec![];

        let mut complexity_a = HashMap::new();
        complexity_a.insert("src/big.rs".to_string(), 10u32);
        let mut complexity_b = HashMap::new();
        complexity_b.insert("src/big.rs".to_string(), 50u32);
        // Three samples, because churn is a running total and is classified
        // on its per-period rate (`period_rates`): N totals give N-1 rates,
        // and a direction needs two of them.
        let mut complexity_c = HashMap::new();
        complexity_c.insert("src/big.rs".to_string(), 90u32);
        // Steady 20 commits per period — a running total that climbs, but a
        // rate that does not move.
        let churn_totals = [20u32, 40u32, 60u32];
        let mut churn_maps = churn_totals.iter().map(|total| {
            let mut m = HashMap::new();
            m.insert("src/big.rs".to_string(), *total);
            m
        });

        let now = chrono::Utc::now();
        let history = vec![
            EntityTrendEntry {
                timestamp: now - chrono::Duration::days(2),
                head: "aaa".into(),
                branch: "main".into(),
                complexity: complexity_a,
                churn: churn_maps.next().unwrap(),
                coupling_degree: HashMap::new(),
                schema_version: 1,
            },
            EntityTrendEntry {
                timestamp: now - chrono::Duration::days(1),
                head: "bbb".into(),
                branch: "main".into(),
                complexity: complexity_b,
                churn: churn_maps.next().unwrap(),
                coupling_degree: HashMap::new(),
                schema_version: 1,
            },
            EntityTrendEntry {
                timestamp: now,
                head: "ccc".into(),
                branch: "main".into(),
                complexity: complexity_c,
                churn: churn_maps.next().unwrap(),
                coupling_degree: HashMap::new(),
                schema_version: 1,
            },
        ];

        attach_entity_trends(&mut hotspots, &mut pairs, &history);

        assert_eq!(
            hotspots[0].complexity_trend,
            Some(crate::scorer::EntityTrendDirection::Growing),
            "10 -> 90 is a 800% increase, well past the 15% threshold"
        );
        assert_eq!(
            hotspots[0].churn_trend,
            Some(crate::scorer::EntityTrendDirection::Stable),
            "a steady 20-per-period churn rate is Stable even while complexity \
             climbs — complexity and churn must classify independently"
        );
    }

    #[test]
    fn attach_entity_trends_leaves_none_for_unmatched_hotspot() {
        use crate::scorer::HotspotFile;

        let mut hotspots = vec![HotspotFile {
            path: "src/never_seen.rs".to_string(),
            role: crate::metrics::file_role::FileRole::Source,
            churn_count: 0,
            bug_commit_count: 0,
            loc: 10,
            total_lines: 10,
            cyclomatic_complexity: 1,
            public_methods: 0,
            properties: 0,
            hotspot_score: 5.0,
            coupling_trend: None,
            content_findings: 0,
            common_findings: 0,
            control_findings: 0,
            inheritance_findings: 0,
            churn_timeline: vec![],
            complexity_trend: None,
            churn_trend: None,
        }];
        let mut pairs: Vec<crate::scorer::CouplingPair> = vec![];

        attach_entity_trends(&mut hotspots, &mut pairs, &[]);

        assert_eq!(hotspots[0].complexity_trend, None);
        assert_eq!(hotspots[0].churn_trend, None);
    }

    #[test]
    fn attach_entity_trends_sorts_history_by_timestamp_before_classifying() {
        use crate::scorer::HotspotFile;
        use chrono::{Duration, Utc};
        use std::collections::HashMap;
        use EntityTrendEntry;

        let mut hotspots = vec![HotspotFile {
            path: "src/big.rs".to_string(),
            role: crate::metrics::file_role::FileRole::Source,
            churn_count: 10,
            bug_commit_count: 0,
            loc: 500,
            total_lines: 500,
            cyclomatic_complexity: 50,
            public_methods: 0,
            properties: 0,
            hotspot_score: 90.0,
            coupling_trend: None,
            content_findings: 0,
            common_findings: 0,
            control_findings: 0,
            inheritance_findings: 0,
            churn_timeline: vec![],
            complexity_trend: None,
            churn_trend: None,
        }];
        let mut pairs: Vec<crate::scorer::CouplingPair> = vec![];

        let now = Utc::now();
        let older = now - Duration::days(10);
        let newer = now;

        let mut complexity_older = HashMap::new();
        complexity_older.insert("src/big.rs".to_string(), 10u32);
        let mut complexity_newer = HashMap::new();
        complexity_newer.insert("src/big.rs".to_string(), 50u32);

        // History vec is in REVERSE chronological order (newest first, oldest
        // last) — exactly what `select_samples`'s `count >= len` shortcut
        // produces, since `collect_commits` yields commits newest-first.
        let history = vec![
            EntityTrendEntry {
                timestamp: newer,
                head: "bbb".into(),
                branch: "main".into(),
                complexity: complexity_newer,
                churn: HashMap::new(),
                coupling_degree: HashMap::new(),
                schema_version: 1,
            },
            EntityTrendEntry {
                timestamp: older,
                head: "aaa".into(),
                branch: "main".into(),
                complexity: complexity_older,
                churn: HashMap::new(),
                coupling_degree: HashMap::new(),
                schema_version: 1,
            },
        ];

        attach_entity_trends(&mut hotspots, &mut pairs, &history);

        // Complexity genuinely grew from 10 (older) to 50 (newer). Without
        // sorting by timestamp first, the pre-fix code would treat the vec's
        // first entry (newer, 50) as "oldest" and the last entry (older, 10)
        // as "newest", computing a -80% change and misclassifying this as
        // Shrinking instead of Growing.
        assert_eq!(
            hotspots[0].complexity_trend,
            Some(crate::scorer::EntityTrendDirection::Growing),
            "history must be sorted by timestamp before classifying, regardless of vec order"
        );
    }

    #[test]
    fn attach_entity_trends_classifies_coupling_pair_trend_with_key_normalization() {
        use chrono::{Duration, Utc};
        use std::collections::HashMap;
        use EntityTrendEntry;

        let mut hotspots: Vec<crate::scorer::HotspotFile> = vec![];
        let mut pairs = vec![crate::scorer::CouplingPair {
            // Reversed order from the stored key ("src/a.rs|src/b.rs") to
            // prove `entity_pair_key`'s normalization is applied at the
            // consumption site, not just at construction.
            file_a: "src/b.rs".to_string(),
            file_b: "src/a.rs".to_string(),
            co_changes: 9,
            coupling_pct: 90.0,
            growth_a: 0,
            growth_b: 0,
            cross_boundary: true,
            is_test_pair: false,
            coupling_trend: None,
        }];

        let now = Utc::now();
        let older = now - Duration::days(10);
        let newer = now;

        // Coupling degree is a running total too, so three samples are needed
        // for two rates: 3 -> 6 -> 21 gives rates 3 then 15, a real
        // acceleration rather than a merely climbing total.
        let mut coupling_newer = HashMap::new();
        coupling_newer.insert("src/a.rs|src/b.rs".to_string(), 21usize);
        let mut coupling_mid = HashMap::new();
        coupling_mid.insert("src/a.rs|src/b.rs".to_string(), 6usize);
        let mut coupling_older = HashMap::new();
        coupling_older.insert("src/a.rs|src/b.rs".to_string(), 3usize);

        // Non-chronological vec order (newest first) to also exercise the
        // Fix 1 sort.
        let history = vec![
            EntityTrendEntry {
                timestamp: newer,
                head: "bbb".into(),
                branch: "main".into(),
                complexity: HashMap::new(),
                churn: HashMap::new(),
                coupling_degree: coupling_newer,
                schema_version: 1,
            },
            EntityTrendEntry {
                timestamp: now - chrono::Duration::days(5),
                head: "mid".into(),
                branch: "main".into(),
                complexity: HashMap::new(),
                churn: HashMap::new(),
                coupling_degree: coupling_mid,
                schema_version: 1,
            },
            EntityTrendEntry {
                timestamp: older,
                head: "aaa".into(),
                branch: "main".into(),
                complexity: HashMap::new(),
                churn: HashMap::new(),
                coupling_degree: coupling_older,
                schema_version: 1,
            },
        ];

        attach_entity_trends(&mut hotspots, &mut pairs, &history);

        assert_eq!(
            pairs[0].coupling_trend,
            Some(crate::scorer::EntityTrendDirection::Growing),
            "3 -> 9 is a 200% increase; key normalization must match \
             file_a/file_b regardless of their order vs. the stored key"
        );
    }

    #[test]
    fn attach_entity_trends_leaves_none_with_only_one_history_point() {
        use crate::scorer::HotspotFile;
        use std::collections::HashMap;
        use EntityTrendEntry;

        let mut hotspots = vec![HotspotFile {
            path: "src/big.rs".to_string(),
            role: crate::metrics::file_role::FileRole::Source,
            churn_count: 10,
            bug_commit_count: 0,
            loc: 500,
            total_lines: 500,
            cyclomatic_complexity: 50,
            public_methods: 0,
            properties: 0,
            hotspot_score: 90.0,
            coupling_trend: None,
            content_findings: 0,
            common_findings: 0,
            control_findings: 0,
            inheritance_findings: 0,
            churn_timeline: vec![],
            complexity_trend: None,
            churn_trend: None,
        }];
        let mut pairs: Vec<crate::scorer::CouplingPair> = vec![];

        let mut complexity = HashMap::new();
        complexity.insert("src/big.rs".to_string(), 10u32);

        let history = vec![EntityTrendEntry {
            timestamp: chrono::Utc::now(),
            head: "aaa".into(),
            branch: "main".into(),
            complexity,
            churn: HashMap::new(),
            coupling_degree: HashMap::new(),
            schema_version: 1,
        }];

        attach_entity_trends(&mut hotspots, &mut pairs, &history);

        assert_eq!(
            hotspots[0].complexity_trend, None,
            "a single history point must leave the trend as None (no signal), not Some(Stable)"
        );
    }

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
    fn build_entity_trend_entry_caps_coupling_pairs_at_top_n_by_co_changes() {
        // `entity_trend_top_n` bounded only the hotspot maps; every
        // qualifying pair was persisted. Pair count is O(n^2) in the limit,
        // so entry size was governed by how coupled the repo is, with no cap
        // at all — measured at 666 keys and 39 KB on a single JSONL line for
        // this repository. Keep the most-coupled pairs, drop the tail.
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        let pairs = [("a", "x", 9usize), ("b", "y", 5usize), ("c", "z", 3usize)];
        for (src, tst, co) in pairs {
            let src_path = format!("src/{src}.rs");
            let test_path = format!("tests/{tst}.rs");
            for path in [&src_path, &test_path] {
                snapshot.files.push(FileEntry {
                    path: path.into(),
                    size_bytes: 1,
                    is_binary: false,
                    depth: 2,
                    blob_oid: String::new(),
                });
                snapshot.commits_by_file.insert(
                    path.into(),
                    (0..co as u32).map(crate::snapshot::CommitId).collect(),
                );
            }
            // co_changes == min_commits, so the ratio is 1.0 and every pair
            // clears the threshold; only the cap can separate them.
            snapshot.file_change_pairs.push((
                PathBuf::from(&src_path),
                PathBuf::from(&test_path),
                co,
            ));
        }

        let entry = build_entity_trend_entry(
            &snapshot,
            &[],
            &CouplingThresholds::default(),
            2, // top_n
            "abc123",
            Utc::now(),
            "main",
        );

        assert_eq!(
            entry.coupling_degree.len(),
            2,
            "coupling pairs must be capped at top_n, got: {:?}",
            entry.coupling_degree
        );
        assert_eq!(
            entry.coupling_degree.get("src/a.rs|tests/x.rs"),
            Some(&9),
            "the most-coupled pair must be kept"
        );
        assert_eq!(
            entry.coupling_degree.get("src/b.rs|tests/y.rs"),
            Some(&5),
            "the second-most-coupled pair must be kept"
        );
        assert!(
            !entry.coupling_degree.contains_key("src/c.rs|tests/z.rs"),
            "the weakest pair must be dropped"
        );
    }

    #[test]
    fn entity_pair_key_sorts_lexicographically_regardless_of_argument_order() {
        assert_eq!(entity_pair_key("b.rs", "a.rs"), "a.rs|b.rs");
        assert_eq!(entity_pair_key("a.rs", "b.rs"), "a.rs|b.rs");
    }
}
