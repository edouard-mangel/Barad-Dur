//! Walking skeleton for the per-entity trend history feature: a fixture
//! repo whose one source file provably grows in cyclomatic complexity
//! across 3 commits, backfilled with sample_count = 3, must produce a
//! monotonically increasing complexity series that `compute_entity_trend`
//! classifies as Growing.

use barad_dur::cache::entity_history;
use barad_dur::cli::BackfillArgs;
use barad_dur::trend::{compute_entity_trend, EntityTrendDirection};
use std::process::Command;

fn git(dir: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

/// Commit with an explicit, distinct author/committer date. Commits made in
/// rapid succession within a test can otherwise land in the same
/// second-resolution timestamp, making chronological ordering ambiguous.
fn git_commit_at(dir: &std::path::Path, message: &str, date: &str) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["commit", "-q", "-m", message])
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .status()
        .unwrap();
    assert!(status.success(), "git commit at {date:?} failed");
}

#[test]
fn backfill_records_a_growing_complexity_series() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path();

    git(path, &["init", "-q"]);
    git(path, &["config", "user.email", "t@e"]);
    git(path, &["config", "user.name", "t"]);

    // Commit 1: a trivial function.
    std::fs::write(path.join("lib.rs"), "fn f(x: i32) -> i32 { x }\n").unwrap();
    git(path, &["add", "-A"]);
    git_commit_at(path, "commit 1", "2024-01-01T00:00:00");

    // Commit 2: add a branch.
    std::fs::write(
        path.join("lib.rs"),
        "fn f(x: i32) -> i32 { if x > 0 { x } else { -x } }\n",
    )
    .unwrap();
    git(path, &["add", "-A"]);
    git_commit_at(path, "commit 2", "2024-01-02T00:00:00");

    // Commit 3: add nested branches.
    std::fs::write(
        path.join("lib.rs"),
        "fn f(x: i32) -> i32 { if x > 0 { if x > 10 { x * 2 } else { x } } else { -x } }\n",
    )
    .unwrap();
    git(path, &["add", "-A"]);
    git_commit_at(path, "commit 3", "2024-01-03T00:00:00");

    let args = BackfillArgs {
        target: path.to_string_lossy().into_owned(),
        no_blame: false,
    };
    barad_dur::backfill::run(&args, path).expect("backfill should succeed");

    let mut entries = entity_history::load_entity_history(path).expect("load entity history");
    assert_eq!(entries.len(), 3, "one entry per sampled commit");

    // Backfill's sample selection returns commits newest-first whenever the
    // repo has no more commits than `sample_count` (the default, 10, exceeds
    // this fixture's 3 commits), so entries land in `entity_trends.json` in
    // that same newest-first write order. Sort chronologically before
    // building the series — a real trend/consumer must do the same, since
    // nothing about the storage format itself guarantees write order.
    entries.sort_by_key(|e| e.timestamp);

    let series: Vec<f64> = entries
        .iter()
        .filter_map(|e| e.complexity.get("lib.rs").copied())
        .map(|c| c as f64)
        .collect();
    assert_eq!(
        series.len(),
        3,
        "lib.rs must appear in every sample's top-N"
    );
    assert!(
        series.windows(2).all(|w| w[1] >= w[0]),
        "complexity series must be monotonically increasing, got: {series:?}"
    );
    assert_eq!(compute_entity_trend(&series), EntityTrendDirection::Growing);
}
