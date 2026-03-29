use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::metrics::{CategoryResult, MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

pub fn compute_coupling(snapshot: &RepoSnapshot) -> CategoryResult {
    let metrics = vec![
        temporal_coupling(snapshot),
        fan_out_coupling(snapshot),
        demeter_violations(snapshot),
    ];
    CategoryResult {
        name: "Coupling".to_string(),
        score: 0,
        metrics,
    }
    .compute_score()
}

fn temporal_coupling(snapshot: &RepoSnapshot) -> MetricValue {
    let suspicious: Vec<String> = snapshot
        .file_change_pairs
        .iter()
        .filter(|(a, b, count)| {
            let a_ch = snapshot.commits_by_file.get(a).map(|c| c.len()).unwrap_or(0);
            let b_ch = snapshot.commits_by_file.get(b).map(|c| c.len()).unwrap_or(0);
            let min_ch = a_ch.min(b_ch);
            min_ch > 0 && (*count as f64 / min_ch as f64) > 0.7
        })
        .map(|(a, b, count)| {
            format!("{} <> {} ({} co-changes)", a.display(), b.display(), count)
        })
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

fn fan_out_coupling(snapshot: &RepoSnapshot) -> MetricValue {
    let mut partners: HashMap<&PathBuf, HashSet<&PathBuf>> = HashMap::new();
    for (a, b, _) in &snapshot.file_change_pairs {
        partners.entry(a).or_default().insert(b);
        partners.entry(b).or_default().insert(a);
    }

    let high_fanout: Vec<String> = partners
        .iter()
        .filter(|(_, ps)| ps.len() > 5)
        .map(|(p, ps)| format!("{} ({} partners)", p.display(), ps.len()))
        .collect();

    let count = high_fanout.len();
    let score = match count {
        0 => 100,
        1..=2 => 75,
        3..=5 => 50,
        _ => 25,
    };

    MetricValue {
        name: "Fan-out coupling".to_string(),
        description: format!("{} files with high fan-out (>5 co-change partners)", count),
        raw_value: RawValue::List(high_fanout),
        score,
    }
}

fn demeter_violations(snapshot: &RepoSnapshot) -> MetricValue {
    let total: u32 = snapshot
        .file_metrics
        .values()
        .map(|m| m.demeter_violations)
        .sum();

    let score = match total {
        0 => 100,
        1..=5 => 75,
        6..=15 => 50,
        _ => 25,
    };

    MetricValue {
        name: "Demeter violations".to_string(),
        description: format!("{} method chain violations detected", total),
        raw_value: RawValue::Count(total as usize),
        score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::*;
    use std::path::PathBuf;

    #[test]
    fn temporal_coupling_detects_pairs() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"), "test".into(), "main".into(), TimeWindow::default(),
        );
        // A and B co-change 9 times out of 10 total → ratio 0.9 > 0.7
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
        assert_eq!(result.score, 75); // 1 suspicious pair
    }

    #[test]
    fn temporal_coupling_ignores_low_ratio_pairs() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"), "test".into(), "main".into(), TimeWindow::default(),
        );
        // A and B co-change 5 times out of 10 → ratio 0.5, below 0.7 threshold
        snapshot.file_change_pairs = vec![(PathBuf::from("a.rs"), PathBuf::from("b.rs"), 5)];
        snapshot.commits_by_file.insert(
            PathBuf::from("a.rs"),
            (0..10).map(|i| format!("c{}", i)).collect(),
        );
        snapshot.commits_by_file.insert(
            PathBuf::from("b.rs"),
            (0..10).map(|i| format!("c{}", i)).collect(),
        );
        let result = temporal_coupling(&snapshot);
        assert_eq!(result.score, 100); // no suspicious pairs
    }

    #[test]
    fn fan_out_coupling_detects_high_fanout() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"), "test".into(), "main".into(), TimeWindow::default(),
        );
        let hub = PathBuf::from("hub.rs");
        // hub.rs co-changes with 6 distinct partners → fan-out = 6 > 5
        for i in 0..6 {
            snapshot.file_change_pairs.push((hub.clone(), PathBuf::from(format!("p{}.rs", i)), 3));
        }
        let result = fan_out_coupling(&snapshot);
        assert_eq!(result.score, 75); // 1 high fan-out file
    }

    #[test]
    fn fan_out_coupling_ignores_low_fanout() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"), "test".into(), "main".into(), TimeWindow::default(),
        );
        // hub.rs co-changes with only 3 partners → fan-out = 3, not > 5
        for i in 0..3 {
            snapshot.file_change_pairs.push((
                PathBuf::from("hub.rs"), PathBuf::from(format!("p{}.rs", i)), 2,
            ));
        }
        let result = fan_out_coupling(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn demeter_violations_sums_file_metrics() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"), "test".into(), "main".into(), TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            PathBuf::from("a.rs"),
            FileComplexity { total_lines: 50, loc: 40, cyclomatic_complexity: 2,
                             public_methods: 1, properties: 0, demeter_violations: 3 },
        );
        snapshot.file_metrics.insert(
            PathBuf::from("b.rs"),
            FileComplexity { total_lines: 50, loc: 40, cyclomatic_complexity: 2,
                             public_methods: 1, properties: 0, demeter_violations: 2 },
        );
        let result = demeter_violations(&snapshot);
        match result.raw_value {
            RawValue::Count(c) => assert_eq!(c, 5),
            _ => panic!("Expected Count"),
        }
        assert_eq!(result.score, 75); // 5 violations → score 75
    }

    #[test]
    fn demeter_violations_scores_100_when_none() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"), "test".into(), "main".into(), TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            PathBuf::from("clean.rs"),
            FileComplexity { total_lines: 50, loc: 40, cyclomatic_complexity: 2,
                             public_methods: 1, properties: 0, demeter_violations: 0 },
        );
        let result = demeter_violations(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn temporal_coupling_boundary_exactly_07_excluded() {
        // ratio = 7/10 = 0.70 exactly — NOT > 0.7, so excluded
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"), "test".into(), "main".into(), TimeWindow::default(),
        );
        snapshot.file_change_pairs = vec![(PathBuf::from("a.rs"), PathBuf::from("b.rs"), 7)];
        snapshot.commits_by_file.insert(PathBuf::from("a.rs"), (0..10).map(|i| format!("c{}", i)).collect());
        snapshot.commits_by_file.insert(PathBuf::from("b.rs"), (0..10).map(|i| format!("c{}", i)).collect());
        let result = temporal_coupling(&snapshot);
        assert_eq!(result.score, 100); // 0.70 is not > 0.70
    }

    #[test]
    fn temporal_coupling_scores_50_with_four_to_eight_pairs() {
        // 4 suspicious pairs → score 50
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"), "test".into(), "main".into(), TimeWindow::default(),
        );
        for i in 0..4usize {
            let a = PathBuf::from(format!("a{}.rs", i));
            let b = PathBuf::from(format!("b{}.rs", i));
            snapshot.file_change_pairs.push((a.clone(), b.clone(), 9));
            snapshot.commits_by_file.insert(a, (0..10).map(|j| format!("c{}_{}", i, j)).collect());
            snapshot.commits_by_file.insert(b, (0..10).map(|j| format!("d{}_{}", i, j)).collect());
        }
        let result = temporal_coupling(&snapshot);
        assert_eq!(result.score, 50);
    }

    #[test]
    fn fan_out_coupling_boundary_exactly_5_not_flagged() {
        // hub.rs with exactly 5 partners — NOT > 5, so not high fan-out
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"), "test".into(), "main".into(), TimeWindow::default(),
        );
        for i in 0..5 {
            snapshot.file_change_pairs.push((
                PathBuf::from("hub.rs"), PathBuf::from(format!("p{}.rs", i)), 2,
            ));
        }
        let result = fan_out_coupling(&snapshot);
        assert_eq!(result.score, 100); // 5 partners is not > 5
    }

    #[test]
    fn demeter_violations_scores_50_with_six_to_fifteen() {
        // total = 6 violations → score 50
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"), "test".into(), "main".into(), TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            PathBuf::from("a.rs"),
            FileComplexity { total_lines: 50, loc: 40, cyclomatic_complexity: 2,
                             public_methods: 1, properties: 0, demeter_violations: 6 },
        );
        let result = demeter_violations(&snapshot);
        assert_eq!(result.score, 50);
    }

    #[test]
    fn demeter_violations_scores_25_above_fifteen() {
        // total = 16 violations → score 25
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"), "test".into(), "main".into(), TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            PathBuf::from("a.rs"),
            FileComplexity { total_lines: 50, loc: 40, cyclomatic_complexity: 2,
                             public_methods: 1, properties: 0, demeter_violations: 16 },
        );
        let result = demeter_violations(&snapshot);
        assert_eq!(result.score, 25);
    }

    #[test]
    fn fan_out_coupling_scores_50_with_three_to_five() {
        // 3 high-fanout files → score 50
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"), "test".into(), "main".into(), TimeWindow::default(),
        );
        for hub_idx in 0..3usize {
            let hub = PathBuf::from(format!("hub{}.rs", hub_idx));
            for i in 0..6 {
                snapshot.file_change_pairs.push((
                    hub.clone(), PathBuf::from(format!("h{}p{}.rs", hub_idx, i)), 2,
                ));
            }
        }
        let result = fan_out_coupling(&snapshot);
        assert_eq!(result.score, 50);
    }

    #[test]
    fn temporal_coupling_ignores_pair_with_zero_commits_for_one_file() {
        // A pair where one file has 0 commits in commits_by_file: min_ch=0, excluded by min_ch > 0
        // (if mutated to >=, 0.0 denominator → infinity > 0.7, pair would be flagged)
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"), "test".into(), "main".into(), TimeWindow::default(),
        );
        // a.rs has 10 commits, b.rs is absent from commits_by_file → min_ch=0
        snapshot.file_change_pairs = vec![(PathBuf::from("a.rs"), PathBuf::from("b.rs"), 5)];
        snapshot.commits_by_file.insert(
            PathBuf::from("a.rs"),
            (0..10).map(|i| format!("c{}", i)).collect(),
        );
        // b.rs intentionally absent → defaults to 0 commits
        let result = temporal_coupling(&snapshot);
        assert_eq!(result.score, 100); // min_ch=0 excludes this pair
    }
}
