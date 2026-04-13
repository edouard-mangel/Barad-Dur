use crate::metrics::{MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

use super::god_objects::is_source_file;

/// Code biomarkers: flags files with excessive nesting depth or high nesting variance.
pub(super) fn biomarkers(snapshot: &RepoSnapshot) -> MetricValue {
    let source_files: Vec<_> = snapshot
        .file_metrics
        .iter()
        .filter(|(p, _)| is_source_file(p))
        .collect();

    let source_total = source_files.len();

    if source_total == 0 {
        return MetricValue {
            name: "Code biomarkers".to_string(),
            description: "No source files found".to_string(),
            raw_value: RawValue::List(vec![]),
            score: 100,
        };
    }

    let flagged: Vec<String> = source_files
        .iter()
        .filter(|(_, m)| m.max_nesting_depth > 4 || m.nesting_variance > 2.0)
        .map(|(p, m)| {
            if m.max_nesting_depth > 4 {
                format!(
                    "{} \u{2014} nesting depth {}",
                    p.display(),
                    m.max_nesting_depth
                )
            } else {
                format!(
                    "{} \u{2014} nesting variance {:.1}",
                    p.display(),
                    m.nesting_variance
                )
            }
        })
        .collect();

    let count = flagged.len();
    let pct = count as f64 / source_total as f64 * 100.0;

    let score = if count == 0 {
        100
    } else if pct <= 3.0 {
        75
    } else if pct <= 10.0 {
        50
    } else {
        25
    };

    MetricValue {
        name: "Code biomarkers".to_string(),
        description: format!(
            "{}/{} source files flagged ({:.1}%)",
            count, source_total, pct
        ),
        raw_value: RawValue::List(flagged),
        score,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::metrics::testutil::make_snapshot;
    use crate::snapshot::*;

    fn add_normal_files(snapshot: &mut RepoSnapshot, count: usize) {
        for i in 0..count {
            snapshot.file_metrics.insert(
                PathBuf::from(format!("normal{}.rs", i)),
                FileComplexity {
                    max_nesting_depth: 2,
                    nesting_variance: 1.0,
                    ..Default::default()
                },
            );
        }
    }

    #[test]
    fn scores_100_when_no_deep_nesting() {
        let mut snapshot = make_snapshot();
        add_normal_files(&mut snapshot, 20);
        let result = biomarkers(&snapshot);
        assert_eq!(result.score, 100);
        assert!(matches!(&result.raw_value, RawValue::List(v) if v.is_empty()));
    }

    #[test]
    fn flags_deep_nesting() {
        let mut snapshot = make_snapshot();
        add_normal_files(&mut snapshot, 99);
        snapshot.file_metrics.insert(
            PathBuf::from("deep.rs"),
            FileComplexity {
                max_nesting_depth: 5,
                nesting_variance: 1.0,
                ..Default::default()
            },
        );
        // 1/100 = 1% <= 3% -> 75
        let result = biomarkers(&snapshot);
        assert_eq!(result.score, 75);
        match &result.raw_value {
            RawValue::List(v) => {
                assert_eq!(v.len(), 1);
                assert!(v[0].contains("nesting depth 5"));
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn flags_high_variance() {
        let mut snapshot = make_snapshot();
        add_normal_files(&mut snapshot, 99);
        snapshot.file_metrics.insert(
            PathBuf::from("varied.rs"),
            FileComplexity {
                max_nesting_depth: 3,
                nesting_variance: 2.5,
                ..Default::default()
            },
        );
        // 1/100 = 1% <= 3% -> 75
        let result = biomarkers(&snapshot);
        assert_eq!(result.score, 75);
        match &result.raw_value {
            RawValue::List(v) => {
                assert_eq!(v.len(), 1);
                assert!(v[0].contains("nesting variance 2.5"));
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn scores_50_at_medium_pct() {
        let mut snapshot = make_snapshot();
        add_normal_files(&mut snapshot, 95);
        for i in 0..5 {
            snapshot.file_metrics.insert(
                PathBuf::from(format!("deep{}.rs", i)),
                FileComplexity {
                    max_nesting_depth: 6,
                    nesting_variance: 1.0,
                    ..Default::default()
                },
            );
        }
        // 5/100 = 5% <= 10% -> 50
        let result = biomarkers(&snapshot);
        assert_eq!(result.score, 50);
    }

    #[test]
    fn scores_25_at_high_pct() {
        let mut snapshot = make_snapshot();
        add_normal_files(&mut snapshot, 88);
        for i in 0..12 {
            snapshot.file_metrics.insert(
                PathBuf::from(format!("deep{}.rs", i)),
                FileComplexity {
                    max_nesting_depth: 7,
                    nesting_variance: 1.0,
                    ..Default::default()
                },
            );
        }
        // 12/100 = 12% > 10% -> 25
        let result = biomarkers(&snapshot);
        assert_eq!(result.score, 25);
    }

    #[test]
    fn empty_repo_scores_100() {
        let snapshot = make_snapshot();
        let result = biomarkers(&snapshot);
        assert_eq!(result.score, 100);
        assert_eq!(result.description, "No source files found");
    }

    #[test]
    fn boundary_depth_4_not_flagged() {
        let mut snapshot = make_snapshot();
        snapshot.file_metrics.insert(
            PathBuf::from("boundary.rs"),
            FileComplexity {
                max_nesting_depth: 4,
                nesting_variance: 1.0,
                ..Default::default()
            },
        );
        let result = biomarkers(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn boundary_variance_2_0_not_flagged() {
        let mut snapshot = make_snapshot();
        snapshot.file_metrics.insert(
            PathBuf::from("boundary.rs"),
            FileComplexity {
                max_nesting_depth: 3,
                nesting_variance: 2.0,
                ..Default::default()
            },
        );
        let result = biomarkers(&snapshot);
        assert_eq!(result.score, 100);
    }
}
