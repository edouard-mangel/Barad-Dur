pub mod gitcli;
mod libgit;

use anyhow::{Context, Result};
use chrono::Utc;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::metrics::complexity;
use crate::snapshot::{
    Author, BlameLine, Commit, FileComplexity, FileEntry, RepoSnapshot, TimeWindow,
};

/// Trait for reporting progress during collection phases.
/// Implemented by `ProgressBar` for real display and `NoProgress` for silent operation.
pub trait Progress: Send + Sync {
    fn inc(&self, delta: u64);
}

impl Progress for ProgressBar {
    fn inc(&self, delta: u64) {
        ProgressBar::inc(self, delta);
    }
}

/// No-op progress reporter — used in tests and when progress display is disabled.
pub struct NoProgress;

impl Progress for NoProgress {
    fn inc(&self, _delta: u64) {}
}

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
        progress: &dyn Progress,
    ) -> Result<HashMap<PathBuf, Vec<BlameLine>>> {
        gitcli::collect_blame(self.repo_path(), files, authors, progress)
    }

    /// Check if this is a shallow clone.
    pub fn is_shallow(&self) -> bool {
        gitcli::is_shallow_clone(self.repo_path())
    }

    /// Analyse working-tree files for static complexity metrics.
    pub fn collect_file_metrics(&self, files: &[FileEntry]) -> HashMap<PathBuf, FileComplexity> {
        self.collect_file_metrics_with_progress(files, &NoProgress)
    }

    fn collect_file_metrics_with_progress(
        &self,
        files: &[FileEntry],
        progress: &dyn Progress,
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
            progress.inc(1);
        }
        map
    }

    /// Build a complete RepoSnapshot with all data and derived indexes.
    pub fn collect_snapshot(&self) -> Result<RepoSnapshot> {
        self.collect_snapshot_with_progress(false)
    }

    /// Build a complete RepoSnapshot, optionally showing progress indicators.
    /// If `verbose` is true, phase timings are printed to stderr.
    pub fn collect_snapshot_with_progress(&self, show_progress: bool) -> Result<RepoSnapshot> {
        self.collect_snapshot_inner(show_progress, false)
    }

    /// Build a complete RepoSnapshot with optional progress and verbose timing.
    pub fn collect_snapshot_verbose(
        &self,
        show_progress: bool,
        verbose: bool,
    ) -> Result<RepoSnapshot> {
        self.collect_snapshot_inner(show_progress, verbose)
    }

    fn collect_snapshot_inner(&self, show_progress: bool, verbose: bool) -> Result<RepoSnapshot> {
        let make_spinner = |msg: &str| -> Option<ProgressBar> {
            if !show_progress {
                return None;
            }
            let sp = ProgressBar::new_spinner();
            sp.set_style(
                ProgressStyle::default_spinner()
                    .template("  {spinner:.cyan} {msg}")
                    .unwrap(),
            );
            sp.set_message(msg.to_string());
            sp.enable_steady_tick(std::time::Duration::from_millis(80));
            Some(sp)
        };

        let bar_style = ProgressStyle::default_bar()
            .template("  {spinner:.cyan} {msg} [{bar:30.cyan/dim}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("━╸─");

        // Phase 1: commits (fast, spinner only)
        let sp = make_spinner("Walking commits...");
        let t = Instant::now();
        let collection = self.collect_commits()?;
        let commits_ms = t.elapsed().as_millis();
        if let Some(s) = sp {
            s.finish_and_clear();
        }

        // Phase 2: file tree (fast, spinner only)
        let sp = make_spinner(&format!(
            "Found {} commits. Collecting file tree...",
            collection.commits.len()
        ));
        let t = Instant::now();
        let files = self.collect_files()?;
        let files_ms = t.elapsed().as_millis();
        if let Some(s) = sp {
            s.finish_and_clear();
        }

        // Phase 3: blame (slow — real progress bar)
        let non_binary: u64 = files.iter().filter(|f| !f.is_binary).count() as u64;
        let blame_bar = if show_progress {
            let pb = ProgressBar::new(non_binary);
            pb.set_style(bar_style.clone());
            pb.set_message("Blaming files");
            pb.enable_steady_tick(std::time::Duration::from_millis(80));
            Some(pb)
        } else {
            None
        };
        let t = Instant::now();
        let blame_progress: &dyn Progress = match &blame_bar {
            Some(pb) => pb,
            None => &NoProgress,
        };
        let blame_map = self.collect_blame(&files, &collection.authors, blame_progress)?;
        let blame_ms = t.elapsed().as_millis();
        if let Some(pb) = blame_bar {
            pb.finish_and_clear();
        }

        // Phase 4: complexity (can be slow on large repos — progress bar)
        let complexity_bar = if show_progress {
            let pb = ProgressBar::new(non_binary);
            pb.set_style(bar_style);
            pb.set_message("Analysing complexity");
            pb.enable_steady_tick(std::time::Duration::from_millis(80));
            Some(pb)
        } else {
            None
        };
        let t = Instant::now();
        let complexity_progress: &dyn Progress = match &complexity_bar {
            Some(pb) => pb,
            None => &NoProgress,
        };
        let file_metrics = self.collect_file_metrics_with_progress(&files, complexity_progress);
        let complexity_ms = t.elapsed().as_millis();
        if let Some(pb) = complexity_bar {
            pb.finish_and_clear();
        }

        // Phase 5: indexes (fast, spinner only)
        let sp = make_spinner("Building indexes...");
        let t = Instant::now();
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
        let indexes_ms = t.elapsed().as_millis();

        if let Some(s) = sp {
            s.finish_and_clear();
        }

        if verbose {
            eprintln!(
                "  Timings: commits {}ms, files {}ms, blame {}ms, complexity {}ms, indexes {}ms",
                commits_ms, files_ms, blame_ms, complexity_ms, indexes_ms
            );
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
        let collector = Collector::open(std::path::Path::new("."), TimeWindow::default())
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
