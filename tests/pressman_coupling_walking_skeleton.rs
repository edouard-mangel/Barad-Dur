//! Walking skeleton: end-to-end Pressman coupling detection on a real repo.
//! Uses BARAD_DUR_TEST_REPO (CI: CI_PROJECT_DIR) or `.` — same convention
//! as the other integration suites.

use barad_dur::collector::Collector;
use barad_dur::config::CouplingThresholds;
use barad_dur::metrics::coupling::compute_coupling;
use barad_dur::snapshot::TimeWindow;
use std::path::PathBuf;

fn test_repo_path() -> PathBuf {
    std::env::var("BARAD_DUR_TEST_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

#[test]
fn analysis_reports_three_pressman_metrics() {
    let Ok(collector) = Collector::open(&test_repo_path(), TimeWindow::default()) else {
        return; // not a git repo (e.g. cargo-mutants temp dir) — skip
    };
    let snapshot = collector
        .collect_snapshot()
        .expect("snapshot collection must succeed");

    let result = compute_coupling(
        &snapshot,
        &CouplingThresholds::default(),
        &barad_dur::config::HealthThresholds::default(),
    );

    for name in ["Content coupling", "Common coupling", "Control coupling"] {
        let metric = result
            .metrics
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("metric '{name}' missing from Coupling category"));
        assert!(
            metric.score.is_some(),
            "barad-dur has Rust files, so '{name}' must be scored"
        );
    }
    // Dogfood expectation: this codebase avoids mutable globals entirely.
    let common = result
        .metrics
        .iter()
        .find(|m| m.name == "Common coupling")
        .unwrap();
    assert_eq!(
        common.score,
        Some(100),
        "unexpected common-coupling findings in barad-dur itself: {:?}",
        common.raw_value
    );
}
