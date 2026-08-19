//! M4: hotspot rows cross-reference Pressman findings — per-kind counts,
//! the Content/Common score multiplier, and the JSON contract the HTML
//! report and dashboard read.

use barad_dur::config::RepoConfig;
use barad_dur::scorer;
use barad_dur::snapshot::{
    Author, ChangeType, Commit, CommitId, CouplingFinding, CouplingKind, FileChange,
    FileComplexity, FileEntry, RepoSnapshot, TimeWindow,
};
use chrono::Utc;
use std::path::PathBuf;

fn synthetic_snapshot() -> RepoSnapshot {
    let mut s = RepoSnapshot::new(
        PathBuf::from("/tmp/m4"),
        "m4".into(),
        "main".into(),
        TimeWindow::default(),
    );

    // Add both files to the snapshot.
    for p in ["src/flagged.rs", "src/plain.rs"] {
        s.files.push(FileEntry {
            path: PathBuf::from(p),
            size_bytes: 1,
            is_binary: false,
            depth: 2,
            blob_oid: String::new(),
        });
        s.file_metrics
            .insert(PathBuf::from(p), FileComplexity::default());
    }

    // Add a single dummy author.
    let author_id = 0;
    s.authors.push(Author {
        id: author_id,
        name: "Test Author".into(),
        email: "test@example.com".into(),
    });

    // Add a single commit that touched both files so they have non-zero churn.
    let now = Utc::now();
    let commit = Commit {
        id: CommitId(0),
        author: author_id,
        timestamp: now,
        message: "test commit".into(),
        files_changed: vec![
            FileChange {
                path: PathBuf::from("src/flagged.rs"),
                additions: 10,
                deletions: 5,
                change_type: ChangeType::Modified,
            },
            FileChange {
                path: PathBuf::from("src/plain.rs"),
                additions: 10,
                deletions: 5,
                change_type: ChangeType::Modified,
            },
        ],
        is_merge: false,
        parent_count: 1,
    };
    s.commits.push(commit);

    // Map both files to the commit.
    s.commits_by_file
        .insert(PathBuf::from("src/flagged.rs"), vec![CommitId(0)]);
    s.commits_by_file
        .insert(PathBuf::from("src/plain.rs"), vec![CommitId(0)]);

    // Add the coupling finding only to flagged file.
    s.coupling_findings = vec![CouplingFinding {
        path: PathBuf::from("src/flagged.rs"),
        line: Some(3),
        kind: CouplingKind::Common,
        evidence: "static mut CACHE: usize = 0;".into(),
    }];
    s
}

#[test]
fn report_hotspots_carry_counts_and_multiplied_score() {
    let snapshot = synthetic_snapshot();
    let cfg = RepoConfig::default();
    let report = scorer::build_report(
        &snapshot,
        Vec::new(),
        None,
        &cfg.weights.as_weight_pairs(),
        &cfg.thresholds,
    );

    let flagged = report
        .file_hotspots
        .iter()
        .find(|h| h.path == "src/flagged.rs")
        .expect("flagged file must be a hotspot row");
    let plain = report
        .file_hotspots
        .iter()
        .find(|h| h.path == "src/plain.rs")
        .expect("plain file must be a hotspot row");

    assert_eq!(
        (
            flagged.content_findings,
            flagged.common_findings,
            flagged.control_findings
        ),
        (0, 1, 0)
    );
    // Identical churn/CC/LOC twins: only the multiplier separates them.
    assert!(
        flagged.hotspot_score > plain.hotspot_score,
        "common finding must raise the hotspot score ({} vs {})",
        flagged.hotspot_score,
        plain.hotspot_score
    );
    let ratio = flagged.hotspot_score / plain.hotspot_score;
    assert!(
        (ratio - cfg.thresholds.coupling.hotspot_multiplier).abs() < 1e-9,
        "score ratio must equal the configured multiplier, got {ratio}"
    );
}

#[test]
fn hotspot_json_contract_for_renderers() {
    let snapshot = synthetic_snapshot();
    let cfg = RepoConfig::default();
    let report = scorer::build_report(
        &snapshot,
        Vec::new(),
        None,
        &cfg.weights.as_weight_pairs(),
        &cfg.thresholds,
    );
    let json = serde_json::to_value(&report.file_hotspots).unwrap();
    let flagged = json
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["path"] == "src/flagged.rs")
        .unwrap();
    // Exact field names the HTML template and dashboard read.
    assert_eq!(flagged["content_findings"], 0);
    assert_eq!(flagged["common_findings"], 1);
    assert_eq!(flagged["control_findings"], 0);
}
