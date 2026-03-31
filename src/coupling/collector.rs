use rayon::prelude::*;
use std::path::PathBuf;

use crate::collector::Collector;
use crate::coupling::discovery::{DiscoveredRepo, SkipReason, SkippedRepo};
use crate::coupling::CouplingConfig;
use crate::snapshot::TimeWindow;

/// Lightweight snapshot carrying only the data needed for coupling analysis.
///
/// Unlike `RepoSnapshot`, this skips file trees, blame, complexity metrics,
/// import graphs, and all derived indexes — cutting per-repo RAM by ~80%.
#[derive(Debug, Clone)]
pub struct CouplingSnapshot {
    pub path: PathBuf,
    pub commit_timestamps: Vec<i64>,
    /// Per-commit author index into `author_names` (parallel to `commit_timestamps`).
    pub commit_author_indices: Vec<usize>,
    pub author_names: Vec<String>,
    pub commit_count: usize,
    pub author_count: usize,
}

/// Outcome of collecting snapshots from multiple repositories.
#[derive(Debug)]
pub struct CollectionResult {
    /// Successfully collected snapshots, keyed by repo name.
    pub snapshots: Vec<(String, CouplingSnapshot)>,
    /// Repos that failed collection (gracefully skipped).
    pub failed: Vec<SkippedRepo>,
}

/// Build a `TimeWindow` from the analysis window duration in the coupling config.
fn time_window_from_config(config: &CouplingConfig) -> TimeWindow {
    let now = chrono::Utc::now();
    let analysis_days = config.analysis_window.as_secs() / (24 * 60 * 60);
    TimeWindow {
        since: Some(now - chrono::Duration::days(analysis_days as i64)),
        until: Some(now),
        default_months: (analysis_days / 30) as u32,
    }
}

/// Attempt to collect a single repo's coupling snapshot.
///
/// Uses `collect_commits()` instead of the full `collect_snapshot_verbose`,
/// extracting only commit timestamps and author names.
///
/// Returns `Ok((name, snapshot))` on success, `Err(SkippedRepo)` on failure.
fn collect_single_repo(
    repo: &DiscoveredRepo,
    time_window: &TimeWindow,
) -> Result<(String, CouplingSnapshot), SkippedRepo> {
    let collector = Collector::open(&repo.path, time_window.clone()).map_err(|e| SkippedRepo {
        path: repo.path.clone(),
        reason: SkipReason::Other(format!("CollectionFailed: {e}")),
    })?;

    let collection = collector.collect_commits().map_err(|e| SkippedRepo {
        path: repo.path.clone(),
        reason: SkipReason::Other(format!("CollectionFailed: {e}")),
    })?;

    let author_names: Vec<String> = collection
        .authors
        .iter()
        .map(|a| a.name.to_lowercase())
        .collect();

    // Build per-commit parallel vectors: timestamp + author index
    let mut commit_timestamps: Vec<i64> = Vec::with_capacity(collection.commits.len());
    let mut commit_author_indices: Vec<usize> = Vec::with_capacity(collection.commits.len());
    for commit in &collection.commits {
        commit_timestamps.push(commit.timestamp.timestamp());
        commit_author_indices.push(commit.author);
    }

    let snapshot = CouplingSnapshot {
        path: repo.path.clone(),
        commit_count: commit_timestamps.len(),
        author_count: author_names.len(),
        commit_timestamps,
        commit_author_indices,
        author_names,
    };

    Ok((repo.name.clone(), snapshot))
}

/// Collect `CouplingSnapshot`s from discovered repos in parallel (rayon).
///
/// Each repo is opened and only commits + authors are collected — no blame,
/// file trees, or complexity metrics. Repos that fail collection are
/// gracefully skipped and reported in the `failed` list rather than aborting
/// the entire pipeline.
pub fn collect_snapshots(repos: &[DiscoveredRepo], config: &CouplingConfig) -> CollectionResult {
    let time_window = time_window_from_config(config);

    let results: Vec<Result<(String, CouplingSnapshot), SkippedRepo>> = repos
        .par_iter()
        .map(|repo| collect_single_repo(repo, &time_window))
        .collect();

    let (snapshots, failed): (Vec<_>, Vec<_>) = results.into_iter().partition(Result::is_ok);

    CollectionResult {
        snapshots: snapshots.into_iter().map(Result::unwrap).collect(),
        failed: failed.into_iter().map(|r| r.unwrap_err()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_window_from_default_config_spans_180_days() {
        let config = CouplingConfig::default();
        let window = time_window_from_config(&config);
        let since = window.since.unwrap();
        let until = window.until.unwrap();
        let days = (until - since).num_days();
        assert!(
            (179..=181).contains(&days),
            "expected ~180 days, got {days}"
        );
    }

    #[test]
    fn collect_snapshots_returns_empty_for_no_repos() {
        let config = CouplingConfig::default();
        let result = collect_snapshots(&[], &config);
        assert!(result.snapshots.is_empty());
        assert!(result.failed.is_empty());
    }

    #[test]
    fn collect_single_repo_fails_gracefully_for_non_git_dir() {
        let temp = tempfile::TempDir::new().unwrap();
        let repo = DiscoveredRepo {
            name: "not-a-repo".to_string(),
            path: temp.path().to_path_buf(),
        };
        let window = TimeWindow::default();
        let result = collect_single_repo(&repo, &window);
        assert!(result.is_err());
        let skipped = result.unwrap_err();
        assert!(matches!(skipped.reason, SkipReason::Other(_)));
    }
}
