//! A repository configured by `barad-dur init` must stay analysable.
//!
//! The config is written by one code path (`init`) and consumed by another
//! (`analyze`, which loads and validates it before collecting anything). A drift
//! between the two is invisible to tests that exercise either side alone, so
//! this suite crosses the boundary through the binary itself.

mod common;

use common::{barad_dur, init_git_repo};

#[test]
fn analyze_succeeds_on_a_repository_configured_by_init() {
    let dir = tempfile::TempDir::new().unwrap();
    init_git_repo(dir.path(), "main");
    let target = dir.path().to_str().unwrap();

    barad_dur().args(["init", target]).assert().success();

    let config = dir
        .path()
        .join(".repository-analysis")
        .join("barad-dur.toml");
    assert!(config.exists(), "init should write {}", config.display());

    let stdout = barad_dur()
        .args(["analyze", target, "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let report: serde_json::Value =
        serde_json::from_slice(&stdout).expect("analyze --json should emit a valid report");
    assert!(
        report["overall_score"].is_number(),
        "report should carry an overall score, got {report}"
    );
}

#[test]
fn gate_succeeds_on_a_repository_configured_by_init() {
    let dir = tempfile::TempDir::new().unwrap();
    init_git_repo(dir.path(), "main");
    let target = dir.path().to_str().unwrap();

    barad_dur().args(["init", target]).assert().success();

    // --min-score 0 keeps the assertion on "the config was accepted", not on
    // whatever score a one-commit repository happens to earn.
    barad_dur()
        .args(["gate", target, "--min-score", "0"])
        .assert()
        .success();
}
