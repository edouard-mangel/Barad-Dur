use std::collections::HashMap;

use crate::config::HealthThresholds;
use crate::metrics::{MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

pub(super) fn bus_factor(snapshot: &RepoSnapshot, _thresholds: &HealthThresholds) -> MetricValue {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::snapshot::*;
    use chrono::Utc;

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
        let now = Utc::now();
        let mut blame_file1 = Vec::new();
        for _ in 0..80 {
            blame_file1.push(BlameLine {
                author_id: 0,
                timestamp: now,
            });
        }
        for _ in 0..20 {
            blame_file1.push(BlameLine {
                author_id: 1,
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
            .map(|_| BlameLine {
                author_id: 0,
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
        let dominated: Vec<BlameLine> = (0..100)
            .map(|j| BlameLine {
                author_id: if j < 80 { 0 } else { 1 },
                timestamp: now,
            })
            .collect();
        snapshot
            .blame_map
            .insert(PathBuf::from("dominated.rs"), dominated);
        for i in 0..4 {
            let lines: Vec<BlameLine> = (0..100)
                .map(|j| BlameLine {
                    author_id: if j < 50 { 0 } else { 1 },
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
        let dominated: Vec<BlameLine> = (0..100)
            .map(|j| BlameLine {
                author_id: if j < 80 { 0 } else { 1 },
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
        for i in 0..2 {
            let lines: Vec<BlameLine> = (0..100)
                .map(|j| BlameLine {
                    author_id: if j < 80 { 0 } else { 1 },
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
}
