/// Walking Skeleton tests for the multi-repository-analysis feature.
///
/// These tests form the outer loop of Outside-In TDD. They verify
/// the coupling subcommand is recognized by the CLI and parses
/// arguments correctly.
///
/// Driving port: `barad-dur coupling <root-dir>` binary invoked via assert_cmd.
/// No internal Rust components are called directly.
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

use barad_dur::coupling::discovery::{discover_repos, SkipReason};

// ---------------------------------------------------------------------------
// AC: coupling_cli_parses_subcommand
//
// GIVEN a user runs `barad-dur coupling <root-dir>`
// WHEN the root-dir argument is provided
// THEN the CLI parses the coupling subcommand without error
//   AND the command exits with code 0
// ---------------------------------------------------------------------------
#[test]
fn coupling_cli_parses_subcommand() {
    let temp = TempDir::new().unwrap();

    Command::cargo_bin("barad-dur")
        .unwrap()
        .arg("coupling")
        .arg(temp.path())
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// AC: coupling_cli_requires_root_dir
//
// GIVEN a user runs `barad-dur coupling` without arguments
// THEN an error message indicates the root-dir is required
// ---------------------------------------------------------------------------
#[test]
fn coupling_cli_requires_root_dir() {
    Command::cargo_bin("barad-dur")
        .unwrap()
        .arg("coupling")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

// ---------------------------------------------------------------------------
// AC: existing_subcommands_unchanged
//
// GIVEN existing subcommands (analyze, backfill, init, gate)
// WHEN any is invoked
// THEN behavior is unchanged (help text still lists them)
// ---------------------------------------------------------------------------
#[test]
fn existing_subcommands_unchanged() {
    // All existing subcommands still appear in help
    Command::cargo_bin("barad-dur")
        .unwrap()
        .arg("--help")
        .assert()
        .stdout(predicate::str::contains("analyze"))
        .stdout(predicate::str::contains("backfill"))
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("gate"))
        .stdout(predicate::str::contains("coupling"));
}

// ---------------------------------------------------------------------------
// AC: discovery_finds_valid_repos
//
// GIVEN a root directory containing subdirectories
// WHEN discover_repos is called
// THEN directories with a valid .git folder and at least one commit are
//   returned as discovered repos
// AND directories without .git or with no commits are returned as skipped
//   with appropriate reasons (NotAGitRepo, NoCommits)
// GIVEN a root directory with no subdirectories
// THEN the discovered list is empty and no error occurs
// ---------------------------------------------------------------------------
#[test]
fn discovery_finds_valid_repos() {
    let root = TempDir::new().unwrap();

    // repo_a: valid git repo with one commit
    let repo_a = root.path().join("repo-a");
    std::fs::create_dir(&repo_a).unwrap();
    git2::Repository::init(&repo_a).unwrap();
    // Create an initial commit so repo-a has at least one commit
    {
        let repo = git2::Repository::open(&repo_a).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let tree_id = repo.index().unwrap().write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();
    }

    // repo_b: git repo with NO commits
    let repo_b = root.path().join("repo-b");
    std::fs::create_dir(&repo_b).unwrap();
    git2::Repository::init(&repo_b).unwrap();

    // not_a_repo: plain directory, no .git
    let not_a_repo = root.path().join("not-a-repo");
    std::fs::create_dir(&not_a_repo).unwrap();

    let result = discover_repos(root.path());

    // repo-a should be discovered
    assert_eq!(result.discovered.len(), 1);
    assert_eq!(result.discovered[0].name, "repo-a");
    assert_eq!(result.discovered[0].path, repo_a);

    // repo-b (no commits) and not-a-repo (no .git) should be skipped
    assert_eq!(result.skipped.len(), 2);

    let skipped_names: Vec<_> = result
        .skipped
        .iter()
        .map(|s| s.path.file_name().unwrap().to_str().unwrap().to_string())
        .collect();
    assert!(skipped_names.contains(&"repo-b".to_string()));
    assert!(skipped_names.contains(&"not-a-repo".to_string()));

    // Check skip reasons
    let repo_b_skip = result.skipped.iter().find(|s| {
        s.path.file_name().unwrap().to_str().unwrap() == "repo-b"
    }).unwrap();
    assert!(matches!(repo_b_skip.reason, SkipReason::NoCommits));

    let not_a_repo_skip = result.skipped.iter().find(|s| {
        s.path.file_name().unwrap().to_str().unwrap() == "not-a-repo"
    }).unwrap();
    assert!(matches!(not_a_repo_skip.reason, SkipReason::NotAGitRepo));
}

#[test]
fn discovery_returns_empty_for_no_subdirectories() {
    let root = TempDir::new().unwrap();
    let result = discover_repos(root.path());
    assert!(result.discovered.is_empty());
    assert!(result.skipped.is_empty());
}
