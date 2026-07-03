//! M2: finding counts flow into reports and history entries — and are
//! honestly absent when detection did not run (ADR-005 backfill path).

use barad_dur::collector::Collector;
use barad_dur::config::RepoConfig;
use barad_dur::metrics::{coupling, evolution, health, hygiene, team};
use barad_dur::scorer;
use barad_dur::snapshot::{FileEntry, RepoSnapshot, TimeWindow};
use std::path::PathBuf;

fn test_repo_path() -> PathBuf {
    std::env::var("BARAD_DUR_TEST_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

#[test]
fn live_analysis_records_finding_counts_in_history_entry() {
    let Ok(collector) = Collector::open(&test_repo_path(), TimeWindow::default()) else {
        return;
    };
    let snapshot = collector.collect_snapshot().expect("snapshot");
    let default_cfg = RepoConfig::default();
    let weight_pairs = default_cfg.weights.as_weight_pairs();
    let categories = vec![
        health::compute_health(&snapshot, &default_cfg.thresholds.health),
        team::compute_team(&snapshot, &default_cfg.thresholds.team),
        evolution::compute_evolution(&snapshot, &default_cfg.thresholds.evolution),
        hygiene::compute_hygiene(&snapshot, &default_cfg.thresholds.hygiene),
        coupling::compute_coupling(&snapshot, &default_cfg.thresholds.coupling),
    ];
    let report = scorer::build_report(
        &snapshot,
        categories,
        None,
        &weight_pairs,
        &default_cfg.thresholds.coupling,
    );

    let counts = report
        .coupling_finding_counts
        .expect("live analysis must produce counts");
    let entry = scorer::build_history_entry(&report, "test-head", None);
    assert_eq!(entry.counts.content_coupling, Some(counts.content));
    assert_eq!(entry.counts.common_coupling, Some(counts.common));
    assert_eq!(entry.counts.control_coupling, Some(counts.control));
}

#[test]
fn backfill_style_snapshot_records_no_counts_and_unscored_metrics() {
    // `collect_snapshot_at` is crate-private since the v0.19.0 exclusion
    // work, so build the ADR-005 shape it produces through the public API:
    // files listed, but `file_metrics` empty because no AST pass ran.
    // That emptiness is exactly the contract `detection_ran` gates on.
    let mut snapshot = RepoSnapshot::new(
        test_repo_path(),
        "test".into(),
        "main".into(),
        TimeWindow::full_history(),
    );
    snapshot.files.push(FileEntry {
        path: PathBuf::from("src/lib.rs"),
        size_bytes: 100,
        is_binary: false,
        depth: 1,
        blob_oid: String::new(),
    });
    let head = "0000000000000000000000000000000000000000".to_string();

    // Pressman metrics must be unscored (detection didn't run)…
    let default_cfg = RepoConfig::default();
    let cat = coupling::compute_coupling(&snapshot, &default_cfg.thresholds.coupling);
    for name in ["Content coupling", "Common coupling", "Control coupling"] {
        let m = cat.metrics.iter().find(|m| m.name == name).unwrap();
        assert_eq!(
            m.score, None,
            "{name} must be unscored on ADR-005 snapshots"
        );
    }

    // …and the report/history carry no counts (mirroring backfill's category list).
    let weight_pairs = default_cfg.weights.as_weight_pairs();
    let categories = vec![
        health::compute_health(&snapshot, &default_cfg.thresholds.health),
        team::compute_team(&snapshot, &default_cfg.thresholds.team),
        evolution::compute_evolution(&snapshot, &default_cfg.thresholds.evolution),
        hygiene::compute_hygiene(&snapshot, &default_cfg.thresholds.hygiene),
    ];
    let report = scorer::build_report(
        &snapshot,
        categories,
        None,
        &weight_pairs,
        &default_cfg.thresholds.coupling,
    );
    assert_eq!(report.coupling_finding_counts, None);
    let entry = scorer::build_history_entry(&report, &head, Some("backfill".into()));
    assert_eq!(entry.counts.content_coupling, None);
}
