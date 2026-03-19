/// Milestone 1 acceptance tests for the adaptive-trends-period feature.
///
/// Safety and correctness coverage for:
///   - US-BF-01: Adaptive sampling (AC-BF-02, AC-BF-04)
///   - US-BF-02: Non-destructive operation and schema validity (AC-BF-08, AC-BF-10, AC-BF-11)
///   - US-BF-03: Guard rails — no-op detection and error handling (AC-BF-05, AC-BF-06)
///
/// All tests are marked #[ignore]. Enable one at a time during implementation.
/// Prerequisite: backfill_walking_skeleton tests must pass before enabling these.
///
/// Driving port: `barad-dur backfill <path>` binary invoked via assert_cmd.
/// No internal Rust components are called directly.
use std::fs;
use tempfile::TempDir;

mod common;
use common::{barad_dur, init_git_repo_with_commits, read_trends_entries};

/// Write a pre-seeded trends.json with the given NDJSON content.
fn seed_trends(repo_dir: &std::path::Path, ndjson: &str) {
    let dir = repo_dir.join(".repository-analysis");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("trends.json"), ndjson).unwrap();
}

// ---------------------------------------------------------------------------
// AC-BF-02: Small repo (5 commits) → all 5 entries written
//
// Given a repository with 5 commits and no existing trends.json
// When the developer runs `barad-dur backfill <path>`
// Then trends.json contains exactly 5 entries (no sampling for small repos)
// And the command exits with code 0
// ---------------------------------------------------------------------------
#[test]
#[ignore]
fn backfill_small_repo_writes_all() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo_with_commits(repo_path, "main", 5);

    barad_dur()
        .args(["backfill", repo_path.to_str().unwrap()])
        .assert()
        .success();

    let entries = read_trends_entries(repo_path);
    assert_eq!(
        entries.len(),
        5,
        "backfill on 5-commit repo should write all 5 entries\nActual: {}",
        entries.len()
    );
}

// ---------------------------------------------------------------------------
// AC-BF-04: Deduplication — pre-existing 3 entries + 15-commit repo → 10 total, 7 new
//
// Given a repository with 15 commits
// And trends.json already contains 3 entries whose SHAs match 3 of the sampled commits
// When the developer runs `barad-dur backfill <path>`
// Then trends.json contains exactly 10 entries
// And the original 3 lines are unchanged (byte-level identity)
// ---------------------------------------------------------------------------
#[test]
fn backfill_deduplication() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo_with_commits(repo_path, "main", 15);

    // Run backfill once to discover which SHAs are actually sampled
    barad_dur()
        .args(["backfill", repo_path.to_str().unwrap()])
        .assert()
        .success();

    let first_run_entries = read_trends_entries(repo_path);
    assert_eq!(first_run_entries.len(), 10, "first run should produce 10 entries");

    // Retain only the 3 oldest entries (last 3) as the pre-existing seed.
    // These SHAs are guaranteed to be in the sampled set (they came from backfill).
    let pre_existing: Vec<_> = first_run_entries[first_run_entries.len() - 3..].to_vec();
    let pre_existing_ndjson: String = pre_existing
        .iter()
        .map(|e| {
            let sha = e["head"].as_str().unwrap();
            format!(
                "{}\n",
                r#"{"timestamp":"2024-01-01T00:00:00Z","head":""#.to_string()
                    + sha
                    + r#"","branch":"main","overall_score":50,"schema_version":1,"source":"backfill","categories":{}}"#
            )
        })
        .collect();

    // Reset trends.json to only those 3 pre-existing entries
    seed_trends(repo_path, &pre_existing_ndjson);

    // Second run: should skip the 3 pre-existing SHAs and write 7 new ones → 10 total
    barad_dur()
        .args(["backfill", repo_path.to_str().unwrap()])
        .assert()
        .success();

    let entries = read_trends_entries(repo_path);
    assert_eq!(
        entries.len(),
        10,
        "deduplication: 3 pre-existing + 7 new should give 10 total\nActual: {}",
        entries.len()
    );

    // Verify the original lines are preserved (seeded SHAs still present)
    let trends_path = repo_path.join(".repository-analysis").join("trends.json");
    let content = fs::read_to_string(&trends_path).unwrap();
    for entry in &pre_existing {
        let sha = entry["head"].as_str().unwrap();
        assert!(
            content.contains(sha),
            "existing SHA {sha} should still be present after deduplication"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-BF-05: No-op guard — second backfill on same repo prints "Backfill already complete"
//
// Given a repository with 15 commits
// And backfill has already been run (trends.json has 10 entries covering all samples)
// When the developer runs `barad-dur backfill <path>` again
// Then the command exits with code 0
// And stdout contains "Backfill already complete"
// And no new entries are written (still exactly 10)
// ---------------------------------------------------------------------------
#[test]
fn backfill_no_op_guard() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo_with_commits(repo_path, "main", 15);

    // First run: fills the 10 samples
    barad_dur()
        .args(["backfill", repo_path.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(read_trends_entries(repo_path).len(), 10);

    // Second run: all sampled SHAs already present
    let output = barad_dur()
        .args(["backfill", repo_path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();

    assert!(
        stdout.contains("Backfill already complete"),
        "stdout should contain 'Backfill already complete'\nActual:\n{stdout}"
    );

    assert_eq!(
        read_trends_entries(repo_path).len(),
        10,
        "no new entries should be written on a no-op backfill"
    );
}

// ---------------------------------------------------------------------------
// AC-BF-06: Empty repository (git init, no commits) → non-zero exit + "No commits" message
//
// Given a directory with only `git init` and no commits
// When the developer runs `barad-dur backfill <path>`
// Then the command exits with a non-zero code
// And the output (stdout or stderr) contains "No commits"
// ---------------------------------------------------------------------------
#[test]
fn backfill_zero_commit_repo_exits_nonzero() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();

    // git init only — no commits
    std::process::Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_path)
        .output()
        .expect("git init failed");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let output = barad_dur()
        .args(["backfill", repo_path.to_str().unwrap()])
        .assert()
        .failure() // non-zero exit
        .get_output()
        .clone();

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        combined.contains("No commits"),
        "output should contain 'No commits' for an empty repository\nActual:\n{combined}"
    );
}

// ---------------------------------------------------------------------------
// AC-BF-08: Non-destructive — uncommitted file unchanged after backfill
//
// Given a repository with 15 commits and an uncommitted file in the working tree
// When the developer runs `barad-dur backfill <path>`
// Then the uncommitted file still exists with its original content
// And the git index is unmodified (git status shows only the original state)
// ---------------------------------------------------------------------------
#[test]
fn backfill_non_destructive() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo_with_commits(repo_path, "main", 15);

    // Create an uncommitted file
    let dirty_file = repo_path.join("uncommitted_work.txt");
    let original_content = "work in progress — do not touch";
    fs::write(&dirty_file, original_content).unwrap();

    barad_dur()
        .args(["backfill", repo_path.to_str().unwrap()])
        .assert()
        .success();

    // File still exists with original content
    let after_content = fs::read_to_string(&dirty_file)
        .expect("uncommitted_work.txt should still exist after backfill");
    assert_eq!(
        after_content, original_content,
        "uncommitted file should be unchanged after backfill"
    );

    // Git index unmodified: git status should show uncommitted_work.txt as untracked only
    let status = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_path)
        .output()
        .expect("git status failed");
    let status_out = String::from_utf8(status.stdout).unwrap();

    assert!(
        status_out.contains("uncommitted_work.txt"),
        "git status should still show the uncommitted file as untracked"
    );
    // No staged changes (no lines starting with 'A', 'M', 'D' in index column)
    let has_staged = status_out
        .lines()
        .any(|l| l.len() >= 2 && l.starts_with(['A', 'M', 'D']));
    assert!(
        !has_staged,
        "backfill must not stage any changes\ngit status:\n{status_out}"
    );
}

// ---------------------------------------------------------------------------
// AC-BF-10: All entries have schema_version = 1
//
// Given a repository with 15 commits and no existing trends.json
// When the developer runs `barad-dur backfill <path>`
// Then every entry in trends.json has schema_version = 1
// ---------------------------------------------------------------------------
#[test]
fn backfill_schema_version_is_1() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo_with_commits(repo_path, "main", 15);

    barad_dur()
        .args(["backfill", repo_path.to_str().unwrap()])
        .assert()
        .success();

    let entries = read_trends_entries(repo_path);
    assert!(!entries.is_empty(), "trends.json should have entries");

    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(
            entry["schema_version"].as_i64().unwrap_or(-1),
            1,
            "entry[{i}].schema_version should be 1\nEntry: {entry}"
        );
        // Also validate non-empty required fields per AC-BF-10
        let head = entry["head"].as_str().unwrap_or("");
        assert_eq!(
            head.len(),
            40,
            "entry[{i}].head should be a 40-char SHA, got: '{head}'"
        );
        assert!(
            !entry["branch"].as_str().unwrap_or("").is_empty(),
            "entry[{i}].branch should be non-empty"
        );
        assert!(
            !entry["timestamp"].as_str().unwrap_or("").is_empty(),
            "entry[{i}].timestamp should be non-empty"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-BF-11: All entries record the current HEAD branch name
//
// Given a repository with 15 commits on branch "main"
// When the developer runs `barad-dur backfill <path>`
// Then every entry in trends.json has branch = "main"
// ---------------------------------------------------------------------------
#[test]
fn backfill_branch_is_current_branch() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();
    init_git_repo_with_commits(repo_path, "main", 15);

    barad_dur()
        .args(["backfill", repo_path.to_str().unwrap()])
        .assert()
        .success();

    let entries = read_trends_entries(repo_path);
    assert!(!entries.is_empty(), "trends.json should have entries");

    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(
            entry["branch"].as_str().unwrap_or(""),
            "main",
            "entry[{i}].branch should be 'main'\nEntry: {entry}"
        );
    }
}
