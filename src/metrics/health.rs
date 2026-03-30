use std::collections::HashMap;

use crate::config::HealthThresholds;
use crate::metrics::{CategoryResult, MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

pub fn compute_health(snapshot: &RepoSnapshot, _thresholds: &HealthThresholds) -> CategoryResult {
    let metrics = vec![
        bus_factor(snapshot, _thresholds),
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
/// For solo projects (single author), this metric is not applicable and scores 100.
fn bus_factor(snapshot: &RepoSnapshot, _thresholds: &HealthThresholds) -> MetricValue {
    if snapshot.authors.len() <= 1 {
        return MetricValue {
            name: "Bus factor".to_string(),
            description: "Solo project — not applicable".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 100,
        };
    }

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

fn is_source_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).unwrap_or(""),
        "rs" | "py" | "go" | "java" | "cs" | "js" | "ts" | "tsx" | "jsx"
            | "kt" | "cpp" | "c" | "h" | "hpp" | "rb" | "php" | "swift" | "scala"
    )
}

/// Files that have grown too large to maintain (god objects / bloaters).
fn god_objects(snapshot: &RepoSnapshot) -> MetricValue {
    let source_total = snapshot
        .file_metrics
        .keys()
        .filter(|p| is_source_file(p))
        .count();

    let gods: Vec<String> = snapshot
        .file_metrics
        .iter()
        .filter(|(p, m)| {
            is_source_file(p)
                && m.cyclomatic_complexity > 0
                && (m.loc > 500 || (m.loc > 300 && m.public_methods > 15))
        })
        .map(|(p, _)| p.display().to_string())
        .collect();

    let count = gods.len();
    let pct = if source_total > 0 {
        count as f64 / source_total as f64 * 100.0
    } else {
        0.0
    };

    let score = if count == 0 {
        100
    } else if pct <= 2.0 {
        75
    } else if pct <= 8.0 {
        50
    } else {
        25
    };

    MetricValue {
        name: "God objects".to_string(),
        description: format!(
            "{}/{} source files oversized ({:.1}%)",
            count, source_total, pct
        ),
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

    let mut cc_values: Vec<u32> = snapshot
        .file_metrics
        .values()
        .map(|m| m.cyclomatic_complexity)
        .collect();
    cc_values.sort_unstable();
    let cc_p75 = cc_values
        .get(cc_values.len().saturating_sub(1) * 3 / 4)
        .copied()
        .unwrap_or(0);

    let mut churn_values: Vec<usize> = snapshot.commits_by_file.values().map(|c| c.len()).collect();
    churn_values.sort_unstable();
    let churn_p75 = churn_values
        .get(churn_values.len().saturating_sub(1) * 3 / 4)
        .copied()
        .unwrap_or(0);

    let hotspots: Vec<String> = snapshot
        .file_metrics
        .iter()
        .filter(|(path, m)| {
            let churn = snapshot
                .commits_by_file
                .get(*path)
                .map(|c| c.len())
                .unwrap_or(0);
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::snapshot::*;
    use chrono::Utc;

    fn add_normal_files(snapshot: &mut RepoSnapshot, count: usize) {
        for i in 0..count {
            snapshot.file_metrics.insert(
                PathBuf::from(format!("normal{}.rs", i)),
                FileComplexity {
                    total_lines: 100,
                    loc: 80,
                    cyclomatic_complexity: 3,
                    public_methods: 2,
                    properties: 1,
                },
            );
        }
    }

    fn two_authors() -> Vec<Author> {
        vec![
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
        ]
    }

    fn make_snapshot_with_blame() -> RepoSnapshot {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        snapshot.authors = two_authors();

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
    fn bus_factor_solo_project_scores_100() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.authors = vec![Author {
            id: 0,
            name: "Alice".into(),
            email: "alice@test.com".into(),
        }];
        let now = Utc::now();
        let blame: Vec<BlameLine> = (0..100)
            .map(|j| BlameLine {
                author_id: 0,
                commit_id: format!("c{}", j),
                timestamp: now,
            })
            .collect();
        snapshot.blame_map.insert(PathBuf::from("file.rs"), blame);

        let result = bus_factor(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, 100);
        assert!(result.description.contains("Solo project"));
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
        snapshot.authors = two_authors();
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
        snapshot.authors = two_authors();
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
        snapshot.authors = two_authors();
        let now = Utc::now();
        let lines: Vec<BlameLine> = (0..100)
            .map(|j| BlameLine {
                author_id: if j < 50 { 0 } else { 1 }, // exactly 50/50
                commit_id: format!("c{}", j),
                timestamp: now,
            })
            .collect();
        snapshot.blame_map.insert(PathBuf::from("file.rs"), lines);
        let result = bus_factor(&snapshot, &HealthThresholds::default());
        // 0% dominated → score 100
        assert_eq!(result.score, 100);
        match result.raw_value {
            RawValue::Percentage(p) => assert!((p - 0.0).abs() < 1.0),
            _ => panic!("Expected Percentage"),
        }
    }

    #[test]
    fn god_objects_detects_large_files() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            PathBuf::from("fat.rs"),
            FileComplexity {
                total_lines: 600,
                loc: 520,
                cyclomatic_complexity: 10,
                public_methods: 5,
                properties: 2,
            },
        );
        add_normal_files(&mut snapshot, 99); // 1/100 = 1% → score 75
        let result = god_objects(&snapshot);
        assert_eq!(result.score, 75);
        match &result.raw_value {
            RawValue::List(v) => assert_eq!(v.len(), 1),
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn god_objects_detects_method_bloat() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            PathBuf::from("bloated.rs"),
            FileComplexity {
                total_lines: 350,
                loc: 310,
                cyclomatic_complexity: 5,
                public_methods: 16,
                properties: 3,
            },
        );
        add_normal_files(&mut snapshot, 99); // 1/100 = 1% → score 75
        let result = god_objects(&snapshot);
        assert_eq!(result.score, 75); // LOC>300 AND methods>15
    }

    #[test]
    fn god_objects_scores_100_when_none() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            PathBuf::from("normal.rs"),
            FileComplexity {
                total_lines: 100,
                loc: 80,
                cyclomatic_complexity: 3,
                public_methods: 5,
                properties: 1,
            },
        );
        let result = god_objects(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn complex_hotspots_finds_high_cc_high_churn_files() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        // 4 files: only "bad.rs" is in top quartile of both CC and churn
        let files: &[(&str, u32, usize)] = &[
            ("bad.rs", 20, 20), // high CC (top 25%), high churn (top 25%)
            ("ok1.rs", 2, 1),
            ("ok2.rs", 3, 2),
            ("ok3.rs", 4, 3),
        ];
        for (name, cc, churn) in files {
            snapshot.file_metrics.insert(
                PathBuf::from(name),
                FileComplexity {
                    total_lines: 100,
                    loc: 80,
                    cyclomatic_complexity: *cc,
                    public_methods: 2,
                    properties: 1,
                },
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
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        // All files have similar CC and churn — no outliers in top quartile of BOTH
        for i in 0..4 {
            snapshot.file_metrics.insert(
                PathBuf::from(format!("f{}.rs", i)),
                FileComplexity {
                    total_lines: 100,
                    loc: 80,
                    cyclomatic_complexity: 5,
                    public_methods: 2,
                    properties: 1,
                },
            );
            snapshot
                .commits_by_file
                .insert(PathBuf::from(format!("f{}.rs", i)), vec![format!("c{}", i)]);
        }
        let result = complex_hotspots(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn god_objects_boundary_loc_500_not_flagged() {
        // loc = 500 is NOT > 500, so not a god object
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            PathBuf::from("boundary.rs"),
            FileComplexity {
                total_lines: 550,
                loc: 500,
                cyclomatic_complexity: 5,
                public_methods: 5,
                properties: 1,
            },
        );
        let result = god_objects(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn god_objects_boundary_loc_501_flagged() {
        // loc = 501 IS > 500, so it IS a god object
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            PathBuf::from("boundary.rs"),
            FileComplexity {
                total_lines: 550,
                loc: 501,
                cyclomatic_complexity: 5,
                public_methods: 5,
                properties: 1,
            },
        );
        add_normal_files(&mut snapshot, 99); // 1/100 = 1% → score 75
        let result = god_objects(&snapshot);
        assert_eq!(result.score, 75);
    }

    #[test]
    fn god_objects_boundary_methods_15_not_flagged() {
        // loc=310, public_methods=15 — methods is NOT > 15, so not flagged
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            PathBuf::from("boundary.rs"),
            FileComplexity {
                total_lines: 350,
                loc: 310,
                cyclomatic_complexity: 5,
                public_methods: 15,
                properties: 1,
            },
        );
        let result = god_objects(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn god_objects_scores_50_at_medium_pct() {
        // 5/100 = 5% → score 50
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        for i in 0..5 {
            snapshot.file_metrics.insert(
                PathBuf::from(format!("big{}.rs", i)),
                FileComplexity {
                    total_lines: 600,
                    loc: 520,
                    cyclomatic_complexity: 5,
                    public_methods: 5,
                    properties: 1,
                },
            );
        }
        add_normal_files(&mut snapshot, 95); // 5/100 = 5% ≤ 8% → score 50
        let result = god_objects(&snapshot);
        assert_eq!(result.score, 50);
    }

    #[test]
    fn god_objects_scores_25_at_high_pct() {
        // 10/100 = 10% → score 25
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        for i in 0..10 {
            snapshot.file_metrics.insert(
                PathBuf::from(format!("big{}.rs", i)),
                FileComplexity {
                    total_lines: 600,
                    loc: 520,
                    cyclomatic_complexity: 5,
                    public_methods: 5,
                    properties: 1,
                },
            );
        }
        add_normal_files(&mut snapshot, 90); // 10/100 = 10% > 8% → score 25
        let result = god_objects(&snapshot);
        assert_eq!(result.score, 25);
    }

    #[test]
    fn god_objects_scores_75_at_low_pct() {
        // 2/100 = 2% → score 75 (≤2%)
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        for i in 0..2 {
            snapshot.file_metrics.insert(
                PathBuf::from(format!("big{}.rs", i)),
                FileComplexity {
                    total_lines: 600,
                    loc: 520,
                    cyclomatic_complexity: 5,
                    public_methods: 5,
                    properties: 1,
                },
            );
        }
        add_normal_files(&mut snapshot, 98); // 2/100 = 2% ≤ 2% → score 75
        let result = god_objects(&snapshot);
        assert_eq!(result.score, 75);
    }

    #[test]
    fn bus_factor_scores_75_at_exactly_10pct() {
        // exactly 10% dominated → NOT < 10.0, so score 75 not 100
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.authors = two_authors();
        let now = Utc::now();
        // 1 dominated out of 10 = exactly 10% → score 75
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
        for i in 0..9 {
            let lines: Vec<BlameLine> = (0..100)
                .map(|j| BlameLine {
                    author_id: if j < 50 { 0 } else { 1 },
                    commit_id: format!("b{}c{}", i, j),
                    timestamp: now,
                })
                .collect();
            snapshot
                .blame_map
                .insert(PathBuf::from(format!("balanced{}.rs", i)), lines);
        }
        let result = bus_factor(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, 75); // 10% is not < 10.0
    }

    #[test]
    fn bus_factor_scores_50_at_exactly_25pct() {
        // 5 dominated out of 20 = exactly 25% → NOT < 25.0, score 50 not 75
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.authors = two_authors();
        let now = Utc::now();
        for i in 0..5 {
            let lines: Vec<BlameLine> = (0..100)
                .map(|j| BlameLine {
                    author_id: if j < 80 { 0 } else { 1 },
                    commit_id: format!("d{}c{}", i, j),
                    timestamp: now,
                })
                .collect();
            snapshot
                .blame_map
                .insert(PathBuf::from(format!("dom{}.rs", i)), lines);
        }
        for i in 0..15 {
            let lines: Vec<BlameLine> = (0..100)
                .map(|j| BlameLine {
                    author_id: if j < 50 { 0 } else { 1 },
                    commit_id: format!("b{}c{}", i, j),
                    timestamp: now,
                })
                .collect();
            snapshot
                .blame_map
                .insert(PathBuf::from(format!("bal{}.rs", i)), lines);
        }
        let result = bus_factor(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, 50); // 25% is not < 25.0
    }

    #[test]
    fn god_objects_boundary_loc_301_with_methods_16() {
        // loc=301, methods=16 → both conditions met: LOC > 300 AND methods > 15 → flagged
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            PathBuf::from("boundary.rs"),
            FileComplexity {
                total_lines: 350,
                loc: 301,
                cyclomatic_complexity: 5,
                public_methods: 16,
                properties: 1,
            },
        );
        add_normal_files(&mut snapshot, 99); // 1/100 = 1% → score 75
        let result = god_objects(&snapshot);
        assert_eq!(result.score, 75); // flagged: 1/100 = 1%
    }

    #[test]
    fn complex_hotspots_ignores_high_cc_low_churn() {
        // A file with very high CC but low churn should NOT be flagged
        // (both conditions required: && not ||)
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        // 4 files: complex.rs has high CC but only 1 commit; churny.rs has low CC but many commits
        let files: &[(&str, u32, usize)] = &[
            ("complex.rs", 100, 1), // high CC, low churn → NOT a hotspot
            ("churny.rs", 1, 50),   // low CC, high churn → NOT a hotspot
            ("normal1.rs", 2, 2),
            ("normal2.rs", 3, 3),
        ];
        for (name, cc, churn) in files {
            snapshot.file_metrics.insert(
                PathBuf::from(name),
                FileComplexity {
                    total_lines: 100,
                    loc: 80,
                    cyclomatic_complexity: *cc,
                    public_methods: 2,
                    properties: 1,
                },
            );
            snapshot.commits_by_file.insert(
                PathBuf::from(name),
                (0..*churn).map(|i| format!("c{}", i)).collect(),
            );
        }
        let result = complex_hotspots(&snapshot);
        assert_eq!(result.score, 100); // no file has BOTH high CC AND high churn
    }

    #[test]
    fn complex_hotspots_scores_50_with_three_hotspots() {
        // 3 hotspots → score 50
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        // 9 normal files + 3 hotspots (high CC AND high churn)
        // 12 total: p75 index = 11*3/4 = 8, which falls in the normal range (CC=2, churn=1)
        // so hotspots (CC=100, churn=50) are strictly above p75
        for i in 0..9usize {
            snapshot.file_metrics.insert(
                PathBuf::from(format!("normal{}.rs", i)),
                FileComplexity {
                    total_lines: 100,
                    loc: 80,
                    cyclomatic_complexity: 2,
                    public_methods: 2,
                    properties: 1,
                },
            );
            snapshot.commits_by_file.insert(
                PathBuf::from(format!("normal{}.rs", i)),
                vec![format!("c{}", i)],
            );
        }
        for i in 0..3usize {
            snapshot.file_metrics.insert(
                PathBuf::from(format!("hot{}.rs", i)),
                FileComplexity {
                    total_lines: 200,
                    loc: 180,
                    cyclomatic_complexity: 100,
                    public_methods: 5,
                    properties: 1,
                },
            );
            snapshot.commits_by_file.insert(
                PathBuf::from(format!("hot{}.rs", i)),
                (0..50).map(|j| format!("h{}c{}", i, j)).collect(),
            );
        }
        let result = complex_hotspots(&snapshot);
        assert_eq!(result.score, 50);
    }

    #[test]
    fn bus_factor_scores_25_at_exactly_50pct() {
        // exactly 50% dominated → NOT < 50.0, so falls to else → score 25 not 50
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.authors = two_authors();
        let now = Utc::now();
        // 2 dominated out of 4 = exactly 50%
        for i in 0..2 {
            let lines: Vec<BlameLine> = (0..100)
                .map(|j| BlameLine {
                    author_id: if j < 80 { 0 } else { 1 },
                    commit_id: format!("d{}c{}", i, j),
                    timestamp: now,
                })
                .collect();
            snapshot
                .blame_map
                .insert(PathBuf::from(format!("dom{}.rs", i)), lines);
        }
        for i in 0..2 {
            let lines: Vec<BlameLine> = (0..100)
                .map(|j| BlameLine {
                    author_id: if j < 50 { 0 } else { 1 },
                    commit_id: format!("b{}c{}", i, j),
                    timestamp: now,
                })
                .collect();
            snapshot
                .blame_map
                .insert(PathBuf::from(format!("bal{}.rs", i)), lines);
        }
        let result = bus_factor(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, 25); // 50% is not < 50.0
    }

    #[test]
    fn god_objects_boundary_loc_300_not_flagged() {
        // loc=300 (not > 300) with many methods → should NOT trigger the compound condition
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            PathBuf::from("boundary.rs"),
            FileComplexity {
                total_lines: 350,
                loc: 300,
                cyclomatic_complexity: 5,
                public_methods: 20,
                properties: 1,
            },
        );
        let result = god_objects(&snapshot);
        assert_eq!(result.score, 100); // loc=300 is not > 300
    }

    #[test]
    fn god_objects_skips_non_source_files() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        // JSON data file — excluded by is_source_file
        snapshot.file_metrics.insert(
            PathBuf::from("dashboard/report.json"),
            FileComplexity {
                total_lines: 1600,
                loc: 1589,
                cyclomatic_complexity: 0,
                public_methods: 0,
                properties: 0,
            },
        );
        // SQL migration file — excluded by is_source_file
        snapshot.file_metrics.insert(
            PathBuf::from("migrations/001_init.sql"),
            FileComplexity {
                total_lines: 700,
                loc: 650,
                cyclomatic_complexity: 0,
                public_methods: 0,
                properties: 0,
            },
        );
        // Rust file that is a pure constant (CC=0, no logic) — excluded by cyclomatic_complexity guard
        snapshot.file_metrics.insert(
            PathBuf::from("src/renderer/html/css.rs"),
            FileComplexity {
                total_lines: 650,
                loc: 641,
                cyclomatic_complexity: 0,
                public_methods: 0,
                properties: 0,
            },
        );
        let result = god_objects(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn complex_hotspots_boundary_at_p75_not_flagged() {
        // Files at exactly cc_p75 and churn_p75 should NOT be flagged (> not >=)
        // 4 files: [1,3,5,5] for both CC and churn
        // p75 index = (4-1)*3/4 = 2 → p75 = values[2] = 5
        // files with CC=5 and churn=5 are NOT > 5 → score=100
        // (if mutated to >=, they would be flagged → score=75, killing the mutant)
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let files: &[(&str, u32, usize)] = &[
            ("f1.rs", 1, 1),
            ("f2.rs", 3, 3),
            ("f3.rs", 5, 5), // at exactly p75 — must NOT be flagged
            ("f4.rs", 5, 5), // at exactly p75 — must NOT be flagged
        ];
        for (name, cc, churn) in files {
            snapshot.file_metrics.insert(
                PathBuf::from(name),
                FileComplexity {
                    total_lines: 100,
                    loc: 80,
                    cyclomatic_complexity: *cc,
                    public_methods: 2,
                    properties: 1,
                },
            );
            snapshot.commits_by_file.insert(
                PathBuf::from(name),
                (0..*churn).map(|i| format!("c{}", i)).collect(),
            );
        }
        let result = complex_hotspots(&snapshot);
        assert_eq!(result.score, 100); // no file strictly above p75 in BOTH dimensions
    }
}
