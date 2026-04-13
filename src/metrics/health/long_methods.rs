use crate::metrics::{MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

use super::god_objects::is_source_file;

/// Functions that are too long or too complex to understand at a glance.
pub(super) fn long_methods(snapshot: &RepoSnapshot) -> MetricValue {
    let all_functions: Vec<_> = snapshot
        .file_metrics
        .iter()
        .filter(|(p, _)| is_source_file(p))
        .flat_map(|(p, m)| m.functions.iter().map(move |f| (p, f)))
        .collect();

    let total = all_functions.len();

    if total == 0 {
        return MetricValue {
            name: "Long methods".to_string(),
            description: "No functions found".to_string(),
            raw_value: RawValue::List(vec![]),
            score: 100,
        };
    }

    let offenders: Vec<String> = all_functions
        .iter()
        .filter(|(_, f)| f.loc > 40 || f.cyclomatic_complexity > 10)
        .map(|(p, f)| {
            format!(
                "{} ({}) \u{2014} {} LOC, CC={}",
                f.name,
                p.display(),
                f.loc,
                f.cyclomatic_complexity
            )
        })
        .collect();

    let count = offenders.len();
    let pct = count as f64 / total as f64 * 100.0;

    let score = if count == 0 {
        100
    } else if pct <= 5.0 {
        75
    } else if pct <= 15.0 {
        50
    } else {
        25
    };

    MetricValue {
        name: "Long methods".to_string(),
        description: format!("{}/{} functions flagged ({:.1}%)", count, total, pct),
        raw_value: RawValue::List(offenders),
        score,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::metrics::testutil::{make_snapshot, normal_function};
    use crate::snapshot::*;

    fn add_file_with_functions(
        snapshot: &mut RepoSnapshot,
        path: &str,
        functions: Vec<FunctionMetrics>,
    ) {
        snapshot.file_metrics.insert(
            PathBuf::from(path),
            FileComplexity {
                functions,
                ..Default::default()
            },
        );
    }

    fn add_normal_functions(snapshot: &mut RepoSnapshot, count: usize) {
        let functions: Vec<FunctionMetrics> = (0..count)
            .map(|i| normal_function(&format!("fn_{i}")))
            .collect();
        add_file_with_functions(snapshot, "src/lib.rs", functions);
    }

    #[test]
    fn scores_100_when_no_long_methods() {
        let mut snapshot = make_snapshot();
        add_normal_functions(&mut snapshot, 20);
        let result = long_methods(&snapshot);
        assert_eq!(result.score, 100);
        assert!(matches!(&result.raw_value, RawValue::List(v) if v.is_empty()));
    }

    #[test]
    fn detects_long_method_by_loc() {
        let mut snapshot = make_snapshot();
        add_normal_functions(&mut snapshot, 19);
        add_file_with_functions(
            &mut snapshot,
            "src/big.rs",
            vec![FunctionMetrics {
                name: "huge_fn".to_string(),
                loc: 85,
                cyclomatic_complexity: 3,
                max_nesting_depth: 2,
            }],
        );
        // 1 out of 20 = 5% -> score 75
        let result = long_methods(&snapshot);
        assert_eq!(result.score, 75);
        match &result.raw_value {
            RawValue::List(v) => {
                assert_eq!(v.len(), 1);
                assert!(v[0].contains("huge_fn"));
                assert!(v[0].contains("85 LOC"));
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn detects_long_method_by_cc() {
        let mut snapshot = make_snapshot();
        add_normal_functions(&mut snapshot, 19);
        add_file_with_functions(
            &mut snapshot,
            "src/complex.rs",
            vec![FunctionMetrics {
                name: "spaghetti".to_string(),
                loc: 30,
                cyclomatic_complexity: 12,
                max_nesting_depth: 5,
            }],
        );
        // 1 out of 20 = 5% -> score 75
        let result = long_methods(&snapshot);
        assert_eq!(result.score, 75);
        match &result.raw_value {
            RawValue::List(v) => {
                assert_eq!(v.len(), 1);
                assert!(v[0].contains("spaghetti"));
                assert!(v[0].contains("CC=12"));
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn scores_50_at_medium_pct() {
        let mut snapshot = make_snapshot();
        // 17 normal + 3 bad = 20 total, 3/20 = 15% -> score 50
        add_normal_functions(&mut snapshot, 17);
        let bad_fns: Vec<FunctionMetrics> = (0..3)
            .map(|i| FunctionMetrics {
                name: format!("bad_{i}"),
                loc: 60,
                cyclomatic_complexity: 3,
                max_nesting_depth: 2,
            })
            .collect();
        add_file_with_functions(&mut snapshot, "src/bad.rs", bad_fns);
        let result = long_methods(&snapshot);
        assert_eq!(result.score, 50);
    }

    #[test]
    fn scores_25_at_high_pct() {
        let mut snapshot = make_snapshot();
        // 16 normal + 4 bad = 20 total, 4/20 = 20% -> score 25
        add_normal_functions(&mut snapshot, 16);
        let bad_fns: Vec<FunctionMetrics> = (0..4)
            .map(|i| FunctionMetrics {
                name: format!("bad_{i}"),
                loc: 60,
                cyclomatic_complexity: 3,
                max_nesting_depth: 2,
            })
            .collect();
        add_file_with_functions(&mut snapshot, "src/bad.rs", bad_fns);
        let result = long_methods(&snapshot);
        assert_eq!(result.score, 25);
    }

    #[test]
    fn empty_repo_scores_100() {
        let snapshot = make_snapshot();
        let result = long_methods(&snapshot);
        assert_eq!(result.score, 100);
        assert_eq!(result.description, "No functions found");
    }

    #[test]
    fn no_functions_scores_100() {
        let mut snapshot = make_snapshot();
        // File exists but has no functions
        snapshot.file_metrics.insert(
            PathBuf::from("src/lib.rs"),
            FileComplexity {
                loc: 50,
                ..Default::default()
            },
        );
        let result = long_methods(&snapshot);
        assert_eq!(result.score, 100);
        assert_eq!(result.description, "No functions found");
    }

    #[test]
    fn boundary_loc_40_not_flagged() {
        let mut snapshot = make_snapshot();
        add_file_with_functions(
            &mut snapshot,
            "src/lib.rs",
            vec![FunctionMetrics {
                name: "boundary".to_string(),
                loc: 40,
                cyclomatic_complexity: 5,
                max_nesting_depth: 2,
            }],
        );
        let result = long_methods(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn boundary_cc_10_not_flagged() {
        let mut snapshot = make_snapshot();
        add_file_with_functions(
            &mut snapshot,
            "src/lib.rs",
            vec![FunctionMetrics {
                name: "boundary".to_string(),
                loc: 30,
                cyclomatic_complexity: 10,
                max_nesting_depth: 3,
            }],
        );
        let result = long_methods(&snapshot);
        assert_eq!(result.score, 100);
    }
}
