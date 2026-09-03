//! Makes a corpus entry's repository exist under the corpus root, cloning
//! it when the manifest carries a URL. Shell layer: runs `git`.

use crate::field_test::corpus::{resolve_path, CorpusEntry};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Transports the harness will clone from. `ext::` and `fd::` run arbitrary
/// commands and a bare path or `-`-prefixed value can smuggle a git flag, so
/// the manifest value is checked before it reaches `git`.
const ALLOWED_URL_PREFIXES: &[&str] = &["https://", "ssh://", "git@", "file://"];

fn is_allowed_url(url: &str) -> bool {
    ALLOWED_URL_PREFIXES
        .iter()
        .any(|prefix| url.starts_with(prefix))
}

fn git_stdout(repo: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .with_context(|| format!("running git {} in {}", args.join(" "), repo.display()))?;
    if !output.status.success() {
        bail!(
            "{} is not a valid Git repository: {}",
            repo.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn validate_repository(entry: &CorpusEntry, repo: &Path) -> Result<()> {
    let inside_work_tree = git_stdout(repo, &["rev-parse", "--is-inside-work-tree"])?;
    if inside_work_tree != "true" {
        bail!(
            "{} is not a Git repository with a working tree",
            repo.display()
        );
    }
    if let Some(expected) = entry.url.as_deref() {
        let actual = git_stdout(repo, &["remote", "get-url", "origin"])?;
        if actual != expected {
            bail!(
                "cached corpus repository {} has origin `{actual}`, expected `{expected}`",
                entry.name
            );
        }
    }
    Ok(())
}

/// Path of `entry`'s repository under `root`, cloning it first when absent.
///
/// A full clone, never shallow: blame ages and first-commit dates feed the
/// decision surface, and a truncated history would silently move baselines.
/// Absent with no URL is a harness error, not a skip — a corpus entry that
/// quietly drops out would let a regression on it pass unseen.
pub fn ensure_present(entry: &CorpusEntry, root: &Path) -> Result<PathBuf> {
    let repo = resolve_path(entry, root);
    if repo.is_dir() {
        validate_repository(entry, &repo)?;
        return Ok(repo);
    }
    let Some(url) = entry.url.as_deref() else {
        bail!(
            "corpus repository {} is missing at {} and has no url in corpus.toml; \
             clone it there by hand or run with BARAD_DUR_CORPUS_SCOPE=public",
            entry.name,
            repo.display()
        );
    };
    if !is_allowed_url(url) {
        bail!(
            "corpus url for {} must start with one of {} (got `{url}`)",
            entry.name,
            ALLOWED_URL_PREFIXES.join(", ")
        );
    }
    std::fs::create_dir_all(root)
        .with_context(|| format!("creating corpus root {}", root.display()))?;
    let staging = tempfile::Builder::new()
        .prefix(&format!(".{}-clone-", entry.name))
        .tempdir_in(root)
        .with_context(|| format!("creating clone staging directory in {}", root.display()))?;
    let staged_repo = staging.path().join("repository");
    let output = Command::new("git")
        .arg("clone")
        .arg("--quiet")
        .arg("--")
        .arg(url)
        .arg(&staged_repo)
        .output()
        .with_context(|| format!("running git clone for {}", entry.name))?;
    if !output.status.success() {
        bail!(
            "git clone {url} failed for {}: {}",
            entry.name,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    validate_repository(entry, &staged_repo)?;
    std::fs::rename(&staged_repo, &repo).with_context(|| {
        format!(
            "moving cloned corpus repository {} into {}",
            entry.name,
            repo.display()
        )
    })?;
    Ok(repo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field_test::corpus::CorpusEntry;
    use std::process::Command;

    fn entry(name: &str, url: Option<&str>) -> CorpusEntry {
        CorpusEntry {
            name: name.into(),
            path: name.into(),
            pin: "0000000".into(),
            lang: "Rust".into(),
            url: url.map(str::to_string),
        }
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?} failed");
    }

    /// A tiny repository with one commit, usable as a clone URL.
    fn seed_repo(dir: &Path) {
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["config", "user.email", "t@example.com"]);
        git(dir, &["config", "user.name", "t"]);
        std::fs::write(dir.join("a.rs"), "fn main() {}\n").unwrap();
        git(dir, &["add", "a.rs"]);
        git(dir, &["commit", "-q", "-m", "seed"]);
    }

    #[test]
    fn a_repository_already_under_the_root_is_used_as_is() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("local-only")).unwrap();
        seed_repo(&root.path().join("local-only"));
        let got = ensure_present(&entry("local-only", None), root.path()).unwrap();
        assert_eq!(got, root.path().join("local-only"));
    }

    #[test]
    fn an_existing_non_repository_is_rejected_as_a_stale_cache_entry() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("incomplete")).unwrap();

        let err = ensure_present(&entry("incomplete", None), root.path()).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("incomplete"), "names the entry: {msg}");
        assert!(
            msg.contains("Git repository"),
            "explains the problem: {msg}"
        );
    }

    #[test]
    fn a_cached_repository_from_the_wrong_origin_is_rejected() {
        let expected = tempfile::tempdir().unwrap();
        seed_repo(expected.path());
        let other = tempfile::tempdir().unwrap();
        seed_repo(other.path());
        let root = tempfile::tempdir().unwrap();
        let cached = root.path().join("cached");
        let status = Command::new("git")
            .args(["clone", "--quiet", "--"])
            .arg(other.path())
            .arg(&cached)
            .status()
            .unwrap();
        assert!(status.success());
        let expected_url = format!("file://{}", expected.path().display());

        let err = ensure_present(&entry("cached", Some(&expected_url)), root.path()).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("origin"), "explains the mismatch: {msg}");
        assert!(
            msg.contains(&expected_url),
            "shows the expected origin: {msg}"
        );
    }

    #[test]
    fn a_missing_repository_with_a_url_is_cloned_under_the_root() {
        let upstream = tempfile::tempdir().unwrap();
        seed_repo(upstream.path());
        let root = tempfile::tempdir().unwrap();
        let url = format!("file://{}", upstream.path().display());
        let got = ensure_present(&entry("cloned", Some(&url)), root.path()).unwrap();
        assert_eq!(got, root.path().join("cloned"));
        assert!(got.join("a.rs").is_file(), "clone has the working tree");
        assert!(
            got.join(".git").exists(),
            "clone has full history, not an export"
        );
    }

    #[test]
    fn only_https_and_ssh_urls_are_cloned() {
        // `ext::` and friends run commands; a leading `-` smuggles a git flag.
        let root = tempfile::tempdir().unwrap();
        for bad in [
            "ext::sh -c id",
            "--upload-pack=id",
            "/tmp/repo",
            "git://host/repo",
        ] {
            let err = ensure_present(&entry("x", Some(bad)), root.path()).unwrap_err();
            assert!(format!("{err:#}").contains("url"), "{bad}: {err:#}");
            assert!(!root.path().join("x").exists(), "{bad} must not clone");
        }
    }

    #[test]
    fn a_missing_repository_without_a_url_is_a_harness_error() {
        let root = tempfile::tempdir().unwrap();
        let err = ensure_present(&entry("private-app", None), root.path()).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("private-app"), "names the entry: {msg}");
        assert!(msg.contains("url"), "explains the remedy: {msg}");
    }
}
