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

/// Regression for the corruption-recovery dead end: `analyze` archives a
/// corrupt `entity_trends.json` and leaves an empty file in its place, so
/// the obvious next move is to re-run `backfill`. Backfill's per-sample skip
/// guard is keyed on the heads already in `trends.json`, and the entity
/// append sits inside that guard, so every sample was skipped and the entity
/// history stayed permanently empty.
#[test]
fn backfill_rebuilds_entity_history_when_only_that_file_was_reset() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path();

    git(path, &["init", "-q"]);
    git(path, &["config", "user.email", "t@e"]);
    git(path, &["config", "user.name", "t"]);

    for (n, body) in [
        (1, "fn f(x: i32) -> i32 { x }\n"),
        (2, "fn f(x: i32) -> i32 { if x > 0 { x } else { -x } }\n"),
    ] {
        std::fs::write(path.join("lib.rs"), body).unwrap();
        git(path, &["add", "-A"]);
        git_commit_at(
            path,
            &format!("commit {n}"),
            &format!("2024-01-0{n}T00:00:00"),
        );
    }

    let args = BackfillArgs {
        target: path.to_string_lossy().into_owned(),
        no_blame: false,
    };
    barad_dur::backfill::run(&args, path).expect("first backfill should succeed");
    let first = entity_history::load_entity_history(path).expect("load entity history");
    assert!(
        !first.is_empty(),
        "precondition: the first backfill must write entity history"
    );

    // Exactly what `load_entity_history_checked`'s recovery leaves behind:
    // the corrupt file archived to .bak, a fresh empty file in its place.
    // `trends.json` is deliberately left intact, as it would be in reality.
    std::fs::write(
        path.join(".repository-analysis").join("entity_trends.json"),
        "",
    )
    .unwrap();

    barad_dur::backfill::run(&args, path).expect("second backfill should succeed");

    let rebuilt = entity_history::load_entity_history(path).expect("reload entity history");
    assert_eq!(
        rebuilt.len(),
        first.len(),
        "backfill must rebuild entity history that was reset, even though every \
         sampled SHA is still present in trends.json"
    );
}

/// Regression for the "analyze first, backfill second" order — the order the
/// README implies and the one anyone tries. `analyze` appends HEAD to
/// trends.json on every run, and backfill's skip guard was keyed on that
/// file, so the newest sample never got entity data. That sample is
/// `series.last()`, i.e. half of every percent-change computation.
#[test]
fn backfill_records_entity_data_for_a_head_already_in_trends_json() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path();

    git(path, &["init", "-q"]);
    git(path, &["config", "user.email", "t@e"]);
    git(path, &["config", "user.name", "t"]);

    for (n, body) in [
        (1, "fn f(x: i32) -> i32 { x }\n"),
        (2, "fn f(x: i32) -> i32 { if x > 0 { x } else { -x } }\n"),
    ] {
        std::fs::write(path.join("lib.rs"), body).unwrap();
        git(path, &["add", "-A"]);
        git_commit_at(
            path,
            &format!("commit {n}"),
            &format!("2024-01-0{n}T00:00:00"),
        );
    }

    let head = String::from_utf8(
        std::process::Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();

    // Stand in for `analyze`: it records HEAD in trends.json on every run,
    // long before anyone thinks to run `backfill`.
    let args = BackfillArgs {
        target: path.to_string_lossy().into_owned(),
        no_blame: false,
    };
    barad_dur::backfill::run(&args, path).expect("seed run should succeed");
    std::fs::remove_file(path.join(".repository-analysis").join("entity_trends.json")).unwrap();

    barad_dur::backfill::run(&args, path).expect("backfill should succeed");

    let entries = entity_history::load_entity_history(path).expect("load entity history");
    assert!(
        entries.iter().any(|e| e.head == head),
        "the newest sample must get entity data even though trends.json already \
         holds its SHA; recorded heads were {:?}",
        entries.iter().map(|e| &e.head).collect::<Vec<_>>()
    );
}
