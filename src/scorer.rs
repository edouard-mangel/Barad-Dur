use serde::Serialize;

use crate::metrics::CategoryResult;
use crate::snapshot::RepoSnapshot;

/// Category weights for overall score computation.
const WEIGHTS: &[(&str, f64)] = &[
    ("Health", 0.30),
    ("Team", 0.30),
    ("Evolution", 0.20),
    ("Git Hygiene", 0.20),
];

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
}

pub fn build_report(
    snapshot: &RepoSnapshot,
    categories: Vec<CategoryResult>,
    remote_meta: Option<RemoteMeta>,
) -> AnalysisReport {
    let overall_score = compute_overall_score(&categories);
    let top_actions = generate_top_actions(&categories);

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
    }
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
    }
}
