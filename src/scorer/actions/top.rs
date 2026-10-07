use crate::metrics::CategoryResult;
use crate::scorer::types::ActionItem;

use super::guidance::{guidance_for, Guidance};

pub(in crate::scorer) fn generate_top_actions(categories: &[CategoryResult]) -> Vec<ActionItem> {
    let mut low_metrics: Vec<(&str, &str, u32)> = Vec::new();

    for cat in categories {
        for metric in &cat.metrics {
            // Unscored metrics (insufficient data) cannot drive suggestions.
            if let Some(score) = metric.score {
                low_metrics.push((&cat.name, &metric.name, score));
            }
        }
    }

    low_metrics.sort_by_key(|m| m.2);

    let scored = low_metrics
        .iter()
        .take(3)
        .filter(|m| m.2 < 80)
        .map(|(cat, metric, score)| {
            let guidance = guidance_for(metric).unwrap_or(Guidance::UNGUIDED);
            ActionItem {
                text: format!(
                    "[{}] {} (score: {}) — {}",
                    cat,
                    metric,
                    score,
                    guidance.advice_text()
                ),
                target_tab: guidance.tab(),
                sort_by: guidance.sort_by(),
            }
        });

    // Advisory metrics don't score, but ones carrying concrete findings
    // still deserve their curated advice — appended after the scored
    // actions so they never displace a real score problem.
    let advisory = categories
        .iter()
        .flat_map(|cat| cat.metrics.iter().map(move |metric| (cat, metric)))
        .filter(|(_, metric)| metric.score.is_none() && advisory_has_findings(&metric.raw_value))
        .take(2)
        .map(|(cat, metric)| {
            let guidance = guidance_for(&metric.name).unwrap_or(Guidance::UNGUIDED);
            ActionItem {
                text: format!(
                    "[{}] {} (advisory) — {}",
                    cat.name,
                    metric.name,
                    guidance.advice_text()
                ),
                target_tab: guidance.tab(),
                sort_by: guidance.sort_by(),
            }
        });

    scored.chain(advisory).collect()
}

/// Whether an unscored metric's raw value carries concrete findings worth an
/// advisory action. Annotations (Integer/Float/Percentage) and N/A text are
/// not findings.
fn advisory_has_findings(raw_value: &crate::metrics::RawValue) -> bool {
    match raw_value {
        crate::metrics::RawValue::List(items) => !items.is_empty(),
        crate::metrics::RawValue::Count(count) => *count > 0,
        _ => false,
    }
}
