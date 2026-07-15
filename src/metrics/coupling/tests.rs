use super::*;
use crate::config::CouplingThresholds;
use crate::metrics::testutil::{make_file, make_snapshot};
use crate::metrics::RawValue;
use crate::snapshot::{CommitId, CouplingFinding, CouplingKind, RepoSnapshot};
use std::path::PathBuf;

#[test]
fn afferent_coupling_empty_graph() {
    let snapshot = make_snapshot();
    let result = afferent_coupling(&snapshot);
    assert_eq!(result.score, None);
}

#[test]
fn afferent_coupling_single_hub_scores_well() {
    let mut snapshot = make_snapshot();
    // 20 source files + 1 hub → 21 files total, only core.rs has Ca=20
    // Median Ca across all 21 files = 0 (most have zero dependents) → score 100
    snapshot.files.push(make_file("core.rs"));
    for i in 0..20 {
        let name = format!("f{}.rs", i);
        snapshot.files.push(make_file(&name));
        snapshot
            .import_graph
            .insert(PathBuf::from(&name), vec![PathBuf::from("core.rs")]);
    }
    let result = afferent_coupling(&snapshot);
    assert_eq!(result.score, Some(100)); // median Ca=0, single hub is fine
}

#[test]
fn afferent_coupling_widespread_deps_scores_lower() {
    let mut snapshot = make_snapshot();
    // 6 hub files, each depended on by 30 source files
    // All files in the repo: 6 hubs + 30 sources = 36
    // Ca distribution: 6 files with Ca=30, 30 files with Ca=0
    // Median Ca = 0 (majority are sources) → still 100
    // But if we flip it: 30 files each depending on all 6 hubs,
    // and the 6 hubs also depend on each other...
    // Let's make a scenario where median is actually high:
    // 10 files, each one is depended upon by all 9 others → Ca=9 each
    for i in 0..10 {
        snapshot.files.push(make_file(&format!("f{}.rs", i)));
        let targets: Vec<PathBuf> = (0..10)
            .filter(|&j| j != i)
            .map(|j| PathBuf::from(format!("f{}.rs", j)))
            .collect();
        snapshot
            .import_graph
            .insert(PathBuf::from(format!("f{}.rs", i)), targets);
    }
    let result = afferent_coupling(&snapshot);
    assert!(
        result.score.unwrap() <= 50,
        "score={:?}, expected <=50",
        result.score
    );
}

#[test]
fn afferent_coupling_description_shows_distribution() {
    let mut snapshot = make_snapshot();
    snapshot.files.push(make_file("core.rs"));
    for i in 0..5 {
        let name = format!("f{}.rs", i);
        snapshot.files.push(make_file(&name));
        snapshot
            .import_graph
            .insert(PathBuf::from(&name), vec![PathBuf::from("core.rs")]);
    }
    let result = afferent_coupling(&snapshot);
    assert!(result.description.contains("median:"));
    assert!(result.description.contains("mean:"));
    assert!(result.description.contains("max:"));
}

#[test]
fn efferent_coupling_empty_graph() {
    let snapshot = make_snapshot();
    let result = efferent_coupling(&snapshot);
    assert_eq!(result.score, None);
}

#[test]
fn efferent_coupling_single_heavy_file_scores_well() {
    let mut snapshot = make_snapshot();
    // 36 files total: 1 heavy (25 imports), 10 light (1 import), 25 deps
    // Median Ce across 36 files: mostly 0 → score 100
    snapshot.files.push(make_file("main.rs"));
    snapshot.import_graph.insert(
        PathBuf::from("main.rs"),
        (0..25)
            .map(|i| PathBuf::from(format!("dep{}.rs", i)))
            .collect(),
    );
    for i in 0..25 {
        snapshot.files.push(make_file(&format!("dep{}.rs", i)));
    }
    for i in 0..10 {
        let name = format!("small{}.rs", i);
        snapshot.files.push(make_file(&name));
        snapshot
            .import_graph
            .insert(PathBuf::from(&name), vec![PathBuf::from("util.rs")]);
    }
    let result = efferent_coupling(&snapshot);
    assert_eq!(result.score, Some(100)); // median Ce ≈ 0
}

#[test]
fn efferent_coupling_all_heavy_scores_low() {
    let mut snapshot = make_snapshot();
    // 10 files, each imports 15 unique deps (all files in the repo)
    // All 10 files have Ce=15, no other files → median Ce=15 → score 25
    for i in 0..10 {
        let name = format!("f{}.rs", i);
        snapshot.files.push(make_file(&name));
        snapshot.import_graph.insert(
            PathBuf::from(&name),
            (0..15)
                .map(|j| PathBuf::from(format!("dep{}_{}.rs", i, j)))
                .collect(),
        );
    }
    let result = efferent_coupling(&snapshot);
    assert_eq!(result.score, Some(25));
}

#[test]
fn efferent_coupling_description_shows_distribution() {
    let mut snapshot = make_snapshot();
    snapshot.files.push(make_file("a.rs"));
    snapshot.files.push(make_file("b.rs"));
    snapshot.files.push(make_file("c.rs"));
    snapshot.import_graph.insert(
        PathBuf::from("a.rs"),
        vec![PathBuf::from("b.rs"), PathBuf::from("c.rs")],
    );
    let result = efferent_coupling(&snapshot);
    assert!(result.description.contains("median:"));
    assert!(result.description.contains("mean:"));
    assert!(result.description.contains("max:"));
}

#[test]
fn circular_deps_none() {
    let mut snapshot = make_snapshot();
    snapshot
        .import_graph
        .insert(PathBuf::from("a.rs"), vec![PathBuf::from("b.rs")]);
    // b does not import a → no cycle
    let result = circular_dependencies(&snapshot);
    assert_eq!(result.score, Some(100));
}

#[test]
fn circular_deps_direct() {
    let mut snapshot = make_snapshot();
    snapshot
        .import_graph
        .insert(PathBuf::from("a.rs"), vec![PathBuf::from("b.rs")]);
    snapshot
        .import_graph
        .insert(PathBuf::from("b.rs"), vec![PathBuf::from("a.rs")]);
    let result = circular_dependencies(&snapshot);
    assert_eq!(result.score, Some(75)); // 1 cycle
}

#[test]
fn circular_deps_transitive_depth2() {
    let mut snapshot = make_snapshot();
    // A→B→C→A
    snapshot
        .import_graph
        .insert(PathBuf::from("a.rs"), vec![PathBuf::from("b.rs")]);
    snapshot
        .import_graph
        .insert(PathBuf::from("b.rs"), vec![PathBuf::from("c.rs")]);
    snapshot
        .import_graph
        .insert(PathBuf::from("c.rs"), vec![PathBuf::from("a.rs")]);
    let result = circular_dependencies(&snapshot);
    assert!(result.score.unwrap() < 100, "should detect depth-2 cycle");
}

#[test]
fn circular_deps_many() {
    let mut snapshot = make_snapshot();
    // 6 direct cycles → score 25
    for i in 0..6 {
        let a = PathBuf::from(format!("a{}.rs", i));
        let b = PathBuf::from(format!("b{}.rs", i));
        snapshot.import_graph.insert(a.clone(), vec![b.clone()]);
        snapshot.import_graph.insert(b, vec![a]);
    }
    let result = circular_dependencies(&snapshot);
    assert_eq!(result.score, Some(25));
}

fn default_thresholds() -> CouplingThresholds {
    CouplingThresholds::default()
}

fn thresholds_with_depth(depth: usize) -> CouplingThresholds {
    CouplingThresholds {
        component_depth: depth,
        ..CouplingThresholds::default()
    }
}

#[test]
fn extract_component_depth2() {
    let path = std::path::Path::new("src/metrics/coupling.rs");
    assert_eq!(extract_component(path, 2), "src/metrics");
}

#[test]
fn extract_component_depth1() {
    let path = std::path::Path::new("src/metrics/coupling.rs");
    assert_eq!(extract_component(path, 1), "src");
}

#[test]
fn extract_component_shallow_path() {
    let path = std::path::Path::new("main.rs");
    assert_eq!(extract_component(path, 2), "main.rs");
}

#[test]
fn change_coupling_same_component_excluded() {
    let mut snapshot = make_snapshot();
    // Both files share the same depth-2 component "src/module"
    snapshot.file_change_pairs.push((
        PathBuf::from("src/module/a.rs"),
        PathBuf::from("src/module/b.rs"),
        5,
    ));
    snapshot.commits_by_file.insert(
        PathBuf::from("src/module/a.rs"),
        (0u32..10).map(CommitId).collect::<Vec<_>>(),
    );
    snapshot.commits_by_file.insert(
        PathBuf::from("src/module/b.rs"),
        (0u32..10).map(CommitId).collect::<Vec<_>>(),
    );
    let result = change_coupling_smells(&snapshot, &default_thresholds());
    assert_eq!(result.score, Some(100));
}

#[test]
fn change_coupling_cross_component_above_threshold_counted() {
    let mut snapshot = make_snapshot();
    snapshot
        .file_change_pairs
        .push((PathBuf::from("src/a.rs"), PathBuf::from("tests/b.rs"), 5));
    snapshot.commits_by_file.insert(
        PathBuf::from("src/a.rs"),
        (0u32..10).map(CommitId).collect::<Vec<_>>(),
    );
    snapshot.commits_by_file.insert(
        PathBuf::from("tests/b.rs"),
        (0u32..10).map(CommitId).collect::<Vec<_>>(),
    );
    let result = change_coupling_smells(&snapshot, &default_thresholds());
    assert_eq!(result.score, Some(75)); // 1 smell
}

#[test]
fn change_coupling_ratio_below_threshold_excluded() {
    let mut snapshot = make_snapshot();
    snapshot
        .file_change_pairs
        .push((PathBuf::from("src/a.rs"), PathBuf::from("tests/b.rs"), 2));
    snapshot.commits_by_file.insert(
        PathBuf::from("src/a.rs"),
        (0u32..10).map(CommitId).collect::<Vec<_>>(),
    );
    snapshot.commits_by_file.insert(
        PathBuf::from("tests/b.rs"),
        (0u32..10).map(CommitId).collect::<Vec<_>>(),
    );
    let result = change_coupling_smells(&snapshot, &default_thresholds());
    assert_eq!(result.score, Some(100));
}

#[test]
fn change_coupling_missing_commits_entry_excluded() {
    let mut snapshot = make_snapshot();
    snapshot
        .file_change_pairs
        .push((PathBuf::from("src/a.rs"), PathBuf::from("tests/b.rs"), 5));
    // No commits_by_file entries → min(0,0) == 0 → skip
    let result = change_coupling_smells(&snapshot, &default_thresholds());
    assert_eq!(result.score, Some(100));
}

fn make_cross_boundary_snapshot(n: usize) -> RepoSnapshot {
    let mut snapshot = make_snapshot();
    for i in 0..n {
        let a = PathBuf::from(format!("src/f{}.rs", i));
        let b = PathBuf::from(format!("tests/f{}.rs", i));
        snapshot.file_change_pairs.push((a.clone(), b.clone(), 5));
        snapshot
            .commits_by_file
            .insert(a, (0u32..10).map(CommitId).collect::<Vec<_>>());
        snapshot
            .commits_by_file
            .insert(b, (0u32..10).map(CommitId).collect::<Vec<_>>());
    }
    snapshot
}

#[test]
fn change_coupling_scoring_bands() {
    assert_eq!(
        change_coupling_smells(&make_snapshot(), &default_thresholds()).score,
        Some(100)
    );
    assert_eq!(
        change_coupling_smells(&make_cross_boundary_snapshot(2), &default_thresholds()).score,
        Some(75)
    );
    assert_eq!(
        change_coupling_smells(&make_cross_boundary_snapshot(4), &default_thresholds()).score,
        Some(50)
    );
    assert_eq!(
        change_coupling_smells(&make_cross_boundary_snapshot(6), &default_thresholds()).score,
        Some(25)
    );
}

#[test]
fn change_coupling_depth1_same_component() {
    let mut snapshot = make_snapshot();
    snapshot
        .file_change_pairs
        .push((PathBuf::from("src/a.rs"), PathBuf::from("src/b.rs"), 5));
    snapshot.commits_by_file.insert(
        PathBuf::from("src/a.rs"),
        (0u32..10).map(CommitId).collect::<Vec<_>>(),
    );
    snapshot.commits_by_file.insert(
        PathBuf::from("src/b.rs"),
        (0u32..10).map(CommitId).collect::<Vec<_>>(),
    );
    let result = change_coupling_smells(&snapshot, &thresholds_with_depth(1));
    assert_eq!(result.score, Some(100));
}

#[test]
fn change_coupling_depth3_different_component() {
    let mut snapshot = make_snapshot();
    snapshot.file_change_pairs.push((
        PathBuf::from("a/b/c/file.rs"),
        PathBuf::from("a/b/d/file.rs"),
        5,
    ));
    snapshot.commits_by_file.insert(
        PathBuf::from("a/b/c/file.rs"),
        (0u32..10).map(CommitId).collect::<Vec<_>>(),
    );
    snapshot.commits_by_file.insert(
        PathBuf::from("a/b/d/file.rs"),
        (0u32..10).map(CommitId).collect::<Vec<_>>(),
    );
    let result = change_coupling_smells(&snapshot, &thresholds_with_depth(3));
    assert_eq!(result.score, Some(75)); // 1 smell
}

#[test]
fn compute_coupling_returns_eight_metrics() {
    let snapshot = make_snapshot();
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    assert_eq!(result.metrics.len(), 8);
    assert_eq!(result.name, "Coupling");
}

#[test]
fn barrel_bypass_cross_component_is_detected() {
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![
        crate::metrics::testutil::make_file("app/main.ts"),
        crate::metrics::testutil::make_file("lib/index.ts"),
        crate::metrics::testutil::make_file("lib/impl.ts"),
    ];
    // app/main.ts deep-imports lib/impl.ts although lib/index.ts exists
    snapshot.import_graph.insert(
        PathBuf::from("app/main.ts"),
        vec![PathBuf::from("lib/impl.ts")],
    );
    let findings = barrel_bypass_findings(&snapshot, 1);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, crate::snapshot::CouplingKind::Content);
    assert_eq!(findings[0].path, PathBuf::from("app/main.ts"));
    assert_eq!(findings[0].line, None);
    assert!(findings[0].evidence.contains("lib/impl.ts"));
}

#[test]
fn barrel_bypass_same_component_is_not_flagged() {
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![
        crate::metrics::testutil::make_file("lib/a.ts"),
        crate::metrics::testutil::make_file("lib/sub/index.ts"),
        crate::metrics::testutil::make_file("lib/sub/impl.ts"),
    ];
    snapshot.import_graph.insert(
        PathBuf::from("lib/a.ts"),
        vec![PathBuf::from("lib/sub/impl.ts")],
    );
    // component_depth 1: both sides are component "lib" → internal structure
    assert!(barrel_bypass_findings(&snapshot, 1).is_empty());
}

#[test]
fn barrel_bypass_without_barrel_is_not_flagged() {
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![
        crate::metrics::testutil::make_file("app/main.ts"),
        crate::metrics::testutil::make_file("lib/impl.ts"), // no index.ts in lib/
    ];
    snapshot.import_graph.insert(
        PathBuf::from("app/main.ts"),
        vec![PathBuf::from("lib/impl.ts")],
    );
    assert!(barrel_bypass_findings(&snapshot, 1).is_empty());
}

#[test]
fn barrel_import_itself_is_not_flagged() {
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![
        crate::metrics::testutil::make_file("app/main.ts"),
        crate::metrics::testutil::make_file("lib/index.ts"),
    ];
    snapshot.import_graph.insert(
        PathBuf::from("app/main.ts"),
        vec![PathBuf::from("lib/index.ts")], // the sanctioned route
    );
    assert!(barrel_bypass_findings(&snapshot, 1).is_empty());
}

#[test]
fn barrel_bypass_ignores_rust_files() {
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![
        crate::metrics::testutil::make_file("app/main.rs"),
        crate::metrics::testutil::make_file("lib/index.ts"),
        crate::metrics::testutil::make_file("lib/util.rs"),
    ];
    snapshot.import_graph.insert(
        PathBuf::from("app/main.rs"),
        vec![PathBuf::from("lib/util.rs")],
    );
    assert!(barrel_bypass_findings(&snapshot, 1).is_empty());
}

#[test]
fn barrel_bypass_findings_are_sorted_deterministically() {
    // HashMap iteration order is unspecified, so without sorting this test
    // is flaky rather than reliably red — but the contract (stable,
    // alphabetically-sorted order) is still the one M3's gate-delta output
    // relies on, so we assert it directly.
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![
        crate::metrics::testutil::make_file("c/main.ts"),
        crate::metrics::testutil::make_file("a/main.ts"),
        crate::metrics::testutil::make_file("b/main.ts"),
        crate::metrics::testutil::make_file("lib/index.ts"),
        crate::metrics::testutil::make_file("lib/impl.ts"),
    ];
    for src in ["c/main.ts", "a/main.ts", "b/main.ts"] {
        snapshot
            .import_graph
            .insert(PathBuf::from(src), vec![PathBuf::from("lib/impl.ts")]);
    }
    let findings = barrel_bypass_findings(&snapshot, 1);
    let paths: Vec<PathBuf> = findings.into_iter().map(|f| f.path).collect();
    assert_eq!(
        paths,
        vec![
            PathBuf::from("a/main.ts"),
            PathBuf::from("b/main.ts"),
            PathBuf::from("c/main.ts"),
        ]
    );
}

// Test helpers for Pressman metrics
fn snapshot_with_findings(findings: Vec<CouplingFinding>) -> RepoSnapshot {
    let mut s = crate::metrics::testutil::make_snapshot();
    s.files = vec![crate::metrics::testutil::make_file("src/a.rs")];
    s.file_metrics.insert(
        std::path::PathBuf::from("src/a.rs"),
        crate::snapshot::FileComplexity::default(),
    );
    s.coupling_findings = findings;
    s
}

fn make_finding(kind: CouplingKind) -> CouplingFinding {
    CouplingFinding {
        path: PathBuf::from("src/a.rs"),
        line: Some(1),
        kind,
        evidence: "e".into(),
    }
}

#[test]
fn pressman_metrics_appear_in_category() {
    let snapshot = snapshot_with_findings(vec![]);
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    for name in ["Content coupling", "Common coupling", "Control coupling"] {
        assert!(
            result.metrics.iter().any(|m| m.name == name),
            "missing metric {name}"
        );
    }
}

#[test]
fn clean_snapshot_scores_100_on_all_pressman_metrics() {
    let snapshot = snapshot_with_findings(vec![]);
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    let m = result
        .metrics
        .iter()
        .find(|m| m.name == "Content coupling")
        .unwrap();
    assert_eq!(m.score, Some(100));
}

#[test]
fn one_content_finding_scores_at_most_50() {
    let snapshot = snapshot_with_findings(vec![make_finding(CouplingKind::Content)]);
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    let m = result
        .metrics
        .iter()
        .find(|m| m.name == "Content coupling")
        .unwrap();
    assert!(
        m.score.unwrap() <= 50,
        "one content finding must hit the cap trigger"
    );
}

#[test]
fn score_pressman_bands_are_exact() {
    // Exact band table — kills arm-deletion mutants that bound-style
    // assertions let through (a deleted arm falls to the neighbor band).
    use crate::snapshot::CouplingKind::*;
    let cases = [
        (Content, 0, 100),
        (Content, 1, 50),
        (Content, 2, 35),
        (Content, 3, 35),
        (Content, 4, 25),
        (Common, 0, 100),
        (Common, 1, 60),
        (Common, 2, 40),
        (Common, 3, 40),
        (Common, 4, 25),
        (Control, 0, 100),
        (Control, 1, 85),
        (Control, 5, 85),
        (Control, 6, 70),
        (Control, 15, 70),
        (Control, 16, 50),
        (Inheritance, 0, 100),
        (Inheritance, 1, 70),
        (Inheritance, 2, 70),
        (Inheritance, 3, 55),
        (Inheritance, 6, 55),
        (Inheritance, 7, 40),
    ];
    for (kind, count, expected) in cases {
        assert_eq!(
            score_pressman(kind, count),
            expected,
            "{kind:?} count {count}"
        );
    }
}

#[test]
fn pressman_metrics_unscored_when_detection_did_not_run() {
    // Backfill-style snapshot (ADR-005): files listed but no AST pass ran,
    // so file_metrics is empty. Empty findings must NOT read as "clean".
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![crate::metrics::testutil::make_file("src/a.rs")];
    // file_metrics deliberately left empty
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    for name in ["Content coupling", "Common coupling", "Control coupling"] {
        let m = result.metrics.iter().find(|m| m.name == name).unwrap();
        assert_eq!(
            m.score, None,
            "{name} must be unscored when the AST pass didn't run"
        );
    }
}

#[test]
fn pressman_metrics_unscored_without_detectable_files() {
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![crate::metrics::testutil::make_file("main.py")];
    snapshot.file_metrics.insert(
        std::path::PathBuf::from("main.py"),
        crate::snapshot::FileComplexity::default(),
    );
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    let m = result
        .metrics
        .iter()
        .find(|m| m.name == "Common coupling")
        .unwrap();
    assert_eq!(m.score, None, "no Rust/TS/JS files → unscored dash");
}

#[test]
fn content_metric_includes_barrel_findings_when_enabled() {
    let mut snapshot = snapshot_with_findings(vec![]);
    snapshot.files = vec![
        crate::metrics::testutil::make_file("app/main.ts"),
        crate::metrics::testutil::make_file("lib/index.ts"),
        crate::metrics::testutil::make_file("lib/impl.ts"),
    ];
    snapshot.file_metrics.insert(
        std::path::PathBuf::from("app/main.ts"),
        crate::snapshot::FileComplexity::default(),
    );
    snapshot.import_graph.insert(
        PathBuf::from("app/main.ts"),
        vec![PathBuf::from("lib/impl.ts")],
    );
    let thresholds = CouplingThresholds {
        component_depth: 1,
        ..Default::default()
    };
    let result = compute_coupling(&snapshot, &thresholds);
    let m = result
        .metrics
        .iter()
        .find(|m| m.name == "Content coupling")
        .unwrap();
    assert!(m.score.unwrap() <= 50);

    let off = CouplingThresholds {
        component_depth: 1,
        content_barrel_rule: false,
        ..Default::default()
    };
    let result_off = compute_coupling(&snapshot, &off);
    let m_off = result_off
        .metrics
        .iter()
        .find(|m| m.name == "Content coupling")
        .unwrap();
    assert_eq!(
        m_off.score,
        Some(100),
        "toggle off → barrel findings excluded"
    );
}

#[test]
fn control_findings_are_scored_leniently() {
    let findings = (0..3)
        .map(|_| make_finding(CouplingKind::Control))
        .collect();
    let snapshot = snapshot_with_findings(findings);
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    let m = result
        .metrics
        .iter()
        .find(|m| m.name == "Control coupling")
        .unwrap();
    assert!(
        m.score.unwrap() > 70,
        "a few flag args must not tank the metric"
    );
}

#[test]
fn severity_cap_limits_category_when_content_coupling_found() {
    // One content finding among otherwise-perfect metrics: flat average
    // would be ~93 (6×100+50)/7 — the cap must pull it to 70.
    let snapshot = snapshot_with_findings(vec![make_finding(CouplingKind::Content)]);
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    assert!(
        result.score <= 70,
        "category must not be green with content coupling present, got {}",
        result.score
    );
    let m = result
        .metrics
        .iter()
        .find(|m| m.name == "Content coupling")
        .unwrap();
    assert!(
        m.description.contains("capped"),
        "cap must be visible in the triggering metric's description"
    );
}

#[test]
fn severity_cap_not_applied_when_clean() {
    let snapshot = snapshot_with_findings(vec![]);
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    let m = result
        .metrics
        .iter()
        .find(|m| m.name == "Content coupling")
        .unwrap();
    assert!(!m.description.contains("capped"));
}

#[test]
fn severity_cap_triggers_on_many_common_findings() {
    let findings = (0..6).map(|_| make_finding(CouplingKind::Common)).collect();
    let snapshot = snapshot_with_findings(findings);
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    assert!(result.score <= 70, "got {}", result.score);
}

#[test]
fn severity_cap_is_derived_from_score_good_min_not_a_bare_literal() {
    // Locks the cap to scorer/types.rs's single source of truth
    // (SCORE_GOOD_MIN) rather than a magic number duplicated in this
    // module. Currently SCORE_GOOD_MIN - 1 == 70, the same value the old
    // hardcoded literal produced, so this does not go red on its own —
    // it's a contract test that fails the moment the two drift apart.
    let expected_cap = crate::scorer::SCORE_GOOD_MIN - 1;
    let snapshot = snapshot_with_findings(vec![make_finding(CouplingKind::Content)]);
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    assert_eq!(result.score, expected_cap);
    let m = result
        .metrics
        .iter()
        .find(|m| m.name == "Content coupling")
        .unwrap();
    assert!(
        m.description.contains(&format!("capped at {expected_cap}")),
        "note must reflect the derived cap value, not a bare literal: {}",
        m.description
    );
}

#[test]
fn finding_counts_match_metrics_including_barrel() {
    let mut snapshot = snapshot_with_findings(vec![
        make_finding(CouplingKind::Common),
        make_finding(CouplingKind::Control),
        make_finding(CouplingKind::Control),
    ]);
    snapshot.files.extend([
        crate::metrics::testutil::make_file("app/main.ts"),
        crate::metrics::testutil::make_file("lib/index.ts"),
        crate::metrics::testutil::make_file("lib/impl.ts"),
    ]);
    snapshot.import_graph.insert(
        PathBuf::from("app/main.ts"),
        vec![PathBuf::from("lib/impl.ts")],
    );
    let thresholds = CouplingThresholds {
        component_depth: 1,
        ..Default::default()
    };
    let counts = pressman_finding_counts(&snapshot, &thresholds).expect("detection ran");
    assert_eq!(counts.content, 1, "barrel bypass counted into content");
    assert_eq!(counts.common, 1);
    assert_eq!(counts.control, 2);

    let off = CouplingThresholds {
        component_depth: 1,
        content_barrel_rule: false,
        ..Default::default()
    };
    assert_eq!(pressman_finding_counts(&snapshot, &off).unwrap().content, 0);
}

#[test]
fn finding_counts_none_when_detection_did_not_run() {
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![crate::metrics::testutil::make_file("src/a.rs")];
    assert!(pressman_finding_counts(&snapshot, &CouplingThresholds::default()).is_none());
}

#[test]
fn finding_counts_include_inheritance_kind() {
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![crate::metrics::testutil::make_file("src/c.ts")];
    snapshot
        .file_metrics
        .insert("src/c.ts".into(), Default::default());
    snapshot.coupling_findings = vec![CouplingFinding {
        path: "src/c.ts".into(),
        line: Some(2),
        kind: CouplingKind::Inheritance,
        evidence: "class C extends B → A (depth 2)".into(),
    }];
    let counts = pressman_finding_counts(&snapshot, &CouplingThresholds::default()).unwrap();
    assert_eq!(
        (
            counts.content,
            counts.common,
            counts.inheritance,
            counts.control
        ),
        (0, 0, 1, 0)
    );
}

#[test]
fn finding_counts_agree_with_metric_finding_lists() {
    // Guards the "single count source" contract: pressman_finding_counts
    // must equal what the three metrics report. Fixture stays under 10
    // findings per kind so RawValue::List length == the full count.
    let mut snapshot = snapshot_with_findings(vec![
        make_finding(CouplingKind::Content),
        make_finding(CouplingKind::Common),
        make_finding(CouplingKind::Control),
        make_finding(CouplingKind::Control),
    ]);
    snapshot.files.extend([
        crate::metrics::testutil::make_file("app/main.ts"),
        crate::metrics::testutil::make_file("lib/index.ts"),
        crate::metrics::testutil::make_file("lib/impl.ts"),
    ]);
    snapshot.import_graph.insert(
        PathBuf::from("app/main.ts"),
        vec![PathBuf::from("lib/impl.ts")],
    );
    let thresholds = CouplingThresholds {
        component_depth: 1,
        ..Default::default()
    };

    let counts = pressman_finding_counts(&snapshot, &thresholds).expect("detection ran");
    let category = compute_coupling(&snapshot, &thresholds);
    let list_len = |name: &str| -> usize {
        let m = category.metrics.iter().find(|m| m.name == name).unwrap();
        match &m.raw_value {
            RawValue::List(items) => items.len(),
            other => panic!("{name} raw_value should be a List, got {other:?}"),
        }
    };
    assert_eq!(
        counts.content,
        list_len("Content coupling"),
        "content: counts fn vs metric list"
    );
    assert_eq!(
        counts.common,
        list_len("Common coupling"),
        "common: counts fn vs metric list"
    );
    assert_eq!(
        counts.control,
        list_len("Control coupling"),
        "control: counts fn vs metric list"
    );
}

#[test]
fn severity_cap_does_not_raise_already_low_scores() {
    // If the average is already below 70 the cap must not touch it.
    let findings = vec![
        make_finding(CouplingKind::Content),
        make_finding(CouplingKind::Content),
        make_finding(CouplingKind::Content),
        make_finding(CouplingKind::Content),
    ];
    let snapshot = snapshot_with_findings(findings);
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    let flat_average_would_be = result.metrics.iter().filter_map(|m| m.score).sum::<u32>()
        / result.metrics.iter().filter(|m| m.score.is_some()).count() as u32;
    assert!(result.score <= flat_average_would_be.min(70));
}

#[test]
fn cap_triggers_track_the_band_table() {
    // A single content finding must trigger the cap; common needs the 4+ band.
    // These assert the *linkage*, not the values — the 16-case band table
    // test pins the values.
    assert_eq!(
        CONTENT_CAP_TRIGGER,
        score_pressman(CouplingKind::Content, 1)
    );
    assert_eq!(COMMON_CAP_TRIGGER, score_pressman(CouplingKind::Common, 4));
}

#[test]
fn qualifying_smell_pairs_matches_change_coupling_count() {
    // Same fixture the scoring-band test uses: 4 cross-boundary smells.
    let snapshot = make_cross_boundary_snapshot(4);
    let via_helper = qualifying_smell_pairs(&snapshot, &default_thresholds()).count();
    // change_coupling_smells(4) scores 50 == the ">5? no, 3..=5" band for 4 smells.
    assert_eq!(
        via_helper, 4,
        "helper must yield exactly the qualifying pairs"
    );
    assert_eq!(
        change_coupling_smells(&snapshot, &default_thresholds()).score,
        Some(50),
        "refactored smell metric must keep its score"
    );
}

#[test]
fn corroboration_degree_counts_distinct_partners() {
    let mut snapshot = make_snapshot();
    // src/a.rs co-changes cross-boundary with tests/b.rs and tests/c.rs.
    for partner in ["tests/b.rs", "tests/c.rs"] {
        snapshot
            .file_change_pairs
            .push((PathBuf::from("src/a.rs"), PathBuf::from(partner), 5));
        snapshot
            .commits_by_file
            .insert(PathBuf::from(partner), (0u32..10).map(CommitId).collect());
    }
    snapshot.commits_by_file.insert(
        PathBuf::from("src/a.rs"),
        (0u32..10).map(CommitId).collect(),
    );

    let deg = corroboration_degree(&snapshot, &default_thresholds());
    assert_eq!(deg.get(&PathBuf::from("src/a.rs")), Some(&2));
    assert_eq!(deg.get(&PathBuf::from("tests/b.rs")), Some(&1));
    assert_eq!(deg.get(&PathBuf::from("tests/c.rs")), Some(&1));
}

#[test]
fn corroboration_degree_excludes_below_threshold_and_same_component() {
    let mut snapshot = make_snapshot();
    // Below ratio (2/10 < 0.30): excluded.
    snapshot
        .file_change_pairs
        .push((PathBuf::from("src/a.rs"), PathBuf::from("tests/b.rs"), 2));
    // Same component (both src/*, depth 2 differs -> actually different;
    // use src/x/ to force same depth-2 component "src/x").
    snapshot
        .file_change_pairs
        .push((PathBuf::from("src/x/a.rs"), PathBuf::from("src/x/b.rs"), 9));
    for f in ["src/a.rs", "tests/b.rs", "src/x/a.rs", "src/x/b.rs"] {
        snapshot
            .commits_by_file
            .insert(PathBuf::from(f), (0u32..10).map(CommitId).collect());
    }
    let deg = corroboration_degree(&snapshot, &default_thresholds());
    assert!(deg.is_empty(), "no pair qualifies: {deg:?}");
}

/// `snapshot_with_findings`, plus a qualifying cross-boundary co-change pair
/// for each given finding-file path so those findings corroborate.
fn snapshot_with_corroborated(findings: Vec<CouplingFinding>) -> RepoSnapshot {
    let mut s = snapshot_with_findings(findings.clone());
    for (i, f) in findings.iter().enumerate() {
        let partner = PathBuf::from(format!("tests/partner{i}.rs"));
        s.file_change_pairs
            .push((f.path.clone(), partner.clone(), 5));
        s.commits_by_file
            .insert(f.path.clone(), (0u32..10).map(CommitId).collect());
        s.commits_by_file
            .insert(partner, (0u32..10).map(CommitId).collect());
    }
    s
}

#[test]
fn corroborated_common_finding_scores_one_band_worse() {
    // 1 dormant Common finding -> count 1 -> 60.
    let dormant = snapshot_with_findings(vec![make_finding(CouplingKind::Common)]);
    let d = compute_coupling(&dormant, &CouplingThresholds::default());
    let d_common = d
        .metrics
        .iter()
        .find(|m| m.name == "Common coupling")
        .unwrap();
    assert_eq!(d_common.score, Some(60));

    // 1 corroborated Common finding -> effective 2 (weight 2.0) -> 40.
    let corr = snapshot_with_corroborated(vec![make_finding(CouplingKind::Common)]);
    let c = compute_coupling(&corr, &CouplingThresholds::default());
    let c_common = c
        .metrics
        .iter()
        .find(|m| m.name == "Common coupling")
        .unwrap();
    assert_eq!(c_common.score, Some(40));
}

#[test]
fn weight_one_reproduces_dormant_scores() {
    let corr = snapshot_with_corroborated(vec![make_finding(CouplingKind::Common)]);
    let thresholds = CouplingThresholds {
        corroboration_weight: 1.0,
        ..CouplingThresholds::default()
    };
    let c = compute_coupling(&corr, &thresholds);
    let common = c
        .metrics
        .iter()
        .find(|m| m.name == "Common coupling")
        .unwrap();
    assert_eq!(
        common.score,
        Some(60),
        "weight 1.0 must equal the dormant score"
    );
}

#[test]
fn corroboration_can_trip_the_severity_cap() {
    // 2 corroborated Common findings on distinct files -> effective 4 -> 25,
    // which is <= the Common cap trigger (25) -> category capped.
    let corr = snapshot_with_corroborated(vec![
        CouplingFinding {
            path: PathBuf::from("src/a.rs"),
            line: Some(1),
            kind: CouplingKind::Common,
            evidence: "static mut A".into(),
        },
        CouplingFinding {
            path: PathBuf::from("src/b.rs"),
            line: Some(1),
            kind: CouplingKind::Common,
            evidence: "static mut B".into(),
        },
    ]);
    let c = compute_coupling(&corr, &CouplingThresholds::default());
    let common = c
        .metrics
        .iter()
        .find(|m| m.name == "Common coupling")
        .unwrap();
    assert_eq!(common.score, Some(25));
    assert!(
        c.score < crate::scorer::SCORE_GOOD_MIN,
        "category must be capped"
    );
}

#[test]
fn corroborated_finding_is_annotated_in_evidence_and_description() {
    let corr = snapshot_with_corroborated(vec![make_finding(CouplingKind::Common)]);
    let c = compute_coupling(&corr, &CouplingThresholds::default());
    let common = c
        .metrics
        .iter()
        .find(|m| m.name == "Common coupling")
        .unwrap();
    assert!(
        common
            .description
            .contains("1 corroborated by change history"),
        "description: {}",
        common.description
    );
    match &common.raw_value {
        RawValue::List(items) => assert!(
            items
                .iter()
                .any(|s| s.contains("corroborated (co-changes with 1 file(s))")),
            "evidence: {items:?}"
        ),
        other => panic!("expected List, got {other:?}"),
    }
}

#[test]
fn all_coupling_findings_equals_findings_plus_gated_barrel() {
    // Barrel-on: a cross-component import bypassing a barrel yields a Content
    // finding via gated_barrel_findings; all_coupling_findings must include it
    // on top of the raw AST findings.
    let mut snapshot = make_snapshot();
    snapshot.files = vec![
        make_file("a/index.ts"),
        make_file("a/impl.ts"),
        make_file("b/main.ts"),
    ];
    snapshot.file_metrics.insert(
        std::path::PathBuf::from("b/main.ts"),
        crate::snapshot::FileComplexity::default(),
    );
    snapshot
        .import_graph
        .insert(PathBuf::from("b/main.ts"), vec![PathBuf::from("a/impl.ts")]);
    let th = default_thresholds();

    let gated = gated_barrel_findings(&snapshot, &th);
    let all = all_coupling_findings(&snapshot, &th);
    assert_eq!(all.len(), snapshot.coupling_findings.len() + gated.len());
    // Turning the rule off drops the barrel findings from both.
    let off = CouplingThresholds {
        content_barrel_rule: false,
        ..default_thresholds()
    };
    assert!(gated_barrel_findings(&snapshot, &off).is_empty());
    assert_eq!(
        all_coupling_findings(&snapshot, &off).len(),
        snapshot.coupling_findings.len()
    );
}

#[test]
fn all_coupling_findings_and_counts_include_inheritance() {
    use crate::snapshot::{BaseRef, ClassRecord};
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![
        crate::metrics::testutil::make_file("src/a.ts"),
        crate::metrics::testutil::make_file("src/b.ts"),
        crate::metrics::testutil::make_file("src/c.ts"),
    ];
    snapshot
        .file_metrics
        .insert("src/c.ts".into(), Default::default());
    snapshot.class_records = vec![
        ClassRecord {
            path: "src/b.ts".into(),
            line: 2,
            class_name: "B".into(),
            base: BaseRef::Resolved {
                path: "src/a.ts".into(),
                name: "A".into(),
            },
        },
        ClassRecord {
            path: "src/c.ts".into(),
            line: 2,
            class_name: "C".into(),
            base: BaseRef::Resolved {
                path: "src/b.ts".into(),
                name: "B".into(),
            },
        },
    ];
    let cfg = CouplingThresholds::default();
    let inh = all_coupling_findings(&snapshot, &cfg)
        .into_iter()
        .filter(|f| f.kind == CouplingKind::Inheritance)
        .count();
    assert_eq!(inh, 1);
    let counts = pressman_finding_counts(&snapshot, &cfg).unwrap();
    assert_eq!(counts.inheritance, 1);
}

#[test]
fn inheritance_metric_row_uses_bands() {
    use crate::snapshot::{BaseRef, ClassRecord};
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![
        crate::metrics::testutil::make_file("src/a.ts"),
        crate::metrics::testutil::make_file("src/b.ts"),
        crate::metrics::testutil::make_file("src/c.ts"),
    ];
    snapshot
        .file_metrics
        .insert("src/c.ts".into(), Default::default());
    snapshot.class_records = vec![
        ClassRecord {
            path: "src/b.ts".into(),
            line: 2,
            class_name: "B".into(),
            base: BaseRef::Resolved {
                path: "src/a.ts".into(),
                name: "A".into(),
            },
        },
        ClassRecord {
            path: "src/c.ts".into(),
            line: 2,
            class_name: "C".into(),
            base: BaseRef::Resolved {
                path: "src/b.ts".into(),
                name: "B".into(),
            },
        },
    ];
    let metrics = compute_coupling(&snapshot, &CouplingThresholds::default());
    let m = metrics
        .metrics
        .iter()
        .find(|m| m.name == "Inheritance coupling")
        .expect("metric row");
    assert_eq!(m.score, Some(70), "1 finding → 70 band");
    assert!(m.description.contains("1 finding(s)"));
}
