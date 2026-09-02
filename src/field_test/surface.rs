use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// How many hotspots enter the baseline. Beyond the top of the ranking the
/// ordering is noise rather than signal.
const HOTSPOT_LIMIT: usize = 20;

/// The part of a report that represents a decision the tool is making.
/// Deliberately smaller than the full report: a baseline that diffs on
/// every run is a baseline everybody learns to ignore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionSurface {
    pub overall_score: i64,
    pub total_files: i64,
    pub total_commits: i64,
    pub total_authors: i64,
    pub score_thresholds: BTreeMap<String, i64>,
    pub coupling_finding_counts: BTreeMap<String, i64>,
    pub categories: Vec<CategorySurface>,
    pub actions: Vec<ActionSurface>,
    pub top_hotspots: Vec<String>,
}

impl DecisionSurface {
    /// The surface of a repository that has never been baselined: no scores,
    /// no recommendations, nothing to have seen before. Auditing against this
    /// makes every current recommendation *new*, which is what a first-ever
    /// run of a repository should look like to a reviewer.
    pub fn empty() -> Self {
        Self {
            overall_score: 0,
            total_files: 0,
            total_commits: 0,
            total_authors: 0,
            score_thresholds: BTreeMap::new(),
            coupling_finding_counts: BTreeMap::new(),
            categories: Vec::new(),
            actions: Vec::new(),
            top_hotspots: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategorySurface {
    pub name: String,
    pub score: Option<i64>,
    pub metrics: Vec<MetricSurface>,
}

/// `score: None` means *unscored* — a metric the analysis could not
/// compute. Collapsing it to a number is the regression this guards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricSurface {
    pub name: String,
    pub score: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ActionSurface {
    pub target_tab: String,
    pub text: String,
}

fn int_at(report: &Value, key: &str) -> i64 {
    report.get(key).and_then(Value::as_i64).unwrap_or_default()
}

fn int_map(report: &Value, key: &str) -> BTreeMap<String, i64> {
    report
        .get(key)
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| v.as_i64().map(|n| (k.clone(), n)))
                .collect()
        })
        .unwrap_or_default()
}

fn actions_from(report: &Value, key: &str) -> Vec<ActionSurface> {
    report
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|a| ActionSurface {
                    target_tab: a
                        .get("target_tab")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    text: a
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Reduce a full report to the decisions it expresses.
pub fn extract_surface(report: &Value) -> DecisionSurface {
    let categories = report
        .get("categories")
        .and_then(Value::as_array)
        .map(|cats| {
            cats.iter()
                .map(|c| CategorySurface {
                    name: c
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    score: c.get("score").and_then(Value::as_i64),
                    metrics: c
                        .get("metrics")
                        .and_then(Value::as_array)
                        .map(|ms| {
                            ms.iter()
                                .map(|m| MetricSurface {
                                    name: m
                                        .get("name")
                                        .and_then(Value::as_str)
                                        .unwrap_or_default()
                                        .to_string(),
                                    score: m.get("score").and_then(Value::as_i64),
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();

    // Both action lists are recommendations to a human; sorting makes the
    // merge order independent of how the renderer happened to emit them.
    let actions = {
        let mut merged = actions_from(report, "top_actions");
        merged.extend(actions_from(report, "coupling_actions"));
        merged.sort();
        merged
    };

    let top_hotspots = report
        .get("file_hotspots")
        .and_then(Value::as_array)
        .map(|hs| {
            hs.iter()
                .take(HOTSPOT_LIMIT)
                .filter_map(|h| h.get("path").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    DecisionSurface {
        overall_score: int_at(report, "overall_score"),
        total_files: int_at(report, "total_files"),
        total_commits: int_at(report, "total_commits"),
        total_authors: int_at(report, "total_authors"),
        score_thresholds: int_map(report, "score_thresholds"),
        coupling_finding_counts: int_map(report, "coupling_finding_counts"),
        categories,
        actions,
        top_hotspots,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_report() -> serde_json::Value {
        json!({
            "overall_score": 55,
            "total_files": 203,
            "total_commits": 2287,
            "total_authors": 27,
            "score_thresholds": { "good_min": 71, "warn_min": 41 },
            "coupling_finding_counts": {
                "common": 4, "content": 2, "control": 5, "inheritance": 0
            },
            "categories": [
                { "name": "Health", "score": 40, "metrics": [
                    { "name": "Bus factor", "score": 25,
                      "description": "prose", "raw_value": { "Count": 1 } },
                    { "name": "Churn-ownership risk", "score": null,
                      "description": "prose", "raw_value": null }
                ]}
            ],
            "top_actions": [
                { "target_tab": "ownership", "text": "[Team] Knowledge distribution" }
            ],
            "coupling_actions": [
                { "target_tab": "coupling", "text": "[Coupling] mod.rs — 2 finding(s)" }
            ],
            "file_hotspots": [
                { "path": "crates/ignore/src/walk.rs", "hotspot_score": 65.1 },
                { "path": "crates/core/main.rs", "hotspot_score": 40.0 }
            ],
            "history": [ { "timestamp": "2026-09-01T12:00:00Z", "head": "abc" } ]
        })
    }

    #[test]
    fn captures_scores_counts_and_thresholds() {
        let s = extract_surface(&sample_report());
        assert_eq!(s.overall_score, 55);
        assert_eq!(s.total_commits, 2287);
        assert_eq!(s.score_thresholds.get("good_min"), Some(&71));
        assert_eq!(s.coupling_finding_counts.get("control"), Some(&5));
    }

    #[test]
    fn preserves_unscored_metrics_as_none() {
        let s = extract_surface(&sample_report());
        let health = &s.categories[0];
        assert_eq!(health.metrics[0].score, Some(25));
        assert_eq!(
            health.metrics[1].score, None,
            "an unscored metric must stay unscored, never become a number"
        );
    }

    #[test]
    fn merges_both_action_lists_in_deterministic_order() {
        let s = extract_surface(&sample_report());
        assert_eq!(s.actions.len(), 2);
        assert_eq!(s.actions[0].target_tab, "coupling");
        assert_eq!(s.actions[1].target_tab, "ownership");
    }

    #[test]
    fn keeps_hotspot_rank_order_and_caps_at_twenty() {
        let s = extract_surface(&sample_report());
        assert_eq!(
            s.top_hotspots,
            vec![
                "crates/ignore/src/walk.rs".to_string(),
                "crates/core/main.rs".to_string(),
            ]
        );
    }

    fn report_with_hotspots(paths: &[String]) -> serde_json::Value {
        let hotspots: Vec<serde_json::Value> = paths
            .iter()
            .enumerate()
            .map(|(i, path)| json!({ "path": path, "hotspot_score": 100.0 - i as f64 }))
            .collect();
        json!({ "file_hotspots": hotspots })
    }

    #[test]
    fn caps_hotspots_at_twenty_when_more_than_twenty_are_present() {
        let paths: Vec<String> = (0..25).map(|i| format!("hot{i:02}.rs")).collect();
        let s = extract_surface(&report_with_hotspots(&paths));
        assert_eq!(
            s.top_hotspots.len(),
            20,
            "must cap at HOTSPOT_LIMIT even when more are present"
        );
        assert_eq!(
            s.top_hotspots.first(),
            Some(&"hot00.rs".to_string()),
            "first element is the top-ranked path"
        );
        assert_eq!(
            s.top_hotspots.last(),
            Some(&"hot19.rs".to_string()),
            "last element is the twentieth-ranked path, pinning both the cap and rank-order prefix taking"
        );
    }

    #[test]
    fn excludes_the_volatile_history_field() {
        let s = extract_surface(&sample_report());
        let encoded = serde_json::to_string(&s).expect("serializes");
        assert!(
            !encoded.contains("timestamp"),
            "history carries a per-run timestamp and must not reach the baseline"
        );
    }

    #[test]
    fn serializes_deterministically() {
        let a = serde_json::to_string(&extract_surface(&sample_report())).unwrap();
        let b = serde_json::to_string(&extract_surface(&sample_report())).unwrap();
        assert_eq!(a, b);
    }
}
