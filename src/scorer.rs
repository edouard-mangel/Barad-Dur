use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::metrics::CategoryResult;
use crate::snapshot::RepoSnapshot;

/// Category weights for overall score computation.
const WEIGHTS: &[(&str, f64)] = &[
    ("Health", 0.30),
    ("Team", 0.30),
    ("Evolution", 0.20),
    ("Git Hygiene", 0.20),
];

#[derive(Debug, Clone, Serialize)]
pub struct HotspotFile {
    pub path: String,
    pub churn_count: usize,
    pub loc: usize,
    pub total_lines: usize,
    pub cyclomatic_complexity: u32,
    pub public_methods: u32,
    pub properties: u32,
    pub hotspot_score: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CouplingPair {
    pub file_a: String,
    pub file_b: String,
    pub co_changes: usize,
    pub coupling_pct: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthorShare {
    pub name: String,
    pub pct: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileOwnership {
    pub path: String,
    pub authors: Vec<AuthorShare>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileAge {
    pub path: String,
    pub last_modified: chrono::DateTime<chrono::Utc>,
    pub days_since_modified: i64,
}

/// Metadata about the remote repository origin (populated when a URL is given).
#[derive(Debug, Clone, Serialize)]
pub struct RemoteMeta {
    pub url: String,
    pub stars: Option<u64>,
    pub description: Option<String>,
    pub language: Option<String>,
    pub open_issues: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisReport {
    pub repo_name: String,
    pub branch: String,
    pub time_window_months: u32,
    pub total_commits: usize,
    pub total_authors: usize,
    pub total_files: usize,
    pub overall_score: u32,
    pub categories: Vec<CategoryResult>,
    pub top_actions: Vec<String>,
    pub remote_meta: Option<RemoteMeta>,
    pub file_hotspots: Vec<HotspotFile>,
    pub coupling_pairs: Vec<CouplingPair>,
    pub author_ownership: Vec<FileOwnership>,
    pub file_ages: Vec<FileAge>,
    pub history: Vec<HistoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryCounts {
    pub commits: usize,
    pub files: usize,
    pub authors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub head: String,
    pub overall_score: u32,
    pub categories: HashMap<String, u32>,
    pub metrics: HashMap<String, u32>,
    pub counts: HistoryCounts,
}

pub fn build_history_entry(report: &AnalysisReport, head: &str) -> HistoryEntry {
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
    }
}

pub fn build_report(
    snapshot: &RepoSnapshot,
    categories: Vec<CategoryResult>,
    remote_meta: Option<RemoteMeta>,
) -> AnalysisReport {
    let overall_score = compute_overall_score(&categories);
    let top_actions = generate_top_actions(&categories);
    let file_hotspots = build_hotspots(snapshot);
    let coupling_pairs = build_coupling_pairs(snapshot);
    let author_ownership = build_author_ownership(snapshot);
    let file_ages = build_file_ages(snapshot);

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

fn compute_overall_score(categories: &[CategoryResult]) -> u32 {
    if categories.is_empty() {
        return 0;
    }

    let mut weighted_sum = 0.0;
    let mut total_weight = 0.0;

    for cat in categories {
        let weight = WEIGHTS
            .iter()
            .find(|(name, _)| *name == cat.name)
            .map(|(_, w)| *w)
            .unwrap_or(0.25); // Default weight for unknown categories

        weighted_sum += cat.score as f64 * weight;
        total_weight += weight;
    }

    if total_weight > 0.0 {
        (weighted_sum / total_weight).round() as u32
    } else {
        0
    }
}

fn generate_top_actions(categories: &[CategoryResult]) -> Vec<String> {
    let mut low_metrics: Vec<(&str, &str, u32)> = Vec::new();

    for cat in categories {
        for metric in &cat.metrics {
            low_metrics.push((&cat.name, &metric.name, metric.score));
        }
    }

    // Sort by score ascending (worst first)
    low_metrics.sort_by_key(|m| m.2);

    // Take top 3 worst metrics and generate suggestions
    low_metrics
        .iter()
        .take(3)
        .filter(|m| m.2 < 80) // Only suggest for metrics below 80
        .map(|(cat, metric, score)| {
            format!(
                "[{}] {} (score: {}) — {}",
                cat,
                metric,
                score,
                suggest_action(metric)
            )
        })
        .collect()
}

fn suggest_action(metric_name: &str) -> &'static str {
    match metric_name {
        "Bus factor" => "Increase code review coverage and pair programming to spread knowledge",
        "Churn hotspots" => "Consider splitting frequently changed files into smaller modules",
        "Temporal coupling" => "Decouple tightly paired files by extracting shared interfaces",
        "Stale code" => "Review untouched files for removal or archival",
        "File complexity" => "Break down large files and reduce directory nesting depth",
        "Knowledge distribution" => "Encourage cross-team contributions and rotate ownership",
        "Contributor activity" => "Onboard more active contributors or check team health",
        "Ownership clarity" => "Assign clear code owners via CODEOWNERS file",
        "Collaboration patterns" => "Break directory silos through cross-functional reviews",
        "Merge patterns" => "Review branching strategy for healthier merge patterns",
        "Growth trend" => "Monitor growth rate and plan for sustainable development",
        "Refactoring ratio" => "Balance new feature work with refactoring of existing code",
        "Code age" => "Plan modernization of oldest code sections",
        "Commit cadence" => "Establish regular commit patterns and avoid large batches",
        "Commit message quality" => "Adopt conventional commits or enforce message guidelines",
        "History cleanliness" => {
            "Clean up merge strategy and enforce linear history where possible"
        }
        "Gitignore coverage" => "Add suspicious files to .gitignore and remove from tracking",
        _ => "Review and improve this metric",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricValue;
    use crate::metrics::RawValue;
    use crate::snapshot::TimeWindow;

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
    fn overall_score_weighted_average() {
        let categories = vec![
            make_category("Health", 80),
            make_category("Team", 60),
            make_category("Evolution", 70),
            make_category("Git Hygiene", 50),
        ];

        let score = compute_overall_score(&categories);
        // 80*0.3 + 60*0.3 + 70*0.2 + 50*0.2 = 24+18+14+10 = 66
        assert_eq!(score, 66);
    }

    #[test]
    fn overall_score_single_category() {
        let categories = vec![make_category("Health", 75)];
        let score = compute_overall_score(&categories);
        assert_eq!(score, 75);
    }

    #[test]
    fn overall_score_empty() {
        let score = compute_overall_score(&[]);
        assert_eq!(score, 0);
    }

    #[test]
    fn top_actions_picks_worst() {
        let categories = vec![
            CategoryResult {
                name: "Health".to_string(),
                score: 50,
                metrics: vec![
                    MetricValue {
                        name: "Bus factor".to_string(),
                        description: "bad".to_string(),
                        raw_value: RawValue::Integer(1),
                        score: 20,
                    },
                    MetricValue {
                        name: "Churn hotspots".to_string(),
                        description: "ok".to_string(),
                        raw_value: RawValue::Count(0),
                        score: 90,
                    },
                ],
            },
            CategoryResult {
                name: "Team".to_string(),
                score: 40,
                metrics: vec![MetricValue {
                    name: "Knowledge distribution".to_string(),
                    description: "bad".to_string(),
                    raw_value: RawValue::Float(0.8),
                    score: 15,
                }],
            },
        ];

        let actions = generate_top_actions(&categories);
        assert!(!actions.is_empty());
        // Worst metric (score 15) should be first
        assert!(actions[0].contains("Knowledge distribution"));
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
        let report = build_report(&snapshot, categories, None);

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
    fn build_history_entry_contains_all_fields() {
        let snapshot = RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let categories = vec![make_category("Health", 80)];
        let report = build_report(&snapshot, categories, None);
        let entry = build_history_entry(&report, "abc123");

        assert_eq!(entry.head, "abc123");
        assert_eq!(entry.overall_score, report.overall_score);
        assert!(entry.categories.contains_key("Health"));
        assert_eq!(entry.counts.commits, 0);
        assert_eq!(entry.counts.files, 0);
        assert_eq!(entry.counts.authors, 0);
    }
}
