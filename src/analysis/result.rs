use crate::metrics::coupling::CouplingEvidence;
use crate::metrics::CategoryResult;

/// What one calculation produced: the selected categories, in canonical
/// order, and their weighted summary. Report enrichment, history, and gate
/// checks all consume this rather than assembling categories themselves.
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub categories: Vec<CategoryResult>,
    /// `None` when no category could be scored.
    pub overall_score: Option<u32>,
    /// Coupling facts derived once for this snapshot and configuration,
    /// shared by the Coupling metric and every report consumer.
    pub coupling_evidence: CouplingEvidence,
}

/// Weighted average of the *scored* categories, the weights renormalised
/// over them so an unscored category neither counts as 100 nor drags the
/// rest down. `None` when nothing is measurable.
pub fn compute_overall_score_with_weights(
    categories: &[CategoryResult],
    weights: &[(&str, f64)],
) -> Option<u32> {
    let weight_of = |name: &str| {
        weights
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, weight)| *weight)
            .unwrap_or(0.25)
    };
    let (weighted_sum, total_weight) = categories
        .iter()
        .filter_map(|cat| cat.score.map(|score| (score as f64, weight_of(&cat.name))))
        .fold((0.0, 0.0), |(sum, total), (score, weight)| {
            (sum + score * weight, total + weight)
        });
    (total_weight > 0.0).then(|| (weighted_sum / total_weight).round() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{MetricValue, RawValue};

    #[test]
    fn overall_score_averages_scored_categories_only_and_is_none_without_any() {
        let scored = |name: &str, score: u32| CategoryResult {
            name: name.to_string(),
            score: Some(score),
            metrics: vec![],
        };
        let unscored = CategoryResult {
            name: "Team".to_string(),
            score: None,
            metrics: vec![],
        };
        let weights: &[(&str, f64)] = &[("Health", 0.35), ("Coupling", 0.20), ("Team", 0.10)];

        assert_eq!(
            compute_overall_score_with_weights(
                &[
                    scored("Health", 80),
                    scored("Coupling", 40),
                    unscored.clone()
                ],
                weights
            ),
            Some(65),
            "weights renormalise over the measurable categories: (80*.35 + 40*.20) / .55"
        );
        assert_eq!(
            compute_overall_score_with_weights(&[unscored], weights),
            None
        );
        assert_eq!(compute_overall_score_with_weights(&[], weights), None);
    }

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
            score: Some(score),
            metrics: vec![MetricValue {
                name: format!("{} metric", name),
                description: "test".to_string(),
                raw_value: RawValue::Integer(0),
                score: Some(score),
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
        assert_eq!(score, Some(66));
    }

    #[test]
    fn overall_score_single_category() {
        let categories = vec![make_category("Health", 75)];
        let score = compute_overall_score_with_weights(&categories, WEIGHTS);
        assert_eq!(score, Some(75));
    }

    #[test]
    fn overall_score_empty_is_unscored() {
        let score = compute_overall_score_with_weights(&[], WEIGHTS);
        assert_eq!(score, None, "nothing measurable is not a zero");
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
        assert_eq!(score, Some(100));
    }
}
