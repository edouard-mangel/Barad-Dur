pub mod gitcli;
mod libgit;

use anyhow::{Context, Result};
use chrono::Utc;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::metrics::complexity;
use crate::snapshot::{Author, BlameLine, Commit, FileComplexity, FileEntry, RepoSnapshot, TimeWindow};

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

    /// Analyse working-tree files for static complexity metrics.
    pub fn collect_file_metrics(
        &self,
        files: &[FileEntry],
    ) -> HashMap<PathBuf, FileComplexity> {
        let root = self.repo_path();
        let mut map = HashMap::new();
        for entry in files {
            if entry.is_binary {
                continue;
            }
            let abs_path = root.join(&entry.path);
            if let Ok(content) = std::fs::read_to_string(&abs_path) {
                let metrics = complexity::analyse_file(&entry.path, &content);
                map.insert(entry.path.clone(), metrics);
            }
        }
        map
    }

    /// Build a complete RepoSnapshot with all data and derived indexes.
    pub fn collect_snapshot(&self) -> Result<RepoSnapshot> {
        self.collect_snapshot_with_progress(false)
    }

    /// Build a complete RepoSnapshot, optionally showing progress indicators.
    pub fn collect_snapshot_with_progress(&self, show_progress: bool) -> Result<RepoSnapshot> {
        let spinner = if show_progress {
            let sp = ProgressBar::new_spinner();
            sp.set_style(
                ProgressStyle::default_spinner()
                    .template("  {spinner:.cyan} {msg}")
                    .unwrap(),
            );
            sp.set_message("Walking commits...");
            sp.enable_steady_tick(std::time::Duration::from_millis(80));
            Some(sp)
        } else {
            None
        };

        let collection = self.collect_commits()?;
        if let Some(sp) = &spinner {
            sp.set_message(format!(
                "Found {} commits. Collecting file tree...",
                collection.commits.len()
            ));
        }

        let files = self.collect_files()?;
        if let Some(sp) = &spinner {
            sp.set_message(format!(
                "Found {} files. Running blame ({} non-binary)...",
                files.len(),
                files.iter().filter(|f| !f.is_binary).count()
            ));
        }

        let blame_map = self.collect_blame(&files, &collection.authors)?;
        if let Some(sp) = &spinner {
            sp.set_message("Analysing file complexity...");
        }

        let file_metrics = self.collect_file_metrics(&files);
        if let Some(sp) = &spinner {
            sp.set_message("Building indexes...");
        }

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
            file_metrics,
        };
        snapshot.build_indexes();

        if let Some(sp) = spinner {
            sp.finish_and_clear();
        }

        Ok(snapshot)
    }

    pub fn repo_path(&self) -> &Path {
        self.repo.workdir().unwrap_or_else(|| self.repo.path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::TimeWindow;

    #[test]
    fn collect_file_metrics_does_not_panic_on_real_repo() {
        let collector =
            Collector::open(std::path::Path::new("."), TimeWindow::default())
                .expect("should open repo");
        let files = collector.collect_files().expect("should collect files");
        let metrics = collector.collect_file_metrics(&files);
        assert!(!metrics.is_empty());
        let rs_file = metrics
            .keys()
            .find(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"));
        assert!(rs_file.is_some(), "expected at least one .rs file");
    }
}
