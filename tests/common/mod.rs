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
