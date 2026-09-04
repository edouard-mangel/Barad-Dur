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

/// Whether `repo`'s object database holds `commit`.
///
/// `cat-file -e` is a plain existence probe: it exits non-zero for an
/// unknown object rather than writing to stdout, so a missing pin is an
/// answer here, not a harness error.
fn contains_commit(repo: &Path, commit: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["cat-file", "-e", &format!("{commit}^{{commit}}")])
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Whether the checkout at `repo` is the repository `entry` declares.
///
/// Identity is the pin, not the remote URL. The same upstream is reached
/// over https by the harness, over ssh by a maintainer, and through a local
/// mirror on a machine that clones from one — all spellings of `origin` that
/// a string comparison would reject while the history is identical. A
/// commit ID, by contrast, is shared history's own name for itself: an
/// unrelated repository cannot hold the pin, and the pin is what the sweep
/// is about to check out.
fn validate_repository(entry: &CorpusEntry, repo: &Path) -> Result<()> {
    let inside_work_tree = git_stdout(repo, &["rev-parse", "--is-inside-work-tree"])?;
    if inside_work_tree != "true" {
        bail!(
            "{} is not a Git repository with a working tree",
            repo.display()
        );
    }
    if !contains_commit(repo, &entry.pin) {
        bail!(
            "corpus repository {} at {} does not contain its pin `{}`; \
             it is not the declared repository, or it needs a fetch",
            entry.name,
            repo.display(),
            entry.pin
        );
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

    /// HEAD of `dir`, for use as a corpus pin.
    fn head(dir: &Path) -> String {
        git_stdout(dir, &["rev-parse", "HEAD"]).expect("rev-parse HEAD")
    }

    /// A tiny repository with one commit, usable as a clone URL.
    fn seed_repo(dir: &Path) {
        seed_repo_with(dir, "fn main() {}\n");
    }

    /// As `seed_repo`, with the commit's content under the caller's control.
    ///
    /// A commit ID hashes content, author and timestamp alike, so two repos
    /// seeded with identical bytes in the same second share the very same
    /// commit object — indistinguishable histories, not merely similar ones.
    /// A test that means "a different repository" has to say so in content.
    fn seed_repo_with(dir: &Path, content: &str) {
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["config", "user.email", "t@example.com"]);
        git(dir, &["config", "user.name", "t"]);
        std::fs::write(dir.join("a.rs"), content).unwrap();
        git(dir, &["add", "a.rs"]);
        git(dir, &["commit", "-q", "-m", "seed"]);
    }

    #[test]
    fn a_repository_already_under_the_root_is_used_as_is() {
        let root = tempfile::tempdir().unwrap();
        let local = root.path().join("local-only");
        std::fs::create_dir(&local).unwrap();
        seed_repo(&local);
        let mut entry = entry("local-only", None);
        entry.pin = head(&local);

        let got = ensure_present(&entry, root.path()).unwrap();
        assert_eq!(got, local);
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

    /// A stale or simply wrong checkout sitting at the expected path is the
    /// failure this guard exists for: analysing it would silently report on
    /// the wrong code. Its history cannot hold the declared pin.
    #[test]
    fn a_cached_repository_that_is_a_different_repository_is_rejected() {
        let expected = tempfile::tempdir().unwrap();
        seed_repo(expected.path());
        let pin = head(expected.path());
        let other = tempfile::tempdir().unwrap();
        seed_repo_with(other.path(), "fn other() {}\n");
        let root = tempfile::tempdir().unwrap();
        let cached = root.path().join("cached");
        let status = Command::new("git")
            .args(["clone", "--quiet", "--"])
            .arg(other.path())
            .arg(&cached)
            .status()
            .unwrap();
        assert!(status.success());
        let mut entry = entry("cached", Some("https://example.invalid/org/cached.git"));
        entry.pin = pin.clone();

        let err = ensure_present(&entry, root.path()).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("cached"), "names the entry: {msg}");
        assert!(msg.contains(&pin), "shows the missing pin: {msg}");
    }

    /// The pin, not the spelling of the remote, is what says "this is the
    /// declared repository". A maintainer reaches the same upstream over ssh
    /// or from a local mirror; the harness clones it over https. All three
    /// share the history the pin names.
    #[test]
    fn a_cached_repository_reached_over_another_transport_is_accepted() {
        let upstream = tempfile::tempdir().unwrap();
        seed_repo(upstream.path());
        let root = tempfile::tempdir().unwrap();
        let cached = root.path().join("cached");
        // Cloned from a bare path — not the https url the manifest declares.
        let status = Command::new("git")
            .args(["clone", "--quiet", "--"])
            .arg(upstream.path())
            .arg(&cached)
            .status()
            .unwrap();
        assert!(status.success());
        let pin = git_stdout(&cached, &["rev-parse", "HEAD"]).unwrap();

        let mut entry = entry("cached", Some("https://example.invalid/org/cached.git"));
        entry.pin = pin;

        let got = ensure_present(&entry, root.path()).unwrap();
        assert_eq!(got, cached);
    }

    #[test]
    fn a_missing_repository_with_a_url_is_cloned_under_the_root() {
        let upstream = tempfile::tempdir().unwrap();
        seed_repo(upstream.path());
        let root = tempfile::tempdir().unwrap();
        let url = format!("file://{}", upstream.path().display());
        let mut entry = entry("cloned", Some(&url));
        entry.pin = head(upstream.path());

        let got = ensure_present(&entry, root.path()).unwrap();
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
