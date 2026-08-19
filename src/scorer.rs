mod actions;
mod audit;
mod builders;
mod types;

pub use actions::compute_overall_score_with_weights;
pub use types::*;

use actions::{generate_coupling_actions, generate_top_actions};
use builders::{
    build_author_cards, build_author_ownership, build_coupling_pairs, build_file_ages,
    build_hotspots, build_import_cycles, build_import_edges, build_per_file_coupling,
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
        // Unscored metrics (insufficient data) carry no trend signal.
        for m in &cat.metrics {
            if let Some(score) = m.score {
                metrics.insert(m.name.clone(), score);
            }
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
            content_coupling: report.coupling_finding_counts.map(|c| c.content),
            common_coupling: report.coupling_finding_counts.map(|c| c.common),
            inheritance_coupling: report.coupling_finding_counts.map(|c| c.inheritance),
            control_coupling: report.coupling_finding_counts.map(|c| c.control),
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
    thresholds: &crate::config::Thresholds,
) -> AnalysisReport {
    let coupling = &thresholds.coupling;
    let overall_score = compute_overall_score_with_weights(&categories, weights);
    let top_actions = generate_top_actions(&categories);
    let coupling_actions = generate_coupling_actions(snapshot, coupling);
    let file_hotspots = build_hotspots(snapshot, coupling);
    let coupling_pairs = build_coupling_pairs(snapshot, coupling.component_depth);
    let author_ownership = build_author_ownership(snapshot);
    let file_ages = build_file_ages(snapshot);
    let author_cards = build_author_cards(snapshot);

    let audit = Some(audit::build_audit_report(snapshot));
    let per_file_coupling = build_per_file_coupling(snapshot);
    let import_edges = build_import_edges(snapshot);
    let import_cycles = build_import_cycles(snapshot);
    let coupling_finding_counts =
        crate::metrics::coupling::pressman_finding_counts(snapshot, coupling);
    let call_graph = crate::metrics::callgraph::call_graph_report(snapshot, &thresholds.health);

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
        coupling_actions,
        remote_meta,
        file_hotspots,
        coupling_pairs,
        author_ownership,
        file_ages,
        author_cards,
        history: Vec::new(),
        dep_ecosystem_reports: Vec::new(),
        audit,
        per_file_coupling,
        import_edges,
        import_cycles,
        coupling_finding_counts,
        call_graph,
        score_thresholds: ScoreThresholds::default(),
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
                score: Some(score),
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
        let report = build_report(
            &snapshot,
            categories,
            None,
            WEIGHTS,
            &crate::config::Thresholds::default(),
        );

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
    fn build_report_populates_audit_field() {
        let snapshot = RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test-repo".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let categories = vec![make_category("Health", 80)];
        let report = build_report(
            &snapshot,
            categories,
            None,
            WEIGHTS,
            &crate::config::Thresholds::default(),
        );
        assert!(
            report.audit.is_some(),
            "build_report must always set audit to Some(...)"
        );
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
        snapshot
            .commits_by_file
            .insert("hot.rs".into(), (0..10).map(CommitId).collect());
        snapshot
            .commits_by_file
            .insert("cold.rs".into(), vec![CommitId(0)]);
        let hotspots = build_hotspots(&snapshot, &crate::config::CouplingThresholds::default());
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
            .insert("a.rs".into(), (0..10).map(CommitId).collect());
        snapshot
            .commits_by_file
            .insert("b.rs".into(), (0..10).map(CommitId).collect());
        let pairs = build_coupling_pairs(&snapshot, 2);
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
        let report = build_report(
            &snapshot,
            categories,
            None,
            WEIGHTS,
            &crate::config::Thresholds::default(),
        );
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
        let report = build_report(
            &snapshot,
            categories,
            None,
            WEIGHTS,
            &crate::config::Thresholds::default(),
        );
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
    fn build_report_populates_per_file_coupling_empty_snapshot() {
        let snapshot = RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test-repo".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let categories = vec![make_category("Health", 80)];
        let report = build_report(
            &snapshot,
            categories,
            None,
            WEIGHTS,
            &crate::config::Thresholds::default(),
        );
        assert!(
            report.per_file_coupling.is_empty(),
            "per_file_coupling should be empty for an empty snapshot"
        );
    }

    #[test]
    fn build_report_populates_import_edges_empty_snapshot() {
        let snapshot = RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test-repo".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let categories = vec![make_category("Health", 80)];
        let report = build_report(
            &snapshot,
            categories,
            None,
            WEIGHTS,
            &crate::config::Thresholds::default(),
        );
        assert!(
            report.import_edges.is_empty(),
            "import_edges should be empty for an empty snapshot"
        );
    }

    fn report_with_detection() -> AnalysisReport {
        let mut snapshot = RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.files = vec![crate::metrics::testutil::make_file("src/a.rs")];
        snapshot.file_metrics.insert(
            std::path::PathBuf::from("src/a.rs"),
            crate::snapshot::FileComplexity::default(),
        );
        build_report(
            &snapshot,
            vec![make_category("Health", 80)],
            None,
            WEIGHTS,
            &crate::config::Thresholds::default(),
        )
    }

    fn report_without_detection() -> AnalysisReport {
        let snapshot = RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        build_report(
            &snapshot,
            vec![make_category("Health", 80)],
            None,
            WEIGHTS,
            &crate::config::Thresholds::default(),
        )
    }

    #[test]
    fn report_embeds_call_graph_when_call_records_exist() {
        let mut snapshot = crate::metrics::testutil::make_snapshot();
        snapshot
            .file_metrics
            .insert("a.ts".into(), crate::snapshot::FileComplexity::default());
        snapshot.call_records = vec![crate::snapshot::CallRecord {
            path: "a.ts".into(),
            caller: "f".into(),
            callee: crate::snapshot::CalleeRef::SameFile("g".into()),
            count: 2,
        }];
        let report = build_report(
            &snapshot,
            vec![make_category("Health", 80)],
            None,
            WEIGHTS,
            &crate::config::Thresholds::default(),
        );
        let cg = report.call_graph.expect("call_graph section");
        assert!((cg.resolution_rate - 1.0).abs() < f64::EPSILON);
        assert_eq!(cg.edges_same_file, 1);
        assert_eq!(cg.edges_resolved, 0);
        assert_eq!(cg.edges_unresolved, 0);
    }

    #[test]
    fn report_call_graph_none_for_backfill_style_snapshot() {
        let report = report_without_detection();
        assert_eq!(report.call_graph, None);
    }

    #[test]
    fn report_embeds_finding_counts_when_detection_ran() {
        let report = report_with_detection();
        assert_eq!(
            report.coupling_finding_counts,
            Some(crate::scorer::CouplingFindingCounts {
                content: 0,
                common: 0,
                inheritance: 0,
                control: 0
            })
        );
    }

    #[test]
    fn report_finding_counts_none_for_backfill_style_snapshot() {
        let report = report_without_detection();
        assert_eq!(report.coupling_finding_counts, None);
    }

    #[test]
    fn history_entry_carries_finding_counts() {
        let report = report_with_detection();
        let entry = build_history_entry(&report, "abc123", None);
        assert_eq!(entry.counts.content_coupling, Some(0));
        assert_eq!(entry.counts.common_coupling, Some(0));
        assert_eq!(entry.counts.inheritance_coupling, Some(0));
        assert_eq!(entry.counts.control_coupling, Some(0));
    }

    #[test]
    fn history_entry_counts_none_without_detection() {
        let report = report_without_detection();
        let entry = build_history_entry(&report, "abc123", Some("backfill".into()));
        assert_eq!(entry.counts.content_coupling, None);
        assert_eq!(entry.counts.common_coupling, None);
        assert_eq!(entry.counts.inheritance_coupling, None);
        assert_eq!(entry.counts.control_coupling, None);
    }

    #[test]
    fn history_counts_old_json_reads_inheritance_as_none() {
        let json = r#"{"commits":1,"files":2,"authors":1,"content_coupling":0,"common_coupling":0,"control_coupling":0}"#;
        let counts: crate::scorer::HistoryCounts = serde_json::from_str(json).unwrap();
        assert_eq!(
            counts.inheritance_coupling, None,
            "pre-M7 entry: not measured, never Some(0)"
        );
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
                BlameLine::new(0, now),
                BlameLine::new(0, now),
                BlameLine::new(0, now),
                BlameLine::new(0, now),
                BlameLine::new(1, now),
            ],
        );
        snapshot.blame_map.insert(
            "src/main.rs".into(),
            vec![
                BlameLine::new(1, now),
                BlameLine::new(1, now),
                BlameLine::new(1, now),
                BlameLine::new(0, now),
                BlameLine::new(0, now),
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
