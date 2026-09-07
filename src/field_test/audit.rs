use crate::field_test::surface::{ActionSurface, DecisionSurface};
use std::collections::BTreeSet;

/// Marks a recommendation the merge removed, so the reviewer answers
/// "was it safe to stop saying this?" rather than reading it as live advice.
pub const WITHDRAWN_PREFIX: &str = "[withdrawn] ";

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

    // Recommendations this merge WITHDREW: in the baseline, gone now. A merge
    // that stops giving advice changes the decision surface as much as one
    // that starts, and "was it safe to stop saying this?" is a Safe-question.
    // Without this the audit is blind to exactly the case where the tool goes
    // silent on a repository it used to have something to say about.
    let after: BTreeSet<&ActionSurface> = current.actions.iter().collect();
    let withdrawn: Vec<ActionSurface> = baseline
        .actions
        .iter()
        .filter(|a| !after.contains(*a))
        .map(|a| ActionSurface {
            target_tab: a.target_tab.clone(),
            text: format!("{WITHDRAWN_PREFIX}{}", a.text),
        })
        .collect();

    let rotated = current
        .actions
        .iter()
        .filter(|a| !fresh_texts.contains(a.text.as_str()))
        .filter(|a| !already_seen.contains(&a.text))
        .take(rotation)
        .cloned();

    fresh
        .iter()
        .cloned()
        .chain(withdrawn)
        .chain(rotated)
        .collect()
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
    fn surfaces_a_recommendation_this_merge_removed() {
        // A merge that stops giving advice changes the decision surface just as
        // much as one that starts. "Was it safe to stop saying this?" is a
        // Safe-question, and it is invisible unless the removal is sampled.
        let picked = select_for_audit(
            &with_actions(&["still here", "withdrawn"]),
            &with_actions(&["still here"]),
            &BTreeSet::new(),
            0,
        );
        assert!(
            picked.iter().any(|a| a.text.contains("withdrawn")),
            "a removed recommendation must be sampled, got: {:?}",
            picked.iter().map(|a| &a.text).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_reworded_recommendation_shows_both_its_old_and_new_form() {
        // P2c samples every recommendation "new or changed by this merge".
        // A rewording (e.g. the score embedded in the text moving 25 -> 50)
        // is a change, and the reviewer needs both halves to judge it.
        let picked = select_for_audit(
            &with_actions(&["Long methods (score: 25) - extract"]),
            &with_actions(&["Long methods (score: 50) - extract"]),
            &BTreeSet::new(),
            0,
        );
        let texts: Vec<&str> = picked.iter().map(|a| a.text.as_str()).collect();
        assert!(
            texts
                .iter()
                .any(|t| t.contains("score: 50") && !t.starts_with(WITHDRAWN_PREFIX)),
            "the new form must be sampled, got: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|t| t.starts_with(WITHDRAWN_PREFIX) && t.contains("score: 25")),
            "the superseded form must be sampled as withdrawn, got: {texts:?}"
        );
    }

    #[test]
    fn an_unchanged_recommendation_is_never_reported_as_withdrawn() {
        let picked = select_for_audit(
            &with_actions(&["a", "b"]),
            &with_actions(&["a", "b"]),
            &BTreeSet::new(),
            0,
        );
        assert!(
            picked.is_empty(),
            "nothing changed, nothing due for rotation: {:?}",
            picked.iter().map(|a| &a.text).collect::<Vec<_>>()
        );
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
