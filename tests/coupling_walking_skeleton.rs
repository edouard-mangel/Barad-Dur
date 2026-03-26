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

use barad_dur::coupling::discovery::{discover_repos, DiscoveredRepo, SkipReason};
use barad_dur::coupling::collector::collect_snapshots;
use barad_dur::coupling::CouplingConfig;

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

// ---------------------------------------------------------------------------
// Helper: create a temp git repo with a file and one commit
// ---------------------------------------------------------------------------
fn create_temp_repo(parent: &std::path::Path, name: &str) -> std::path::PathBuf {
    let repo_path = parent.join(name);
    std::fs::create_dir(&repo_path).unwrap();
    let repo = git2::Repository::init(&repo_path).unwrap();
    let sig = git2::Signature::now("Alice", "alice@example.com").unwrap();

    // Create a file so there's something in the tree
    let file_path = repo_path.join("README.md");
    std::fs::write(&file_path, format!("# {name}\n")).unwrap();

    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("README.md")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();

    repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
        .unwrap();

    repo_path
}

// ---------------------------------------------------------------------------
// AC: collector_skips_blame_and_handles_failures
//
// GIVEN a list of discovered repos
// WHEN snapshots are collected for coupling analysis
// THEN each repo produces a RepoSnapshot with commits and authors populated
//   AND blame_map is empty (skip-blame optimization)
//   AND repos that fail collection appear in the skipped list
//     with a CollectionFailed reason rather than aborting the pipeline
// ---------------------------------------------------------------------------
#[test]
fn collector_skips_blame_and_handles_failures() {
    let root = TempDir::new().unwrap();

    // Create two valid repos
    let repo_a_path = create_temp_repo(root.path(), "repo-a");
    let repo_b_path = create_temp_repo(root.path(), "repo-b");

    // Create a "broken" repo entry pointing to a non-existent path
    let broken_path = root.path().join("broken-repo");
    std::fs::create_dir(&broken_path).unwrap();
    // Not a git repo — collection should fail gracefully

    let discovered = vec![
        DiscoveredRepo { name: "repo-a".to_string(), path: repo_a_path },
        DiscoveredRepo { name: "repo-b".to_string(), path: repo_b_path },
        DiscoveredRepo { name: "broken-repo".to_string(), path: broken_path.clone() },
    ];

    let config = CouplingConfig::default();
    let result = collect_snapshots(&discovered, &config);

    // Two repos should have been collected successfully
    assert_eq!(
        result.snapshots.len(), 2,
        "expected 2 successful snapshots, got {}",
        result.snapshots.len()
    );

    // Each snapshot should have commits and authors populated
    for (name, snapshot) in &result.snapshots {
        assert!(
            !snapshot.commits.is_empty(),
            "snapshot for '{}' should have commits",
            name
        );
        assert!(
            !snapshot.authors.is_empty(),
            "snapshot for '{}' should have authors",
            name
        );
        // blame_map MUST be empty (skip-blame optimization)
        assert!(
            snapshot.blame_map.is_empty(),
            "snapshot for '{}' should have empty blame_map (skip-blame)",
            name
        );
    }

    // The broken repo should appear in the failed list
    assert_eq!(
        result.failed.len(), 1,
        "expected 1 failed repo, got {}",
        result.failed.len()
    );
    assert_eq!(result.failed[0].path, broken_path);
    assert!(
        matches!(result.failed[0].reason, SkipReason::Other(_)),
        "broken repo should have CollectionFailed-style reason, got {:?}",
        result.failed[0].reason
    );
}
