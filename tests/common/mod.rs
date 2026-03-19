/// Shared test helpers for integration tests.
use assert_cmd::Command;
use std::fs;
use std::path::Path;

pub fn barad_dur() -> Command {
    #[allow(deprecated)]
    Command::cargo_bin("barad-dur").unwrap()
}

/// Build a minimal git repository in `dir` with one commit on `branch`.
/// Returns the HEAD commit SHA (40 hex chars).
pub fn init_git_repo(dir: &Path, branch: &str) -> String {
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

/// Build a git repository with `n` commits (each commit adds a file commit_N.txt).
/// Returns Vec<String> of commit SHAs (newest first).
#[allow(dead_code)]
pub fn init_git_repo_with_commits(dir: &Path, branch: &str, n: usize) -> Vec<String> {
    assert!(n >= 1, "n must be at least 1");

    // First commit via the existing helper
    let first_sha = init_git_repo(dir, branch);
    let mut shas = vec![first_sha];

    // Add n-1 more commits
    for i in 2..=n {
        let filename = format!("commit_{i}.txt");
        fs::write(dir.join(&filename), format!("commit {i}\n"))
            .expect("write commit file failed");

        std::process::Command::new("git")
            .args(["add", &filename])
            .current_dir(dir)
            .output()
            .expect("git add failed");

        std::process::Command::new("git")
            .args(["commit", "-m", &format!("Commit {i}")])
            .current_dir(dir)
            .output()
            .expect("git commit failed");

        let out = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir)
            .output()
            .expect("git rev-parse failed");

        let sha = String::from_utf8(out.stdout)
            .expect("utf8")
            .trim()
            .to_string();
        shas.push(sha);
    }

    // Return newest-first
    shas.reverse();
    shas
}

/// Read trends.json from a repo dir and return parsed entries as Vec<serde_json::Value>.
#[allow(dead_code)]
pub fn read_trends_entries(repo_dir: &Path) -> Vec<serde_json::Value> {
    let path = repo_dir.join(".repository-analysis").join("trends.json");
    let content = std::fs::read_to_string(&path).expect("trends.json should exist");
    content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("each line should be valid JSON"))
        .collect()
}
