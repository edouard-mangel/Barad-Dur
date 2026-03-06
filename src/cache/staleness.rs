use crate::snapshot::RepoSnapshot;

/// Check if the cached snapshot is stale by comparing HEAD commit hashes.
pub fn is_stale(cached: &RepoSnapshot, current_head: &str) -> bool {
    cached.head_commit != current_head
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::TimeWindow;
    use std::path::PathBuf;

    #[test]
    fn same_head_is_not_stale() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.head_commit = "abc123".to_string();
        assert!(!is_stale(&snapshot, "abc123"));
    }

    #[test]
    fn different_head_is_stale() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.head_commit = "abc123".to_string();
        assert!(is_stale(&snapshot, "def456"));
    }
}
