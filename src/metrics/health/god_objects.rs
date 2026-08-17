use crate::config::HealthThresholds;
use crate::metrics::file_role::{classify, FileRole};
use crate::metrics::{MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

/// Production source code only — tests, config, and docs are other roles.
pub(super) fn is_source_file(path: &std::path::Path) -> bool {
    classify(path) == FileRole::Source
}

/// Median of a non-empty slice (sorts in place); 0.0 for an empty slice.
fn median(values: &mut [usize]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_unstable();
    let len = values.len();
    #[allow(clippy::manual_is_multiple_of)]
    if len % 2 == 0 {
        (values[len / 2 - 1] + values[len / 2]) as f64 / 2.0
    } else {
        values[len / 2] as f64
    }
}

/// A file structurally dominates the codebase when its import-graph degree
/// clears both an absolute floor and a multiple of the repo's median degree
/// — the floor alone keeps small/sparse repos from flagging on noise.
fn is_structural_hub(degree: usize, median_degree: f64, thresholds: &HealthThresholds) -> bool {
    degree >= thresholds.god_node_min_degree
        && (degree as f64) > median_degree * thresholds.god_node_degree_multiplier
}

/// Files that have grown too large to maintain (god objects / bloaters), or
/// that dominate the import graph as a structural hub.
pub(super) fn god_objects(snapshot: &RepoSnapshot, thresholds: &HealthThresholds) -> MetricValue {
    let mut incoming: std::collections::HashMap<&std::path::Path, usize> =
        std::collections::HashMap::new();
    for targets in snapshot.import_graph.values() {
        for target in targets {
            *incoming.entry(target.as_path()).or_insert(0) += 1;
        }
    }
    let degree_of = |p: &std::path::Path| -> usize {
        let outgoing = snapshot.import_graph.get(p).map(|v| v.len()).unwrap_or(0);
        let inc = incoming.get(p).copied().unwrap_or(0);
        outgoing + inc
    };

    let mut degrees: Vec<usize> = snapshot
        .file_metrics
        .keys()
        .filter(|p| is_source_file(p))
        .map(|p| degree_of(p))
        .collect();
    let median_degree = median(&mut degrees);

    let source_total = degrees.len();

    let gods: Vec<String> = snapshot
        .file_metrics
        .iter()
        .filter(|(p, m)| {
            is_source_file(p)
                && m.cyclomatic_complexity > 0
                && (m.loc > 500
                    || (m.loc > 300 && m.public_methods > 15)
                    || is_structural_hub(degree_of(p), median_degree, thresholds))
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
        score: Some(score),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::snapshot::*;

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
                    ..Default::default()
                },
            );
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
                ..Default::default()
            },
        );
        add_normal_files(&mut snapshot, 99); // 1/100 = 1% → score 75
        let result = god_objects(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(75));
        match &result.raw_value {
            RawValue::List(v) => assert_eq!(v.len(), 1),
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn god_objects_ignores_test_files_in_flagging_and_denominator() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        // A huge test suite must be neither flagged nor counted as a source file.
        snapshot.file_metrics.insert(
            PathBuf::from("src/metrics/coupling/tests.rs"),
            FileComplexity {
                total_lines: 1200,
                loc: 1100,
                cyclomatic_complexity: 17,
                public_methods: 0,
                properties: 0,
                ..Default::default()
            },
        );
        add_normal_files(&mut snapshot, 10);
        let result = god_objects(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(100));
        assert_eq!(result.description, "0/10 source files oversized (0.0%)");
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
                ..Default::default()
            },
        );
        add_normal_files(&mut snapshot, 99); // 1/100 = 1% → score 75
        let result = god_objects(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(75)); // LOC>300 AND methods>15
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
                ..Default::default()
            },
        );
        let result = god_objects(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(100));
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
                ..Default::default()
            },
        );
        let result = god_objects(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(100));
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
                ..Default::default()
            },
        );
        add_normal_files(&mut snapshot, 99); // 1/100 = 1% → score 75
        let result = god_objects(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(75));
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
                ..Default::default()
            },
        );
        let result = god_objects(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(100));
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
                    ..Default::default()
                },
            );
        }
        add_normal_files(&mut snapshot, 95); // 5/100 = 5% ≤ 8% → score 50
        let result = god_objects(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(50));
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
                    ..Default::default()
                },
            );
        }
        add_normal_files(&mut snapshot, 90); // 10/100 = 10% > 8% → score 25
        let result = god_objects(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(25));
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
                    ..Default::default()
                },
            );
        }
        add_normal_files(&mut snapshot, 98); // 2/100 = 2% ≤ 2% → score 75
        let result = god_objects(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(75));
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
                ..Default::default()
            },
        );
        add_normal_files(&mut snapshot, 99); // 1/100 = 1% → score 75
        let result = god_objects(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(75)); // flagged: 1/100 = 1%
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
                ..Default::default()
            },
        );
        let result = god_objects(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(100)); // loc=300 is not > 300
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
                ..Default::default()
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
                ..Default::default()
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
                ..Default::default()
            },
        );
        let result = god_objects(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(100));
    }

    #[test]
    fn god_objects_flags_structural_hub_by_import_degree() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        // hub.rs is small, but imported by 20 leaf files → degree 20.
        snapshot.file_metrics.insert(
            PathBuf::from("hub.rs"),
            FileComplexity {
                total_lines: 20,
                loc: 10,
                cyclomatic_complexity: 1,
                public_methods: 1,
                properties: 0,
                ..Default::default()
            },
        );
        for i in 0..20 {
            let name = format!("leaf{i}.rs");
            snapshot.file_metrics.insert(
                PathBuf::from(&name),
                FileComplexity {
                    total_lines: 20,
                    loc: 10,
                    cyclomatic_complexity: 1,
                    public_methods: 1,
                    properties: 0,
                    ..Default::default()
                },
            );
            snapshot
                .import_graph
                .insert(PathBuf::from(&name), vec![PathBuf::from("hub.rs")]);
        }
        let result = god_objects(&snapshot, &HealthThresholds::default());
        match &result.raw_value {
            RawValue::List(v) => {
                assert_eq!(v, &["hub.rs".to_string()], "only the hub should be flagged");
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn god_objects_does_not_flag_hub_below_the_min_degree_floor() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        // small.rs has degree 3 — well above median (0) * multiplier, but
        // under the default min-degree floor of 8, so it must not flag.
        snapshot.file_metrics.insert(
            PathBuf::from("small.rs"),
            FileComplexity {
                total_lines: 20,
                loc: 10,
                cyclomatic_complexity: 1,
                public_methods: 1,
                properties: 0,
                ..Default::default()
            },
        );
        for i in 0..3 {
            let name = format!("leaf{i}.rs");
            snapshot.file_metrics.insert(
                PathBuf::from(&name),
                FileComplexity {
                    total_lines: 20,
                    loc: 10,
                    cyclomatic_complexity: 1,
                    public_methods: 1,
                    properties: 0,
                    ..Default::default()
                },
            );
            snapshot
                .import_graph
                .insert(PathBuf::from(&name), vec![PathBuf::from("small.rs")]);
        }
        let result = god_objects(&snapshot, &HealthThresholds::default());
        assert_eq!(result.score, Some(100));
    }
}
