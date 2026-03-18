/// Walking Skeleton tests for the historical-trends feature.
///
/// These tests form the outer loop of Outside-In TDD. They define observable user outcomes
/// for US-01 (Auto-record Trend Snapshot) and US-02 (Inline Delta Display).
///
/// All tests are marked #[ignore] — enable one at a time during implementation.
/// First test to enable: `first_run_creates_trend_store` (US-01 walking skeleton).
///
/// Driving port: `barad-dur analyze <path>` binary invoked via assert_cmd.
/// No internal components are tested directly.
use assert_cmd::Command;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn barad_dur() -> Command {
    #[allow(deprecated)]
    Command::cargo_bin("barad-dur").unwrap()
}

/// Build a minimal git repository in `dir` with one commit on `branch`.
/// Returns the HEAD commit SHA (40 hex chars).
fn init_git_repo(dir: &Path, branch: &str) -> String {
    // Configure git identity (required for commits)
    std::process::Command::new("git")
        .args(["init", "-b", branch])
        .current_dir(dir)
        .output()
        .expect("git init failed");

    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir)
        .output()
        .expect("git config email failed");

    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir)
        .output()
        .expect("git config name failed");

    // Create a file and commit it
    fs::write(dir.join("README.md"), "# Test repo\n").expect("write README failed");

    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir)
        .output()
        .expect("git add failed");

    std::process::Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(dir)
        .output()
        .expect("git commit failed");

    // Return HEAD SHA
    let out = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .expect("git rev-parse failed");

    String::from_utf8(out.stdout)
        .expect("utf8")
        .trim()
        .to_string()
}

fn read_trends_json(repo_dir: &Path) -> serde_json::Value {
    let path = repo_dir.join(".repository-analysis").join("trends.json");
    let content = fs::read_to_string(&path).expect("trends.json should exist");
    // trends.json is NDJSON (one JSON object per line) — parse all lines into an array
    let entries: Vec<serde_json::Value> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("each line should be valid JSON"))
        .collect();
    serde_json::Value::Array(entries)
}

// ---------------------------------------------------------------------------
// US-01 Walking Skeleton: First run creates trend store
//
// Given no `.repository-analysis/trends.json` file exists
// When the user runs `barad-dur analyze .` and the analysis succeeds
// Then trends.json is created with exactly 1 entry
// And the entry contains: UTC timestamp, HEAD commit SHA, branch name,
//     overall_score, and all 4 category scores
// And the CLI output contains "Trend: first snapshot recorded"
// And the command exits with code 0
// ---------------------------------------------------------------------------
#[test]
fn first_run_creates_trend_store() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    let head_sha = init_git_repo(repo_path, "main");

    // Given: no trends.json exists
    let trends_path = repo_path.join(".repository-analysis").join("trends.json");
    assert!(!trends_path.exists(), "trends.json should not exist before first run");

    // When: user runs analyze
    let output = barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();

    // Then: trends.json created with 1 entry
    let entries = read_trends_json(repo_path);
    assert_eq!(
        entries.as_array().unwrap().len(),
        1,
        "trends.json should have exactly 1 entry after first run"
    );

    let entry = &entries[0];

    // And: entry contains required fields
    let timestamp = entry["timestamp"].as_str().expect("timestamp should be a string");
    assert!(
        timestamp.contains('T') && (timestamp.ends_with('Z') || timestamp.contains('+')),
        "timestamp should be ISO8601 UTC: {timestamp}"
    );

    let recorded_sha = entry["commit"].as_str().expect("commit should be a string");
    assert_eq!(
        recorded_sha, head_sha,
        "recorded commit SHA should match HEAD"
    );

    assert_eq!(
        entry["branch"].as_str().expect("branch should be a string"),
        "main",
        "branch should be 'main'"
    );

    assert!(
        entry["overall_score"].is_number(),
        "overall_score should be a number"
    );

    let cat_scores = entry["category_scores"]
        .as_object()
        .expect("category_scores should be an object");
    for key in &["Health", "Team", "Evolution", "Git Hygiene"] {
        assert!(
            cat_scores.contains_key(*key),
            "category_scores should contain key '{key}'"
        );
    }

    // And: CLI output mentions first snapshot
    assert!(
        stdout.contains("Trend: first snapshot recorded"),
        "stdout should contain 'Trend: first snapshot recorded'\nActual stdout:\n{stdout}"
    );
}

// ---------------------------------------------------------------------------
// US-01 Walking Skeleton: Second run appends to trend store
//
// Given trends.json exists with 1 entry
// When the user runs `barad-dur analyze .` again
// Then trends.json contains exactly 2 entries
// And the entries are ordered by timestamp ascending
// ---------------------------------------------------------------------------
#[test]
fn second_run_appends_to_trend_store() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo(repo_path, "main");

    // Given: first run has already created trends.json with 1 entry
    barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success();

    let entries_after_first = read_trends_json(repo_path);
    assert_eq!(entries_after_first.as_array().unwrap().len(), 1);

    // When: user runs analyze a second time
    barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success();

    // Then: trends.json contains exactly 2 entries
    let entries = read_trends_json(repo_path);
    let arr = entries.as_array().unwrap();
    assert_eq!(arr.len(), 2, "trends.json should have exactly 2 entries after second run");

    // And: entries are ordered by timestamp ascending
    let ts0 = arr[0]["timestamp"].as_str().expect("timestamp[0]");
    let ts1 = arr[1]["timestamp"].as_str().expect("timestamp[1]");
    assert!(
        ts0 <= ts1,
        "entries should be ordered by timestamp ascending: {ts0} <= {ts1}"
    );
}

// ---------------------------------------------------------------------------
// US-02 Walking Skeleton: Positive delta shown inline after two runs
//
// This is the simplest complete user journey for US-02:
// a user who has run the tool before sees their progress inline.
//
// Given the user has run barad-dur analyze at least once before (trend history exists)
// When the user runs barad-dur analyze again on the same branch
// Then the CLI output shows a delta vs the prior run
// And the output includes a direction indicator
// And the command exits with code 0
//
// Note: We cannot control the score values in a real repo, so we verify the
// *structure* of the delta output (delta marker present), not its exact value.
// ---------------------------------------------------------------------------
#[test]
#[ignore]
fn delta_displayed_inline_after_prior_run_on_same_branch() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo(repo_path, "main");

    // Given: at least one prior trend snapshot exists (first run)
    barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success();

    // When: user runs analyze again on the same branch
    let output = barad_dur()
        .args(["analyze", repo_path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();

    // Then: output contains a delta vs last run ("+N vs last run" or "-N vs last run")
    let has_delta = stdout.contains("vs last run");
    assert!(
        has_delta,
        "stdout should contain 'vs last run' delta indicator\nActual stdout:\n{stdout}"
    );

    // And: output contains a direction indicator
    let has_direction = stdout.contains("improving")
        || stdout.contains("declining")
        || stdout.contains("stable");
    assert!(
        has_direction,
        "stdout should contain a direction indicator (improving/declining/stable)\nActual stdout:\n{stdout}"
    );
}
