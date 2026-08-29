use crate::config::HealthThresholds;
use crate::metrics::file_role::{classify, FileRole};
use crate::metrics::{score_prevalence, MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

/// Production source code only — tests, config, and docs are other roles.
pub(super) fn is_source_file(path: &std::path::Path) -> bool {
    classify(path) == FileRole::Source
}

/// The degree a file must reach to count as a structural hub: the higher of
/// an absolute floor and a percentile of the repo's own degree distribution.
///
/// The floor lets a genuinely uncoupled repository flag nothing — a purely
/// relative rule always fires on its own top decile, however healthy the
/// codebase. The percentile handles the opposite case: where most files are
/// heavily connected, a degree of 8 is unremarkable and the bar must rise.
///
/// This replaces `degree > median * 4`, which never bound. Across five real
/// repositories the degree median was 0-2, so that term topped out at 8 —
/// never above the floor — and so never once decided an outcome.
fn hub_threshold(degrees: &[usize], thresholds: &HealthThresholds) -> usize {
    let mut sorted: Vec<usize> = degrees.to_vec();
    sorted.sort_unstable();
    let percentile = if sorted.is_empty() {
        0
    } else {
        // Nearest rank: ceil(p * n) - 1. A floored index would give n - 1 —
        // the maximum — for any n <= 10 at p = 0.90, letting a lone outlier
        // set the bar that excludes every one of its peers.
        let rank = (thresholds.god_node_degree_percentile * sorted.len() as f64).ceil() as usize;
        sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
    };
    percentile.max(thresholds.god_node_min_degree)
}

/// A file structurally dominates the codebase when its degree reaches the
/// repo's hub threshold.
fn is_structural_hub(degree: usize, threshold: usize) -> bool {
    degree >= threshold
}

/// Why a file was flagged — LOC, method bloat, structural centrality, a
/// name-smell annotation, or a combination — so the report says what
/// actually tripped, not just what. `None` when neither condition fires.
fn god_reason(
    path: &std::path::Path,
    m: &crate::snapshot::FileComplexity,
    degree: usize,
    hub_threshold: usize,
) -> Option<String> {
    let mut reasons = Vec::new();
    // The size/method-bloat rungs only mean something for files with actual
    // logic — a CC=0 file (e.g. a pure re-export barrel) is exempt from
    // those, but not from the hub check below, which is about import-graph
    // centrality and has nothing to do with cyclomatic complexity.
    if m.cyclomatic_complexity > 0 {
        if m.loc > 500 {
            reasons.push(format!("{} loc", m.loc));
        } else if m.loc > 300 && m.public_methods > 15 {
            reasons.push(format!(
                "{} loc, {} public methods",
                m.loc, m.public_methods
            ));
        }
    }
    if is_structural_hub(degree, hub_threshold) {
        reasons.push(format!(
            "structural hub — {degree} connections (threshold {hub_threshold})"
        ));
    }
    if !reasons.is_empty() && crate::metrics::name_smell::has_smelly_name(path) {
        reasons.push("generic name suggests broad responsibility".to_string());
    }
    (!reasons.is_empty()).then(|| reasons.join("; "))
}

/// Files flagged as god objects, with their reason string — the single
/// definition `god_objects()`'s display list and any downstream action
/// generator (`generate_refactoring_actions`) share, so they never diverge
/// on which files qualify. Callers building a full report compute this once
/// and pass it to both `compute_health` and `build_report`.
pub fn god_object_files(
    snapshot: &RepoSnapshot,
    thresholds: &HealthThresholds,
) -> Vec<(std::path::PathBuf, String)> {
    let incoming = crate::metrics::incoming_import_counts(&snapshot.import_graph);

    // Computed once per file here, then looked up (never recomputed) for
    // both the median and the per-file hub check below.
    let degrees: std::collections::HashMap<&std::path::Path, usize> = snapshot
        .file_metrics
        .keys()
        .filter(|p| is_source_file(p))
        .map(|p| {
            let outgoing = crate::metrics::outgoing_degree(&snapshot.import_graph, p);
            let inc = incoming.get(p.as_path()).copied().unwrap_or(0);
            (p.as_path(), outgoing + inc)
        })
        .collect();

    let degree_values: Vec<usize> = degrees.values().copied().collect();
    let hub_bar = hub_threshold(&degree_values, thresholds);

    let mut flagged: Vec<(std::path::PathBuf, String)> = snapshot
        .file_metrics
        .iter()
        .filter(|(p, _)| is_source_file(p))
        .filter_map(|(p, m)| {
            let degree = degrees.get(p.as_path()).copied().unwrap_or(0);
            god_reason(p, m, degree, hub_bar).map(|reason| (p.clone(), reason))
        })
        .collect();
    // snapshot.file_metrics is a HashMap — sort for deterministic report output.
    // Sort by the display string, not PathBuf component ordering — matches
    // the pre-extraction behavior of sorting the fully-formatted
    // "path — reason" strings (PathBuf::cmp compares path components, which
    // can disagree with plain byte-string comparison, e.g. for
    // "src-utils.rs" vs "src/utils.rs").
    flagged.sort_by(|a, b| a.0.to_string_lossy().cmp(&b.0.to_string_lossy()));
    flagged
}

/// Files that have grown too large to maintain (god objects / bloaters), or
/// that dominate the import graph as a structural hub. Takes the already-
/// computed `god_object_files` result rather than recomputing it, so
/// `compute_health` and a report's refactoring-action generator can share
/// one pass over the snapshot instead of two.
pub(super) fn god_objects(
    snapshot: &RepoSnapshot,
    flagged_god_objects: &[(std::path::PathBuf, String)],
) -> MetricValue {
    let source_total = snapshot
        .file_metrics
        .keys()
        .filter(|p| is_source_file(p))
        .count();

    let gods: Vec<String> = flagged_god_objects
        .iter()
        .map(|(p, reason)| format!("{} — {reason}", p.display()))
        .collect();

    let count = gods.len();
    let pct = if source_total > 0 {
        count as f64 / source_total as f64 * 100.0
    } else {
        0.0
    };

    let score = score_prevalence(count, source_total);

    MetricValue {
        name: "God objects".to_string(),
        description: format!(
            "{}/{} source files oversized or structurally overconnected ({:.1}%)",
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

    #[test]
    fn hub_threshold_is_the_floor_when_the_graph_is_sparse() {
        // Measured across five real repositories, the degree median is 0-2
        // and p90 is 4-10. Below the floor the absolute bar must govern, or
        // a repository with no real hubs starts flagging ordinary files.
        let th = HealthThresholds::default();
        let sparse: Vec<usize> = std::iter::repeat_n(0, 90).chain(1..=10).collect();
        assert_eq!(hub_threshold(&sparse, &th), th.god_node_min_degree);
    }

    #[test]
    fn hub_threshold_rises_above_the_floor_on_a_dense_graph() {
        // The point of a relative term: where most files are heavily
        // connected, a degree of 8 is unremarkable and must not be flagged.
        let th = HealthThresholds::default();
        let dense: Vec<usize> = (1..=100).map(|i| i / 2).collect();
        let threshold = hub_threshold(&dense, &th);
        assert!(
            threshold > th.god_node_min_degree,
            "p90 of a dense graph must outrank the floor, got {threshold}"
        );
    }

    #[test]
    fn hub_threshold_replaces_a_median_multiplier_that_never_bound() {
        // The previous rule was `degree > median * 4`. Across every repo
        // measured the median was 0-2, so that term maxed out at 8 — never
        // above the floor of 8, so it never once decided an outcome. The
        // percentile form does: on a mautic-shaped graph (median 2, p90 10)
        // it raises the bar, which is exactly where it should.
        let th = HealthThresholds::default();
        // Shaped to mautic's measured distribution: median 2, p90 10.
        let mautic_shaped: Vec<usize> = std::iter::repeat_n(0, 30)
            .chain(std::iter::repeat_n(2, 30))
            .chain(std::iter::repeat_n(4, 20))
            .chain(std::iter::repeat_n(10, 10))
            .chain(std::iter::repeat_n(40, 10))
            .collect();
        let median_rule = (crate::metrics::median(&mautic_shaped) * 4.0) as usize;
        assert!(
            median_rule <= th.god_node_min_degree,
            "the old term never exceeded the floor: {median_rule}"
        );
        assert!(
            hub_threshold(&mautic_shaped, &th) > th.god_node_min_degree,
            "the percentile term must actually bind"
        );
    }

    #[test]
    fn hub_threshold_uses_nearest_rank_not_a_floored_index() {
        // p90 must be the 90th-percentile value, not the maximum. With a
        // floored index, `floor(n * 0.9) == n - 1` for any n <= 10, so the
        // single most-connected file would define the bar every other file
        // is measured against — and nothing below it could ever be flagged.
        let th = HealthThresholds::default();
        // Nine files at degree 20, one at 400. p90 of that is 20, not 400.
        let one_huge_outlier: Vec<usize> = std::iter::repeat_n(20, 9).chain([400]).collect();
        assert_eq!(
            hub_threshold(&one_huge_outlier, &th),
            20,
            "the outlier must not set the threshold that excludes its peers"
        );
    }

    #[test]
    fn hub_threshold_on_an_empty_graph_is_the_floor() {
        let th = HealthThresholds::default();
        assert_eq!(hub_threshold(&[], &th), th.god_node_min_degree);
    }

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
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
        // 1/100 = 1% (prevalence 90), capped by the count band until the
        // source population passes the trusted-size threshold.
        assert_eq!(result.score, Some(75));
        match &result.raw_value {
            RawValue::List(v) => {
                assert_eq!(v.len(), 1);
                assert_eq!(
                    v[0], "fat.rs — 520 loc",
                    "size-flagged entries must name the LOC reason, not just the path"
                );
            }
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
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
        assert_eq!(result.score, Some(100));
        assert_eq!(
            result.description,
            "0/10 source files oversized or structurally overconnected (0.0%)"
        );
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
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
        // one finding among 100 source files — count band still governs
        assert_eq!(result.score, Some(75));
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
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
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
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
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
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
        // 1/100 = 1%, count band governs below the trusted population
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
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
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
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
        // medium prevalence, still inside the count-band-capped range
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
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
        // high prevalence, still inside the count-band-capped range
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
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
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
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
        // flagged: 1/100 = 1%, capped by the count band
        assert_eq!(result.score, Some(75));
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
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
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
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
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
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
        match &result.raw_value {
            RawValue::List(v) => {
                assert_eq!(v.len(), 1, "only the hub should be flagged");
                assert!(
                    v[0].starts_with("hub.rs — structural hub"),
                    "entry should name the hub reason, got: {}",
                    v[0]
                );
                assert!(
                    v[0].contains("20 connections"),
                    "entry should show the degree, got: {}",
                    v[0]
                );
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn god_objects_flags_barrel_file_hub_despite_zero_complexity() {
        // A pure re-export barrel file has no branches, so cyclomatic_complexity
        // is 0 — that must not exempt it from the (unrelated) hub check.
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            PathBuf::from("index.ts"),
            FileComplexity {
                total_lines: 3,
                loc: 3,
                cyclomatic_complexity: 0,
                public_methods: 0,
                properties: 0,
                ..Default::default()
            },
        );
        for i in 0..20 {
            let name = format!("leaf{i}.ts");
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
                .insert(PathBuf::from(&name), vec![PathBuf::from("index.ts")]);
        }
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
        match &result.raw_value {
            RawValue::List(v) => {
                assert!(
                    v.iter().any(|e| e.starts_with("index.ts — structural hub")),
                    "barrel file with CC=0 must still be flagged as a hub, got: {v:?}"
                );
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn god_objects_list_is_sorted_for_determinism() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        for name in ["zeta.rs", "alpha.rs", "mu.rs"] {
            snapshot.file_metrics.insert(
                PathBuf::from(name),
                FileComplexity {
                    total_lines: 600,
                    loc: 600,
                    cyclomatic_complexity: 1,
                    public_methods: 1,
                    properties: 0,
                    ..Default::default()
                },
            );
        }
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
        match &result.raw_value {
            RawValue::List(v) => {
                let mut sorted = v.clone();
                sorted.sort();
                assert_eq!(
                    v, &sorted,
                    "god objects list must be sorted for deterministic report output"
                );
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
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
        assert_eq!(result.score, Some(100));
    }

    #[test]
    fn god_objects_notes_generic_name_on_flagged_file() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            PathBuf::from("UserManager.rs"),
            FileComplexity {
                total_lines: 600,
                loc: 520,
                cyclomatic_complexity: 10,
                public_methods: 5,
                properties: 2,
                ..Default::default()
            },
        );
        add_normal_files(&mut snapshot, 99);
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
        match &result.raw_value {
            RawValue::List(v) => {
                assert_eq!(v.len(), 1);
                assert_eq!(
                    v[0], "UserManager.rs — 520 loc; generic name suggests broad responsibility",
                    "smelly-named flagged file must get the name-based reason appended"
                );
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn god_objects_smelly_name_alone_does_not_trigger_flag() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        // Small, well-scoped file with a "smelly" stem — name alone must never flag it.
        snapshot.file_metrics.insert(
            PathBuf::from("common.rs"),
            FileComplexity {
                total_lines: 100,
                loc: 80,
                cyclomatic_complexity: 3,
                public_methods: 2,
                properties: 1,
                ..Default::default()
            },
        );
        let result = god_objects(
            &snapshot,
            &god_object_files(&snapshot, &HealthThresholds::default()),
        );
        assert_eq!(result.score, Some(100));
        match &result.raw_value {
            RawValue::List(v) => assert!(v.is_empty(), "name-smell alone must not create a flag"),
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn god_object_files_matches_god_objects_flagged_set() {
        // Regression guard for the extraction below: `god_objects()`'s display
        // list and `god_object_files()`'s structured list must always agree on
        // which files qualify (same "one definition, not two" rule as the M5
        // corroboration-predicate extraction).
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
        add_normal_files(&mut snapshot, 99);
        let thresholds = HealthThresholds::default();
        let result = god_objects(&snapshot, &god_object_files(&snapshot, &thresholds));
        let files = god_object_files(&snapshot, &thresholds);
        let expected: Vec<String> = files
            .iter()
            .map(|(p, reason)| format!("{} — {reason}", p.display()))
            .collect();
        match &result.raw_value {
            RawValue::List(v) => assert_eq!(v, &expected),
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn god_object_files_sorts_by_display_string_not_pathbuf_components() {
        // Regression guard: PathBuf::cmp compares path COMPONENTS
        // ("src-utils.rs" is one component; "src/utils.rs" is two: "src"
        // then "utils.rs"), which reorders these two paths relative to
        // plain byte-string comparison ('-' 0x2D < '/' 0x2F). The
        // pre-extraction code sorted the fully-formatted "path — reason"
        // strings, so the sort key here must be the path's display string,
        // not the PathBuf itself, to stay behavior-identical.
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        for name in ["src/utils.rs", "src-utils.rs"] {
            snapshot.file_metrics.insert(
                PathBuf::from(name),
                FileComplexity {
                    total_lines: 600,
                    loc: 520,
                    cyclomatic_complexity: 10,
                    public_methods: 5,
                    properties: 2,
                    ..Default::default()
                },
            );
        }
        let flagged = god_object_files(&snapshot, &HealthThresholds::default());
        let paths: Vec<String> = flagged
            .iter()
            .map(|(p, _)| p.display().to_string())
            .collect();
        assert_eq!(
            paths,
            vec!["src-utils.rs".to_string(), "src/utils.rs".to_string()],
            "must sort by display string ('-' < '/'), not PathBuf components"
        );
    }
}
