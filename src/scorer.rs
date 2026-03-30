mod actions;
mod types;

pub use actions::compute_overall_score_with_weights;
pub use types::*;

use actions::{generate_top_actions, score_commit_message};

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

fn build_hotspots(snapshot: &RepoSnapshot) -> Vec<HotspotFile> {
    let mut files: Vec<HotspotFile> = snapshot
        .files
        .iter()
        .filter(|f| !f.is_binary)
        .map(|f| {
            let churn = snapshot
                .commits_by_file
                .get(&f.path)
                .map(|v| v.len())
                .unwrap_or(0);
            let metrics = snapshot
                .file_metrics
                .get(&f.path)
                .cloned()
                .unwrap_or_default();
            HotspotFile {
                path: f.path.to_string_lossy().to_string(),
                churn_count: churn,
                loc: metrics.loc,
                total_lines: metrics.total_lines,
                cyclomatic_complexity: metrics.cyclomatic_complexity,
                public_methods: metrics.public_methods,
                properties: metrics.properties,
                hotspot_score: 0.0,
            }
        })
        .collect();

    if files.is_empty() {
        return files;
    }

    let max_churn = files
        .iter()
        .map(|f| f.churn_count)
        .max()
        .unwrap_or(1)
        .max(1);
    let max_cc = files
        .iter()
        .map(|f| f.cyclomatic_complexity as usize)
        .max()
        .unwrap_or(1)
        .max(1);
    let max_loc = files.iter().map(|f| f.loc).max().unwrap_or(1).max(1);

    for f in &mut files {
        let churn_norm = f.churn_count as f64 / max_churn as f64;
        let cc_norm = f.cyclomatic_complexity as f64 / max_cc as f64;
        let loc_norm = f.loc as f64 / max_loc as f64;
        f.hotspot_score = (churn_norm * 0.5 + cc_norm * 0.3 + loc_norm * 0.2) * 100.0;
    }

    files.sort_by(|a, b| b.hotspot_score.partial_cmp(&a.hotspot_score).unwrap());
    files
}

fn build_coupling_pairs(snapshot: &RepoSnapshot) -> Vec<CouplingPair> {
    snapshot
        .file_change_pairs
        .iter()
        .map(|(a, b, co)| {
            let a_changes = snapshot
                .commits_by_file
                .get(a)
                .map(|v| v.len())
                .unwrap_or(0);
            let b_changes = snapshot
                .commits_by_file
                .get(b)
                .map(|v| v.len())
                .unwrap_or(0);
            let min_changes = a_changes.min(b_changes).max(1);
            let coupling_pct = (*co as f64 / min_changes as f64 * 100.0).min(100.0);
            CouplingPair {
                file_a: a.to_string_lossy().to_string(),
                file_b: b.to_string_lossy().to_string(),
                co_changes: *co,
                coupling_pct,
            }
        })
        .collect()
}

fn build_author_ownership(snapshot: &RepoSnapshot) -> Vec<FileOwnership> {
    snapshot
        .blame_map
        .iter()
        .map(|(path, lines)| {
            let mut author_counts: HashMap<usize, usize> = HashMap::new();
            for line in lines {
                *author_counts.entry(line.author_id).or_insert(0) += 1;
            }
            let total = lines.len().max(1);
            let mut authors: Vec<AuthorShare> = author_counts
                .into_iter()
                .map(|(id, count)| {
                    let name = snapshot
                        .authors
                        .get(id)
                        .map(|a| a.name.clone())
                        .unwrap_or_else(|| format!("author-{}", id));
                    AuthorShare {
                        name,
                        pct: count as f64 / total as f64 * 100.0,
                    }
                })
                .collect();
            authors.sort_by(|a, b| b.pct.partial_cmp(&a.pct).unwrap());
            FileOwnership {
                path: path.to_string_lossy().to_string(),
                authors,
            }
        })
        .collect()
}

fn build_file_ages(snapshot: &RepoSnapshot) -> Vec<FileAge> {
    let now = chrono::Utc::now();
    let fallback = snapshot.created_at - chrono::Duration::days(365 * 5);
    let mut ages: Vec<FileAge> = snapshot
        .files
        .iter()
        .filter(|f| !f.is_binary)
        .map(|f| {
            let last_modified = snapshot
                .commits_by_file
                .get(&f.path)
                .and_then(|commit_ids| {
                    commit_ids
                        .iter()
                        .filter_map(|cid| snapshot.commits.iter().find(|c| &c.id == cid))
                        .map(|c| c.timestamp)
                        .max()
                })
                .unwrap_or(fallback);
            let days = (now - last_modified).num_days().max(0);
            FileAge {
                path: f.path.to_string_lossy().to_string(),
                last_modified,
                days_since_modified: days,
            }
        })
        .collect();
    ages.sort_by(|a, b| b.days_since_modified.cmp(&a.days_since_modified));
    ages
}

fn build_author_cards(snapshot: &RepoSnapshot) -> Vec<AuthorCard> {
    let now = chrono::Utc::now();

    // Pre-compute per-author blame lines across all files
    let mut author_lines: HashMap<usize, usize> = HashMap::new();
    let mut author_file_pcts: HashMap<usize, Vec<(String, f64)>> = HashMap::new();
    let mut author_files_owned: HashMap<usize, usize> = HashMap::new();

    for (path, blame_lines) in &snapshot.blame_map {
        let total = blame_lines.len().max(1);
        let mut counts: HashMap<usize, usize> = HashMap::new();
        for bl in blame_lines {
            *counts.entry(bl.author_id).or_insert(0) += 1;
        }
        for (&author_id, &count) in &counts {
            *author_lines.entry(author_id).or_insert(0) += count;
            let pct = count as f64 / total as f64 * 100.0;
            author_file_pcts
                .entry(author_id)
                .or_default()
                .push((path.to_string_lossy().to_string(), pct));
            if pct > 50.0 {
                *author_files_owned.entry(author_id).or_insert(0) += 1;
            }
        }
    }

    let mut cards: Vec<AuthorCard> = snapshot
        .authors
        .iter()
        .map(|author| {
            let commit_ids = snapshot
                .commits_by_author
                .get(&author.id)
                .cloned()
                .unwrap_or_default();

            let author_commits: Vec<&crate::snapshot::Commit> = commit_ids
                .iter()
                .filter_map(|cid| snapshot.commits.iter().find(|c| &c.id == cid))
                .collect();

            let commit_count = author_commits.len();

            let last_active = author_commits
                .iter()
                .map(|c| c.timestamp)
                .max()
                .unwrap_or(snapshot.created_at);
            let days_since_active = (now - last_active).num_days().max(0);

            let avg_commit_quality = if author_commits.is_empty() {
                0.0
            } else {
                let total_q: f64 = author_commits
                    .iter()
                    .map(|c| score_commit_message(&c.message))
                    .sum();
                total_q / author_commits.len() as f64
            };

            let mut file_pcts = author_file_pcts
                .get(&author.id)
                .cloned()
                .unwrap_or_default();
            file_pcts.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let top_files: Vec<String> = file_pcts.iter().take(5).map(|(p, _)| p.clone()).collect();

            let mut dirs = std::collections::HashSet::new();
            for commit in &author_commits {
                for fc in &commit.files_changed {
                    if let Some(parent) = fc.path.parent() {
                        dirs.insert(parent.to_string_lossy().to_string());
                    }
                }
            }

            AuthorCard {
                name: author.name.clone(),
                email: author.email.clone(),
                commit_count,
                files_owned: *author_files_owned.get(&author.id).unwrap_or(&0),
                lines_owned: *author_lines.get(&author.id).unwrap_or(&0),
                avg_commit_quality,
                top_files,
                last_active,
                days_since_active,
                directories_touched: dirs.len(),
            }
        })
        .collect();

    cards.sort_by(|a, b| b.commit_count.cmp(&a.commit_count));
    cards
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
            (0..10).map(|i| format!("c{}", i)).collect(),
        );
        snapshot
            .commits_by_file
            .insert("cold.rs".into(), vec!["c0".into()]);
        let hotspots = build_hotspots(&snapshot);
        assert_eq!(hotspots[0].path, "hot.rs");
        assert!(hotspots[0].hotspot_score > hotspots[1].hotspot_score);
    }

    #[test]
    fn build_coupling_pairs_computes_pct() {
        use crate::snapshot::*;
        let mut snapshot = RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "t".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.file_change_pairs = vec![("a.rs".into(), "b.rs".into(), 8)];
        snapshot
            .commits_by_file
            .insert("a.rs".into(), (0..10).map(|i| format!("c{}", i)).collect());
        snapshot
            .commits_by_file
            .insert("b.rs".into(), (0..10).map(|i| format!("c{}", i)).collect());
        let pairs = build_coupling_pairs(&snapshot);
        assert_eq!(pairs.len(), 1);
        assert!((pairs[0].coupling_pct - 80.0).abs() < 1.0);
    }

    #[test]
    fn build_file_ages_sorts_oldest_first() {
        use crate::snapshot::*;
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
                id: "c1".into(),
                author: 0,
                timestamp: now - Duration::days(5),
                message: "".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
            Commit {
                id: "c2".into(),
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
            .insert("new.rs".into(), vec!["c1".into()]);
        snapshot
            .commits_by_file
            .insert("old.rs".into(), vec!["c2".into()]);
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
                id: "c1".into(),
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
                id: "c2".into(),
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
                id: "c3".into(),
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
                    commit_id: "c1".into(),
                    timestamp: now,
                },
                BlameLine {
                    author_id: 0,
                    commit_id: "c1".into(),
                    timestamp: now,
                },
                BlameLine {
                    author_id: 0,
                    commit_id: "c1".into(),
                    timestamp: now,
                },
                BlameLine {
                    author_id: 0,
                    commit_id: "c1".into(),
                    timestamp: now,
                },
                BlameLine {
                    author_id: 1,
                    commit_id: "c3".into(),
                    timestamp: now,
                },
            ],
        );
        snapshot.blame_map.insert(
            "src/main.rs".into(),
            vec![
                BlameLine {
                    author_id: 1,
                    commit_id: "c3".into(),
                    timestamp: now,
                },
                BlameLine {
                    author_id: 1,
                    commit_id: "c3".into(),
                    timestamp: now,
                },
                BlameLine {
                    author_id: 1,
                    commit_id: "c3".into(),
                    timestamp: now,
                },
                BlameLine {
                    author_id: 0,
                    commit_id: "c1".into(),
                    timestamp: now,
                },
                BlameLine {
                    author_id: 0,
                    commit_id: "c1".into(),
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
