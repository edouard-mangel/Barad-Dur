use crate::coupling::collector::CouplingSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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

/// Weight multiplier for co-changes where the same author committed to both repos.
const SAME_AUTHOR_WEIGHT: f64 = 3.0;

/// Analyze temporal coupling across all pairs of repository snapshots.
///
/// Merges all commits into a single sorted timeline and uses binary search
/// per commit to find neighbors within `window`. For each pair (A, B):
///
/// 1. Counts co-changes (commits in A within ±window of a commit in B).
/// 2. Same-author co-changes (same person committed to both) are weighted 3×
///    higher than different-author co-changes.
/// 3. A statistical baseline (expected coincidental co-changes given commit
///    frequencies) is subtracted to filter out "office hours" false positives.
/// 4. The adjusted count is normalized into a 0-100 score.
///
/// Pairs with fewer than 3 raw co-changes are filtered out.
/// Returns pairs sorted by temporal_score descending.
pub fn analyze_temporal_coupling(
    snapshots: &[(String, CouplingSnapshot)],
    window: Duration,
) -> Vec<TemporalCouplingPair> {
    if snapshots.len() < 2 {
        return Vec::new();
    }

    let window_secs = window.as_secs() as i64;

    let commit_counts: Vec<usize> = snapshots.iter().map(|(_, s)| s.commit_count).collect();

    // Step 1: Merge all commits into a sorted timeline: (timestamp, repo_idx, author_name)
    let total_commits: usize = commit_counts.iter().sum();
    let mut timeline: Vec<(i64, usize, String)> = Vec::with_capacity(total_commits);
    for (repo_idx, (_, snap)) in snapshots.iter().enumerate() {
        for (i, &ts) in snap.commit_timestamps.iter().enumerate() {
            let author_idx = snap.commit_author_indices.get(i).copied().unwrap_or(0);
            let author_name = snap
                .author_names
                .get(author_idx)
                .cloned()
                .unwrap_or_default();
            timeline.push((ts, repo_idx, author_name));
        }
    }
    timeline.sort_unstable_by_key(|&(ts, _, _)| ts);

    let timestamps_only: Vec<i64> = timeline.iter().map(|&(ts, _, _)| ts).collect();

    // Step 2: For each commit, find neighbors within ±window.
    // Track both same-author and different-author co-changes per directed pair.
    // Key: (source_repo, neighbor_repo) → (same_author_count, diff_author_count)
    let mut directed_counts: HashMap<(usize, usize), (usize, usize)> = HashMap::new();

    for (idx, &(ts, repo, ref author)) in timeline.iter().enumerate() {
        let _ = idx;
        let lower = ts - window_secs;
        let upper = ts + window_secs;

        let start = timestamps_only.partition_point(|&t| t < lower);
        let end = timestamps_only.partition_point(|&t| t <= upper);

        // For each neighbor repo, check if any neighbor has the same author
        let mut same_author_repos: HashSet<usize> = HashSet::new();
        let mut diff_author_repos: HashSet<usize> = HashSet::new();

        for &(_, other_repo, ref other_author) in &timeline[start..end] {
            if other_repo != repo {
                if other_author == author {
                    same_author_repos.insert(other_repo);
                } else {
                    diff_author_repos.insert(other_repo);
                }
            }
        }

        // A repo with same-author match is not also counted as diff-author
        for other_repo in &same_author_repos {
            let entry = directed_counts.entry((repo, *other_repo)).or_insert((0, 0));
            entry.0 += 1;
        }
        for other_repo in &diff_author_repos {
            if !same_author_repos.contains(other_repo) {
                let entry = directed_counts.entry((repo, *other_repo)).or_insert((0, 0));
                entry.1 += 1;
            }
        }
    }

    // Step 3: Compute analysis period for statistical baseline
    let (min_ts, max_ts) = timeline
        .iter()
        .fold((i64::MAX, i64::MIN), |(lo, hi), &(ts, _, _)| {
            (lo.min(ts), hi.max(ts))
        });
    let analysis_period_secs = (max_ts - min_ts).max(1) as f64;

    // Step 4: Build pairs — merge both directions, apply weighting and baseline
    let mut seen_pairs: HashMap<(usize, usize), (usize, usize, usize, usize)> = HashMap::new();
    for (&(src, dst), &(same, diff)) in &directed_counts {
        let key = directed_key(src, dst);
        let entry = seen_pairs.entry(key).or_insert((0, 0, 0, 0));
        if src == key.0 {
            entry.0 = entry.0.max(same);
            entry.1 = entry.1.max(diff);
        } else {
            entry.2 = entry.2.max(same);
            entry.3 = entry.3.max(diff);
        }
    }

    let mut pairs: Vec<TemporalCouplingPair> = seen_pairs
        .into_iter()
        .filter_map(
            |((idx_a, idx_b), (same_a_to_b, diff_a_to_b, same_b_to_a, diff_b_to_a))| {
                let raw_co_changes_a = same_a_to_b + diff_a_to_b;
                let raw_co_changes_b = same_b_to_a + diff_b_to_a;
                let co_changes = raw_co_changes_a.max(raw_co_changes_b);

                if co_changes < 3 {
                    return None;
                }

                // Weighted co-changes: same-author counts 3×
                let same_author = same_a_to_b.max(same_b_to_a);
                let diff_author = diff_a_to_b.max(diff_b_to_a);
                let weighted = same_author as f64 * SAME_AUTHOR_WEIGHT + diff_author as f64;

                // Statistical baseline: expected coincidental co-changes
                let n_a = commit_counts[idx_a] as f64;
                let n_b = commit_counts[idx_b] as f64;
                let expected = n_a * n_b * (2.0 * window_secs as f64) / analysis_period_secs;

                let adjusted = (weighted - expected).max(0.0);
                let min_commits = n_a.min(n_b);
                let temporal_score = if min_commits > 0.0 {
                    (adjusted / min_commits * 100.0).min(100.0)
                } else {
                    0.0
                };

                let confidence = classify_confidence(co_changes);

                Some(TemporalCouplingPair {
                    repo_a: snapshots[idx_a].0.clone(),
                    repo_b: snapshots[idx_b].0.clone(),
                    co_changes,
                    temporal_score,
                    confidence,
                })
            },
        )
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
