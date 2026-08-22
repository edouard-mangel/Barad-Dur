use crate::metrics::{score_prevalence, MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

use super::god_objects::is_source_file;

fn percentile_75<T: Ord + Copy + Default>(mut values: Vec<T>) -> T {
    values.sort_unstable();
    values
        .get(values.len().saturating_sub(1) * 3 / 4)
        .copied()
        .unwrap_or_default()
}

/// Files in the top quartile of both cyclomatic complexity and churn — the Tornhill composite.
pub(super) fn complex_hotspots(snapshot: &RepoSnapshot) -> MetricValue {
    if snapshot.file_metrics.is_empty() {
        return MetricValue {
            name: "Complex hotspots".to_string(),
            description: "No AST data available".to_string(),
            raw_value: RawValue::Count(0),
            score: None,
        };
    }

    // Percentiles and flagging both run over production source only, so test
    // suites and CI files neither appear as hotspots nor skew the thresholds.
    let cc_p75 = percentile_75(
        snapshot
            .file_metrics
            .iter()
            .filter(|(p, _)| is_source_file(p))
            .map(|(_, m)| m.cyclomatic_complexity)
            .collect(),
    );
    let churn_p75 = percentile_75(
        snapshot
            .commits_by_file
            .iter()
            .filter(|(p, _)| is_source_file(p))
            .map(|(_, c)| c.len())
            .collect(),
    );

    let hotspots: Vec<String> = snapshot
        .file_metrics
        .iter()
        .filter(|(path, m)| {
            let churn = snapshot
                .commits_by_file
                .get(*path)
                .map(|c| c.len())
                .unwrap_or(0);
            is_source_file(path) && m.cyclomatic_complexity > cc_p75 && churn > churn_p75
        })
        .map(|(p, _)| p.display().to_string())
        .collect();

    let count = hotspots.len();
    let source_total = snapshot
        .file_metrics
        .keys()
        .filter(|path| is_source_file(path))
        .count();
    let pct = if source_total == 0 {
        0.0
    } else {
        count as f64 / source_total as f64 * 100.0
    };
    let score = score_prevalence(count, source_total);

    MetricValue {
        name: "Complex hotspots".to_string(),
        description: format!(
            "{count}/{source_total} source files with high complexity and high churn ({pct:.1}%)"
        ),
        raw_value: RawValue::List(hotspots),
        score: Some(score),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::snapshot::*;

    // --- percentile_75 ---

    #[test]
    fn percentile_75_empty_returns_default() {
        assert_eq!(percentile_75::<u32>(vec![]), 0);
    }

    #[test]
    fn percentile_75_single_element() {
        assert_eq!(percentile_75(vec![42u32]), 42);
    }

    #[test]
    fn percentile_75_four_elements() {
        // sorted: [1, 2, 3, 4] — index = (4-1)*3/4 = 2 → value 3
        assert_eq!(percentile_75(vec![4u32, 1, 3, 2]), 3);
    }

    #[test]
    fn percentile_75_eight_elements() {
        // sorted: [1,2,3,4,5,6,7,8] — index = (8-1)*3/4 = 5 → value 6
        assert_eq!(percentile_75(vec![8u32, 3, 1, 6, 2, 7, 4, 5]), 6);
    }

    #[test]
    fn percentile_75_unsorted_input_sorted_before_lookup() {
        // Same values in reverse — must sort first
        assert_eq!(
            percentile_75(vec![10u32, 8, 6, 4, 2]),
            percentile_75(vec![2u32, 4, 6, 8, 10])
        );
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
                    ..Default::default()
                },
            );
            snapshot.commits_by_file.insert(
                PathBuf::from(name),
                (0..*churn).map(|i| CommitId(i as u32)).collect(),
            );
        }
        let result = complex_hotspots(&snapshot);
        assert_eq!(result.score, Some(75)); // 1 hotspot → score 75
        match &result.raw_value {
            RawValue::List(v) => assert_eq!(v.len(), 1),
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn complex_hotspots_ignores_test_and_config_files() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        // High-CC high-churn files that are not production source: neither may
        // be flagged, nor may they inflate the percentile thresholds.
        let noise: &[(&str, u32, usize)] = &[
            ("tests/e2e_milestone_1.rs", 90, 60),
            (".gitlab-ci.yml", 0, 100),
        ];
        let source: &[(&str, u32, usize)] = &[
            ("src/hot.rs", 20, 20),
            ("src/ok1.rs", 2, 1),
            ("src/ok2.rs", 3, 2),
            ("src/ok3.rs", 4, 3),
        ];
        for (name, cc, churn) in noise.iter().chain(source) {
            snapshot.file_metrics.insert(
                PathBuf::from(name),
                FileComplexity {
                    total_lines: 100,
                    loc: 80,
                    cyclomatic_complexity: *cc,
                    public_methods: 2,
                    properties: 1,
                    ..Default::default()
                },
            );
            snapshot.commits_by_file.insert(
                PathBuf::from(name),
                (0..*churn).map(|i| CommitId(i as u32)).collect(),
            );
        }
        let result = complex_hotspots(&snapshot);
        // Only src/hot.rs is a hotspot among source files → score 75.
        assert_eq!(result.score, Some(75));
        match &result.raw_value {
            RawValue::List(v) => assert_eq!(v, &["src/hot.rs"]),
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
                    ..Default::default()
                },
            );
            snapshot.commits_by_file.insert(
                PathBuf::from(format!("f{}.rs", i)),
                vec![CommitId(i as u32)],
            );
        }
        let result = complex_hotspots(&snapshot);
        assert_eq!(result.score, Some(100));
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
                    ..Default::default()
                },
            );
            snapshot.commits_by_file.insert(
                PathBuf::from(name),
                (0..*churn).map(|i| CommitId(i as u32)).collect(),
            );
        }
        let result = complex_hotspots(&snapshot);
        assert_eq!(result.score, Some(100)); // no file has BOTH high CC AND high churn
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
        for i in 0..9usize {
            snapshot.file_metrics.insert(
                PathBuf::from(format!("normal{}.rs", i)),
                FileComplexity {
                    total_lines: 100,
                    loc: 80,
                    cyclomatic_complexity: 2,
                    public_methods: 2,
                    properties: 1,
                    ..Default::default()
                },
            );
            snapshot.commits_by_file.insert(
                PathBuf::from(format!("normal{}.rs", i)),
                vec![CommitId(i as u32)],
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
                    ..Default::default()
                },
            );
            snapshot.commits_by_file.insert(
                PathBuf::from(format!("hot{}.rs", i)),
                (0..50).map(|j| CommitId((i * 50 + j) as u32)).collect(),
            );
        }
        let result = complex_hotspots(&snapshot);
        assert_eq!(result.score, Some(50));
    }

    #[test]
    fn complex_hotspots_boundary_at_p75_not_flagged() {
        // Files at exactly cc_p75 and churn_p75 should NOT be flagged (> not >=)
        // 4 files: [1,3,5,5] for both CC and churn
        // p75 index = (4-1)*3/4 = 2 → p75 = values[2] = 5
        // files with CC=5 and churn=5 are NOT > 5 → score=100
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
                    ..Default::default()
                },
            );
            snapshot.commits_by_file.insert(
                PathBuf::from(name),
                (0..*churn).map(|i| CommitId(i as u32)).collect(),
            );
        }
        let result = complex_hotspots(&snapshot);
        assert_eq!(result.score, Some(100)); // no file strictly above p75 in BOTH dimensions
    }
}
