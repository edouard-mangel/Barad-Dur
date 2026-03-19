/// Milestone 2 acceptance tests for the adaptive-trends-period feature.
///
/// Developer experience coverage for:
///   - US-BF-02: Progress feedback and fast mode (AC-BF-07, AC-BF-03)
///
/// All tests are marked #[ignore]. Enable one at a time during implementation.
/// Prerequisite: backfill_milestone_1 tests must pass before enabling these.
///
/// Driving port: `barad-dur backfill <path>` binary invoked via assert_cmd.
/// No internal Rust components are called directly.
use tempfile::TempDir;

mod common;
use common::{barad_dur, init_git_repo_with_commits, read_trends_entries};

// ---------------------------------------------------------------------------
// AC-BF-07: Progress output — stdout contains "[1/" pattern during run
//
// Given a repository with 15 commits and no existing trends.json
// When the developer runs `barad-dur backfill <path>`
// Then stdout contains a progress marker matching "[1/" (first entry)
// And stdout contains a progress marker matching "[10/10]" or "[N/N]" (last entry)
// ---------------------------------------------------------------------------
#[test]
fn backfill_progress_output() {
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

    assert!(
        stdout.contains("[1/"),
        "stdout should contain '[1/' progress marker (first entry)\nActual:\n{stdout}"
    );

    // Last progress marker: [N/N] pattern where N matches total entries
    let entries = read_trends_entries(repo_path);
    let n = entries.len();
    let last_marker = format!("[{n}/{n}]");
    assert!(
        stdout.contains(&last_marker),
        "stdout should contain '{last_marker}' progress marker (last entry)\nActual:\n{stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-BF-03: --no-blame flag — entries written, exit 0
//
// Given a repository with 15 commits and no existing trends.json
// When the developer runs `barad-dur backfill <path> --no-blame`
// Then the command exits with code 0
// And trends.json contains entries (git blame is skipped but results are still valid)
// ---------------------------------------------------------------------------
#[test]
fn backfill_no_blame_flag_writes_entries() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo_with_commits(repo_path, "main", 15);

    barad_dur()
        .args(["backfill", repo_path.to_str().unwrap(), "--no-blame"])
        .assert()
        .success();

    let entries = read_trends_entries(repo_path);
    assert!(
        !entries.is_empty(),
        "backfill --no-blame should still write entries to trends.json"
    );
    assert_eq!(
        entries.len(),
        10,
        "--no-blame backfill on 15-commit repo should still produce 10 entries\nActual: {}",
        entries.len()
    );
}
