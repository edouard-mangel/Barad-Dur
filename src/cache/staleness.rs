use crate::snapshot::{RepoSnapshot, TimeWindow};
use chrono::{DateTime, Duration, Utc};

/// Check if the cached snapshot is stale relative to the current HEAD and requested time window.
///
/// Two time windows are considered equivalent when:
/// - Both `since` fields are None, or both are Some with values within 1 hour
/// - Same for `until`
///
/// The 1-hour tolerance lets repeated runs of the same relative spec (e.g. `--since 3months`)
/// reuse the cache; specs that differ by days or more (e.g. `3months` vs `9months`) are treated
/// as distinct and will trigger a fresh collection.
pub(crate) fn is_stale(cached: &RepoSnapshot, current_head: &str, requested: &TimeWindow) -> bool {
    if cached.head_commit != current_head {
        return true;
    }
    !windows_equivalent(&cached.time_window, requested)
}

fn windows_equivalent(a: &TimeWindow, b: &TimeWindow) -> bool {
    timestamps_equivalent(a.since.as_ref(), b.since.as_ref())
        && timestamps_equivalent(a.until.as_ref(), b.until.as_ref())
}

fn timestamps_equivalent(a: Option<&DateTime<Utc>>, b: Option<&DateTime<Utc>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => (*a - *b).abs() < Duration::hours(1),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::TimeWindow;
    use chrono::Utc;
    use std::path::PathBuf;

    fn snapshot_with(window: TimeWindow, head: &str) -> RepoSnapshot {
        let mut s = RepoSnapshot::new(PathBuf::from("/tmp"), "test".into(), "main".into(), window);
        s.head_commit = head.to_string();
        s
    }

    #[test]
    fn same_head_same_window_is_not_stale() {
        let w = TimeWindow::default();
        let s = snapshot_with(w.clone(), "abc123");
        assert!(!is_stale(&s, "abc123", &w));
    }

    #[test]
    fn different_head_is_stale() {
        let w = TimeWindow::default();
        let s = snapshot_with(w.clone(), "abc123");
        assert!(is_stale(&s, "def456", &w));
    }

    #[test]
    fn same_head_different_window_is_stale() {
        let now = Utc::now();
        let six_months = TimeWindow {
            since: Some(now - chrono::Duration::days(180)),
            until: Some(now),
            default_months: 6,
        };
        let three_months = TimeWindow {
            since: Some(now - chrono::Duration::days(90)),
            until: Some(now),
            default_months: 0,
        };
        let s = snapshot_with(six_months, "abc123");
        assert!(is_stale(&s, "abc123", &three_months));
    }

    #[test]
    fn full_history_vs_default_is_stale() {
        let s = snapshot_with(TimeWindow::default(), "abc123");
        assert!(is_stale(&s, "abc123", &TimeWindow::full_history()));
    }

    #[test]
    fn full_history_reused_across_runs() {
        let s = snapshot_with(TimeWindow::full_history(), "abc123");
        assert!(!is_stale(&s, "abc123", &TimeWindow::full_history()));
    }

    #[test]
    fn same_relative_spec_within_tolerance_is_not_stale() {
        let now = Utc::now();
        let w1 = TimeWindow {
            since: Some(now - chrono::Duration::days(90)),
            until: Some(now),
            default_months: 0,
        };
        // Simulate a second run 30 minutes later with the same spec
        let w2 = TimeWindow {
            since: Some(now - chrono::Duration::days(90) + chrono::Duration::minutes(30)),
            until: Some(now + chrono::Duration::minutes(30)),
            default_months: 0,
        };
        let s = snapshot_with(w1, "abc123");
        assert!(!is_stale(&s, "abc123", &w2));
    }
}
