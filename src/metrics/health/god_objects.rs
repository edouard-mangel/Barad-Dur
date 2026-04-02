use crate::metrics::{MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

pub(super) fn is_source_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).unwrap_or(""),
        "rs" | "py"
            | "go"
            | "java"
            | "cs"
            | "js"
            | "ts"
            | "tsx"
            | "jsx"
            | "kt"
            | "cpp"
            | "c"
            | "h"
            | "hpp"
            | "rb"
            | "php"
            | "swift"
            | "scala"
    )
}

/// Files that have grown too large to maintain (god objects / bloaters).
pub(super) fn god_objects(snapshot: &RepoSnapshot) -> MetricValue {
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
                ..Default::default()
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
                ..Default::default()
            },
        );
        let result = god_objects(&snapshot);
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
                ..Default::default()
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
                ..Default::default()
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
                ..Default::default()
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
                    ..Default::default()
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
                    ..Default::default()
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
                    ..Default::default()
                },
            );
        }
        add_normal_files(&mut snapshot, 98); // 2/100 = 2% ≤ 2% → score 75
        let result = god_objects(&snapshot);
        assert_eq!(result.score, 75);
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
        let result = god_objects(&snapshot);
        assert_eq!(result.score, 75); // flagged: 1/100 = 1%
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
        let result = god_objects(&snapshot);
        assert_eq!(result.score, 100);
    }
}
