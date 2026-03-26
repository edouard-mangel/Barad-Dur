use rayon::prelude::*;

use crate::collector::Collector;
use crate::coupling::discovery::{DiscoveredRepo, SkipReason, SkippedRepo};
use crate::coupling::CouplingConfig;
use crate::snapshot::{RepoSnapshot, TimeWindow};

/// Outcome of collecting snapshots from multiple repositories.
#[derive(Debug)]
pub struct CollectionResult {
    /// Successfully collected snapshots, keyed by repo name.
    pub snapshots: Vec<(String, RepoSnapshot)>,
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

/// Attempt to collect a single repo's snapshot with skip-blame optimization.
///
/// Returns `Ok((name, snapshot))` on success, `Err(SkippedRepo)` on failure.
fn collect_single_repo(
    repo: &DiscoveredRepo,
    time_window: &TimeWindow,
) -> Result<(String, RepoSnapshot), SkippedRepo> {
    let collector = Collector::open(&repo.path, time_window.clone()).map_err(|e| SkippedRepo {
        path: repo.path.clone(),
        reason: SkipReason::Other(format!("CollectionFailed: {e}")),
    })?;

    let snapshot = collector
        .collect_snapshot_verbose(
            false, // show_progress
            false, // verbose
            true,  // skip_blame -- coupling only needs commits + authors
            true,  // no_cache
            &[],   // exclude_patterns
            true,  // use_default_excludes
        )
        .map_err(|e| SkippedRepo {
            path: repo.path.clone(),
            reason: SkipReason::Other(format!("CollectionFailed: {e}")),
        })?;

    Ok((repo.name.clone(), snapshot))
}

/// Collect `RepoSnapshot`s from discovered repos in parallel (rayon).
///
/// Each repo is opened and collected with `skip_blame = true` since coupling
/// analysis only needs commits and authors, not per-line blame data. Repos
/// that fail collection are gracefully skipped and reported in the `failed`
/// list rather than aborting the entire pipeline.
pub fn collect_snapshots(repos: &[DiscoveredRepo], config: &CouplingConfig) -> CollectionResult {
    let time_window = time_window_from_config(config);

    let results: Vec<Result<(String, RepoSnapshot), SkippedRepo>> = repos
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
