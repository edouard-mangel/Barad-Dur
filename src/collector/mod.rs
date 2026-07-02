pub mod deps;
mod exclude;
pub mod gitcli;
mod ignore_file;
mod import_resolver;
mod libgit;
mod progress;
mod snapshot_builder;
mod types;

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::snapshot::{
    Author, AuthorId, BlameLine, FileComplexity, FileEntry, RepoSnapshot, TimeWindow,
};

pub use exclude::is_excluded;
pub use ignore_file::exclude_fingerprint;
pub(crate) use ignore_file::BaradDurIgnore;
pub use progress::{NoProgress, Progress};
pub use types::CommitCollection;

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
        // Prefer remote URL so the name is stable across worktrees.
        if let Ok(remote) = self.repo.find_remote("origin") {
            if let Ok(url) = remote.url() {
                let segment = url.rsplit('/').next().unwrap_or(url);
                let segment = segment.rsplit(':').next().unwrap_or(segment);
                let name = segment.trim_end_matches(".git");
                if !name.is_empty() {
                    return name.to_string();
                }
            }
        }
        // Fallback for repos with no remote.
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
            .and_then(|h| h.shorthand().ok().map(String::from))
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
        raw_email_to_id: &HashMap<String, AuthorId>,
        progress: &dyn Progress,
    ) -> Result<HashMap<PathBuf, Vec<BlameLine>>> {
        gitcli::collect_blame(self.repo_path(), files, authors, raw_email_to_id, progress)
    }

    /// Collect blame data, reusing cached entries for unchanged blobs.
    /// Returns (blame_map, updated_cache).
    pub fn collect_blame_cached(
        &self,
        files: &[FileEntry],
        authors: &[Author],
        raw_email_to_id: &HashMap<String, AuthorId>,
        cache: &crate::cache::blame::BlameCache,
        progress: &dyn Progress,
    ) -> Result<(
        HashMap<PathBuf, Vec<BlameLine>>,
        crate::cache::blame::BlameCache,
    )> {
        gitcli::collect_blame_cached(
            self.repo_path(),
            files,
            authors,
            raw_email_to_id,
            cache,
            progress,
        )
    }

    /// Check if this is a shallow clone.
    pub fn is_shallow(&self) -> bool {
        gitcli::is_shallow_clone(self.repo_path())
    }

    /// Analyse working-tree files for static complexity metrics.
    pub fn collect_file_metrics(&self, files: &[FileEntry]) -> HashMap<PathBuf, FileComplexity> {
        let (metrics, _) = self.collect_file_metrics_with_progress(files, &NoProgress);
        metrics
    }

    /// Build a complete RepoSnapshot with all data and derived indexes.
    pub fn collect_snapshot(&self) -> Result<RepoSnapshot> {
        self.collect_snapshot_with_progress(false)
    }

    /// Build a complete RepoSnapshot, optionally showing progress indicators.
    pub fn collect_snapshot_with_progress(&self, show_progress: bool) -> Result<RepoSnapshot> {
        self.collect_snapshot_inner(show_progress, false, false, false, &[], &[], true)
    }

    /// Build a complete RepoSnapshot with full control over display and phases.
    #[allow(clippy::too_many_arguments)]
    pub fn collect_snapshot_verbose(
        &self,
        show_progress: bool,
        verbose: bool,
        skip_blame: bool,
        no_cache: bool,
        cli_exclude_patterns: &[String],
        cli_exclude_extensions: &[String],
        use_default_excludes: bool,
    ) -> Result<RepoSnapshot> {
        self.collect_snapshot_inner(
            show_progress,
            verbose,
            skip_blame,
            no_cache,
            cli_exclude_patterns,
            cli_exclude_extensions,
            use_default_excludes,
        )
    }

    pub fn repo_path(&self) -> &Path {
        self.repo.workdir().unwrap_or_else(|| self.repo.path())
    }
}
