use chrono::{DateTime, Utc};

/// A commit identified by its SHA and commit timestamp.
pub type CommitRef = (String, DateTime<Utc>);

/// Select a representative sample of commits spaced evenly in time.
///
/// Pure function — no I/O, no side effects.
///
/// `commits` is expected newest-first (standard git log order).
///
/// - When `count == 0` or `commits` is empty, returns empty.
/// - When `count >= len`, returns all commits (by SHA).
/// - When `count == 1`, returns the newest commit.
/// - Otherwise, projects `count` evenly-spaced target timestamps across
///   `[oldest..newest]` and selects the nearest commit to each target.
///   Duplicate selections (dense commit clusters) are collapsed, so the
///   result may contain fewer than `count` items.
pub fn select_samples(commits: &[CommitRef], count: usize) -> Vec<String> {
    let len = commits.len();
    if count == 0 || len == 0 {
        return Vec::new();
    }
    if count >= len {
        return commits.iter().map(|(sha, _)| sha.clone()).collect();
    }
    if count == 1 {
        return vec![commits[0].0.clone()];
    }

    // commits[0] = newest, commits[len-1] = oldest
    let min_t = commits[len - 1].1.timestamp();
    let max_t = commits[0].1.timestamp();
    let range = max_t - min_t;

    let mut selected: Vec<String> = Vec::with_capacity(count);
    let mut last_idx: Option<usize> = None;

    for i in 0..count {
        let target_t = if range == 0 {
            min_t
        } else {
            min_t + i as i64 * range / (count - 1) as i64
        };

        let idx = commits
            .iter()
            .enumerate()
            .min_by_key(|(_, (_, ts))| (ts.timestamp() - target_t).abs())
            .map(|(j, _)| j)
            .unwrap(); // safe: len >= 2

        if Some(idx) != last_idx {
            selected.push(commits[idx].0.clone());
            last_idx = Some(idx);
        }
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// Build N commits evenly spread over 2024, newest-first.
    fn make_dated_commits(n: usize) -> Vec<CommitRef> {
        let start = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap();
        let total_secs = (end - start).num_seconds();
        // Produce in oldest-first order, then reverse to newest-first
        let mut commits: Vec<CommitRef> = (0..n)
            .map(|i| {
                let offset = if n > 1 {
                    i as i64 * total_secs / (n - 1) as i64
                } else {
                    0
                };
                let ts = start + chrono::Duration::seconds(offset);
                (format!("sha{:04}", i), ts)
            })
            .collect();
        commits.reverse(); // newest-first
        commits
    }

    #[test]
    fn select_samples_fifteen_commits_count_ten_returns_ten() {
        let commits = make_dated_commits(15);
        let result = select_samples(&commits, 10);
        assert_eq!(result.len(), 10, "should return exactly 10 samples");
    }

    #[test]
    fn select_samples_five_commits_count_ten_returns_all_five() {
        let commits = make_dated_commits(5);
        let result = select_samples(&commits, 10);
        assert_eq!(result.len(), 5, "count >= len should return all commits");
    }

    #[test]
    fn select_samples_empty_vec_returns_empty() {
        let commits: Vec<CommitRef> = Vec::new();
        let result = select_samples(&commits, 10);
        assert!(result.is_empty(), "empty input should produce empty output");
    }

    #[test]
    fn select_samples_count_zero_returns_empty() {
        let commits = make_dated_commits(15);
        let result = select_samples(&commits, 0);
        assert!(result.is_empty(), "count=0 should produce empty output");
    }

    #[test]
    fn select_samples_anchors_oldest_and_newest() {
        // With evenly-spaced commits, the first and last targets should
        // snap to the oldest and newest commits respectively.
        let commits = make_dated_commits(20);
        let result = select_samples(&commits, 5);
        let newest_sha = &commits[0].0;
        let oldest_sha = &commits[19].0;
        assert_eq!(result.first().unwrap(), oldest_sha, "first sample should be oldest");
        assert_eq!(result.last().unwrap(), newest_sha, "last sample should be newest");
    }

    #[test]
    fn select_samples_same_timestamp_collapses() {
        // All commits at identical timestamps: count < len to bypass the count>=len shortcut.
        // Every target time equals min_t == max_t, so all targets snap to the same commit
        // → only 1 unique result is returned.
        let ts = Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap();
        let commits: Vec<CommitRef> = (0..10).map(|i| (format!("sha{i:02}"), ts)).collect();
        let result = select_samples(&commits, 5);
        assert_eq!(result.len(), 1, "identical timestamps should collapse to 1 sample");
    }
}
