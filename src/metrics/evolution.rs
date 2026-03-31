use std::collections::HashMap;

use chrono::Utc;

use crate::metrics::{CategoryResult, MetricValue, RawValue};
use crate::snapshot::{ChangeType, RepoSnapshot};

pub fn compute_evolution(
    snapshot: &RepoSnapshot,
    thresholds: &crate::config::EvolutionThresholds,
) -> CategoryResult {
    let metrics = vec![
        growth_trend(snapshot, thresholds),
        refactoring_ratio(snapshot, thresholds),
        code_age(snapshot, thresholds),
        commit_cadence(snapshot, thresholds),
    ];

    CategoryResult {
        name: "Evolution".to_string(),
        score: 0,
        metrics,
    }
    .compute_score()
}

/// Net file count change over the time window.
fn growth_trend(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::EvolutionThresholds,
) -> MetricValue {
    if snapshot.commits.is_empty() {
        return MetricValue {
            name: "Growth trend".to_string(),
            description: "No commits".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
        };
    }

    let mut files_added: i64 = 0;
    let mut files_deleted: i64 = 0;
    let mut lines_added: i64 = 0;
    let mut lines_deleted: i64 = 0;

    for commit in &snapshot.commits {
        if !snapshot.time_window.contains(&commit.timestamp) {
            continue;
        }
        for fc in &commit.files_changed {
            match fc.change_type {
                ChangeType::Added => files_added += 1,
                ChangeType::Deleted => files_deleted += 1,
                _ => {}
            }
            lines_added += fc.additions as i64;
            lines_deleted += fc.deletions as i64;
        }
    }

    let net_files = files_added - files_deleted;
    let net_lines = lines_added - lines_deleted;

    // Rapid growth can be a smell (more code = more maintenance)
    let total_files = snapshot.files.len() as i64;
    let growth_pct = if total_files > 0 {
        (net_files as f64 / total_files as f64) * 100.0
    } else {
        0.0
    };

    let score = if growth_pct.abs() > 50.0 {
        40 // Rapid change (growth or shrink)
    } else if growth_pct.abs() > 20.0 {
        65
    } else {
        90 // Stable
    };

    MetricValue {
        name: "Growth trend".to_string(),
        description: format!("{:+} files, {:+} lines in window", net_files, net_lines),
        raw_value: RawValue::Integer(net_files),
        score,
    }
}

const STRUCTURAL_KEYWORDS: &[&str] = &[
    "refactor",
    "restructur",
    "reorganiz",
    "extract",
    "tidy",
    "clean up",
    "simplif",
    "consolidat",
    "rename",
    "move",
    "dedup",
    "remove dead",
    "dead code",
];

fn is_structural_investment(commit: &crate::snapshot::Commit) -> bool {
    let msg = commit.message.to_lowercase();
    if STRUCTURAL_KEYWORDS.iter().any(|kw| msg.contains(kw)) {
        return true;
    }
    let total_del: u32 = commit.files_changed.iter().map(|fc| fc.deletions).sum();
    let total_add: u32 = commit.files_changed.iter().map(|fc| fc.additions).sum();
    for fc in &commit.files_changed {
        match fc.change_type {
            ChangeType::Renamed | ChangeType::Deleted => return true,
            _ => {}
        }
    }
    if total_del > 50 {
        let denom = total_add + total_del;
        if denom > 0 && (total_del as f64 / denom as f64) > 0.40 {
            return true;
        }
    }
    false
}

/// Ratio of commits that invest in structural maintenance (refactoring, cleanup, reorganization).
fn refactoring_ratio(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::EvolutionThresholds,
) -> MetricValue {
    if snapshot.commits.is_empty() {
        return MetricValue {
            name: "Refactoring ratio".to_string(),
            description: "No commits".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
        };
    }

    let window_commits: Vec<_> = snapshot
        .commits
        .iter()
        .filter(|c| snapshot.time_window.contains(&c.timestamp) && !c.is_merge)
        .collect();

    if window_commits.is_empty() {
        return MetricValue {
            name: "Refactoring ratio".to_string(),
            description: "No commits in window".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
        };
    }

    let total = window_commits.len();
    let n_structural = window_commits
        .iter()
        .filter(|c| is_structural_investment(c))
        .count();

    let ratio = n_structural as f64 / total as f64;
    let pct = ratio * 100.0;

    // More structural investment is always better — directional scoring.
    let score = if ratio < 0.05 {
        25 // structural debt accumulating
    } else if ratio < 0.15 {
        55 // low investment
    } else if ratio < 0.30 {
        80 // healthy investment
    } else {
        92 // strong investment
    };

    MetricValue {
        name: "Refactoring ratio".to_string(),
        description: format!(
            "{} of {} commits invest in structure ({:.0}%)",
            n_structural, total, pct
        ),
        raw_value: RawValue::Float(ratio),
        score,
    }
}

/// Median age of code based on blame timestamps.
fn code_age(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::EvolutionThresholds,
) -> MetricValue {
    let mut timestamps: Vec<_> = snapshot
        .blame_map
        .values()
        .flat_map(|lines| lines.iter().map(|l| l.timestamp))
        .collect();

    if timestamps.is_empty() {
        return MetricValue {
            name: "Code age".to_string(),
            description: "No blame data".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
        };
    }

    timestamps.sort();
    let median = timestamps[timestamps.len() / 2];
    let now = Utc::now();
    let age_days = (now - median).num_days();
    let age_months = age_days as f64 / 30.0;

    let description = if age_months > 12.0 {
        format!("{:.0} months (median code age)", age_months)
    } else {
        format!("{:.1} months (median code age)", age_months)
    };

    // Very old code might be stale; very new code might be unstable
    let score = if age_months > 24.0 {
        40
    } else if age_months > 12.0 {
        60
    } else if age_months > 3.0 {
        90 // Sweet spot
    } else {
        70 // Very new
    };

    MetricValue {
        name: "Code age".to_string(),
        description,
        raw_value: RawValue::Float(age_months),
        score,
    }
}

/// Commit frequency and regularity.
fn commit_cadence(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::EvolutionThresholds,
) -> MetricValue {
    let window_commits: Vec<_> = snapshot
        .commits
        .iter()
        .filter(|c| snapshot.time_window.contains(&c.timestamp))
        .collect();

    if window_commits.is_empty() {
        return MetricValue {
            name: "Commit cadence".to_string(),
            description: "No commits in window".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
        };
    }

    // Group commits by day
    let mut daily_counts: HashMap<i64, usize> = HashMap::new();
    for commit in &window_commits {
        let day = commit.timestamp.date_naive().and_hms_opt(0, 0, 0).unwrap();
        let day_key = day.and_utc().timestamp() / 86400;
        *daily_counts.entry(day_key).or_insert(0) += 1;
    }

    let counts: Vec<f64> = daily_counts.values().map(|&c| c as f64).collect();
    let n = counts.len() as f64;
    let mean = counts.iter().sum::<f64>() / n;

    let variance = counts.iter().map(|c| (c - mean).powi(2)).sum::<f64>() / n;
    let std_dev = variance.sqrt();
    let cv = if mean > 0.0 { std_dev / mean } else { 0.0 }; // Coefficient of variation

    let regularity = if cv < 0.5 {
        "regular"
    } else if cv < 1.0 {
        "moderate"
    } else {
        "irregular"
    };

    let total_days = if let (Some(first), Some(last)) = (
        window_commits.iter().map(|c| c.timestamp).min(),
        window_commits.iter().map(|c| c.timestamp).max(),
    ) {
        ((last - first).num_days() + 1).max(1) as f64
    } else {
        1.0
    };
    let commits_per_day = window_commits.len() as f64 / total_days;

    let score = match regularity {
        "regular" => 90,
        "moderate" => 70,
        _ => 50,
    };

    MetricValue {
        name: "Commit cadence".to_string(),
        description: format!("{:.1} commits/day, {} pattern", commits_per_day, regularity),
        raw_value: RawValue::Float(commits_per_day),
        score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::*;
    use chrono::Duration;
    use std::path::PathBuf;

    #[test]
    fn growth_trend_detects_net_growth() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        let now = Utc::now();
        snapshot.files = (0..100)
            .map(|i| FileEntry {
                path: PathBuf::from(format!("f{}.rs", i)),
                size_bytes: 100,
                is_binary: false,
                depth: 1,
                blob_oid: String::new(),
            })
            .collect();

        // 15 files added, 0 deleted → +15% growth
        snapshot.commits.push(Commit {
            id: "c1".into(),
            author: 0,
            timestamp: now - Duration::days(10),
            message: "add files".into(),
            files_changed: (0..15)
                .map(|i| FileChange {
                    path: PathBuf::from(format!("new{}.rs", i)),
                    additions: 50,
                    deletions: 0,
                    change_type: ChangeType::Added,
                })
                .collect(),
            is_merge: false,
            parent_count: 1,
        });

        let result = growth_trend(&snapshot, &crate::config::EvolutionThresholds::default());
        match result.raw_value {
            RawValue::Integer(v) => assert_eq!(v, 15),
            _ => panic!("Expected Integer"),
        }
    }

    fn make_snapshot() -> RepoSnapshot {
        RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        )
    }

    fn plain_commit(id: &str, msg: &str, ts: chrono::DateTime<Utc>) -> Commit {
        Commit {
            id: id.into(),
            author: 0,
            timestamp: ts,
            message: msg.into(),
            files_changed: vec![FileChange {
                path: PathBuf::from("src/lib.rs"),
                additions: 5,
                deletions: 3,
                change_type: ChangeType::Modified,
            }],
            is_merge: false,
            parent_count: 1,
        }
    }

    #[test]
    fn structural_investment_keyword_commits() {
        // 3 "refactor" commits out of 10 total → ratio 0.30 → score 92
        let mut snapshot = make_snapshot();
        let now = Utc::now();
        for i in 0..7 {
            snapshot.commits.push(plain_commit(
                &format!("p{}", i),
                "add feature",
                now - Duration::days(i + 1),
            ));
        }
        for i in 0..3 {
            snapshot.commits.push(plain_commit(
                &format!("r{}", i),
                "refactor module layout",
                now - Duration::days(i + 8),
            ));
        }
        let result = refactoring_ratio(&snapshot, &crate::config::EvolutionThresholds::default());
        match result.raw_value {
            RawValue::Float(r) => assert!((r - 0.30).abs() < 0.01, "Expected ~0.30, got {}", r),
            _ => panic!("Expected Float"),
        }
        assert_eq!(result.score, 92);
    }

    #[test]
    fn structural_investment_rename_commits() {
        // commits with ChangeType::Renamed are counted as structural
        let mut snapshot = make_snapshot();
        let now = Utc::now();
        for i in 0..8 {
            snapshot.commits.push(plain_commit(
                &format!("p{}", i),
                "fix bug",
                now - Duration::days(i + 1),
            ));
        }
        for i in 0..2 {
            snapshot.commits.push(Commit {
                id: format!("rn{}", i),
                author: 0,
                timestamp: now - Duration::days(i as i64 + 9),
                message: "update path".into(),
                files_changed: vec![FileChange {
                    path: PathBuf::from("old.rs"),
                    additions: 0,
                    deletions: 0,
                    change_type: ChangeType::Renamed,
                }],
                is_merge: false,
                parent_count: 1,
            });
        }
        let result = refactoring_ratio(&snapshot, &crate::config::EvolutionThresholds::default());
        match result.raw_value {
            RawValue::Float(r) => assert!((r - 0.20).abs() < 0.01, "Expected ~0.20, got {}", r),
            _ => panic!("Expected Float"),
        }
        assert!(result.score >= 80);
    }

    #[test]
    fn structural_investment_deletion_commits() {
        // commits with ChangeType::Deleted files are counted as structural
        let mut snapshot = make_snapshot();
        let now = Utc::now();
        for i in 0..9 {
            snapshot.commits.push(plain_commit(
                &format!("p{}", i),
                "add stuff",
                now - Duration::days(i + 1),
            ));
        }
        snapshot.commits.push(Commit {
            id: "del1".into(),
            author: 0,
            timestamp: now - Duration::days(10),
            message: "remove unused module".into(),
            files_changed: vec![FileChange {
                path: PathBuf::from("old_module.rs"),
                additions: 0,
                deletions: 200,
                change_type: ChangeType::Deleted,
            }],
            is_merge: false,
            parent_count: 1,
        });
        let result = refactoring_ratio(&snapshot, &crate::config::EvolutionThresholds::default());
        match result.raw_value {
            RawValue::Float(r) => assert!((r - 0.10).abs() < 0.01, "Expected ~0.10, got {}", r),
            _ => panic!("Expected Float"),
        }
        assert_eq!(result.score, 55);
    }

    #[test]
    fn structural_investment_none_scores_low() {
        // all ChangeType::Added commits → ratio 0.0 → score 25
        let mut snapshot = make_snapshot();
        let now = Utc::now();
        for i in 0..10 {
            snapshot.commits.push(Commit {
                id: format!("a{}", i),
                author: 0,
                timestamp: now - Duration::days(i + 1),
                message: "add new file".into(),
                files_changed: vec![FileChange {
                    path: PathBuf::from(format!("new{}.rs", i)),
                    additions: 20,
                    deletions: 0,
                    change_type: ChangeType::Added,
                }],
                is_merge: false,
                parent_count: 1,
            });
        }
        let result = refactoring_ratio(&snapshot, &crate::config::EvolutionThresholds::default());
        match result.raw_value {
            RawValue::Float(r) => assert_eq!(r, 0.0),
            _ => panic!("Expected Float"),
        }
        assert_eq!(result.score, 25);
    }

    #[test]
    fn code_age_computes_median() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::full_history(),
        );

        let now = Utc::now();
        let eight_months_ago = now - Duration::days(240);
        let mut blame = Vec::new();
        for _ in 0..100 {
            blame.push(BlameLine {
                author_id: 0,
                timestamp: eight_months_ago,
            });
        }
        snapshot.blame_map.insert(PathBuf::from("f.rs"), blame);

        let result = code_age(&snapshot, &crate::config::EvolutionThresholds::default());
        match result.raw_value {
            RawValue::Float(months) => {
                assert!(
                    months > 7.0 && months < 9.0,
                    "Expected ~8 months, got {}",
                    months
                )
            }
            _ => panic!("Expected Float"),
        }
    }

    #[test]
    fn commit_cadence_detects_regularity() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        let now = Utc::now();
        // 4 commits per day for 30 days → regular
        for day in 0..30 {
            for i in 0..4 {
                snapshot.commits.push(Commit {
                    id: format!("c{}_{}", day, i),
                    author: 0,
                    timestamp: now - Duration::days(day) + Duration::hours(i),
                    message: "work".into(),
                    files_changed: vec![],
                    is_merge: false,
                    parent_count: 1,
                });
            }
        }

        let result = commit_cadence(&snapshot, &crate::config::EvolutionThresholds::default());
        assert!(result.description.contains("regular") || result.description.contains("moderate"));
        assert!(result.score >= 70);
    }
}
