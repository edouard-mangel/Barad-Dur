//! M02 milestone 3: the display report consumes the analysis result, and
//! history records come from the result plus snapshot metadata — so `gate`
//! and `backfill` never need to build ownership, ages, audit, or any other
//! display section to reach their decisions and records.
use barad_dur::{
    analysis::{calculate, AnalysisInputs, CategorySelection},
    config::RepoConfig,
    metrics::coupling::CouplingReach,
    scorer,
    snapshot::{
        Author, Commit, CommitId, CouplingFinding, CouplingKind, FileComplexity, FileEntry,
        RepoSnapshot, TimeWindow,
    },
};
use chrono::{Duration, TimeZone, Utc};

fn reference() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 12, 0, 0, 0).unwrap()
}

/// One author, one recent commit touching one Rust file with one content
/// finding — enough for scored categories and detected coupling.
fn snapshot() -> RepoSnapshot {
    let mut snapshot = RepoSnapshot::new(
        "/tmp".into(),
        "test".into(),
        "trunk".into(),
        TimeWindow::default(),
    );
    snapshot.files.push(FileEntry {
        path: "src/a.rs".into(),
        size_bytes: 1,
        is_binary: false,
        depth: 2,
        blob_oid: String::new(),
    });
    snapshot
        .file_metrics
        .insert("src/a.rs".into(), FileComplexity::default());
    snapshot.coupling_findings.push(CouplingFinding {
        path: "src/a.rs".into(),
        line: Some(3),
        kind: CouplingKind::Content,
        evidence: "#[path] import".into(),
    });
    snapshot.authors.push(Author {
        id: 0,
        name: "Alice".into(),
        email: "alice@example.test".into(),
    });
    snapshot.commits.push(Commit {
        id: CommitId(0),
        author: 0,
        timestamp: reference() - Duration::days(3),
        message: "feat: a".into(),
        files_changed: vec![],
        is_merge: false,
        parent_count: 1,
    });
    snapshot
        .commits_by_file
        .insert("src/a.rs".into(), vec![CommitId(0)]);
    snapshot.commits_by_author.insert(0, vec![CommitId(0)]);
    snapshot
}

fn analysis(
    snapshot: &RepoSnapshot,
    selection: CategorySelection,
) -> barad_dur::analysis::AnalysisResult {
    let cfg = RepoConfig::default();
    let weights = cfg.weights.as_weight_pairs();
    calculate(&AnalysisInputs {
        reference_time: reference(),
        snapshot,
        selection,
        thresholds: &cfg.thresholds,
        weights: &weights,
        dependency_evidence: &[],
        god_objects: &[],
        coupling_reach: &CouplingReach::default(),
    })
}

#[test]
fn report_consumes_the_analysis_result_verbatim() {
    let snapshot = snapshot();
    let result = analysis(&snapshot, CategorySelection::GATE);
    let expected_names: Vec<String> = result.categories.iter().map(|c| c.name.clone()).collect();
    let expected_scores: Vec<Option<u32>> = result.categories.iter().map(|c| c.score).collect();
    let expected_counts = result.coupling_evidence.finding_counts();
    let expected_overall = result.overall_score;

    let cfg = RepoConfig::default();
    let report = scorer::build_report(
        reference(),
        &snapshot,
        result,
        None,
        &cfg.thresholds,
        &[],
        &CouplingReach::default(),
    );
    let names: Vec<String> = report.categories.iter().map(|c| c.name.clone()).collect();
    let scores: Vec<Option<u32>> = report.categories.iter().map(|c| c.score).collect();
    assert_eq!(names, expected_names);
    assert_eq!(scores, expected_scores);
    assert_eq!(report.overall_score, expected_overall);
    assert_eq!(report.coupling_finding_counts, expected_counts);
    assert_eq!(report.branch, "trunk");
}

#[test]
fn filtered_report_carries_no_health_advice() {
    let snapshot = snapshot();
    let cfg = RepoConfig::default();
    let report = scorer::build_report(
        reference(),
        &snapshot,
        analysis(
            &snapshot,
            CategorySelection::from_filters(false, true, false, false, false),
        ),
        None,
        &cfg.thresholds,
        &[("src/a.rs".into(), "oversized".into())],
        &CouplingReach::default(),
    );
    assert_eq!(report.categories.len(), 1);
    assert!(
        report
            .top_actions
            .iter()
            .all(|a| !a.text.starts_with("[Health]")),
        "{:?}",
        report.top_actions
    );
}

#[test]
fn history_entry_comes_from_the_result_and_snapshot_metadata() {
    let snapshot = snapshot();
    let result = analysis(&snapshot, CategorySelection::BACKFILL);
    let at = reference() - Duration::days(30);
    let entry =
        scorer::build_history_entry(at, &result, &snapshot, "abc123", Some("backfill".into()));

    assert_eq!(entry.timestamp, at);
    assert_eq!(entry.head, "abc123");
    assert_eq!(entry.branch, "trunk");
    assert_eq!(entry.source.as_deref(), Some("backfill"));
    assert_eq!(entry.overall_score, result.overall_score);
    let mut names: Vec<&String> = entry.categories.keys().collect();
    names.sort();
    assert_eq!(names, ["Evolution", "Git Hygiene", "Health", "Team"]);
    for category in &result.categories {
        assert_eq!(entry.categories[&category.name], category.score);
        for metric in &category.metrics {
            assert_eq!(
                entry.metrics.get(&metric.name).copied(),
                metric.score,
                "scored metrics only: {}",
                metric.name
            );
        }
    }
    assert_eq!(entry.counts.commits, 1);
    assert_eq!(entry.counts.files, 1);
    assert_eq!(entry.counts.authors, 1);
    let counts = result.coupling_evidence.finding_counts().unwrap();
    assert_eq!(entry.counts.content_coupling, Some(counts.content));
    assert_eq!(entry.counts.control_coupling, Some(counts.control));
}
