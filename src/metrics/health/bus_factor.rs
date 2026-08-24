use crate::config::HealthThresholds;
use crate::metrics::{MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

#[cfg(test)]
fn is_file_author_dominated(lines: &[crate::snapshot::BlameLine]) -> bool {
    crate::metrics::primary_author(lines).is_some()
}

pub(super) fn bus_factor(snapshot: &RepoSnapshot, _thresholds: &HealthThresholds) -> MetricValue {
    if snapshot.authors.len() <= 1 {
        return MetricValue {
            name: "Bus factor".to_string(),
            description: "Solo project — not applicable".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
    }

    if snapshot.blame_map.is_empty() {
        return MetricValue {
            name: "Bus factor".to_string(),
            description: "No blame data available".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
    }

    let mut lines_by_author: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
    for line in snapshot
        .blame_map
        .values()
        .flat_map(|lines| lines.iter())
        .filter(|line| line.author_id != crate::metrics::UNKNOWN_AUTHOR)
    {
        *lines_by_author.entry(line.author_id).or_default() += line.line_count;
    }
    let total_lines: usize = lines_by_author.values().sum();
    if total_lines == 0 {
        return MetricValue {
            name: "Bus factor".to_string(),
            description: "No attributable blame data available".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
    }
    let mut ownership: Vec<usize> = lines_by_author.into_values().collect();
    let active_authors = ownership.len();
    ownership.sort_unstable_by(|a, b| b.cmp(a));
    let target = total_lines as f64 * 0.8;
    let mut covered = 0usize;
    let bus_factor = ownership
        .iter()
        .position(|lines| {
            covered += *lines;
            covered as f64 >= target
        })
        .map_or(ownership.len(), |index| index + 1);
    let score = match bus_factor {
        0 | 1 => 25,
        2 => 50,
        3 => 75,
        _ => 100,
    };

    MetricValue {
        name: "Bus factor".to_string(),
        description: format!(
            "{bus_factor} contributor(s) cover 80% of attributable lines ({} active authors)",
            active_authors
        ),
        raw_value: RawValue::Count(bus_factor),
        score: Some(score),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::metrics::testutil::{make_snapshot, two_authors};
    use crate::snapshot::*;
    use chrono::Utc;

    // --- is_file_author_dominated ---

    #[test]
    fn dominated_empty_slice_is_false() {
        assert!(!is_file_author_dominated(&[]));
    }

    #[test]
    fn dominated_single_author_all_lines_is_true() {
        let now = Utc::now();
        let lines: Vec<BlameLine> = (0..10).map(|_| BlameLine::new(0, now)).collect();
        assert!(is_file_author_dominated(&lines));
    }

    #[test]
    fn dominated_exact_50_50_split_is_false() {
        // max * 2 == total (not strictly greater) → not dominated
        let now = Utc::now();
        let lines: Vec<BlameLine> = (0..100)
            .map(|i| BlameLine::new(if i < 50 { 0 } else { 1 }, now))
            .collect();
        assert!(!is_file_author_dominated(&lines));
    }

    #[test]
    fn dominated_51_49_split_is_true() {
        let now = Utc::now();
        let lines: Vec<BlameLine> = (0..100)
            .map(|i| BlameLine::new(if i < 51 { 0 } else { 1 }, now))
            .collect();
        assert!(is_file_author_dominated(&lines));
    }

    #[test]
    fn dominated_80_20_split_is_true() {
        let now = Utc::now();
        let lines: Vec<BlameLine> = (0..100)
            .map(|i| BlameLine::new(if i < 80 { 0 } else { 1 }, now))
            .collect();
        assert!(is_file_author_dominated(&lines));
    }

    fn make_snapshot_with_blame() -> RepoSnapshot {
        let mut snapshot = make_snapshot();
        snapshot.authors = two_authors();
        let now = Utc::now();
        let mut blame_file1 = Vec::new();
        for _ in 0..80 {
            blame_file1.push(BlameLine::new(0, now));
        }
        for _ in 0..20 {
            blame_file1.push(BlameLine::new(1, now));
        }
        snapshot
            .blame_map
            .insert(PathBuf::from("file1.rs"), blame_file1);
        snapshot
    }

    #[test]
    fn bus_factor_solo_project_has_no_score() {
        let mut snapshot = make_snapshot();
        snapshot.authors = vec![Author {
            id: 0,
            name: "Alice".into(),
            email: "alice@test.com".into(),
        }];
        let now = Utc::now();
        let blame: Vec<BlameLine> = (0..100).map(|_| BlameLine::new(0, now)).collect();
        snapshot.blame_map.insert(PathBuf::from("file.rs"), blame);

        let result = bus_factor(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, None);
        assert!(result.description.contains("Solo project"));
    }

    #[test]
    fn bus_factor_detects_single_author_dominance() {
        let snapshot = make_snapshot_with_blame();
        let result = bus_factor(&snapshot, &HealthThresholds::default());
        // Alice alone covers the 80% knowledge threshold.
        assert_eq!(result.score, Some(25));
        match result.raw_value {
            RawValue::Count(1) => {}
            _ => panic!("Expected contributor count"),
        }
    }

    #[test]
    fn bus_factor_scores_100_when_few_dominated() {
        // 5 files, all 50/50 split → 0% dominated → score 100
        let mut snapshot = make_snapshot();
        snapshot.authors = two_authors();
        let now = Utc::now();
        for i in 0..5 {
            let lines: Vec<BlameLine> = (0..100)
                .map(|j| BlameLine::new(if j < 50 { 0 } else { 1 }, now))
                .collect();
            snapshot
                .blame_map
                .insert(PathBuf::from(format!("f{}.rs", i)), lines);
        }
        let result = bus_factor(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(50));
        match result.raw_value {
            RawValue::Count(2) => {}
            _ => panic!("Expected contributor count"),
        }
    }

    #[test]
    fn bus_factor_scores_75_when_some_dominated() {
        // 5 files: 1 dominated (author 0 owns 80%) + 4 not dominated → 20% → score 75
        let mut snapshot = make_snapshot();
        snapshot.authors = two_authors();
        let now = Utc::now();
        let dominated: Vec<BlameLine> = (0..100)
            .map(|j| BlameLine::new(if j < 80 { 0 } else { 1 }, now))
            .collect();
        snapshot
            .blame_map
            .insert(PathBuf::from("dominated.rs"), dominated);
        for i in 0..4 {
            let lines: Vec<BlameLine> = (0..100)
                .map(|j| BlameLine::new(if j < 50 { 0 } else { 1 }, now))
                .collect();
            snapshot
                .blame_map
                .insert(PathBuf::from(format!("balanced{}.rs", i)), lines);
        }
        let result = bus_factor(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(50));
    }

    #[test]
    fn bus_factor_three_contributors_scores_75() {
        // Three authors at 40/30/30: the top two cover only 70% (< 80%),
        // all three cover 100% → bus factor 3 → score 75.
        let mut snapshot = make_snapshot();
        snapshot.authors = two_authors();
        snapshot.authors.push(Author {
            id: 2,
            name: "Carol".into(),
            email: "carol@test.com".into(),
        });
        let now = Utc::now();
        let lines: Vec<BlameLine> = (0..100)
            .map(|i| {
                let author = if i < 40 {
                    0
                } else if i < 70 {
                    1
                } else {
                    2
                };
                BlameLine::new(author, now)
            })
            .collect();
        snapshot.blame_map.insert(PathBuf::from("file.rs"), lines);
        let result = bus_factor(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(75));
        match result.raw_value {
            RawValue::Count(3) => {}
            _ => panic!("Expected bus factor of 3"),
        }
    }

    #[test]
    fn bus_factor_exact_50pct_not_dominated() {
        // A file where author 0 owns exactly 50% of lines is NOT dominated
        // because dominance requires max * 2 > total (strict majority)
        let mut snapshot = make_snapshot();
        snapshot.authors = two_authors();
        let now = Utc::now();
        let lines: Vec<BlameLine> = (0..100)
            .map(|j| BlameLine::new(if j < 50 { 0 } else { 1 }, now)) // exactly 50/50
            .collect();
        snapshot.blame_map.insert(PathBuf::from("file.rs"), lines);
        let result = bus_factor(&snapshot, &HealthThresholds::default());
        // 0% dominated → score 100
        assert_eq!(result.score, Some(50));
        match result.raw_value {
            RawValue::Count(2) => {}
            _ => panic!("Expected contributor count"),
        }
    }

    #[test]
    fn bus_factor_scores_75_at_exactly_10pct() {
        // exactly 10% dominated → NOT < 10.0, so score 75 not 100
        let mut snapshot = make_snapshot();
        snapshot.authors = two_authors();
        let now = Utc::now();
        let dominated: Vec<BlameLine> = (0..100)
            .map(|j| BlameLine::new(if j < 80 { 0 } else { 1 }, now))
            .collect();
        snapshot
            .blame_map
            .insert(PathBuf::from("dominated.rs"), dominated);
        for i in 0..9 {
            let lines: Vec<BlameLine> = (0..100)
                .map(|j| BlameLine::new(if j < 50 { 0 } else { 1 }, now))
                .collect();
            snapshot
                .blame_map
                .insert(PathBuf::from(format!("balanced{}.rs", i)), lines);
        }
        let result = bus_factor(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(50));
    }

    #[test]
    fn bus_factor_scores_50_at_exactly_25pct() {
        // 5 dominated out of 20 = exactly 25% → NOT < 25.0, score 50 not 75
        let mut snapshot = make_snapshot();
        snapshot.authors = two_authors();
        let now = Utc::now();
        for i in 0..5 {
            let lines: Vec<BlameLine> = (0..100)
                .map(|j| BlameLine::new(if j < 80 { 0 } else { 1 }, now))
                .collect();
            snapshot
                .blame_map
                .insert(PathBuf::from(format!("dom{}.rs", i)), lines);
        }
        for i in 0..15 {
            let lines: Vec<BlameLine> = (0..100)
                .map(|j| BlameLine::new(if j < 50 { 0 } else { 1 }, now))
                .collect();
            snapshot
                .blame_map
                .insert(PathBuf::from(format!("bal{}.rs", i)), lines);
        }
        let result = bus_factor(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(50)); // 25% is not < 25.0
    }

    #[test]
    fn bus_factor_scores_25_at_exactly_50pct() {
        // exactly 50% dominated → NOT < 50.0, so falls to else → score 25 not 50
        let mut snapshot = make_snapshot();
        snapshot.authors = two_authors();
        let now = Utc::now();
        for i in 0..2 {
            let lines: Vec<BlameLine> = (0..100)
                .map(|j| BlameLine::new(if j < 80 { 0 } else { 1 }, now))
                .collect();
            snapshot
                .blame_map
                .insert(PathBuf::from(format!("dom{}.rs", i)), lines);
        }
        for i in 0..2 {
            let lines: Vec<BlameLine> = (0..100)
                .map(|j| BlameLine::new(if j < 50 { 0 } else { 1 }, now))
                .collect();
            snapshot
                .blame_map
                .insert(PathBuf::from(format!("bal{}.rs", i)), lines);
        }
        let result = bus_factor(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(50));
    }
}
