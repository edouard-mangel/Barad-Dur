use crate::field_test::surface::DecisionSurface;
use std::collections::{BTreeMap, BTreeSet};

/// A human-readable account of how two decision surfaces differ.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SurfaceDiff {
    pub changes: Vec<String>,
}

impl SurfaceDiff {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn render(&self) -> String {
        self.changes.join("\n")
    }
}

fn show(score: Option<i64>) -> String {
    score.map_or_else(|| "unscored".to_string(), |n| n.to_string())
}

fn scalar(label: &str, before: i64, after: i64) -> Option<String> {
    (before != after).then(|| format!("  {label}: {before} -> {after}"))
}

fn map_changes(
    label: &str,
    baseline: &BTreeMap<String, i64>,
    current: &BTreeMap<String, i64>,
) -> impl Iterator<Item = String> {
    let mut changes = Vec::new();

    let all_keys: BTreeSet<_> = baseline
        .keys()
        .chain(current.keys())
        .collect::<std::collections::BTreeSet<_>>();

    for key in all_keys {
        match (baseline.get(key), current.get(key)) {
            (Some(b), Some(c)) if b != c => {
                changes.push(format!("  {label}[{key}]: {b} -> {c}"));
            }
            (Some(b), None) => {
                changes.push(format!("  {label}[{key}]: {b} -> removed"));
            }
            (None, Some(c)) => {
                changes.push(format!("  {label}[{key}]: added -> {c}"));
            }
            _ => {}
        }
    }

    changes.into_iter()
}

/// Compare a committed baseline against a freshly measured surface.
pub fn diff_surfaces(baseline: &DecisionSurface, current: &DecisionSurface) -> SurfaceDiff {
    let scalars = [
        scalar(
            "overall_score",
            baseline.overall_score,
            current.overall_score,
        ),
        scalar("total_files", baseline.total_files, current.total_files),
        scalar(
            "total_commits",
            baseline.total_commits,
            current.total_commits,
        ),
        scalar(
            "total_authors",
            baseline.total_authors,
            current.total_authors,
        ),
    ];

    let threshold_changes = map_changes(
        "score_thresholds",
        &baseline.score_thresholds,
        &current.score_thresholds,
    );
    let coupling_changes = map_changes(
        "coupling_finding_counts",
        &baseline.coupling_finding_counts,
        &current.coupling_finding_counts,
    );

    // Compare categories by collecting all category names from both surfaces
    let baseline_cat_names: BTreeSet<_> = baseline
        .categories
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    let current_cat_names: BTreeSet<_> =
        current.categories.iter().map(|c| c.name.as_str()).collect();
    let all_cat_names: BTreeSet<_> = baseline_cat_names
        .union(&current_cat_names)
        .copied()
        .collect();

    let category_changes = all_cat_names.iter().flat_map(|cat_name| {
        let b_cat = baseline
            .categories
            .iter()
            .find(|c| c.name.as_str() == *cat_name);
        let c_cat = current
            .categories
            .iter()
            .find(|c| c.name.as_str() == *cat_name);

        let mut changes = Vec::new();

        // Compare category scores
        match (b_cat.map(|c| c.score), c_cat.map(|c| c.score)) {
            (Some(b_score), Some(c_score)) if b_score != c_score => {
                changes.push(format!(
                    "  category {}: {} -> {}",
                    cat_name,
                    show(b_score),
                    show(c_score)
                ));
            }
            _ => {}
        }

        // Report removed categories
        if b_cat.is_some() && c_cat.is_none() {
            changes.push(format!("  category {} removed", cat_name));
            return changes.into_iter();
        }

        // Report added categories
        if b_cat.is_none() && c_cat.is_some() {
            changes.push(format!("  category {} added", cat_name));
            return changes.into_iter();
        }

        // Compare metrics within the category
        let b_metrics: &[_] = b_cat.map(|c| c.metrics.as_slice()).unwrap_or(&[]);
        let c_metrics: &[_] = c_cat.map(|c| c.metrics.as_slice()).unwrap_or(&[]);

        let b_metric_names: BTreeSet<_> = b_metrics.iter().map(|m| m.name.as_str()).collect();
        let c_metric_names: BTreeSet<_> = c_metrics.iter().map(|m| m.name.as_str()).collect();
        let all_metric_names: BTreeSet<_> =
            b_metric_names.union(&c_metric_names).copied().collect();

        for metric_name in all_metric_names {
            let b_metric = b_metrics.iter().find(|m| m.name.as_str() == metric_name);
            let c_metric = c_metrics.iter().find(|m| m.name.as_str() == metric_name);

            match (b_metric.map(|m| m.score), c_metric.map(|m| m.score)) {
                (Some(b_score), Some(c_score)) if b_score != c_score => {
                    changes.push(format!(
                        "  metric {}/{}: {} -> {}",
                        cat_name,
                        metric_name,
                        show(b_score),
                        show(c_score)
                    ));
                }
                _ => {}
            }

            if b_metric.is_some() && c_metric.is_none() {
                changes.push(format!("  metric {}/{} removed", cat_name, metric_name));
            }

            if b_metric.is_none() && c_metric.is_some() {
                changes.push(format!("  metric {}/{} added", cat_name, metric_name));
            }
        }

        changes.into_iter()
    });

    let before: BTreeSet<_> = baseline.actions.iter().collect();
    let after: BTreeSet<_> = current.actions.iter().collect();
    let removed = before
        .difference(&after)
        .map(|a| format!("  - {} [{}]", a.text, a.target_tab));
    let added = after
        .difference(&before)
        .map(|a| format!("  + {} [{}]", a.text, a.target_tab));

    let hotspots = (baseline.top_hotspots != current.top_hotspots).then(|| {
        // Find the first differing rank
        let max_len = baseline.top_hotspots.len().max(current.top_hotspots.len());
        for rank in 0..max_len {
            match (
                baseline.top_hotspots.get(rank),
                current.top_hotspots.get(rank),
            ) {
                (Some(b_path), Some(c_path)) if b_path != c_path => {
                    return format!(
                        "  top_hotspots ranking changed at rank {}: {} -> {}",
                        rank + 1,
                        b_path,
                        c_path
                    );
                }
                (Some(b_path), None) => {
                    return format!(
                        "  top_hotspots ranking changed at rank {}: {} removed",
                        rank + 1,
                        b_path
                    );
                }
                (None, Some(c_path)) => {
                    return format!(
                        "  top_hotspots ranking changed at rank {}: {} added",
                        rank + 1,
                        c_path
                    );
                }
                _ => continue,
            }
        }
        "  top_hotspots ranking changed".to_string()
    });

    let changes = scalars
        .into_iter()
        .flatten()
        .chain(threshold_changes)
        .chain(coupling_changes)
        .chain(category_changes)
        .chain(removed)
        .chain(added)
        .chain(hotspots)
        .collect();

    SurfaceDiff { changes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field_test::surface::{
        ActionSurface, CategorySurface, DecisionSurface, MetricSurface,
    };

    fn base_surface() -> DecisionSurface {
        DecisionSurface {
            overall_score: 55,
            total_files: 10,
            total_commits: 100,
            total_authors: 3,
            score_thresholds: BTreeMap::from([("good_min".into(), 71), ("warn_min".into(), 41)]),
            coupling_finding_counts: BTreeMap::from([("common".into(), 5)]),
            categories: vec![CategorySurface {
                name: "Health".into(),
                score: Some(40),
                metrics: vec![MetricSurface {
                    name: "Bus factor".into(),
                    score: Some(25),
                }],
            }],
            actions: vec![ActionSurface {
                target_tab: "ownership".into(),
                text: "old advice".into(),
            }],
            top_hotspots: vec!["a.rs".into(), "b.rs".into()],
        }
    }

    #[test]
    fn identical_surfaces_produce_no_changes() {
        let a = base_surface();
        assert!(diff_surfaces(&a, &a).is_empty());
    }

    #[test]
    fn reports_a_changed_overall_score() {
        let baseline = base_surface();
        let mut current = base_surface();
        current.overall_score = 60;

        let d = diff_surfaces(&baseline, &current);
        assert!(!d.is_empty());
        assert!(d.render().contains("overall_score"), "got: {}", d.render());
        assert!(d.render().contains("55"));
        assert!(d.render().contains("60"));
    }

    #[test]
    fn reports_a_metric_that_stopped_being_unscored() {
        let mut baseline = base_surface();
        let current = base_surface();
        baseline.categories[0].metrics[0].score = None;

        let d = diff_surfaces(&baseline, &current);
        let text = d.render();
        assert!(text.contains("Bus factor"), "got: {text}");
        assert!(text.contains("unscored"), "got: {text}");
    }

    #[test]
    fn reports_added_and_removed_recommendations() {
        let mut baseline = base_surface();
        let mut current = base_surface();
        baseline.actions[0].text = "old advice".into();
        current.actions[0].text = "new advice".into();

        let d = diff_surfaces(&baseline, &current);
        let text = d.render();
        assert!(text.contains("- old advice"), "got: {text}");
        assert!(text.contains("+ new advice"), "got: {text}");
    }

    #[test]
    fn reports_changed_score_threshold() {
        let baseline = base_surface();
        let mut current = base_surface();
        current.score_thresholds.insert("good_min".into(), 75);

        let d = diff_surfaces(&baseline, &current);
        let text = d.render();
        assert!(!d.is_empty(), "threshold change should be detected");
        assert!(text.contains("score_thresholds"), "got: {text}");
        assert!(text.contains("71"), "got: {text}");
        assert!(text.contains("75"), "got: {text}");
    }

    #[test]
    fn reports_removed_score_threshold() {
        let baseline = base_surface();
        let mut current = base_surface();
        current.score_thresholds.remove("warn_min");

        let d = diff_surfaces(&baseline, &current);
        let text = d.render();
        assert!(!d.is_empty(), "threshold removal should be detected");
        assert!(text.contains("score_thresholds[warn_min]"), "got: {text}");
        assert!(text.contains("removed"), "got: {text}");
    }

    #[test]
    fn reports_added_score_threshold() {
        let baseline = base_surface();
        let mut current = base_surface();
        current.score_thresholds.insert("new_threshold".into(), 50);

        let d = diff_surfaces(&baseline, &current);
        let text = d.render();
        assert!(!d.is_empty(), "new threshold should be detected");
        assert!(
            text.contains("score_thresholds[new_threshold]"),
            "got: {text}"
        );
        assert!(text.contains("added"), "got: {text}");
    }

    #[test]
    fn reports_changed_coupling_finding_counts() {
        let baseline = base_surface();
        let mut current = base_surface();
        current.coupling_finding_counts.insert("common".into(), 8);

        let d = diff_surfaces(&baseline, &current);
        let text = d.render();
        assert!(!d.is_empty(), "coupling count change should be detected");
        assert!(text.contains("coupling_finding_counts"), "got: {text}");
        assert!(text.contains("5"), "got: {text}");
        assert!(text.contains("8"), "got: {text}");
    }

    #[test]
    fn reports_removed_coupling_finding_counts() {
        let baseline = base_surface();
        let mut current = base_surface();
        current.coupling_finding_counts.remove("common");

        let d = diff_surfaces(&baseline, &current);
        let text = d.render();
        assert!(!d.is_empty(), "coupling count removal should be detected");
        assert!(
            text.contains("coupling_finding_counts[common]"),
            "got: {text}"
        );
        assert!(text.contains("removed"), "got: {text}");
    }

    #[test]
    fn reports_added_coupling_finding_counts() {
        let baseline = base_surface();
        let mut current = base_surface();
        current
            .coupling_finding_counts
            .insert("inheritance".into(), 3);

        let d = diff_surfaces(&baseline, &current);
        let text = d.render();
        assert!(!d.is_empty(), "new coupling count should be detected");
        assert!(
            text.contains("coupling_finding_counts[inheritance]"),
            "got: {text}"
        );
        assert!(text.contains("added"), "got: {text}");
    }

    #[test]
    fn reports_changed_category_score() {
        let baseline = base_surface();
        let mut current = base_surface();
        current.categories[0].score = Some(50);

        let d = diff_surfaces(&baseline, &current);
        let text = d.render();
        assert!(!d.is_empty(), "category score change should be detected");
        assert!(text.contains("category Health"), "got: {text}");
        assert!(text.contains("40"), "got: {text}");
        assert!(text.contains("50"), "got: {text}");
    }

    #[test]
    fn reports_removed_category() {
        let baseline = base_surface();
        let mut current = base_surface();
        current.categories.clear();

        let d = diff_surfaces(&baseline, &current);
        let text = d.render();
        assert!(!d.is_empty(), "removed category should be detected");
        assert!(text.contains("category Health removed"), "got: {text}");
    }

    #[test]
    fn reports_added_category() {
        let baseline = base_surface();
        let mut current = base_surface();
        current.categories.push(CategorySurface {
            name: "Evolution".into(),
            score: Some(60),
            metrics: vec![],
        });

        let d = diff_surfaces(&baseline, &current);
        let text = d.render();
        assert!(!d.is_empty(), "added category should be detected");
        assert!(text.contains("category Evolution added"), "got: {text}");
    }

    #[test]
    fn reports_removed_metric() {
        let baseline = base_surface();
        let mut current = base_surface();
        current.categories[0].metrics.clear();

        let d = diff_surfaces(&baseline, &current);
        let text = d.render();
        assert!(!d.is_empty(), "removed metric should be detected");
        assert!(
            text.contains("metric Health/Bus factor removed"),
            "got: {text}"
        );
    }

    #[test]
    fn reports_added_metric() {
        let baseline = base_surface();
        let mut current = base_surface();
        current.categories[0].metrics.push(MetricSurface {
            name: "Churn".into(),
            score: Some(30),
        });

        let d = diff_surfaces(&baseline, &current);
        let text = d.render();
        assert!(!d.is_empty(), "added metric should be detected");
        assert!(text.contains("metric Health/Churn added"), "got: {text}");
    }

    #[test]
    fn reports_top_hotspots_ranking_changed_with_specific_rank() {
        let baseline = base_surface();
        let mut current = base_surface();
        current.top_hotspots[0] = "c.rs".into();

        let d = diff_surfaces(&baseline, &current);
        let text = d.render();
        assert!(!d.is_empty(), "hotspot change should be detected");
        assert!(
            text.contains("top_hotspots ranking changed at rank 1"),
            "got: {text}"
        );
        assert!(text.contains("a.rs -> c.rs"), "got: {text}");
    }

    #[test]
    fn reports_removed_hotspot() {
        let baseline = base_surface();
        let mut current = base_surface();
        current.top_hotspots.pop();

        let d = diff_surfaces(&baseline, &current);
        let text = d.render();
        assert!(!d.is_empty(), "hotspot removal should be detected");
        assert!(
            text.contains("top_hotspots ranking changed at rank 2"),
            "got: {text}"
        );
        assert!(text.contains("b.rs removed"), "got: {text}");
    }

    #[test]
    fn reports_added_hotspot() {
        let baseline = base_surface();
        let mut current = base_surface();
        current.top_hotspots.push("c.rs".into());

        let d = diff_surfaces(&baseline, &current);
        let text = d.render();
        assert!(!d.is_empty(), "hotspot addition should be detected");
        assert!(
            text.contains("top_hotspots ranking changed at rank 3"),
            "got: {text}"
        );
        assert!(text.contains("c.rs added"), "got: {text}");
    }
}
