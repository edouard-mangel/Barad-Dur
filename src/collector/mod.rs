mod libgit;
pub mod gitcli;

use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::snapshot::{Author, BlameLine, Commit, FileEntry, RepoSnapshot, TimeWindow};

/// Result of collecting commits — includes deduplicated author list.
pub struct CommitCollection {
    pub commits: Vec<Commit>,
    pub authors: Vec<Author>,
}

pub struct Collector {
    repo: git2::Repository,
    pub time_window: TimeWindow,
}

impl Collector {
    pub fn open(path: &Path, time_window: TimeWindow) -> Result<Self> {
        let repo = git2::Repository::discover(path).with_context(|| {
            format!(
                "'{}' is not a git repository. Run from a repo root or pass a path.",
                path.display()
            )
        })?;
        Ok(Collector { repo, time_window })
    }

    pub fn repo_name(&self) -> String {
        self.repo
            .workdir()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    pub fn default_branch(&self) -> String {
        self.repo
            .head()
            .ok()
            .and_then(|h| h.shorthand().map(String::from))
            .unwrap_or_else(|| "main".to_string())
    }

    pub fn head_commit_hash(&self) -> Result<String> {
        let head = self.repo.head().context("Failed to get HEAD")?;
        let oid = head.target().context("HEAD has no target")?;
        Ok(oid.to_string())
    }

    /// Collect all commits in the time window, along with deduplicated authors.
    pub fn collect_commits(&self) -> Result<CommitCollection> {
        libgit::collect_commits(&self.repo, &self.time_window)
    }

    /// Collect the current file tree from HEAD.
    pub fn collect_files(&self) -> Result<Vec<FileEntry>> {
        libgit::collect_files(&self.repo)
    }

    /// Collect blame data for all non-binary files.
    pub fn collect_blame(
        &self,
        files: &[FileEntry],
        authors: &[Author],
    ) -> Result<HashMap<PathBuf, Vec<BlameLine>>> {
        gitcli::collect_blame(self.repo_path(), files, authors)
    }

    /// Check if this is a shallow clone.
    pub fn is_shallow(&self) -> bool {
        gitcli::is_shallow_clone(self.repo_path())
    }

    /// Build a complete RepoSnapshot with all data and derived indexes.
    pub fn collect_snapshot(&self) -> Result<RepoSnapshot> {
        let collection = self.collect_commits()?;
        let files = self.collect_files()?;
        let blame_map = self.collect_blame(&files, &collection.authors)?;
        let head = self.head_commit_hash()?;

        let mut snapshot = RepoSnapshot {
            path: self.repo_path().to_path_buf(),
            name: self.repo_name(),
            default_branch: self.default_branch(),
            time_window: self.time_window.clone(),
            head_commit: head,
            created_at: Utc::now(),
            commits: collection.commits,
            files,
            authors: collection.authors,
            blame_map,
            commits_by_author: HashMap::new(),
            commits_by_file: HashMap::new(),
            file_change_pairs: Vec::new(),
        };
        snapshot.build_indexes();
        Ok(snapshot)
    }

    pub fn repo_path(&self) -> &Path {
        self.repo.workdir().unwrap_or_else(|| self.repo.path())
    }
}
