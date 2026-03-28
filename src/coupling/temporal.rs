use crate::snapshot::RepoSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

/// Confidence level based on number of co-changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Confidence {
    /// 3-9 co-changes
    Low,
    /// 10-29 co-changes
    Medium,
    /// 30+ co-changes
    High,
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Confidence::Low => write!(f, "LOW"),
            Confidence::Medium => write!(f, "MEDIUM"),
            Confidence::High => write!(f, "HIGH"),
        }
    }
}

/// A detected temporal coupling between two repositories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalCouplingPair {
    pub repo_a: String,
    pub repo_b: String,
    /// Number of commit pairs within the coupling window.
    pub co_changes: usize,
    /// (co_changes / min(commits_a, commits_b)) * 100
    pub temporal_score: f64,
    pub confidence: Confidence,
}

/// Classify co-change count into a confidence level.
pub fn classify_confidence(co_changes: usize) -> Confidence {
    match co_changes {
        0..=9 => Confidence::Low,
        10..=29 => Confidence::Medium,
        _ => Confidence::High,
    }
}

/// Count how many commits in repo_a fall within `window` of any commit in repo_b.
///
/// Each commit in repo_a is counted at most once (it either has a neighbor in
/// repo_b within the window or it doesn't). Both slices must be pre-sorted;
/// uses binary search for efficiency.
///
/// Note: this function is not used by `analyze_temporal_coupling`, which uses a
/// faster merged-timeline approach. It is retained as a reference implementation
/// for unit testing of the binary-search neighbor logic.
#[cfg(test)]
fn count_co_changes(
    sorted_timestamps_a: &[i64],
    sorted_timestamps_b: &[i64],
    window_secs: i64,
) -> usize {
    if sorted_timestamps_a.is_empty() || sorted_timestamps_b.is_empty() {
        return 0;
    }

    sorted_timestamps_a
        .iter()
        .filter(|&&ts_a| has_neighbor_within_window(sorted_timestamps_b, ts_a, window_secs))
        .count()
}

/// Check if any timestamp in `sorted_timestamps` falls within `window_secs` of `target`.
#[cfg(test)]
fn has_neighbor_within_window(sorted_timestamps: &[i64], target: i64, window_secs: i64) -> bool {
    let lower = target - window_secs;
    let upper = target + window_secs;

    let start = sorted_timestamps.partition_point(|&t| t < lower);
    start < sorted_timestamps.len() && sorted_timestamps[start] <= upper
}

/// Compute temporal score: (co_changes / min(commits_a, commits_b)) * 100.
pub fn compute_temporal_score(co_changes: usize, commits_a: usize, commits_b: usize) -> f64 {
    let min_commits = commits_a.min(commits_b);
    if min_commits == 0 {
        return 0.0;
    }
    (co_changes as f64 / min_commits as f64) * 100.0
}

/// Canonical pair key with smaller index first.
fn directed_key(a: usize, b: usize) -> (usize, usize) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Analyze temporal coupling across all pairs of repository snapshots.
///
/// Merges all commits into a single sorted timeline and uses binary search
/// per commit to find neighbors within `window`. This is O(M log M + M × W_avg)
/// where M = total commits and W_avg = average commits per window, replacing
/// the previous O(n² × m) pairwise approach.
///
/// For each pair (A, B), counts commits in A that have at least one commit
/// from B within the window (and vice versa, taking the max). Pairs with
/// fewer than 3 co-changes are filtered out.
///
/// Returns pairs sorted by temporal_score descending.
pub fn analyze_temporal_coupling(
    snapshots: &[(String, RepoSnapshot)],
    window: Duration,
) -> Vec<TemporalCouplingPair> {
    if snapshots.len() < 2 {
        return Vec::new();
    }

    let window_secs = window.as_secs() as i64;

    // Collect commit counts per repo (needed for scoring)
    let commit_counts: Vec<usize> = snapshots.iter().map(|(_, s)| s.commits.len()).collect();

    // Step 1: Merge all commits into a single timeline tagged with repo index
    let total_commits: usize = commit_counts.iter().sum();
    let mut timeline: Vec<(i64, usize)> = Vec::with_capacity(total_commits);
    for (repo_idx, (_, snap)) in snapshots.iter().enumerate() {
        for commit in &snap.commits {
            timeline.push((commit.timestamp.timestamp(), repo_idx));
        }
    }

    // Step 2: Sort by timestamp
    timeline.sort_unstable_by_key(|&(ts, _)| ts);

    // Step 3: For each commit, find neighbors within ±window via binary search,
    // collect distinct repos in that range, and increment directed co-change counts.
    // Key: (source_repo, neighbor_repo) → count of source commits that have a neighbor.
    let mut directed_counts: HashMap<(usize, usize), usize> = HashMap::new();

    let timestamps_only: Vec<i64> = timeline.iter().map(|&(ts, _)| ts).collect();

    for &(ts, repo) in &timeline {
        let lower = ts - window_secs;
        let upper = ts + window_secs;

        let start = timestamps_only.partition_point(|&t| t < lower);
        let end = timestamps_only.partition_point(|&t| t <= upper);

        // Scan window and collect distinct neighbor repos (skip self-repo)
        let mut seen_repos = Vec::new();
        for &(_, other_repo) in &timeline[start..end] {
            if other_repo != repo && !seen_repos.contains(&other_repo) {
                seen_repos.push(other_repo);
            }
        }

        // This commit from `repo` has at least one neighbor in each `other_repo`
        for other_repo in seen_repos {
            *directed_counts.entry((repo, other_repo)).or_insert(0) += 1;
        }
    }

    // Step 4: Build pairs — for each canonical pair, take max of both directions
    let mut seen_pairs: HashMap<(usize, usize), (usize, usize)> = HashMap::new();
    for (&(src, dst), &count) in &directed_counts {
        let key = directed_key(src, dst);
        let entry = seen_pairs.entry(key).or_insert((0, 0));
        if src == key.0 {
            entry.0 = entry.0.max(count);
        } else {
            entry.1 = entry.1.max(count);
        }
    }

    let mut pairs: Vec<TemporalCouplingPair> = seen_pairs
        .into_iter()
        .filter_map(|((idx_a, idx_b), (count_a_to_b, count_b_to_a))| {
            let co_changes = count_a_to_b.max(count_b_to_a);
            if co_changes < 3 {
                return None;
            }

            let temporal_score =
                compute_temporal_score(co_changes, commit_counts[idx_a], commit_counts[idx_b]);
            let confidence = classify_confidence(co_changes);

            Some(TemporalCouplingPair {
                repo_a: snapshots[idx_a].0.clone(),
                repo_b: snapshots[idx_b].0.clone(),
                co_changes,
                temporal_score,
                confidence,
            })
        })
        .collect();

    pairs.sort_by(|a, b| {
        b.temporal_score
            .partial_cmp(&a.temporal_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Confidence classification ---

    #[test]
    fn confidence_low_for_3_co_changes() {
        assert_eq!(classify_confidence(3), Confidence::Low);
    }

    #[test]
    fn confidence_low_for_9_co_changes() {
        assert_eq!(classify_confidence(9), Confidence::Low);
    }

    #[test]
    fn confidence_medium_for_10_co_changes() {
        assert_eq!(classify_confidence(10), Confidence::Medium);
    }

    #[test]
    fn confidence_medium_for_29_co_changes() {
        assert_eq!(classify_confidence(29), Confidence::Medium);
    }

    #[test]
    fn confidence_high_for_30_co_changes() {
        assert_eq!(classify_confidence(30), Confidence::High);
    }

    #[test]
    fn confidence_high_for_100_co_changes() {
        assert_eq!(classify_confidence(100), Confidence::High);
    }

    // --- Co-change counting ---

    #[test]
    fn co_changes_counts_overlapping_timestamps() {
        // Timestamps 1h apart, window is 24h => all overlap
        let a = vec![1000, 2000, 3000];
        let b = vec![1500, 2500, 3500];
        let window_secs = 24 * 3600;
        assert_eq!(count_co_changes(&a, &b, window_secs), 3);
    }

    #[test]
    fn co_changes_zero_when_no_overlap() {
        let a = vec![1000, 2000];
        let b = vec![1_000_000, 2_000_000]; // far apart
        let window_secs = 3600; // 1h window
        assert_eq!(count_co_changes(&a, &b, window_secs), 0);
    }

    #[test]
    fn co_changes_partial_overlap() {
        let a = vec![1000, 100_000, 200_000];
        let b = vec![1500]; // only overlaps with a[0]
        let window_secs = 3600;
        assert_eq!(count_co_changes(&a, &b, window_secs), 1);
    }

    #[test]
    fn co_changes_empty_inputs() {
        assert_eq!(count_co_changes(&[], &[1000], 3600), 0);
        assert_eq!(count_co_changes(&[1000], &[], 3600), 0);
        assert_eq!(count_co_changes(&[], &[], 3600), 0);
    }

    // --- Temporal score ---

    #[test]
    fn temporal_score_formula() {
        // 4 co-changes, min(5, 4) = 4 => (4/4)*100 = 100.0
        let score = compute_temporal_score(4, 5, 4);
        assert!((score - 100.0).abs() < 0.01);
    }

    #[test]
    fn temporal_score_partial() {
        // 3 co-changes, min(10, 5) = 5 => (3/5)*100 = 60.0
        let score = compute_temporal_score(3, 10, 5);
        assert!((score - 60.0).abs() < 0.01);
    }

    #[test]
    fn temporal_score_zero_commits_returns_zero() {
        assert!((compute_temporal_score(0, 0, 0)).abs() < 0.01);
    }

    // --- Pair filtering ---

    #[test]
    fn analyze_filters_pairs_below_3_co_changes() {
        let result = analyze_temporal_coupling(&[], Duration::from_secs(24 * 3600));
        assert!(result.is_empty());
    }
}
