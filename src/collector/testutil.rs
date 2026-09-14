//! Throwaway git repositories for collector tests.
//!
//! Every `git` call here runs with the developer's global and system
//! configuration masked and an explicit identity, so a `commit.gpgsign`,
//! a `core.hooksPath` or a missing `user.email` on the machine running the
//! suite cannot turn a fixture into a failure.
use std::path::Path;

use crate::snapshot::RepoSnapshot;

/// Run one git command in `dir`, isolated from the host's configuration.
pub(crate) fn git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@e")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@e")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

/// The SHA `HEAD` points at in `dir`.
pub(crate) fn head(dir: &Path) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// Write every `(path, bytes)` of `tree` under `dir`, creating directories.
pub(crate) fn write_tree(dir: &Path, tree: &[(&str, &[u8])]) {
    for (name, bytes) in tree {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
}

/// One commit on `main` holding `tree`; returns the directory and the SHA.
pub(crate) fn repository(tree: &[(&str, &[u8])]) -> (tempfile::TempDir, String) {
    let dir = tempfile::TempDir::new().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    write_tree(dir.path(), tree);
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "init"]);
    let sha = head(dir.path());
    (dir, sha)
}

/// `repository` for text-only trees.
pub(crate) fn repository_of_text(files: &[(&str, &str)]) -> (tempfile::TempDir, String) {
    let tree: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(name, text)| (*name, text.as_bytes()))
        .collect();
    repository(&tree)
}

/// The snapshot's listed paths, as strings, in emission order.
pub(crate) fn snapshot_paths(snapshot: &RepoSnapshot) -> Vec<String> {
    snapshot
        .files
        .iter()
        .map(|f| f.path.to_string_lossy().into_owned())
        .collect()
}
