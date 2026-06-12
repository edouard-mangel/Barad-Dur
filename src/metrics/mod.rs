pub mod complexity;
pub mod coupling;
pub mod deps;
pub mod evolution;
pub mod health;
pub mod hygiene;
pub mod team;
#[cfg(test)]
pub mod testutil;

use std::collections::HashMap;

use serde::Serialize;

use crate::snapshot::{AuthorId, BlameLine};

#[derive(Debug, Clone, Serialize)]
pub struct MetricValue {
    pub name: String,
    pub description: String,
    pub raw_value: RawValue,
    /// 0–100, or `None` when the repository lacks the data to judge this
    /// metric (solo project, no blame, no commits in window, …).
    /// Serialized as `null`; renderers display a dash.
    pub score: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub enum RawValue {
    Integer(i64),
    Float(f64),
    Percentage(f64),
    Count(usize),
    Text(String),
    List(Vec<String>),
}

impl std::fmt::Display for RawValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RawValue::Integer(v) => write!(f, "{}", v),
            RawValue::Float(v) => write!(f, "{:.2}", v),
            RawValue::Percentage(v) => write!(f, "{:.0}%", v),
            RawValue::Count(v) => write!(f, "{}", v),
            RawValue::Text(v) => write!(f, "{}", v),
            RawValue::List(v) => write!(f, "{}", v.join(", ")),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryResult {
    pub name: String,
    pub score: u32,
    pub metrics: Vec<MetricValue>,
}

/// Score a count using the standard four-band scale: 0→100, 1-2→75, 3-5→50, _→25.
pub(crate) fn score_count_bands(count: usize) -> u32 {
    match count {
        0 => 100,
        1..=2 => 75,
        3..=5 => 50,
        _ => 25,
    }
}

/// Accumulate blame line counts per author from a slice of blame lines.
pub(crate) fn author_line_counts(lines: &[BlameLine]) -> HashMap<AuthorId, usize> {
    let mut counts: HashMap<AuthorId, usize> = HashMap::new();
    for line in lines {
        *counts.entry(line.author_id).or_insert(0) += line.line_count;
    }
    counts
}

impl CategoryResult {
    /// Compute category score as the average of scored metrics. Unscored
    /// metrics (`score: None`, insufficient data) don't drag the average.
    /// When *no* metric could be scored the category defaults to 100 — the
    /// historical effective behavior for not-applicable categories (e.g.
    /// Team on a solo repo), so gates don't fail on missing data.
    pub fn compute_score(mut self) -> Self {
        let scored: Vec<u32> = self.metrics.iter().filter_map(|m| m.score).collect();
        self.score = if self.metrics.is_empty() {
            0
        } else if scored.is_empty() {
            100
        } else {
            scored.iter().sum::<u32>() / scored.len() as u32
        };
        self
    }
}

#[cfg(test)]
mod score_tests {
    use super::*;

    fn metric(score: Option<u32>) -> MetricValue {
        MetricValue {
            name: "m".into(),
            description: "d".into(),
            raw_value: RawValue::Count(0),
            score,
        }
    }

    #[test]
    fn category_average_skips_unscored_metrics() {
        let cat = CategoryResult {
            name: "Test".into(),
            score: 0,
            metrics: vec![metric(Some(80)), metric(None), metric(Some(40))],
        }
        .compute_score();
        assert_eq!(cat.score, 60, "None metrics must not drag the average");
    }

    #[test]
    fn category_with_no_scored_metrics_defaults_to_100() {
        let cat = CategoryResult {
            name: "Test".into(),
            score: 0,
            metrics: vec![metric(None), metric(None)],
        }
        .compute_score();
        assert_eq!(cat.score, 100);
    }

    #[test]
    fn category_with_no_metrics_scores_zero() {
        let cat = CategoryResult {
            name: "Test".into(),
            score: 0,
            metrics: vec![],
        }
        .compute_score();
        assert_eq!(cat.score, 0);
    }
}
