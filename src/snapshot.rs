use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub type AuthorId = usize;
pub type CommitId = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: PathBuf,
    pub additions: u32,
    pub deletions: u32,
    pub change_type: ChangeType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub id: CommitId,
    pub author: AuthorId,
    pub timestamp: DateTime<Utc>,
    pub message: String,
    pub files_changed: Vec<FileChange>,
    pub is_merge: bool,
    pub parent_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub is_binary: bool,
    pub depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub id: AuthorId,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlameLine {
    pub author_id: AuthorId,
    pub commit_id: CommitId,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWindow {
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub default_months: u32,
}

impl Default for TimeWindow {
    fn default() -> Self {
        let now = Utc::now();
        TimeWindow {
            since: Some(now - Duration::days(180)),
            until: Some(now),
            default_months: 6,
        }
    }
}

impl TimeWindow {
    pub fn full_history() -> Self {
        TimeWindow {
            since: None,
            until: None,
            default_months: 0,
        }
    }

    pub fn contains(&self, timestamp: &DateTime<Utc>) -> bool {
        if let Some(since) = &self.since {
            if timestamp < since {
                return false;
            }
        }
        if let Some(until) = &self.until {
            if timestamp > until {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSnapshot {
    pub path: PathBuf,
    pub name: String,
    pub default_branch: String,
    pub time_window: TimeWindow,
    pub head_commit: String,
    pub created_at: DateTime<Utc>,

    pub commits: Vec<Commit>,
    pub files: Vec<FileEntry>,
    pub authors: Vec<Author>,
    pub blame_map: HashMap<PathBuf, Vec<BlameLine>>,

    pub commits_by_author: HashMap<AuthorId, Vec<CommitId>>,
    pub commits_by_file: HashMap<PathBuf, Vec<CommitId>>,
    pub file_change_pairs: Vec<(PathBuf, PathBuf, usize)>,
}

impl RepoSnapshot {
    pub fn new(path: PathBuf, name: String, branch: String, window: TimeWindow) -> Self {
        RepoSnapshot {
            path,
            name,
            default_branch: branch,
            time_window: window,
            head_commit: String::new(),
            created_at: Utc::now(),
            commits: Vec::new(),
            files: Vec::new(),
            authors: Vec::new(),
            blame_map: HashMap::new(),
            commits_by_author: HashMap::new(),
            commits_by_file: HashMap::new(),
            file_change_pairs: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_window_default_is_six_months() {
        let window = TimeWindow::default();
        assert_eq!(window.default_months, 6);
        assert!(window.since.is_some());
        assert!(window.until.is_some());

        let since = window.since.unwrap();
        let until = window.until.unwrap();
        let diff = until - since;
        // Should be approximately 180 days (allow some tolerance for test execution time)
        assert!(diff.num_days() >= 179 && diff.num_days() <= 181);
    }

    #[test]
    fn time_window_contains_timestamp_in_range() {
        let window = TimeWindow::default();
        let now = Utc::now();
        // A timestamp from 3 months ago should be within the default 6-month window
        let three_months_ago = now - Duration::days(90);
        assert!(window.contains(&three_months_ago));
    }

    #[test]
    fn time_window_excludes_timestamp_before_window() {
        let window = TimeWindow::default();
        let now = Utc::now();
        // A timestamp from 1 year ago should be outside the default 6-month window
        let one_year_ago = now - Duration::days(365);
        assert!(!window.contains(&one_year_ago));
    }

    #[test]
    fn time_window_excludes_timestamp_after_window() {
        let now = Utc::now();
        let window = TimeWindow {
            since: Some(now - Duration::days(180)),
            until: Some(now - Duration::days(30)),
            default_months: 6,
        };
        // Current time should be after the window's until
        assert!(!window.contains(&now));
    }

    #[test]
    fn time_window_full_history_contains_everything() {
        let window = TimeWindow::full_history();
        let ancient = Utc::now() - Duration::days(10000);
        let future = Utc::now() + Duration::days(10000);
        assert!(window.contains(&ancient));
        assert!(window.contains(&future));
    }

    #[test]
    fn repo_snapshot_new_creates_empty() {
        let snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test-repo".to_string(),
            "main".to_string(),
            TimeWindow::default(),
        );
        assert_eq!(snapshot.name, "test-repo");
        assert_eq!(snapshot.default_branch, "main");
        assert!(snapshot.commits.is_empty());
        assert!(snapshot.files.is_empty());
        assert!(snapshot.authors.is_empty());
        assert!(snapshot.blame_map.is_empty());
    }

    #[test]
    fn repo_snapshot_serialization_roundtrip() {
        let snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test-repo".to_string(),
            "main".to_string(),
            TimeWindow::default(),
        );
        let encoded = bincode::serialize(&snapshot).expect("Failed to serialize");
        let decoded: RepoSnapshot =
            bincode::deserialize(&encoded).expect("Failed to deserialize");
        assert_eq!(decoded.name, snapshot.name);
        assert_eq!(decoded.default_branch, snapshot.default_branch);
        assert_eq!(decoded.path, snapshot.path);
    }
}
