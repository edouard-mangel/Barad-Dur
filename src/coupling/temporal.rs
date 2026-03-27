use crate::snapshot::RepoSnapshot;
use serde::{Deserialize, Serialize};
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
pub fn count_co_changes(
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
fn has_neighbor_within_window(sorted_timestamps: &[i64], target: i64, window_secs: i64) -> bool {
    let lower = target - window_secs;
    let upper = target + window_secs;

    // Binary search for the insertion point of `lower`
    let start = sorted_timestamps.partition_point(|&t| t < lower);

    // Check if any element from `start` onward is within range
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

/// Extract sorted Unix timestamps from a snapshot's commits.
fn extract_sorted_timestamps(snapshot: &RepoSnapshot) -> Vec<i64> {
    let mut timestamps: Vec<i64> = snapshot
        .commits
        .iter()
        .map(|c| c.timestamp.timestamp())
        .collect();
    timestamps.sort_unstable();
    timestamps
}

/// Analyze a single pair using pre-sorted timestamps; return coupling pair if >= 3 co-changes.
fn analyze_pair(
    name_a: &str,
    timestamps_a: &[i64],
    commits_a: usize,
    name_b: &str,
    timestamps_b: &[i64],
    commits_b: usize,
    window_secs: i64,
) -> Option<TemporalCouplingPair> {
    // Count co-changes bidirectionally and take the max
    let co_changes_a_to_b = count_co_changes(timestamps_a, timestamps_b, window_secs);
    let co_changes_b_to_a = count_co_changes(timestamps_b, timestamps_a, window_secs);
    let co_changes = co_changes_a_to_b.max(co_changes_b_to_a);

    if co_changes < 3 {
        return None;
    }

    let temporal_score = compute_temporal_score(co_changes, commits_a, commits_b);
    let confidence = classify_confidence(co_changes);

    Some(TemporalCouplingPair {
        repo_a: name_a.to_string(),
        repo_b: name_b.to_string(),
        co_changes,
        temporal_score,
        confidence,
    })
}

/// Analyze temporal coupling across all pairs of repository snapshots.
///
/// For each pair (A, B), counts commits in A that fall within `window` of
/// any commit in B (and vice versa, taking the max). Pairs with fewer than
/// 3 co-changes are filtered out.
///
/// Returns pairs sorted by temporal_score descending.
pub fn analyze_temporal_coupling(
    snapshots: &[(String, RepoSnapshot)],
    window: Duration,
) -> Vec<TemporalCouplingPair> {
    let window_secs = window.as_secs() as i64;

    // Pre-compute sorted timestamps once per repo (avoids O(n²) redundant work)
    let cached: Vec<(&str, Vec<i64>, usize)> = snapshots
        .iter()
        .map(|(name, snap)| {
            let ts = extract_sorted_timestamps(snap);
            let commit_count = snap.commits.len();
            (name.as_str(), ts, commit_count)
        })
        .collect();

    let pair_count = cached.len() * cached.len().saturating_sub(1) / 2;
    let mut pairs: Vec<TemporalCouplingPair> = Vec::with_capacity(pair_count);

    for i in 0..cached.len() {
        for j in (i + 1)..cached.len() {
            let (name_a, ts_a, commits_a) = &cached[i];
            let (name_b, ts_b, commits_b) = &cached[j];
            if let Some(pair) =
                analyze_pair(name_a, ts_a, *commits_a, name_b, ts_b, *commits_b, window_secs)
            {
                pairs.push(pair);
            }
        }
    }

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
