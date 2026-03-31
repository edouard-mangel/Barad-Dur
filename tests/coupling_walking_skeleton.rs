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

use barad_dur::coupling::collector::collect_snapshots;
use barad_dur::coupling::discovery::{discover_repos, DiscoveredRepo, SkipReason};
use barad_dur::coupling::temporal::{analyze_temporal_coupling, Confidence};
use barad_dur::coupling::CouplingConfig;
use barad_dur::renderer::coupling_cli::render_coupling_table;

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
    let repo_b_skip = result
        .skipped
        .iter()
        .find(|s| s.path.file_name().unwrap().to_str().unwrap() == "repo-b")
        .unwrap();
    assert!(matches!(repo_b_skip.reason, SkipReason::NoCommits));

    let not_a_repo_skip = result
        .skipped
        .iter()
        .find(|s| s.path.file_name().unwrap().to_str().unwrap() == "not-a-repo")
        .unwrap();
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
        DiscoveredRepo {
            name: "repo-a".to_string(),
            path: repo_a_path,
        },
        DiscoveredRepo {
            name: "repo-b".to_string(),
            path: repo_b_path,
        },
        DiscoveredRepo {
            name: "broken-repo".to_string(),
            path: broken_path.clone(),
        },
    ];

    let config = CouplingConfig::default();
    let result = collect_snapshots(&discovered, &config);

    // Two repos should have been collected successfully
    assert_eq!(
        result.snapshots.len(),
        2,
        "expected 2 successful snapshots, got {}",
        result.snapshots.len()
    );

    // Each snapshot should have commits and authors populated
    for (name, snapshot) in &result.snapshots {
        assert!(
            !snapshot.commit_timestamps.is_empty(),
            "snapshot for '{}' should have commits",
            name
        );
        assert!(
            !snapshot.author_names.is_empty(),
            "snapshot for '{}' should have authors",
            name
        );
    }

    // The broken repo should appear in the failed list
    assert_eq!(
        result.failed.len(),
        1,
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

// ---------------------------------------------------------------------------
// Helper: create a temp repo with multiple commits at specified timestamps
// ---------------------------------------------------------------------------
fn create_repo_with_commits(
    parent: &std::path::Path,
    name: &str,
    timestamps: &[i64], // Unix timestamps for each commit
) -> std::path::PathBuf {
    let repo_path = parent.join(name);
    std::fs::create_dir(&repo_path).unwrap();
    let repo = git2::Repository::init(&repo_path).unwrap();

    let file_path = repo_path.join("main.rs");
    let mut parent_commit: Option<git2::Oid> = None;

    for (i, &ts) in timestamps.iter().enumerate() {
        let content = format!("// version {i}\nfn main() {{}}\n");
        std::fs::write(&file_path, &content).unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("main.rs")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        let sig =
            git2::Signature::new("Alice", "alice@example.com", &git2::Time::new(ts, 0)).unwrap();

        let oid = if let Some(parent_oid) = parent_commit {
            let parent = repo.find_commit(parent_oid).unwrap();
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                &format!("commit {i}"),
                &tree,
                &[&parent],
            )
            .unwrap()
        } else {
            repo.commit(Some("HEAD"), &sig, &sig, &format!("commit {i}"), &tree, &[])
                .unwrap()
        };
        parent_commit = Some(oid);
    }

    repo_path
}

// ---------------------------------------------------------------------------
// AC: temporal_coupling_end_to_end
//
// GIVEN two repos where commits overlap within a 24h window at least 3 times
// WHEN temporal coupling is computed
// THEN the pair appears in the report with temporal_score =
//   (co_changes / min(commits_a, commits_b)) * 100
// AND confidence is HIGH for 30+ co-changes, MEDIUM for 10-29, LOW for 3-9
//
// GIVEN two repos with fewer than 3 co-changes within the window
// THEN the pair does not appear in the report
//
// GIVEN the CLI output format
// WHEN the report is rendered
// THEN pairs are displayed in a ranked table sorted by temporal score descending
// ---------------------------------------------------------------------------
#[test]
fn temporal_coupling_end_to_end() {
    let root = TempDir::new().unwrap();

    // Base timestamp: ~30 days ago (well within the default 180-day analysis window)
    let base_ts: i64 = chrono::Utc::now().timestamp() - 30 * 24 * 3600;
    let one_hour: i64 = 3600;
    let two_days: i64 = 2 * 24 * 3600;

    // repo-alpha: 5 commits, 4 of which overlap with repo-beta within 24h
    let alpha_times = vec![
        base_ts,                 // overlaps with beta[0]
        base_ts + two_days,      // overlaps with beta[1]
        base_ts + 2 * two_days,  // overlaps with beta[2]
        base_ts + 3 * two_days,  // overlaps with beta[3]
        base_ts + 10 * two_days, // no overlap with beta
    ];
    create_repo_with_commits(root.path(), "repo-alpha", &alpha_times);

    // repo-beta: 4 commits, all overlap with repo-alpha within 24h
    let beta_times = vec![
        base_ts + one_hour,                // within 24h of alpha[0]
        base_ts + two_days + one_hour,     // within 24h of alpha[1]
        base_ts + 2 * two_days + one_hour, // within 24h of alpha[2]
        base_ts + 3 * two_days + one_hour, // within 24h of alpha[3]
    ];
    create_repo_with_commits(root.path(), "repo-beta", &beta_times);

    // repo-gamma: 5 commits, only 2 overlap with alpha (below threshold of 3)
    let gamma_times = vec![
        base_ts + one_hour,            // overlaps with alpha[0]
        base_ts + two_days + one_hour, // overlaps with alpha[1]
        base_ts + 20 * two_days,       // no overlap
        base_ts + 21 * two_days,       // no overlap
        base_ts + 22 * two_days,       // no overlap
    ];
    create_repo_with_commits(root.path(), "repo-gamma", &gamma_times);

    // Step 1: Discover repos
    let discovery = discover_repos(root.path());
    assert_eq!(discovery.discovered.len(), 3, "should find 3 repos");

    // Step 2: Collect snapshots
    let config = CouplingConfig::default();
    let collection = collect_snapshots(&discovery.discovered, &config);
    assert_eq!(collection.snapshots.len(), 3, "should collect 3 snapshots");

    // Step 3: Analyze temporal coupling
    let window = std::time::Duration::from_secs(24 * 60 * 60);
    let pairs = analyze_temporal_coupling(&collection.snapshots, window);

    // alpha-beta should appear: 4 co-changes, min(5,4)=4, score = (4/4)*100 = 100.0
    let alpha_beta = pairs.iter().find(|p| {
        (p.repo_a == "repo-alpha" && p.repo_b == "repo-beta")
            || (p.repo_a == "repo-beta" && p.repo_b == "repo-alpha")
    });
    assert!(
        alpha_beta.is_some(),
        "alpha-beta pair should appear (4 co-changes >= 3)"
    );
    let ab = alpha_beta.unwrap();
    assert_eq!(ab.co_changes, 4);
    let expected_score = (4.0 / 4.0) * 100.0;
    assert!(
        (ab.temporal_score - expected_score).abs() < 0.01,
        "expected score {expected_score}, got {}",
        ab.temporal_score
    );
    assert_eq!(
        ab.confidence,
        Confidence::Low,
        "4 co-changes -> LOW confidence"
    );

    // alpha-gamma should NOT appear: only 2 co-changes < 3
    let alpha_gamma = pairs.iter().find(|p| {
        (p.repo_a == "repo-alpha" && p.repo_b == "repo-gamma")
            || (p.repo_a == "repo-gamma" && p.repo_b == "repo-alpha")
    });
    assert!(
        alpha_gamma.is_none(),
        "alpha-gamma pair should NOT appear (only 2 co-changes < 3)"
    );

    // beta-gamma: beta has 4 commits, gamma has 5. Overlaps at base_ts+1h and
    // base_ts+two_days+1h => 2 co-changes < 3, should NOT appear
    let beta_gamma = pairs.iter().find(|p| {
        (p.repo_a == "repo-beta" && p.repo_b == "repo-gamma")
            || (p.repo_a == "repo-gamma" && p.repo_b == "repo-beta")
    });
    assert!(
        beta_gamma.is_none(),
        "beta-gamma pair should NOT appear (< 3 co-changes)"
    );

    // Step 4: Verify CLI rendering
    let table = render_coupling_table(&pairs);
    assert!(
        table.contains("repo-alpha"),
        "CLI table should contain repo-alpha"
    );
    assert!(
        table.contains("repo-beta"),
        "CLI table should contain repo-beta"
    );
    assert!(
        !table.contains("repo-gamma"),
        "CLI table should NOT contain repo-gamma (no pairs)"
    );
    // Table should contain column headers
    assert!(
        table.contains("Score"),
        "CLI table should have Score column"
    );
    assert!(
        table.contains("Confidence"),
        "CLI table should have Confidence column"
    );
}
