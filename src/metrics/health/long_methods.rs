use std::path::Path;

use crate::config::HealthThresholds;
use crate::metrics::{MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

use super::god_objects::is_source_file;

/// Functions that are too long or too complex to understand at a glance.
pub(super) fn long_methods(snapshot: &RepoSnapshot, thresholds: &HealthThresholds) -> MetricValue {
    // The applicable LOC threshold is a property of the FILE, so it is
    // resolved once per file here rather than re-derived for every one of the
    // repository's functions.
    let all_functions: Vec<_> = snapshot
        .file_metrics
        .iter()
        .filter(|(p, _)| is_source_file(p))
        .flat_map(|(p, m)| {
            let loc_threshold = applicable_loc_threshold(p, thresholds);
            m.functions.iter().map(move |f| (p, f, loc_threshold))
        })
        .collect();

    let total = all_functions.len();

    if total == 0 {
        return MetricValue {
            name: "Long methods".to_string(),
            description: "No functions found".to_string(),
            raw_value: RawValue::List(vec![]),
            score: None,
        };
    }

    let offenders: Vec<String> = all_functions
        .iter()
        .filter(|(_, function, loc_threshold)| is_long_method(function, *loc_threshold, thresholds))
        .map(|(p, f, _)| {
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
        score: Some(score),
    }
}

/// The LOC threshold that applies to a file: declarative UI is allowed to be
/// longer, since its length is markup rather than control flow.
fn applicable_loc_threshold(path: &Path, thresholds: &HealthThresholds) -> usize {
    if crate::metrics::file_role::is_declarative_ui(path) {
        thresholds.long_method_ui_loc
    } else {
        thresholds.long_method_loc
    }
}

fn is_long_method(
    function: &crate::snapshot::FunctionMetrics,
    loc_threshold: usize,
    thresholds: &HealthThresholds,
) -> bool {
    function.cyclomatic_complexity > thresholds.long_method_cc
        || (function.cyclomatic_complexity > thresholds.long_method_cc_floor
            && function.loc > loc_threshold)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::metrics::testutil::{make_snapshot, normal_function};
    use crate::snapshot::*;

    fn default_thresholds() -> crate::config::HealthThresholds {
        crate::config::HealthThresholds::default()
    }

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
        let result = long_methods(&snapshot, &default_thresholds());
        assert_eq!(result.score, Some(100));
        assert!(matches!(&result.raw_value, RawValue::List(v) if v.is_empty()));
    }

    #[test]
    fn detects_moderately_complex_long_method() {
        let mut snapshot = make_snapshot();
        add_normal_functions(&mut snapshot, 19);
        add_file_with_functions(
            &mut snapshot,
            "src/big.rs",
            vec![FunctionMetrics {
                responsibility: None,
                is_test: false,
                name: "huge_fn".to_string(),
                loc: 85,
                cyclomatic_complexity: 6,
                max_nesting_depth: 2,
            }],
        );
        // 1 out of 20 = 5% -> score 75
        let result = long_methods(&snapshot, &default_thresholds());
        assert_eq!(result.score, Some(75));
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
                responsibility: None,
                is_test: false,
                name: "spaghetti".to_string(),
                loc: 30,
                cyclomatic_complexity: 12,
                max_nesting_depth: 5,
            }],
        );
        // 1 out of 20 = 5% -> score 75
        let result = long_methods(&snapshot, &default_thresholds());
        assert_eq!(result.score, Some(75));
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
    fn functions_in_test_files_are_not_counted() {
        let mut snapshot = make_snapshot();
        add_normal_functions(&mut snapshot, 19);
        // A giant test function must not be flagged — nor swell the total.
        add_file_with_functions(
            &mut snapshot,
            "tests/coupling_milestone_1.rs",
            vec![FunctionMetrics {
                responsibility: None,
                is_test: false,
                name: "end_to_end".to_string(),
                loc: 120,
                cyclomatic_complexity: 2,
                max_nesting_depth: 2,
            }],
        );
        let result = long_methods(&snapshot, &default_thresholds());
        assert_eq!(result.score, Some(100));
        assert_eq!(result.description, "0/19 functions flagged (0.0%)");
    }

    #[test]
    fn scores_50_at_medium_pct() {
        let mut snapshot = make_snapshot();
        // 17 normal + 3 bad = 20 total, 3/20 = 15% -> score 50
        add_normal_functions(&mut snapshot, 17);
        let bad_fns: Vec<FunctionMetrics> = (0..3)
            .map(|i| FunctionMetrics {
                responsibility: None,
                is_test: false,
                name: format!("bad_{i}"),
                loc: 60,
                cyclomatic_complexity: 6,
                max_nesting_depth: 2,
            })
            .collect();
        add_file_with_functions(&mut snapshot, "src/bad.rs", bad_fns);
        let result = long_methods(&snapshot, &default_thresholds());
        assert_eq!(result.score, Some(50));
    }

    #[test]
    fn scores_25_at_high_pct() {
        let mut snapshot = make_snapshot();
        // 16 normal + 4 bad = 20 total, 4/20 = 20% -> score 25
        add_normal_functions(&mut snapshot, 16);
        let bad_fns: Vec<FunctionMetrics> = (0..4)
            .map(|i| FunctionMetrics {
                responsibility: None,
                is_test: false,
                name: format!("bad_{i}"),
                loc: 60,
                cyclomatic_complexity: 6,
                max_nesting_depth: 2,
            })
            .collect();
        add_file_with_functions(&mut snapshot, "src/bad.rs", bad_fns);
        let result = long_methods(&snapshot, &default_thresholds());
        assert_eq!(result.score, Some(25));
    }

    #[test]
    fn empty_repo_has_no_score() {
        let snapshot = make_snapshot();
        let result = long_methods(&snapshot, &default_thresholds());
        assert_eq!(result.score, None);
        assert_eq!(result.description, "No functions found");
    }

    #[test]
    fn no_functions_has_no_score() {
        let mut snapshot = make_snapshot();
        // File exists but has no functions
        snapshot.file_metrics.insert(
            PathBuf::from("src/lib.rs"),
            FileComplexity {
                loc: 50,
                ..Default::default()
            },
        );
        let result = long_methods(&snapshot, &default_thresholds());
        assert_eq!(result.score, None);
        assert_eq!(result.description, "No functions found");
    }

    #[test]
    fn boundary_loc_40_not_flagged() {
        let mut snapshot = make_snapshot();
        add_file_with_functions(
            &mut snapshot,
            "src/lib.rs",
            vec![FunctionMetrics {
                responsibility: None,
                is_test: false,
                name: "boundary".to_string(),
                loc: 40,
                cyclomatic_complexity: 5,
                max_nesting_depth: 2,
            }],
        );
        let result = long_methods(&snapshot, &default_thresholds());
        assert_eq!(result.score, Some(100));
    }

    #[test]
    fn boundary_cc_10_not_flagged() {
        let mut snapshot = make_snapshot();
        add_file_with_functions(
            &mut snapshot,
            "src/lib.rs",
            vec![FunctionMetrics {
                responsibility: None,
                is_test: false,
                name: "boundary".to_string(),
                loc: 30,
                cyclomatic_complexity: 10,
                max_nesting_depth: 3,
            }],
        );
        let result = long_methods(&snapshot, &default_thresholds());
        assert_eq!(result.score, Some(100));
    }

    #[test]
    fn declarative_ui_uses_tiered_strict_boundaries() {
        let mut snapshot = make_snapshot();
        add_file_with_functions(
            &mut snapshot,
            "src/view.tsx",
            vec![
                FunctionMetrics {
                    responsibility: None,
                    is_test: false,
                    name: "loc_boundary".into(),
                    loc: 80,
                    cyclomatic_complexity: 6,
                    max_nesting_depth: 2,
                },
                FunctionMetrics {
                    responsibility: None,
                    is_test: false,
                    name: "cc_floor".into(),
                    loc: 200,
                    cyclomatic_complexity: 5,
                    max_nesting_depth: 2,
                },
                FunctionMetrics {
                    responsibility: None,
                    is_test: false,
                    name: "moderately_complex_and_long".into(),
                    loc: 81,
                    cyclomatic_complexity: 6,
                    max_nesting_depth: 2,
                },
                FunctionMetrics {
                    responsibility: None,
                    is_test: false,
                    name: "short_but_highly_complex".into(),
                    loc: 20,
                    cyclomatic_complexity: 11,
                    max_nesting_depth: 2,
                },
            ],
        );

        let result = long_methods(&snapshot, &default_thresholds());
        let RawValue::List(findings) = result.raw_value else {
            panic!("expected findings")
        };
        assert_eq!(result.description, "2/4 functions flagged (50.0%)");
        assert!(findings
            .iter()
            .any(|finding| finding.contains("moderately_complex_and_long")));
        assert!(findings
            .iter()
            .any(|finding| finding.contains("short_but_highly_complex")));
        assert!(!findings
            .iter()
            .any(|finding| finding.contains("loc_boundary")));
        assert!(!findings.iter().any(|finding| finding.contains("cc_floor")));
    }

    #[test]
    fn jsx_uses_ui_threshold_but_other_extensions_do_not() {
        // Function names are deliberately NOT the paths: with name == path,
        // `contains("src/view.js")` is also satisfied by a "src/view.jsx"
        // finding, so a positive assertion could pass on the wrong finding.
        let mut snapshot = make_snapshot();
        for (path, name) in [
            ("src/view.jsx", "jsx_component"),
            ("src/view.tsx", "tsx_component"),
        ] {
            add_file_with_functions(
                &mut snapshot,
                path,
                vec![FunctionMetrics {
                    responsibility: None,
                    is_test: false,
                    name: name.into(),
                    loc: 60,
                    cyclomatic_complexity: 6,
                    max_nesting_depth: 2,
                }],
            );
        }
        for (path, name) in [("src/view.js", "js_module"), ("src/view.ts", "ts_module")] {
            add_file_with_functions(
                &mut snapshot,
                path,
                vec![FunctionMetrics {
                    responsibility: None,
                    is_test: false,
                    name: name.into(),
                    loc: 60,
                    cyclomatic_complexity: 6,
                    max_nesting_depth: 2,
                }],
            );
        }

        let result = long_methods(&snapshot, &default_thresholds());
        let RawValue::List(findings) = result.raw_value else {
            panic!("expected findings")
        };
        assert_eq!(result.description, "2/4 functions flagged (50.0%)");
        let flagged = |name: &str| findings.iter().any(|f| f.contains(name));
        assert!(
            flagged("js_module"),
            "60 LOC in .js exceeds long_method_loc"
        );
        assert!(
            flagged("ts_module"),
            "60 LOC in .ts exceeds long_method_loc"
        );
        assert!(
            !flagged("jsx_component"),
            "60 LOC in .jsx is below long_method_ui_loc"
        );
        assert!(
            !flagged("tsx_component"),
            "60 LOC in .tsx is below long_method_ui_loc"
        );
    }

    #[test]
    fn non_ui_requires_complexity_above_floor_and_loc_threshold() {
        let mut snapshot = make_snapshot();
        add_file_with_functions(
            &mut snapshot,
            "src/lib.rs",
            vec![
                FunctionMetrics {
                    responsibility: None,
                    is_test: false,
                    name: "moderate".into(),
                    loc: 41,
                    cyclomatic_complexity: 6,
                    max_nesting_depth: 2,
                },
                FunctionMetrics {
                    responsibility: None,
                    is_test: false,
                    name: "long_declarative".into(),
                    loc: 200,
                    cyclomatic_complexity: 5,
                    max_nesting_depth: 2,
                },
            ],
        );

        let result = long_methods(&snapshot, &default_thresholds());
        let RawValue::List(findings) = result.raw_value else {
            panic!("expected findings")
        };
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("moderate"));
    }

    #[test]
    fn all_four_thresholds_are_configurable() {
        // Every fixture sits so that exactly one threshold decides it, and the
        // two UI fixtures straddle long_method_ui_loc: without them a mutant
        // reading long_method_loc for .tsx would survive this test.
        let mut snapshot = make_snapshot();
        add_file_with_functions(
            &mut snapshot,
            "src/lib.rs",
            vec![
                // 31 > long_method_loc 30, CC 4 > floor 3 -> flagged
                FunctionMetrics {
                    responsibility: None,
                    is_test: false,
                    name: "rust_over_loc".into(),
                    loc: 31,
                    cyclomatic_complexity: 4,
                    max_nesting_depth: 2,
                },
                // very long but CC 3 is not above the floor -> not flagged
                FunctionMetrics {
                    responsibility: None,
                    is_test: false,
                    name: "rust_under_cc_floor".into(),
                    loc: 100,
                    cyclomatic_complexity: 3,
                    max_nesting_depth: 2,
                },
            ],
        );
        add_file_with_functions(
            &mut snapshot,
            "src/view.tsx",
            vec![
                // 40 is above long_method_loc 30 but NOT above ui_loc 50:
                // this is the fixture that pins the .tsx tier.
                FunctionMetrics {
                    responsibility: None,
                    is_test: false,
                    name: "ui_between_loc_and_ui_loc".into(),
                    loc: 40,
                    cyclomatic_complexity: 4,
                    max_nesting_depth: 2,
                },
                // 51 > ui_loc 50 -> flagged
                FunctionMetrics {
                    responsibility: None,
                    is_test: false,
                    name: "ui_over_ui_loc".into(),
                    loc: 51,
                    cyclomatic_complexity: 4,
                    max_nesting_depth: 2,
                },
                // CC 8 > long_method_cc 7 -> flagged regardless of LOC
                FunctionMetrics {
                    responsibility: None,
                    is_test: false,
                    name: "cc_over_ceiling".into(),
                    loc: 10,
                    cyclomatic_complexity: 8,
                    max_nesting_depth: 2,
                },
            ],
        );
        let thresholds = crate::config::HealthThresholds {
            long_method_loc: 30,
            long_method_ui_loc: 50,
            long_method_cc_floor: 3,
            long_method_cc: 7,
            ..Default::default()
        };

        let result = long_methods(&snapshot, &thresholds);
        let RawValue::List(findings) = result.raw_value else {
            panic!("expected findings")
        };
        assert_eq!(result.description, "3/5 functions flagged (60.0%)");
        let flagged = |name: &str| findings.iter().any(|f| f.contains(name));
        assert!(flagged("rust_over_loc"), "long_method_loc must decide");
        assert!(flagged("ui_over_ui_loc"), "long_method_ui_loc must decide");
        assert!(flagged("cc_over_ceiling"), "long_method_cc must decide");
        assert!(
            !flagged("rust_under_cc_floor"),
            "long_method_cc_floor must exclude CC at the floor"
        );
        assert!(
            !flagged("ui_between_loc_and_ui_loc"),
            ".tsx must use long_method_ui_loc, not long_method_loc"
        );
    }
}
