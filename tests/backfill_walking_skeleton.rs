/// Walking Skeleton tests for the adaptive-trends-period feature.
///
/// These tests form the outer loop of Outside-In TDD. They define the minimum
/// observable user outcome for US-BF-01: a developer can run `barad-dur backfill`
/// on a repository and get a populated trends history in a single command.
///
/// All tests are marked #[ignore] — enable one at a time during implementation.
/// First test to enable: `backfill_large_repo_writes_ten_entries` (AC-BF-01 walking skeleton).
///
/// Driving port: `barad-dur backfill <path>` binary invoked via assert_cmd.
/// No internal Rust components are called directly.
use tempfile::TempDir;

mod common;
use common::{barad_dur, init_git_repo_with_commits, read_trends_entries};

// ---------------------------------------------------------------------------
// AC-BF-01 Walking Skeleton: Large repo → adaptive sampling writes 10 entries
//
// This is the minimum skeleton. It proves: binary compiles and is wired, the
// subcommand is recognised, adaptive sampling fires for repos > 10 commits, and
// the output file is written to the correct location.
//
// Given a repository with 15 commits and no existing trends.json
// When the developer runs `barad-dur backfill <path>`
// Then trends.json contains exactly 10 entries
// And the command exits with code 0
// And stdout contains "10 entries written"
// ---------------------------------------------------------------------------
#[test]
fn backfill_large_repo_writes_ten_entries() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo_with_commits(repo_path, "main", 15);

    let output = barad_dur()
        .args(["backfill", repo_path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();

    let entries = read_trends_entries(repo_path);
    assert_eq!(
        entries.len(),
        10,
        "backfill on 15-commit repo should write exactly 10 entries (adaptive sampling)\nActual entries: {}",
        entries.len()
    );

    assert!(
        stdout.contains("10 entries written"),
        "stdout should contain '10 entries written'\nActual stdout:\n{stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-BF-10 (partial): First backfill entry has source = "backfill"
//
// Given a repository with 15 commits and no existing trends.json
// When the developer runs `barad-dur backfill <path>`
// Then the first entry in trends.json has source = "backfill"
//
// This validates that ADR-006 (backfill provenance tagging) is wired end-to-end.
// ---------------------------------------------------------------------------
#[test]
#[ignore]
fn backfill_writes_source_backfill_field() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo_with_commits(repo_path, "main", 15);

    barad_dur()
        .args(["backfill", repo_path.to_str().unwrap()])
        .assert()
        .success();

    let entries = read_trends_entries(repo_path);
    assert!(
        !entries.is_empty(),
        "trends.json should contain entries after backfill"
    );

    let first = &entries[0];
    assert_eq!(
        first["source"].as_str().unwrap_or(""),
        "backfill",
        "first entry should have source = \"backfill\"\nEntry: {first}"
    );
}

// ---------------------------------------------------------------------------
// AC-BF-01: Backfill exits with code 0
//
// Given a repository with 15 commits
// When the developer runs `barad-dur backfill <path>`
// Then the command exits with code 0
//
// Explicit exit-code check isolated from the entry-count assertion above,
// so a CI failure pinpoints whether the issue is crash vs wrong output.
// ---------------------------------------------------------------------------
#[test]
fn backfill_exits_zero() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo_with_commits(repo_path, "main", 15);

    barad_dur()
        .args(["backfill", repo_path.to_str().unwrap()])
        .assert()
        .success(); // assert_cmd checks exit code 0
}
