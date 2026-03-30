use crate::metrics::CategoryResult;

use super::types::ActionItem;

pub fn compute_overall_score_with_weights(
    categories: &[CategoryResult],
    weights: &[(&str, f64)],
) -> u32 {
    if categories.is_empty() {
        return 0;
    }

    let mut weighted_sum = 0.0;
    let mut total_weight = 0.0;

    for cat in categories {
        let weight = weights
            .iter()
            .find(|(name, _)| *name == cat.name)
            .map(|(_, w)| *w)
            .unwrap_or(0.25);

        weighted_sum += cat.score as f64 * weight;
        total_weight += weight;
    }

    if total_weight > 0.0 {
        (weighted_sum / total_weight).round() as u32
    } else {
        0
    }
}

pub(super) fn generate_top_actions(categories: &[CategoryResult]) -> Vec<ActionItem> {
    let mut low_metrics: Vec<(&str, &str, u32)> = Vec::new();

    for cat in categories {
        for metric in &cat.metrics {
            low_metrics.push((&cat.name, &metric.name, metric.score));
        }
    }

    low_metrics.sort_by_key(|m| m.2);

    low_metrics
        .iter()
        .take(3)
        .filter(|m| m.2 < 80)
        .map(|(cat, metric, score)| {
            let (target_tab, sort_by) = target_tab_for_metric(metric);
            ActionItem {
                text: format!(
                    "[{}] {} (score: {}) — {}",
                    cat,
                    metric,
                    score,
                    suggest_action(metric)
                ),
                target_tab: target_tab.map(String::from),
                sort_by: sort_by.map(String::from),
            }
        })
        .collect()
}

fn target_tab_for_metric(metric_name: &str) -> (Option<&'static str>, Option<&'static str>) {
    match metric_name {
        "Bus factor" => (Some("ownership"), Some("authors")),
        "God objects" => (Some("hotspots"), Some("complexity")),
        "Complex hotspots" => (Some("hotspots"), Some("complexity")),
        "Afferent coupling" => (Some("coupling"), None),
        "Efferent coupling" => (Some("coupling"), None),
        "Circular dependencies" => (Some("coupling"), None),
        "Knowledge distribution" => (Some("ownership"), None),
        "Ownership clarity" => (Some("ownership"), None),
        "Collaboration patterns" => (Some("ownership"), None),
        "Code age" => (Some("age"), None),
        "Growth trend" => (Some("trends"), None),
        "Refactoring ratio" => (Some("hotspots"), None),
        "Commit cadence" => (Some("trends"), None),
        _ => (None, None),
    }
}

fn suggest_action(metric_name: &str) -> &'static str {
    match metric_name {
        "Bus factor" => "Increase code review coverage and pair programming to spread knowledge",
        "God objects" => {
            "Break down large files by extracting responsibilities into smaller modules"
        }
        "Complex hotspots" => {
            "Prioritize refactoring files with both high complexity and high churn"
        }
        "Afferent coupling" => {
            "Reduce dependents on high-Ca files by introducing abstractions or splitting modules"
        }
        "Efferent coupling" => "Reduce imports by extracting shared interfaces or facades",
        "Circular dependencies" => {
            "Break circular imports by extracting shared types into a separate module"
        }
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

pub(super) fn score_commit_message(msg: &str) -> f64 {
    let trimmed = msg.trim();
    let len = trimmed.len();

    let mut score: f64 = 10.0; // base points for having any message

    // Length score: 0-40 points
    score += match len {
        0..=3 => 0.0,
        4..=10 => 10.0,
        11..=50 => 30.0,
        _ => 40.0,
    };

    // Conventional commit prefix: +30 points
    let prefixes = [
        "feat:",
        "fix:",
        "docs:",
        "style:",
        "refactor:",
        "perf:",
        "test:",
        "chore:",
        "ci:",
        "build:",
    ];
    if prefixes.iter().any(|p| trimmed.starts_with(p)) {
        score += 30.0;
    }

    // Descriptive (>20 chars or has body): +20 points
    if trimmed.contains('\n') || len > 20 {
        score += 20.0;
    }

    // Penalty for low-effort messages
    let lower = trimmed.to_lowercase();
    if lower == "wip" || lower == "fix" || lower == "update" || lower == "." {
        score = score.min(10.0);
    }

    score.min(100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{CategoryResult, MetricValue, RawValue};

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
    fn overall_score_weighted_average() {
        let categories = vec![
            make_category("Health", 80),
            make_category("Team", 60),
            make_category("Evolution", 70),
            make_category("Git Hygiene", 50),
            make_category("Coupling", 60),
        ];
        let score = compute_overall_score_with_weights(&categories, WEIGHTS);
        // 80*0.25 + 60*0.10 + 70*0.25 + 50*0.20 + 60*0.20 = 20+6+17.5+10+12 = 65.5 → 66
        assert_eq!(score, 66);
    }

    #[test]
    fn overall_score_single_category() {
        let categories = vec![make_category("Health", 75)];
        let score = compute_overall_score_with_weights(&categories, WEIGHTS);
        assert_eq!(score, 75);
    }

    #[test]
    fn overall_score_empty() {
        let score = compute_overall_score_with_weights(&[], WEIGHTS);
        assert_eq!(score, 0);
    }

    #[test]
    fn overall_score_custom_weights() {
        let categories = vec![
            make_category("Health", 100),
            make_category("Team", 0),
            make_category("Evolution", 0),
            make_category("Git Hygiene", 0),
        ];
        let weights = vec![
            ("Health", 1.0),
            ("Team", 0.0),
            ("Evolution", 0.0),
            ("Git Hygiene", 0.0),
        ];
        let score = compute_overall_score_with_weights(&categories, &weights);
        assert_eq!(score, 100);
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
        assert!(actions[0].text.contains("Knowledge distribution"));
    }

    #[test]
    fn score_commit_message_quality() {
        assert!(score_commit_message("feat: add login flow with validation") > 80.0);
        assert!(score_commit_message("fix: typo") > 40.0);
        assert!(score_commit_message("wip") < 20.0);
        assert!(score_commit_message("") < 15.0);
    }
}
