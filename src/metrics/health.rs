use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::HealthThresholds;
use crate::metrics::{CategoryResult, MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

pub fn compute_health(snapshot: &RepoSnapshot, thresholds: &HealthThresholds) -> CategoryResult {
    let metrics = vec![
        bus_factor(snapshot, thresholds),
        churn_hotspots(snapshot, thresholds),
        temporal_coupling(snapshot, thresholds),
        stale_code(snapshot),
        file_complexity(snapshot, thresholds),
        god_objects(snapshot),
        complex_hotspots(snapshot),
    ];

    CategoryResult {
        name: "Health".to_string(),
        score: 0,
        metrics,
    }
    .compute_score()
}

/// Percentage of files that are single-author dominated (one author owns >50% of lines).
fn bus_factor(snapshot: &RepoSnapshot, _thresholds: &HealthThresholds) -> MetricValue {
    if snapshot.blame_map.is_empty() {
        return MetricValue {
            name: "Bus factor".to_string(),
            description: "No blame data available".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
        };
    }

    let total_files = snapshot.blame_map.len();
    let dominated = snapshot
        .blame_map
        .values()
        .filter(|lines| {
            if lines.is_empty() {
                return false;
            }
            let mut author_lines: HashMap<usize, usize> = HashMap::new();
            for line in lines.iter() {
                *author_lines.entry(line.author_id).or_insert(0) += 1;
            }
            let total: usize = author_lines.values().sum();
            let max: usize = author_lines.values().copied().max().unwrap_or(0);
            max * 2 > total
        })
        .count();

    let pct = (dominated as f64 / total_files as f64) * 100.0;

    let score = if pct < 10.0 {
        100
    } else if pct < 25.0 {
        75
    } else if pct < 50.0 {
        50
    } else {
        25
    };

    MetricValue {
        name: "Bus factor".to_string(),
        description: format!("{:.0}% of files single-author dominated", pct),
        raw_value: RawValue::Percentage(pct),
        score,
    }
}

/// Files with highest change frequency.
fn churn_hotspots(snapshot: &RepoSnapshot, _thresholds: &HealthThresholds) -> MetricValue {
    if snapshot.commits_by_file.is_empty() {
        return MetricValue {
            name: "Churn hotspots".to_string(),
            description: "No commit data".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 100,
        };
    }

    let mut file_counts: Vec<(&PathBuf, usize)> = snapshot
        .commits_by_file
        .iter()
        .map(|(path, commits)| (path, commits.len()))
        .collect();

    file_counts.sort_by(|a, b| b.1.cmp(&a.1));

    // Top 5% of files by change frequency
    let threshold_idx = (file_counts.len() as f64 * 0.05).ceil() as usize;
    let threshold_idx = threshold_idx.max(1).min(file_counts.len());

    let hotspots: Vec<String> = file_counts[..threshold_idx]
        .iter()
        .map(|(p, c)| format!("{} ({})", p.display(), c))
        .collect();

    let total_changes: usize = file_counts.iter().map(|(_, c)| *c).sum();
    let hotspot_changes: usize = file_counts[..threshold_idx].iter().map(|(_, c)| *c).sum();

    let concentration = if total_changes > 0 {
        (hotspot_changes as f64 / total_changes as f64) * 100.0
    } else {
        0.0
    };

    // If top 5% accounts for >60% of changes, that's bad
    let score = if concentration > 60.0 {
        30
    } else if concentration > 40.0 {
        60
    } else {
        90
    };

    MetricValue {
        name: "Churn hotspots".to_string(),
        description: format!(
            "{} files account for {:.0}% of changes",
            threshold_idx, concentration
        ),
        raw_value: RawValue::List(hotspots),
        score,
    }
}

/// File pairs that change together suspiciously often.
fn temporal_coupling(snapshot: &RepoSnapshot, _thresholds: &HealthThresholds) -> MetricValue {
    let suspicious: Vec<String> = snapshot
        .file_change_pairs
        .iter()
        .filter(|(a, b, count)| {
            let a_changes = snapshot
                .commits_by_file
                .get(a)
                .map(|c| c.len())
                .unwrap_or(0);
            let b_changes = snapshot
                .commits_by_file
                .get(b)
                .map(|c| c.len())
                .unwrap_or(0);
            let min_changes = a_changes.min(b_changes);
            if min_changes == 0 {
                return false;
            }
            (*count as f64 / min_changes as f64) > 0.7
        })
        .map(|(a, b, count)| format!("{} <> {} ({} co-changes)", a.display(), b.display(), count))
        .collect();

    let count = suspicious.len();
    let score = match count {
        0 => 100,
        1..=3 => 75,
        4..=8 => 50,
        _ => 25,
    };

    MetricValue {
        name: "Temporal coupling".to_string(),
        description: format!("{} suspicious file pairs detected", count),
        raw_value: RawValue::Count(count),
        score,
    }
}

/// Files not touched in the time window.
fn stale_code(snapshot: &RepoSnapshot) -> MetricValue {
    if snapshot.files.is_empty() {
        return MetricValue {
            name: "Stale code".to_string(),
            description: "No files".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 100,
        };
    }

    let stale_count = snapshot
        .files
        .iter()
        .filter(|f| !f.is_binary)
        .filter(|f| {
            snapshot
                .commits_by_file
                .get(&f.path)
                .map(|commits| {
                    !commits.iter().any(|cid| {
                        snapshot
                            .commits
                            .iter()
                            .any(|c| &c.id == cid && snapshot.time_window.contains(&c.timestamp))
                    })
                })
                .unwrap_or(true) // File not in commits_by_file = stale
        })
        .count();

    let total_files = snapshot.files.iter().filter(|f| !f.is_binary).count();
    let pct = if total_files > 0 {
        (stale_count as f64 / total_files as f64) * 100.0
    } else {
        0.0
    };

    let score = if pct > 50.0 {
        25
    } else if pct > 30.0 {
        50
    } else if pct > 10.0 {
        75
    } else {
        100
    };

    MetricValue {
        name: "Stale code".to_string(),
        description: format!("{:.0}% of files untouched in window", pct),
        raw_value: RawValue::Percentage(pct),
        score,
    }
}

/// Files that have grown too large to maintain (god objects / bloaters).
fn god_objects(snapshot: &RepoSnapshot) -> MetricValue {
    let gods: Vec<String> = snapshot
        .file_metrics
        .iter()
        .filter(|(_, m)| m.loc > 500 || (m.loc > 300 && m.public_methods > 15))
        .map(|(p, _)| p.display().to_string())
        .collect();

    let count = gods.len();
    let score = match count {
        0 => 100,
        1..=2 => 75,
        3..=5 => 50,
        _ => 25,
    };

    MetricValue {
        name: "God objects".to_string(),
        description: format!("{} oversized files detected", count),
        raw_value: RawValue::List(gods),
        score,
    }
}

/// Files in the top quartile of both cyclomatic complexity and churn — the Tornhill composite.
fn complex_hotspots(snapshot: &RepoSnapshot) -> MetricValue {
    if snapshot.file_metrics.is_empty() {
        return MetricValue {
            name: "Complex hotspots".to_string(),
            description: "No AST data available".to_string(),
            raw_value: RawValue::Count(0),
            score: 100,
        };
    }

    let mut cc_values: Vec<u32> = snapshot.file_metrics.values()
        .map(|m| m.cyclomatic_complexity)
        .collect();
    cc_values.sort_unstable();
    let cc_p75 = cc_values
        .get(cc_values.len().saturating_sub(1) * 3 / 4)
        .copied()
        .unwrap_or(0);

    let mut churn_values: Vec<usize> = snapshot.commits_by_file.values()
        .map(|c| c.len())
        .collect();
    churn_values.sort_unstable();
    let churn_p75 = churn_values
        .get(churn_values.len().saturating_sub(1) * 3 / 4)
        .copied()
        .unwrap_or(0);

    let hotspots: Vec<String> = snapshot
        .file_metrics
        .iter()
        .filter(|(path, m)| {
            let churn = snapshot.commits_by_file.get(*path).map(|c| c.len()).unwrap_or(0);
            m.cyclomatic_complexity > cc_p75 && churn > churn_p75
        })
        .map(|(p, _)| p.display().to_string())
        .collect();

    let count = hotspots.len();
    let score = match count {
        0 => 100,
        1..=2 => 75,
        3..=5 => 50,
        _ => 25,
    };

    MetricValue {
        name: "Complex hotspots".to_string(),
        description: format!("{} files with high complexity and high churn", count),
        raw_value: RawValue::List(hotspots),
        score,
    }
}

/// File size distribution and directory nesting depth.
fn file_complexity(snapshot: &RepoSnapshot, _thresholds: &HealthThresholds) -> MetricValue {
    let large_files = snapshot
        .files
        .iter()
        .filter(|f| !f.is_binary && f.size_bytes > 50_000) // ~1000 lines approx
        .count();

    let deep_dirs = snapshot.files.iter().filter(|f| f.depth > 5).count();

    let binary_count = snapshot.files.iter().filter(|f| f.is_binary).count();

    let issues = large_files + deep_dirs;
    let score = match issues {
        0 => 100,
        1..=3 => 80,
        4..=8 => 60,
        _ => 40,
    };

    MetricValue {
        name: "File complexity".to_string(),
        description: format!(
            "{} large files, {} deep-nested dirs, {} binaries",
            large_files, deep_dirs, binary_count
        ),
        raw_value: RawValue::Count(issues),
        score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::*;
    use chrono::{Duration, Utc};

    fn make_snapshot_with_blame() -> RepoSnapshot {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        // 2 authors
        snapshot.authors = vec![
            Author {
                id: 0,
                name: "Alice".into(),
                email: "alice@test.com".into(),
            },
            Author {
                id: 1,
                name: "Bob".into(),
                email: "bob@test.com".into(),
            },
        ];

        // Blame: file1 is 80% Alice, 20% Bob (bus factor = 1)
        let now = Utc::now();
        let mut blame_file1 = Vec::new();
        for _ in 0..80 {
            blame_file1.push(BlameLine {
                author_id: 0,
                commit_id: "c1".into(),
                timestamp: now,
            });
        }
        for _ in 0..20 {
            blame_file1.push(BlameLine {
                author_id: 1,
                commit_id: "c2".into(),
                timestamp: now,
            });
        }
        snapshot
            .blame_map
            .insert(PathBuf::from("file1.rs"), blame_file1);

        snapshot
    }

    #[test]
    fn bus_factor_detects_single_author_dominance() {
        let snapshot = make_snapshot_with_blame();
        let result = bus_factor(&snapshot, &HealthThresholds::default());
        // Alice owns 80% → 1/1 file dominated → 100% → score = 25
        assert_eq!(result.score, 25);
        match result.raw_value {
            RawValue::Percentage(p) => assert!((p - 100.0).abs() < 1.0),
            _ => panic!("Expected Percentage"),
        }
    }

    #[test]
    fn bus_factor_scores_100_when_few_dominated() {
        // 5 files, all 50/50 split → 0% dominated → score 100
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let now = Utc::now();
        for i in 0..5 {
            let lines: Vec<BlameLine> = (0..100)
                .map(|j| BlameLine {
                    author_id: if j < 50 { 0 } else { 1 },
                    commit_id: format!("c{}", j),
                    timestamp: now,
                })
                .collect();
            snapshot
                .blame_map
                .insert(PathBuf::from(format!("f{}.rs", i)), lines);
        }
        let result = bus_factor(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, 100);
        match result.raw_value {
            RawValue::Percentage(p) => assert!((p - 0.0).abs() < 1.0),
            _ => panic!("Expected Percentage"),
        }
    }

    #[test]
    fn bus_factor_scores_75_when_some_dominated() {
        // 5 files: 1 dominated (author 0 owns 80%) + 4 not dominated → 20% → score 75
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let now = Utc::now();
        // 1 dominated file: author 0 owns 80%
        let dominated: Vec<BlameLine> = (0..100)
            .map(|j| BlameLine {
                author_id: if j < 80 { 0 } else { 1 },
                commit_id: format!("c{}", j),
                timestamp: now,
            })
            .collect();
        snapshot
            .blame_map
            .insert(PathBuf::from("dominated.rs"), dominated);
        // 4 balanced files: 50/50
        for i in 0..4 {
            let lines: Vec<BlameLine> = (0..100)
                .map(|j| BlameLine {
                    author_id: if j < 50 { 0 } else { 1 },
                    commit_id: format!("c{}{}", i, j),
                    timestamp: now,
                })
                .collect();
            snapshot
                .blame_map
                .insert(PathBuf::from(format!("balanced{}.rs", i)), lines);
        }
        let result = bus_factor(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, 75);
    }

    #[test]
    fn bus_factor_exact_50pct_not_dominated() {
        // A file where author 0 owns exactly 50% of lines is NOT dominated
        // because dominance requires max * 2 > total (strict majority)
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let now = Utc::now();
        let lines: Vec<BlameLine> = (0..100)
            .map(|j| BlameLine {
                author_id: if j < 50 { 0 } else { 1 }, // exactly 50/50
                commit_id: format!("c{}", j),
                timestamp: now,
            })
            .collect();
        snapshot
            .blame_map
            .insert(PathBuf::from("file.rs"), lines);
        let result = bus_factor(&snapshot, &HealthThresholds::default());
        // 0% dominated → score 100
        assert_eq!(result.score, 100);
        match result.raw_value {
            RawValue::Percentage(p) => assert!((p - 0.0).abs() < 1.0),
            _ => panic!("Expected Percentage"),
        }
    }

    #[test]
    fn churn_hotspots_detects_concentration() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        // 10 files, 1 has ~75% of all commits
        let mut commits_by_file = HashMap::new();
        commits_by_file.insert(
            PathBuf::from("hot.rs"),
            (0..30).map(|i| format!("c{}", i)).collect(),
        );
        for i in 1..=9 {
            commits_by_file.insert(
                PathBuf::from(format!("file{}.rs", i)),
                vec![format!("x{}", i)],
            );
        }
        snapshot.commits_by_file = commits_by_file;

        let result = churn_hotspots(&snapshot, &HealthThresholds::default());
        assert!(result.score <= 60, "High concentration should lower score");
    }

    #[test]
    fn temporal_coupling_detects_pairs() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        // A and B co-change 9 times, A changes 10 times, B changes 10 times
        snapshot.file_change_pairs = vec![(PathBuf::from("a.rs"), PathBuf::from("b.rs"), 9)];
        snapshot.commits_by_file.insert(
            PathBuf::from("a.rs"),
            (0..10).map(|i| format!("c{}", i)).collect(),
        );
        snapshot.commits_by_file.insert(
            PathBuf::from("b.rs"),
            (0..10).map(|i| format!("c{}", i)).collect(),
        );

        let result = temporal_coupling(&snapshot, &HealthThresholds::default());
        match result.raw_value {
            RawValue::Count(c) => assert_eq!(c, 1),
            _ => panic!("Expected Count"),
        }
    }

    #[test]
    fn stale_code_detects_untouched_files() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        let now = Utc::now();
        snapshot.files = (0..5)
            .map(|i| FileEntry {
                path: PathBuf::from(format!("f{}.rs", i)),
                size_bytes: 100,
                is_binary: false,
                depth: 1,
                blob_oid: String::new(),
            })
            .collect();

        // Only 3 files have recent commits
        for i in 0..3 {
            snapshot
                .commits_by_file
                .insert(PathBuf::from(format!("f{}.rs", i)), vec![format!("c{}", i)]);
            snapshot.commits.push(Commit {
                id: format!("c{}", i),
                author: 0,
                timestamp: now - Duration::days(10),
                message: "msg".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            });
        }

        let result = stale_code(&snapshot);
        match result.raw_value {
            RawValue::Percentage(p) => assert!((p - 40.0).abs() < 1.0, "Expected ~40% stale"),
            _ => panic!("Expected Percentage"),
        }
    }

    #[test]
    fn god_objects_detects_large_files() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"), "test".into(), "main".into(), TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            PathBuf::from("fat.rs"),
            FileComplexity { total_lines: 600, loc: 520, cyclomatic_complexity: 10,
                             public_methods: 5, properties: 2, demeter_violations: 0 },
        );
        snapshot.file_metrics.insert(
            PathBuf::from("small.rs"),
            FileComplexity { total_lines: 100, loc: 80, cyclomatic_complexity: 3,
                             public_methods: 2, properties: 1, demeter_violations: 0 },
        );
        let result = god_objects(&snapshot);
        assert_eq!(result.score, 75); // 1 god object
        match &result.raw_value {
            RawValue::List(v) => assert_eq!(v.len(), 1),
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn god_objects_detects_method_bloat() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"), "test".into(), "main".into(), TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            PathBuf::from("bloated.rs"),
            FileComplexity { total_lines: 350, loc: 310, cyclomatic_complexity: 5,
                             public_methods: 16, properties: 3, demeter_violations: 0 },
        );
        let result = god_objects(&snapshot);
        assert_eq!(result.score, 75); // 1 god object (LOC>300 AND methods>15)
    }

    #[test]
    fn god_objects_scores_100_when_none() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"), "test".into(), "main".into(), TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            PathBuf::from("normal.rs"),
            FileComplexity { total_lines: 100, loc: 80, cyclomatic_complexity: 3,
                             public_methods: 5, properties: 1, demeter_violations: 0 },
        );
        let result = god_objects(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn complex_hotspots_finds_high_cc_high_churn_files() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"), "test".into(), "main".into(), TimeWindow::default(),
        );
        // 4 files: only "bad.rs" is in top quartile of both CC and churn
        let files: &[(&str, u32, usize)] = &[
            ("bad.rs",  20, 20), // high CC (top 25%), high churn (top 25%)
            ("ok1.rs",   2,  1),
            ("ok2.rs",   3,  2),
            ("ok3.rs",   4,  3),
        ];
        for (name, cc, churn) in files {
            snapshot.file_metrics.insert(
                PathBuf::from(name),
                FileComplexity { total_lines: 100, loc: 80, cyclomatic_complexity: *cc,
                                 public_methods: 2, properties: 1, demeter_violations: 0 },
            );
            snapshot.commits_by_file.insert(
                PathBuf::from(name),
                (0..*churn).map(|i| format!("c{}", i)).collect(),
            );
        }
        let result = complex_hotspots(&snapshot);
        assert_eq!(result.score, 75); // 1 hotspot → score 75
        match &result.raw_value {
            RawValue::List(v) => assert_eq!(v.len(), 1),
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn complex_hotspots_scores_100_when_none() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"), "test".into(), "main".into(), TimeWindow::default(),
        );
        // All files have similar CC and churn — no outliers in top quartile of BOTH
        for i in 0..4 {
            snapshot.file_metrics.insert(
                PathBuf::from(format!("f{}.rs", i)),
                FileComplexity { total_lines: 100, loc: 80, cyclomatic_complexity: 5,
                                 public_methods: 2, properties: 1, demeter_violations: 0 },
            );
            snapshot.commits_by_file.insert(
                PathBuf::from(format!("f{}.rs", i)),
                vec![format!("c{}", i)],
            );
        }
        let result = complex_hotspots(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn file_complexity_flags_large_and_deep() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        snapshot.files = vec![
            FileEntry {
                path: "big.rs".into(),
                size_bytes: 100_000,
                is_binary: false,
                depth: 1,
                blob_oid: String::new(),
            },
            FileEntry {
                path: "deep/a/b/c/d/e/f.rs".into(),
                size_bytes: 100,
                is_binary: false,
                depth: 7,
                blob_oid: String::new(),
            },
            FileEntry {
                path: "normal.rs".into(),
                size_bytes: 500,
                is_binary: false,
                depth: 1,
                blob_oid: String::new(),
            },
        ];

        let result = file_complexity(&snapshot, &HealthThresholds::default());
        match result.raw_value {
            RawValue::Count(c) => assert_eq!(c, 2, "1 large + 1 deep = 2"),
            _ => panic!("Expected Count"),
        }
    }
}
