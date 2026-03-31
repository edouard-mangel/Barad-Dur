mod actions;
mod builders;
mod types;

pub use actions::compute_overall_score_with_weights;
pub use types::*;

use actions::generate_top_actions;
use builders::{
    build_author_cards, build_author_ownership, build_coupling_pairs, build_file_ages,
    build_hotspots,
};

use std::collections::HashMap;

use crate::metrics::CategoryResult;
use crate::snapshot::RepoSnapshot;

pub fn build_history_entry(
    report: &AnalysisReport,
    head: &str,
    source: Option<String>,
) -> HistoryEntry {
    let mut categories = HashMap::new();
    let mut metrics = HashMap::new();

    for cat in &report.categories {
        categories.insert(cat.name.clone(), cat.score);
        for m in &cat.metrics {
            metrics.insert(m.name.clone(), m.score);
        }
    }

    HistoryEntry {
        timestamp: chrono::Utc::now(),
        head: head.to_string(),
        overall_score: report.overall_score,
        categories,
        metrics,
        counts: HistoryCounts {
            commits: report.total_commits,
            files: report.total_files,
            authors: report.total_authors,
        },
        branch: report.branch.clone(),
        schema_version: 1,
        source,
    }
}

pub fn build_report(
    snapshot: &RepoSnapshot,
    categories: Vec<CategoryResult>,
    remote_meta: Option<RemoteMeta>,
    weights: &[(&str, f64)],
) -> AnalysisReport {
    let overall_score = compute_overall_score_with_weights(&categories, weights);
    let top_actions = generate_top_actions(&categories);
    let file_hotspots = build_hotspots(snapshot);
    let coupling_pairs = build_coupling_pairs(snapshot);
    let author_ownership = build_author_ownership(snapshot);
    let file_ages = build_file_ages(snapshot);
    let author_cards = build_author_cards(snapshot);

    AnalysisReport {
        repo_name: snapshot.name.clone(),
        branch: snapshot.default_branch.clone(),
        time_window_months: snapshot.time_window.default_months,
        total_commits: snapshot.commits.len(),
        total_authors: snapshot.authors.len(),
        total_files: snapshot.files.len(),
        overall_score,
        categories,
        top_actions,
        remote_meta,
        file_hotspots,
        coupling_pairs,
        author_ownership,
        file_ages,
        author_cards,
        history: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricValue;
    use crate::metrics::RawValue;
    use crate::snapshot::TimeWindow;

    const WEIGHTS: &[(&str, f64)] = &[
        ("Health", 0.25),
        ("Team", 0.10),
        ("Evolution", 0.25),
        ("Git Hygiene", 0.20),
        ("Coupling", 0.20),
    ];

    fn make_category(name: &str, score: u32) -> CategoryResult {
        CategoryResult {
            name: name.to_string(),
            score,
            metrics: vec![MetricValue {
                name: format!("{} metric", name),
                description: "test".to_string(),
                raw_value: RawValue::Integer(0),
                score,
            }],
        }
    }

    #[test]
    fn build_report_populates_fields() {
        let snapshot = RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test-repo".into(),
            "main".into(),
            TimeWindow::default(),
        );

        let categories = vec![make_category("Health", 80)];
        let report = build_report(&snapshot, categories, None, WEIGHTS);

        assert_eq!(report.repo_name, "test-repo");
        assert_eq!(report.branch, "main");
        assert_eq!(report.overall_score, 80);
        assert_eq!(report.categories.len(), 1);
        // New file analysis fields should be empty for an empty snapshot
        assert!(report.file_hotspots.is_empty());
        assert!(report.coupling_pairs.is_empty());
        assert!(report.author_ownership.is_empty());
        assert!(report.file_ages.is_empty());
    }

    #[test]
    fn build_hotspots_ranks_by_score() {
        use crate::snapshot::*;
        use builders::build_hotspots;
        let mut snapshot = RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "t".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.files = vec![
            FileEntry {
                path: "hot.rs".into(),
                size_bytes: 5000,
                is_binary: false,
                depth: 1,
                blob_oid: String::new(),
            },
            FileEntry {
                path: "cold.rs".into(),
                size_bytes: 100,
                is_binary: false,
                depth: 1,
                blob_oid: String::new(),
            },
        ];
        snapshot.commits_by_file.insert(
            "hot.rs".into(),
            (0..10).map(|i| CommitId(i)).collect(),
        );
        snapshot
            .commits_by_file
            .insert("cold.rs".into(), vec![CommitId(0)]);
        let hotspots = build_hotspots(&snapshot);
        assert_eq!(hotspots[0].path, "hot.rs");
        assert!(hotspots[0].hotspot_score > hotspots[1].hotspot_score);
    }

    #[test]
    fn build_coupling_pairs_computes_pct() {
        use crate::snapshot::*;
        use builders::build_coupling_pairs;
        let mut snapshot = RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "t".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.file_change_pairs = vec![("a.rs".into(), "b.rs".into(), 8)];
        snapshot
            .commits_by_file
            .insert("a.rs".into(), (0..10).map(|i| CommitId(i)).collect());
        snapshot
            .commits_by_file
            .insert("b.rs".into(), (0..10).map(|i| CommitId(i)).collect());
        let pairs = build_coupling_pairs(&snapshot);
        assert_eq!(pairs.len(), 1);
        assert!((pairs[0].coupling_pct - 80.0).abs() < 1.0);
    }

    #[test]
    fn build_file_ages_sorts_oldest_first() {
        use crate::snapshot::*;
        use builders::build_file_ages;
        use chrono::{Duration, Utc};
        let mut snapshot = RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "t".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let now = Utc::now();
        snapshot.files = vec![
            FileEntry {
                path: "new.rs".into(),
                size_bytes: 100,
                is_binary: false,
                depth: 1,
                blob_oid: String::new(),
            },
            FileEntry {
                path: "old.rs".into(),
                size_bytes: 100,
                is_binary: false,
                depth: 1,
                blob_oid: String::new(),
            },
        ];
        snapshot.commits = vec![
            Commit {
                id: CommitId(0),
                author: 0,
                timestamp: now - Duration::days(5),
                message: "".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
            Commit {
                id: CommitId(1),
                author: 0,
                timestamp: now - Duration::days(100),
                message: "".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
        ];
        snapshot
            .commits_by_file
            .insert("new.rs".into(), vec![CommitId(0)]);
        snapshot
            .commits_by_file
            .insert("old.rs".into(), vec![CommitId(1)]);
        let ages = build_file_ages(&snapshot);
        assert_eq!(ages[0].path, "old.rs");
        assert!(ages[0].days_since_modified > ages[1].days_since_modified);
    }

    #[test]
    fn history_entry_deserializes_without_branch_and_schema_version() {
        // Legacy NDJSON entries without branch or schema_version must deserialize cleanly
        // and populate serde defaults (empty string / 0).
        let legacy_json = r#"{"timestamp":"2024-01-01T00:00:00Z","head":"abc123","overall_score":75,"categories":{},"metrics":{},"counts":{"commits":10,"files":5,"authors":2}}"#;
        let entry: HistoryEntry = serde_json::from_str(legacy_json)
            .expect("legacy HistoryEntry without branch/schema_version should deserialize");
        assert_eq!(entry.branch, "", "branch should default to empty string");
        assert_eq!(
            entry.schema_version, 0,
            "schema_version should default to 0"
        );
        assert_eq!(entry.head, "abc123");
    }

    #[test]
    fn build_history_entry_contains_all_fields() {
        let snapshot = RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let categories = vec![make_category("Health", 80)];
        let report = build_report(&snapshot, categories, None, WEIGHTS);
        let entry = build_history_entry(&report, "abc123", None);

        assert_eq!(entry.head, "abc123");
        assert_eq!(entry.overall_score, report.overall_score);
        assert!(entry.categories.contains_key("Health"));
        assert_eq!(entry.counts.commits, 0);
        assert_eq!(entry.counts.files, 0);
        assert_eq!(entry.counts.authors, 0);
    }

    #[test]
    fn build_report_has_author_cards_field() {
        let snapshot = RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test-repo".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let categories = vec![make_category("Health", 80)];
        let report = build_report(&snapshot, categories, None, WEIGHTS);
        assert!(report.author_cards.is_empty());
    }

    #[test]
    fn build_author_cards_empty_snapshot() {
        use builders::build_author_cards;
        let snapshot = RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "t".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let cards = build_author_cards(&snapshot);
        assert!(cards.is_empty());
    }

    #[test]
    fn build_author_cards_from_snapshot() {
        use crate::snapshot::*;
        use builders::build_author_cards;
        use chrono::{Duration, Utc};

        let now = Utc::now();
        let mut snapshot = RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "t".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.authors = vec![
            Author {
                id: 0,
                name: "Alice".into(),
                email: "alice@x.com".into(),
            },
            Author {
                id: 1,
                name: "Bob".into(),
                email: "bob@x.com".into(),
            },
        ];
        snapshot.commits = vec![
            Commit {
                id: CommitId(0),
                author: 0,
                timestamp: now - Duration::days(10),
                message: "feat: add login flow with validation".into(),
                files_changed: vec![
                    FileChange {
                        path: "src/auth.rs".into(),
                        additions: 50,
                        deletions: 0,
                        change_type: ChangeType::Modified,
                    },
                    FileChange {
                        path: "src/main.rs".into(),
                        additions: 5,
                        deletions: 0,
                        change_type: ChangeType::Modified,
                    },
                ],
                is_merge: false,
                parent_count: 1,
            },
            Commit {
                id: CommitId(1),
                author: 0,
                timestamp: now - Duration::days(5),
                message: "fix: handle edge case in auth".into(),
                files_changed: vec![FileChange {
                    path: "src/auth.rs".into(),
                    additions: 10,
                    deletions: 2,
                    change_type: ChangeType::Modified,
                }],
                is_merge: false,
                parent_count: 1,
            },
            Commit {
                id: CommitId(2),
                author: 1,
                timestamp: now - Duration::days(100),
                message: "wip".into(),
                files_changed: vec![FileChange {
                    path: "src/main.rs".into(),
                    additions: 3,
                    deletions: 1,
                    change_type: ChangeType::Modified,
                }],
                is_merge: false,
                parent_count: 1,
            },
        ];
        snapshot.blame_map.insert(
            "src/auth.rs".into(),
            vec![
                BlameLine {
                    author_id: 0,
                    timestamp: now,
                },
                BlameLine {
                    author_id: 0,
                    timestamp: now,
                },
                BlameLine {
                    author_id: 0,
                    timestamp: now,
                },
                BlameLine {
                    author_id: 0,
                    timestamp: now,
                },
                BlameLine {
                    author_id: 1,
                    timestamp: now,
                },
            ],
        );
        snapshot.blame_map.insert(
            "src/main.rs".into(),
            vec![
                BlameLine {
                    author_id: 1,
                    timestamp: now,
                },
                BlameLine {
                    author_id: 1,
                    timestamp: now,
                },
                BlameLine {
                    author_id: 1,
                    timestamp: now,
                },
                BlameLine {
                    author_id: 0,
                    timestamp: now,
                },
                BlameLine {
                    author_id: 0,
                    timestamp: now,
                },
            ],
        );
        snapshot.build_indexes();

        let cards = build_author_cards(&snapshot);
        assert_eq!(cards.len(), 2);

        let alice = cards.iter().find(|c| c.name == "Alice").unwrap();
        assert_eq!(alice.commit_count, 2);
        assert_eq!(alice.files_owned, 1); // >50% of auth.rs
        assert_eq!(alice.lines_owned, 6); // 4 in auth.rs + 2 in main.rs
        assert!(alice.avg_commit_quality > 50.0);
        assert!(alice.days_since_active < 30);
        assert_eq!(alice.directories_touched, 1); // only "src"

        let bob = cards.iter().find(|c| c.name == "Bob").unwrap();
        assert_eq!(bob.commit_count, 1);
        assert_eq!(bob.files_owned, 1); // >50% of main.rs
        assert_eq!(bob.lines_owned, 4); // 1 in auth.rs + 3 in main.rs
        assert!(bob.avg_commit_quality < 30.0); // "wip"
        assert!(bob.days_since_active > 90);
    }
}
