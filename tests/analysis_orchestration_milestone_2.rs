//! M02 milestone 2: coupling evidence is derived once per snapshot and
//! configuration, then shared by every consumer — the Coupling metric, the
//! finding counts, hotspot badges, coupling actions, and the gate ratchet —
//! so none of them can disagree about which findings count.
use barad_dur::{
    analysis::{calculate, AnalysisInputs, CategorySelection},
    config::RepoConfig,
    metrics::coupling::{CouplingEvidence, CouplingReach},
    scorer,
    snapshot::{
        CouplingFinding, CouplingKind, FileComplexity, FileEntry, RepoSnapshot, TimeWindow,
    },
};
use chrono::{TimeZone, Utc};

fn empty_snapshot() -> RepoSnapshot {
    RepoSnapshot::new(
        "/tmp".into(),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    )
}

/// Detection ran (file metrics present) over one Rust source file carrying
/// one AST content-coupling finding.
fn snapshot_with_one_content_finding() -> RepoSnapshot {
    let mut snapshot = empty_snapshot();
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
    snapshot
}

#[test]
fn one_derivation_feeds_the_metric_the_counts_the_hotspot_and_the_action() {
    let cfg = RepoConfig::default();
    let weights = cfg.weights.as_weight_pairs();
    let snapshot = snapshot_with_one_content_finding();
    let reach = CouplingReach::default();
    let at = Utc.with_ymd_and_hms(2026, 9, 12, 0, 0, 0).unwrap();

    let result = calculate(&AnalysisInputs {
        reference_time: at,
        snapshot: &snapshot,
        selection: CategorySelection::GATE,
        thresholds: &cfg.thresholds,
        weights: &weights,
        dependency_evidence: &[],
        god_objects: &[],
        coupling_reach: &reach,
    });

    let counts = result
        .coupling_evidence
        .finding_counts()
        .expect("detection ran");
    assert_eq!(
        (
            counts.content,
            counts.common,
            counts.inheritance,
            counts.control
        ),
        (1, 0, 0, 0)
    );
    let content = result
        .categories
        .iter()
        .find(|c| c.name == "Coupling")
        .unwrap()
        .metrics
        .iter()
        .find(|m| m.name == "Content coupling")
        .unwrap();
    assert!(
        content.description.starts_with("1 finding(s)"),
        "{}",
        content.description
    );

    let report = scorer::build_report(at, &snapshot, result, None, &cfg.thresholds, &[], &reach);
    assert_eq!(report.coupling_finding_counts, Some(counts));
    let hotspot = report
        .file_hotspots
        .iter()
        .find(|h| h.path == "src/a.rs")
        .expect("source file is a hotspot row");
    assert_eq!(hotspot.content_findings, 1);
    assert_eq!(report.coupling_actions.len(), 1);
    assert!(report.coupling_actions[0].text.contains("src/a.rs"));
}

#[test]
fn evidence_without_detection_carries_no_counts() {
    let cfg = RepoConfig::default();
    let evidence = CouplingEvidence::derive(&empty_snapshot(), &cfg.thresholds.coupling);
    assert!(!evidence.detection_ran);
    assert_eq!(
        evidence.finding_counts(),
        None,
        "not collected is not clean"
    );
    assert!(evidence.findings.is_empty());
    assert!(evidence.corroboration.is_empty());
}
