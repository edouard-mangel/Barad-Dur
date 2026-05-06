use std::collections::{HashMap, HashSet};

use chrono::{Datelike, NaiveDate, Utc};

use crate::snapshot::{CommitId, RepoSnapshot};

use super::types::{AuditReport, CrisisFile, DeadFile, DirConcentration, VelocityBucket};

const CRISIS_KEYWORDS: &[&str] = &[
    "fix",
    "hotfix",
    "revert",
    "urgent",
    "broken",
    "oops",
    "emergency",
    "critical",
    "crash",
];

pub fn build_audit_report(snapshot: &RepoSnapshot) -> AuditReport {
    AuditReport {
        crisis_files: build_crisis_files(snapshot),
        dir_concentration: build_dir_concentration(snapshot),
        dead_files: build_dead_files(snapshot),
        velocity_buckets: build_velocity_buckets(snapshot),
    }
}

fn build_crisis_files(snapshot: &RepoSnapshot) -> Vec<CrisisFile> {
    let crisis_ids: HashSet<CommitId> = snapshot
        .commits
        .iter()
        .filter(|c| {
            let msg = c.message.to_lowercase();
            CRISIS_KEYWORDS.iter().any(|kw| msg.contains(kw))
        })
        .map(|c| c.id)
        .collect();

    let mut files: Vec<CrisisFile> = snapshot
        .commits_by_file
        .iter()
        .filter_map(|(path, commit_ids)| {
            let total = commit_ids.len();
            if total == 0 {
                return None;
            }
            let crisis = commit_ids
                .iter()
                .filter(|id| crisis_ids.contains(id))
                .count();
            Some(CrisisFile {
                path: path.to_string_lossy().into_owned(),
                crisis_commit_count: crisis,
                total_commit_count: total,
                crisis_ratio: crisis as f64 / total as f64,
            })
        })
        .collect();

    files.sort_by(|a, b| {
        b.crisis_ratio
            .partial_cmp(&a.crisis_ratio)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.crisis_commit_count.cmp(&a.crisis_commit_count))
    });
    files.truncate(20);
    files
}

fn build_dir_concentration(snapshot: &RepoSnapshot) -> Vec<DirConcentration> {
    let mut map: HashMap<String, (usize, usize)> = HashMap::new();

    for (path, metrics) in &snapshot.file_metrics {
        let dir = path
            .parent()
            .and_then(|p| {
                let s = p.to_string_lossy();
                if s.is_empty() {
                    None
                } else {
                    Some(s.into_owned())
                }
            })
            .unwrap_or_else(|| "(root)".to_string());

        let entry = map.entry(dir).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += metrics.total_lines;
    }

    let total_loc: usize = map.values().map(|(_, loc)| loc).sum();
    let divisor = total_loc.max(1) as f64;

    let mut dirs: Vec<DirConcentration> = map
        .into_iter()
        .map(|(dir, (file_count, loc))| DirConcentration {
            dir,
            file_count,
            loc,
            pct_of_total: loc as f64 / divisor * 100.0,
        })
        .collect();

    dirs.sort_by(|a, b| b.loc.cmp(&a.loc));
    dirs
}

fn build_dead_files(snapshot: &RepoSnapshot) -> Vec<DeadFile> {
    let now = Utc::now();

    let mut files: Vec<DeadFile> = snapshot
        .commits_by_file
        .iter()
        .filter_map(|(path, commit_ids)| {
            let churn = commit_ids.len();
            if churn > 1 {
                return None;
            }
            // Find most-recent commit timestamp for this file
            let last_ts = commit_ids
                .iter()
                .filter_map(|id| {
                    snapshot
                        .commits
                        .iter()
                        .find(|c| c.id == *id)
                        .map(|c| c.timestamp)
                })
                .max()?;

            let days = (now - last_ts).num_days();
            if days <= 180 {
                return None;
            }

            Some(DeadFile {
                path: path.to_string_lossy().into_owned(),
                days_since_modified: days,
                churn_count: churn,
            })
        })
        .collect();

    files.sort_by(|a, b| b.days_since_modified.cmp(&a.days_since_modified));
    files
}

fn build_velocity_buckets(snapshot: &RepoSnapshot) -> Vec<VelocityBucket> {
    // (iso_year, iso_week) → (commit_count, author_set)
    let mut map: HashMap<(i32, u32), (usize, HashSet<usize>)> = HashMap::new();

    for commit in &snapshot.commits {
        let iso = commit.timestamp.iso_week();
        let key = (iso.year(), iso.week());
        let entry = map.entry(key).or_insert((0, HashSet::new()));
        entry.0 += 1;
        entry.1.insert(commit.author);
    }

    let mut buckets: Vec<VelocityBucket> = map
        .into_iter()
        .map(|((year, week), (commit_count, authors))| {
            let monday = NaiveDate::from_isoywd_opt(year, week, chrono::Weekday::Mon)
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| format!("{year}-W{week:02}"));
            VelocityBucket {
                week_start: monday,
                commit_count,
                author_count: authors.len(),
            }
        })
        .collect();

    buckets.sort_by(|a, b| a.week_start.cmp(&b.week_start));
    buckets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{Commit, CommitId, RepoSnapshot, TimeWindow};

    fn empty_snapshot() -> RepoSnapshot {
        RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        )
    }

    fn make_commit(id: u32, author: usize, ts: chrono::DateTime<chrono::Utc>, msg: &str) -> Commit {
        Commit {
            id: CommitId(id),
            author,
            timestamp: ts,
            message: msg.into(),
            files_changed: vec![],
            is_merge: false,
            parent_count: 1,
        }
    }

    #[test]
    fn empty_snapshot_returns_empty_audit() {
        let snap = empty_snapshot();
        let report = build_audit_report(&snap);
        assert!(report.crisis_files.is_empty());
        assert!(report.dir_concentration.is_empty());
        assert!(report.dead_files.is_empty());
        assert!(report.velocity_buckets.is_empty());
    }

    #[test]
    fn crisis_files_counts_keywords() {
        use crate::snapshot::*;
        use chrono::Utc;

        let mut snap = empty_snapshot();
        snap.commit_interner.intern("aaa");
        snap.commit_interner.intern("bbb");

        let fix_commit = Commit {
            id: CommitId(0),
            author: 0,
            timestamp: Utc::now(),
            message: "fix: crash on startup".into(),
            files_changed: vec![],
            is_merge: false,
            parent_count: 1,
        };
        let feat_commit = Commit {
            id: CommitId(1),
            author: 0,
            timestamp: Utc::now(),
            message: "feat: add button".into(),
            files_changed: vec![],
            is_merge: false,
            parent_count: 1,
        };
        snap.commits = vec![fix_commit, feat_commit];

        let path = std::path::PathBuf::from("src/main.rs");
        snap.commits_by_file
            .insert(path, vec![CommitId(0), CommitId(1)]);

        let report = build_audit_report(&snap);
        assert_eq!(report.crisis_files.len(), 1);
        let cf = &report.crisis_files[0];
        assert_eq!(cf.crisis_commit_count, 1);
        assert_eq!(cf.total_commit_count, 2);
        assert!((cf.crisis_ratio - 0.5).abs() < 1e-9);
    }

    #[test]
    fn velocity_buckets_groups_by_week() {
        use crate::snapshot::*;
        use chrono::{TimeZone, Utc};

        let mut snap = empty_snapshot();
        snap.commit_interner.intern("aaa");
        snap.commit_interner.intern("bbb");

        // Two commits in same week, one in the next
        let t1 = Utc.with_ymd_and_hms(2024, 1, 8, 0, 0, 0).unwrap(); // week 2
        let t2 = Utc.with_ymd_and_hms(2024, 1, 9, 0, 0, 0).unwrap(); // week 2
        let t3 = Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap(); // week 3

        snap.commits = vec![
            Commit {
                id: CommitId(0),
                author: 0,
                timestamp: t1,
                message: "a".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
            Commit {
                id: CommitId(1),
                author: 1,
                timestamp: t2,
                message: "b".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
            Commit {
                id: CommitId(2),
                author: 0,
                timestamp: t3,
                message: "c".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
        ];

        let buckets = build_velocity_buckets(&snap);
        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets[0].commit_count, 2);
        assert_eq!(buckets[0].author_count, 2);
        assert_eq!(buckets[1].commit_count, 1);
        assert_eq!(buckets[1].author_count, 1);
    }

    #[test]
    fn dead_files_filters_threshold() {
        use crate::snapshot::*;
        use chrono::{Duration, Utc};

        let mut snap = empty_snapshot();
        snap.commit_interner.intern("aaa");
        snap.commit_interner.intern("bbb");

        let old_ts = Utc::now() - Duration::days(400);
        let new_ts = Utc::now() - Duration::days(10);

        snap.commits = vec![
            Commit {
                id: CommitId(0),
                author: 0,
                timestamp: old_ts,
                message: "old".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
            Commit {
                id: CommitId(1),
                author: 0,
                timestamp: new_ts,
                message: "new".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
        ];

        snap.commits_by_file
            .insert(std::path::PathBuf::from("old.rs"), vec![CommitId(0)]);
        snap.commits_by_file
            .insert(std::path::PathBuf::from("new.rs"), vec![CommitId(1)]);

        let dead = build_dead_files(&snap);
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0].path, "old.rs");
    }

    #[test]
    fn dead_files_silently_skips_file_with_unknown_commit_id() {
        // CommitId(99) is not in snapshot.commits; the max()? short-circuits and drops the file.
        let mut snap = empty_snapshot();
        snap.commits_by_file
            .insert("orphaned.rs".into(), vec![CommitId(99)]);

        let dead = build_dead_files(&snap);
        assert!(
            dead.is_empty(),
            "file referencing an unknown CommitId must be silently skipped"
        );
    }

    // ---- dir_concentration ----

    #[test]
    fn dir_concentration_groups_by_directory() {
        use crate::snapshot::FileComplexity;

        let mut snap = empty_snapshot();
        snap.file_metrics.insert(
            "src/a.rs".into(),
            FileComplexity {
                total_lines: 100,
                ..Default::default()
            },
        );
        snap.file_metrics.insert(
            "src/b.rs".into(),
            FileComplexity {
                total_lines: 200,
                ..Default::default()
            },
        );
        snap.file_metrics.insert(
            "tests/c.rs".into(),
            FileComplexity {
                total_lines: 50,
                ..Default::default()
            },
        );

        let dirs = build_dir_concentration(&snap);
        let src = dirs
            .iter()
            .find(|d| d.dir == "src")
            .expect("src dir missing");
        assert_eq!(src.file_count, 2);
        assert_eq!(src.loc, 300);

        let tests = dirs
            .iter()
            .find(|d| d.dir == "tests")
            .expect("tests dir missing");
        assert_eq!(tests.file_count, 1);
        assert_eq!(tests.loc, 50);
    }

    #[test]
    fn dir_concentration_root_files_use_root_key() {
        use crate::snapshot::FileComplexity;

        let mut snap = empty_snapshot();
        snap.file_metrics.insert(
            "Cargo.toml".into(),
            FileComplexity {
                total_lines: 30,
                ..Default::default()
            },
        );

        let dirs = build_dir_concentration(&snap);
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].dir, "(root)");
        assert_eq!(dirs[0].file_count, 1);
        assert_eq!(dirs[0].loc, 30);
    }

    #[test]
    fn dir_concentration_sorted_by_loc_desc() {
        use crate::snapshot::FileComplexity;

        let mut snap = empty_snapshot();
        snap.file_metrics.insert(
            "small/x.rs".into(),
            FileComplexity {
                total_lines: 10,
                ..Default::default()
            },
        );
        snap.file_metrics.insert(
            "big/y.rs".into(),
            FileComplexity {
                total_lines: 500,
                ..Default::default()
            },
        );
        snap.file_metrics.insert(
            "mid/z.rs".into(),
            FileComplexity {
                total_lines: 200,
                ..Default::default()
            },
        );

        let dirs = build_dir_concentration(&snap);
        assert_eq!(dirs[0].loc, 500);
        assert_eq!(dirs[1].loc, 200);
        assert_eq!(dirs[2].loc, 10);
    }

    #[test]
    fn dir_concentration_pct_sums_to_100() {
        use crate::snapshot::FileComplexity;

        let mut snap = empty_snapshot();
        for i in 0..5u32 {
            snap.file_metrics.insert(
                format!("dir{i}/file.rs").into(),
                FileComplexity {
                    total_lines: (i + 1) as usize * 100,
                    ..Default::default()
                },
            );
        }

        let dirs = build_dir_concentration(&snap);
        let total_pct: f64 = dirs.iter().map(|d| d.pct_of_total).sum();
        assert!(
            (total_pct - 100.0).abs() < 1e-6,
            "pct_of_total should sum to 100, got {total_pct}"
        );
    }

    #[test]
    fn dir_concentration_uses_full_parent_path() {
        // path.parent() on "src/metrics/b.rs" → "src/metrics", NOT "src".
        // So "src/a.rs" and "src/metrics/b.rs" land in separate groups.
        use crate::snapshot::FileComplexity;

        let mut snap = empty_snapshot();
        snap.file_metrics.insert(
            "src/a.rs".into(),
            FileComplexity {
                total_lines: 100,
                ..Default::default()
            },
        );
        snap.file_metrics.insert(
            "src/metrics/b.rs".into(),
            FileComplexity {
                total_lines: 200,
                ..Default::default()
            },
        );

        let dirs = build_dir_concentration(&snap);
        assert_eq!(
            dirs.len(),
            2,
            "full parent path means the two files must be in separate groups"
        );

        let src = dirs
            .iter()
            .find(|d| d.dir == "src")
            .expect("\"src\" group must exist");
        assert_eq!(src.loc, 100, "\"src\" group must only contain src/a.rs");

        let metrics = dirs
            .iter()
            .find(|d| d.dir == "src/metrics")
            .expect("\"src/metrics\" group must exist");
        assert_eq!(
            metrics.loc, 200,
            "\"src/metrics\" group must only contain src/metrics/b.rs"
        );
    }

    // ---- crisis_files ----

    #[test]
    fn crisis_files_substring_match_is_intentional() {
        // msg.contains("fix") matches "prefix" — this is deliberate substring matching.
        // A commit like "add prefix matching" IS classified as a crisis commit.
        use chrono::Utc;

        let mut snap = empty_snapshot();
        snap.commit_interner.intern("sha");
        snap.commits = vec![make_commit(0, 0, Utc::now(), "add prefix matching")];
        snap.commits_by_file
            .insert("src/lib.rs".into(), vec![CommitId(0)]);

        let files = build_crisis_files(&snap);
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].crisis_commit_count, 1,
            "\"prefix\" contains \"fix\" as a substring — documented intentional false-positive"
        );
    }

    #[test]
    fn crisis_files_includes_zero_ratio_files() {
        // There is no `if crisis == 0 { skip }` guard. Every file in commits_by_file appears,
        // including those with crisis_ratio == 0.0.
        use chrono::Utc;

        let mut snap = empty_snapshot();
        snap.commit_interner.intern("sha");
        snap.commits = vec![make_commit(0, 0, Utc::now(), "feat: add feature")];
        snap.commits_by_file
            .insert("src/clean.rs".into(), vec![CommitId(0)]);

        let files = build_crisis_files(&snap);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].crisis_commit_count, 0);
        assert_eq!(
            files[0].crisis_ratio, 0.0,
            "zero-ratio files are included, not filtered out"
        );
    }

    #[test]
    fn crisis_files_sorted_by_ratio_desc() {
        use chrono::Utc;

        let mut snap = empty_snapshot();
        snap.commit_interner.intern("a");
        snap.commit_interner.intern("b");
        snap.commit_interner.intern("c");

        snap.commits = vec![
            make_commit(0, 0, Utc::now(), "fix something"),
            make_commit(1, 0, Utc::now(), "feat something"),
            make_commit(2, 0, Utc::now(), "fix another"),
        ];
        // high.rs: 1/1 crisis = ratio 1.0
        snap.commits_by_file
            .insert("high.rs".into(), vec![CommitId(0)]);
        // low.rs: 1/2 crisis = ratio 0.5
        snap.commits_by_file
            .insert("low.rs".into(), vec![CommitId(1), CommitId(2)]);

        let files = build_crisis_files(&snap);
        assert_eq!(files[0].path, "high.rs", "highest ratio must be first");
        assert_eq!(files[1].path, "low.rs");
    }

    #[test]
    fn crisis_files_capped_at_20() {
        use chrono::Utc;

        let mut snap = empty_snapshot();
        // One crisis commit shared across 21 files
        snap.commit_interner.intern("sha");
        snap.commits = vec![make_commit(0, 0, Utc::now(), "hotfix: critical")];
        for i in 0u32..21 {
            snap.commits_by_file
                .insert(format!("file{i}.rs").into(), vec![CommitId(0)]);
        }

        let files = build_crisis_files(&snap);
        assert_eq!(files.len(), 20, "result must be capped at 20");
    }

    #[test]
    fn crisis_files_matches_uppercase_keywords() {
        use chrono::Utc;

        let mut snap = empty_snapshot();
        snap.commit_interner.intern("sha");
        snap.commits = vec![make_commit(0, 0, Utc::now(), "FIX: Something uppercase")];
        snap.commits_by_file
            .insert("src/lib.rs".into(), vec![CommitId(0)]);

        let files = build_crisis_files(&snap);
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].crisis_commit_count, 1,
            "uppercase FIX must be matched via to_lowercase"
        );
    }

    // ---- dead_files ----

    #[test]
    fn dead_files_excludes_high_churn() {
        use chrono::{Duration, Utc};

        let mut snap = empty_snapshot();
        snap.commit_interner.intern("a");
        snap.commit_interner.intern("b");
        let old_ts = Utc::now() - Duration::days(400);
        snap.commits = vec![
            make_commit(0, 0, old_ts, "first"),
            make_commit(1, 0, old_ts, "second"),
        ];
        // churn=2 → should NOT be dead even though old
        snap.commits_by_file
            .insert("active.rs".into(), vec![CommitId(0), CommitId(1)]);

        let dead = build_dead_files(&snap);
        assert!(
            dead.is_empty(),
            "churn=2 file must not be classified as dead"
        );
    }

    #[test]
    fn dead_files_boundary_exactly_180_days_not_dead() {
        use chrono::{Duration, Utc};

        let mut snap = empty_snapshot();
        snap.commit_interner.intern("sha");
        let ts = Utc::now() - Duration::days(180);
        snap.commits = vec![make_commit(0, 0, ts, "old")];
        snap.commits_by_file
            .insert("boundary.rs".into(), vec![CommitId(0)]);

        let dead = build_dead_files(&snap);
        assert!(
            dead.is_empty(),
            "exactly 180 days must NOT be dead (threshold is strictly > 180)"
        );
    }

    #[test]
    fn dead_files_sorted_oldest_first() {
        use chrono::{Duration, Utc};

        let mut snap = empty_snapshot();
        snap.commit_interner.intern("a");
        snap.commit_interner.intern("b");
        let older = Utc::now() - Duration::days(500);
        let newer = Utc::now() - Duration::days(200);
        snap.commits = vec![
            make_commit(0, 0, older, "old"),
            make_commit(1, 0, newer, "newer"),
        ];
        snap.commits_by_file
            .insert("ancient.rs".into(), vec![CommitId(0)]);
        snap.commits_by_file
            .insert("stale.rs".into(), vec![CommitId(1)]);

        let dead = build_dead_files(&snap);
        assert_eq!(dead.len(), 2);
        assert!(
            dead[0].days_since_modified > dead[1].days_since_modified,
            "oldest file must come first"
        );
    }

    // ---- velocity_buckets ----

    #[test]
    fn velocity_same_author_deduped_in_week() {
        use chrono::{TimeZone, Utc};

        let mut snap = empty_snapshot();
        snap.commit_interner.intern("a");
        snap.commit_interner.intern("b");
        let t1 = Utc.with_ymd_and_hms(2024, 1, 8, 9, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2024, 1, 9, 10, 0, 0).unwrap();
        // Same author (0) commits twice in the same week
        snap.commits = vec![
            make_commit(0, 0, t1, "first"),
            make_commit(1, 0, t2, "second"),
        ];

        let buckets = build_velocity_buckets(&snap);
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].commit_count, 2);
        assert_eq!(
            buckets[0].author_count, 1,
            "same author twice must count as 1 unique author"
        );
    }

    #[test]
    fn velocity_week_start_is_monday_iso_date() {
        use chrono::{TimeZone, Utc};

        let mut snap = empty_snapshot();
        snap.commit_interner.intern("sha");
        // 2024-01-10 is a Wednesday in ISO week 2; the Monday of that week is 2024-01-08
        let t = Utc.with_ymd_and_hms(2024, 1, 10, 0, 0, 0).unwrap();
        snap.commits = vec![make_commit(0, 0, t, "mid-week commit")];

        let buckets = build_velocity_buckets(&snap);
        assert_eq!(buckets.len(), 1);
        assert_eq!(
            buckets[0].week_start, "2024-01-08",
            "week_start must be the Monday of that ISO week"
        );
    }

    #[test]
    fn velocity_buckets_sorted_chronologically() {
        use chrono::{TimeZone, Utc};

        let mut snap = empty_snapshot();
        // Insert commits out-of-order to stress the sort
        let t_later = Utc.with_ymd_and_hms(2024, 3, 18, 0, 0, 0).unwrap();
        let t_earlier = Utc.with_ymd_and_hms(2024, 1, 8, 0, 0, 0).unwrap();
        let t_middle = Utc.with_ymd_and_hms(2024, 2, 5, 0, 0, 0).unwrap();
        snap.commits = vec![
            make_commit(0, 0, t_later, "c"),
            make_commit(1, 0, t_earlier, "a"),
            make_commit(2, 0, t_middle, "b"),
        ];

        let buckets = build_velocity_buckets(&snap);
        let dates: Vec<&str> = buckets.iter().map(|b| b.week_start.as_str()).collect();
        let mut sorted = dates.clone();
        sorted.sort();
        assert_eq!(
            dates, sorted,
            "buckets must be in chronological (ascending) order"
        );
    }
}
