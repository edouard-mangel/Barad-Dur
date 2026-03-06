use std::collections::HashMap;
use std::path::PathBuf;

use crate::metrics::{CategoryResult, MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

pub fn compute_health(snapshot: &RepoSnapshot) -> CategoryResult {
    let metrics = vec![
        bus_factor(snapshot),
        churn_hotspots(snapshot),
        temporal_coupling(snapshot),
        stale_code(snapshot),
        file_complexity(snapshot),
    ];

    CategoryResult {
        name: "Health".to_string(),
        score: 0,
        metrics,
    }
    .compute_score()
}

/// Minimum number of authors needed to cover 50% of code knowledge.
fn bus_factor(snapshot: &RepoSnapshot) -> MetricValue {
    if snapshot.blame_map.is_empty() {
        return MetricValue {
            name: "Bus factor".to_string(),
            description: "No blame data available".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
        };
    }

    let mut min_bus_factor = usize::MAX;

    for blame_lines in snapshot.blame_map.values() {
        if blame_lines.is_empty() {
            continue;
        }

        // Count lines per author
        let mut author_lines: HashMap<usize, usize> = HashMap::new();
        for line in blame_lines {
            *author_lines.entry(line.author_id).or_insert(0) += 1;
        }

        // Sort by lines descending
        let mut counts: Vec<usize> = author_lines.values().copied().collect();
        counts.sort_unstable_by(|a, b| b.cmp(a));

        let total: usize = counts.iter().sum();
        let threshold = total / 2;
        let mut accumulated = 0;
        let mut authors_needed = 0;

        for count in &counts {
            accumulated += count;
            authors_needed += 1;
            if accumulated >= threshold {
                break;
            }
        }

        min_bus_factor = min_bus_factor.min(authors_needed);
    }

    if min_bus_factor == usize::MAX {
        min_bus_factor = 0;
    }

    let score = match min_bus_factor {
        0 => 0,
        1 => 20,
        2 => 50,
        3 => 75,
        _ => 100,
    };

    let label = match min_bus_factor {
        0 => "none",
        1 => "critical",
        2 => "risky",
        3 => "good",
        _ => "excellent",
    };

    MetricValue {
        name: "Bus factor".to_string(),
        description: format!("{} ({})", min_bus_factor, label),
        raw_value: RawValue::Integer(min_bus_factor as i64),
        score,
    }
}

/// Files with highest change frequency.
fn churn_hotspots(snapshot: &RepoSnapshot) -> MetricValue {
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
fn temporal_coupling(snapshot: &RepoSnapshot) -> MetricValue {
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

/// File size distribution and directory nesting depth.
fn file_complexity(snapshot: &RepoSnapshot) -> MetricValue {
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
        let result = bus_factor(&snapshot);
        // Alice owns 80% → bus factor = 1
        assert_eq!(result.score, 20);
        match result.raw_value {
            RawValue::Integer(v) => assert_eq!(v, 1),
            _ => panic!("Expected Integer"),
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

        let result = churn_hotspots(&snapshot);
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

        let result = temporal_coupling(&snapshot);
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
            },
            FileEntry {
                path: "deep/a/b/c/d/e/f.rs".into(),
                size_bytes: 100,
                is_binary: false,
                depth: 7,
            },
            FileEntry {
                path: "normal.rs".into(),
                size_bytes: 500,
                is_binary: false,
                depth: 1,
            },
        ];

        let result = file_complexity(&snapshot);
        match result.raw_value {
            RawValue::Count(c) => assert_eq!(c, 2, "1 large + 1 deep = 2"),
            _ => panic!("Expected Count"),
        }
    }
}
