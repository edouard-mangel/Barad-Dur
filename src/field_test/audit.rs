use crate::field_test::surface::{ActionSurface, DecisionSurface};
use std::collections::BTreeSet;

/// Pick the recommendations a reviewer must read this merge: every one that
/// is new or changed, plus a bounded rotating slice of ones never audited.
pub fn select_for_audit(
    baseline: &DecisionSurface,
    current: &DecisionSurface,
    already_seen: &BTreeSet<String>,
    rotation: usize,
) -> Vec<ActionSurface> {
    let before: BTreeSet<&ActionSurface> = baseline.actions.iter().collect();

    let fresh: Vec<ActionSurface> = current
        .actions
        .iter()
        .filter(|a| !before.contains(*a))
        .cloned()
        .collect();

    let fresh_texts: BTreeSet<&str> = fresh.iter().map(|a| a.text.as_str()).collect();

    let rotated = current
        .actions
        .iter()
        .filter(|a| !fresh_texts.contains(a.text.as_str()))
        .filter(|a| !already_seen.contains(&a.text))
        .take(rotation)
        .cloned();

    fresh.iter().cloned().chain(rotated).collect()
}

/// Render the worksheet a reviewer fills in. The rubric is not decoration:
/// BD-001 passed "True" and failed only "Safe".
pub fn render_worksheet(repo: &str, items: &[ActionSurface]) -> String {
    let header = format!(
        "## {repo}\n\n\
         For each recommendation below, answer all three. \
         Any **Safe** failure blocks the merge; \
         **True** and **Actionable** failures become tickets.\n\n\
         | # | Recommendation | True? | Safe? | Actionable? | Notes |\n\
         |---|---|---|---|---|---|\n"
    );
    let rows = items.iter().enumerate().map(|(i, a)| {
        let text = a.text.replace('|', "\\|");
        format!("| {} | {} | | | | |\n", i + 1, text)
    });
    header
        .chars()
        .chain(rows.collect::<String>().chars())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field_test::surface::{ActionSurface, DecisionSurface};
    use std::collections::{BTreeMap, BTreeSet};

    fn with_actions(texts: &[&str]) -> DecisionSurface {
        DecisionSurface {
            overall_score: Some(1),
            total_files: 1,
            total_commits: 1,
            total_authors: 1,
            score_thresholds: BTreeMap::new(),
            coupling_finding_counts: BTreeMap::new(),
            categories: vec![],
            actions: texts
                .iter()
                .map(|t| ActionSurface {
                    target_tab: "x".into(),
                    text: (*t).into(),
                })
                .collect(),
            top_hotspots: vec![],
        }
    }

    #[test]
    fn always_includes_every_new_recommendation() {
        let picked = select_for_audit(
            &with_actions(&["old"]),
            &with_actions(&["old", "brand new"]),
            &BTreeSet::new(),
            0,
        );
        assert!(picked.iter().any(|a| a.text == "brand new"));
    }

    #[test]
    fn rotates_through_unseen_pre_existing_recommendations() {
        let seen: BTreeSet<String> = ["a".to_string()].into_iter().collect();
        let picked = select_for_audit(
            &with_actions(&["a", "b", "c"]),
            &with_actions(&["a", "b", "c"]),
            &seen,
            2,
        );
        let texts: Vec<_> = picked.iter().map(|a| a.text.as_str()).collect();
        assert!(!texts.contains(&"a"), "already audited");
        assert_eq!(texts.len(), 2, "rotation slice is bounded");
    }

    #[test]
    fn rotation_slice_is_bounded_even_when_much_is_unseen() {
        let picked = select_for_audit(
            &with_actions(&["a", "b", "c", "d", "e", "f"]),
            &with_actions(&["a", "b", "c", "d", "e", "f"]),
            &BTreeSet::new(),
            5,
        );
        assert_eq!(picked.len(), 5);
    }

    #[test]
    fn worksheet_carries_the_true_safe_actionable_rubric() {
        let sheet = render_worksheet("ripgrep", &with_actions(&["do a thing"]).actions);
        assert!(sheet.contains("do a thing"));
        assert!(sheet.contains("True"));
        assert!(sheet.contains("Safe"));
        assert!(sheet.contains("Actionable"));
    }
}
