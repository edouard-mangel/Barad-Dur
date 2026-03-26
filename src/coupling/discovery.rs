use std::path::{Path, PathBuf};

/// A repository successfully discovered under the root directory.
#[derive(Debug, Clone)]
pub struct DiscoveredRepo {
    pub name: String,
    pub path: PathBuf,
}

/// Reason why a subdirectory was skipped during discovery.
#[derive(Debug, Clone)]
pub enum SkipReason {
    NotAGitRepo,
    NoCommits,
    PermissionDenied,
    Other(String),
}

/// A subdirectory that was skipped during discovery.
#[derive(Debug, Clone)]
pub struct SkippedRepo {
    pub path: PathBuf,
    pub reason: SkipReason,
}

/// Result of scanning a root directory for git repositories.
#[derive(Debug, Clone)]
pub struct DiscoveryResult {
    pub discovered: Vec<DiscoveredRepo>,
    pub skipped: Vec<SkippedRepo>,
}

/// Scan a root directory for valid git repositories in its immediate subdirectories.
///
/// A subdirectory is "discovered" if it contains a valid `.git` folder and has
/// at least one commit. Otherwise it is "skipped" with an appropriate reason.
pub fn discover_repos(root: &Path) -> DiscoveryResult {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => {
            return DiscoveryResult {
                discovered: Vec::new(),
                skipped: Vec::new(),
            };
        }
    };

    let (discovered, skipped): (Vec<_>, Vec<_>) = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .map(|path| classify_directory(&path))
        .partition(Result::is_ok);

    DiscoveryResult {
        discovered: discovered.into_iter().map(Result::unwrap).collect(),
        skipped: skipped.into_iter().map(|r| r.unwrap_err()).collect(),
    }
}

/// Classify a single directory as either a discovered repo or a skipped entry.
///
/// Returns `Ok(DiscoveredRepo)` for valid repos with commits,
/// `Err(SkippedRepo)` otherwise.
fn classify_directory(path: &Path) -> Result<DiscoveredRepo, SkippedRepo> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let repo = git2::Repository::open(path).map_err(|_| SkippedRepo {
        path: path.to_path_buf(),
        reason: SkipReason::NotAGitRepo,
    })?;

    let has_commits = repo
        .head()
        .ok()
        .and_then(|head| head.peel_to_commit().ok())
        .is_some();

    if has_commits {
        Ok(DiscoveredRepo {
            name,
            path: path.to_path_buf(),
        })
    } else {
        Err(SkippedRepo {
            path: path.to_path_buf(),
            reason: SkipReason::NoCommits,
        })
    }
}
