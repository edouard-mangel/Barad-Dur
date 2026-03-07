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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileComplexity {
    pub total_lines: usize,
    pub loc: usize,
    pub cyclomatic_complexity: u32,
    pub public_methods: u32,
    pub properties: u32,
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
    pub file_metrics: HashMap<PathBuf, FileComplexity>,
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
            file_metrics: HashMap::new(),
        }
    }

    /// Build all derived indexes from the core data.
    pub fn build_indexes(&mut self) {
        self.build_commits_by_author();
        self.build_commits_by_file();
        self.build_file_change_pairs();
    }

    fn build_commits_by_author(&mut self) {
        self.commits_by_author.clear();
        for commit in &self.commits {
            self.commits_by_author
                .entry(commit.author)
                .or_default()
                .push(commit.id.clone());
        }
    }

    fn build_commits_by_file(&mut self) {
        self.commits_by_file.clear();
        for commit in &self.commits {
            for fc in &commit.files_changed {
                self.commits_by_file
                    .entry(fc.path.clone())
                    .or_default()
                    .push(commit.id.clone());
            }
        }
    }

    fn build_file_change_pairs(&mut self) {
        use std::collections::HashMap as Map;
        let mut pair_counts: Map<(PathBuf, PathBuf), usize> = Map::new();

        for commit in &self.commits {
            let paths: Vec<&PathBuf> = commit.files_changed.iter().map(|fc| &fc.path).collect();
            // For each unique pair of files in this commit, increment counter
            for i in 0..paths.len() {
                for j in (i + 1)..paths.len() {
                    let (a, b) = if paths[i] < paths[j] {
                        (paths[i].clone(), paths[j].clone())
                    } else {
                        (paths[j].clone(), paths[i].clone())
                    };
                    *pair_counts.entry((a, b)).or_insert(0) += 1;
                }
            }
        }

        // Keep pairs with at least 3 co-changes
        self.file_change_pairs = pair_counts
            .into_iter()
            .filter(|&(_, count)| count >= 3)
            .map(|((a, b), count)| (a, b, count))
            .collect();

        // Sort by count descending for easy access
        self.file_change_pairs.sort_by(|a, b| b.2.cmp(&a.2));
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
        assert!(snapshot.file_metrics.is_empty());
    }

    #[test]
    fn file_metrics_starts_empty() {
        let snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        assert!(snapshot.file_metrics.is_empty());
    }

    fn make_commit(id: &str, author: AuthorId, files: Vec<&str>) -> Commit {
        Commit {
            id: id.to_string(),
            author,
            timestamp: Utc::now(),
            message: format!("Commit {}", id),
            files_changed: files
                .into_iter()
                .map(|f| FileChange {
                    path: PathBuf::from(f),
                    additions: 1,
                    deletions: 0,
                    change_type: ChangeType::Modified,
                })
                .collect(),
            is_merge: false,
            parent_count: 1,
        }
    }

    #[test]
    fn build_commits_by_author_groups_correctly() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.commits = vec![
            make_commit("a1", 0, vec!["f1"]),
            make_commit("a2", 0, vec!["f2"]),
            make_commit("b1", 1, vec!["f1"]),
        ];
        snapshot.build_indexes();

        assert_eq!(snapshot.commits_by_author[&0].len(), 2);
        assert_eq!(snapshot.commits_by_author[&1].len(), 1);
    }

    #[test]
    fn build_commits_by_file_maps_correctly() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.commits = vec![
            make_commit("c1", 0, vec!["a.rs", "b.rs"]),
            make_commit("c2", 1, vec!["a.rs"]),
        ];
        snapshot.build_indexes();

        assert_eq!(snapshot.commits_by_file[&PathBuf::from("a.rs")].len(), 2);
        assert_eq!(snapshot.commits_by_file[&PathBuf::from("b.rs")].len(), 1);
    }

    #[test]
    fn build_file_change_pairs_detects_coupling() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        // Files A and B change together in 5 commits, C only once
        snapshot.commits = vec![
            make_commit("c1", 0, vec!["a.rs", "b.rs"]),
            make_commit("c2", 0, vec!["a.rs", "b.rs"]),
            make_commit("c3", 0, vec!["a.rs", "b.rs"]),
            make_commit("c4", 0, vec!["a.rs", "b.rs", "c.rs"]),
            make_commit("c5", 0, vec!["a.rs", "b.rs"]),
        ];
        snapshot.build_indexes();

        // a.rs + b.rs should appear as a coupled pair (5 co-changes >= 3)
        let ab_pair = snapshot.file_change_pairs.iter().find(|(a, b, _)| {
            (a == &PathBuf::from("a.rs") && b == &PathBuf::from("b.rs"))
                || (a == &PathBuf::from("b.rs") && b == &PathBuf::from("a.rs"))
        });
        assert!(ab_pair.is_some(), "a.rs and b.rs should be coupled");
        assert_eq!(ab_pair.unwrap().2, 5);

        // a.rs + c.rs only co-changed once, should NOT appear (threshold is 3)
        let ac_pair = snapshot.file_change_pairs.iter().find(|(a, b, _)| {
            (a == &PathBuf::from("a.rs") && b == &PathBuf::from("c.rs"))
                || (a == &PathBuf::from("c.rs") && b == &PathBuf::from("a.rs"))
        });
        assert!(
            ac_pair.is_none(),
            "a.rs and c.rs should NOT be coupled (only 1 co-change)"
        );
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
        let decoded: RepoSnapshot = bincode::deserialize(&encoded).expect("Failed to deserialize");
        assert_eq!(decoded.name, snapshot.name);
        assert_eq!(decoded.default_branch, snapshot.default_branch);
        assert_eq!(decoded.path, snapshot.path);
    }
}
