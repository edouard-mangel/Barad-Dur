pub(crate) mod callgraph;
pub(crate) mod churn;
pub mod complexity;
pub mod coupling;
pub mod deps;
pub mod evolution;
pub mod file_role;
pub mod health;
pub mod hygiene;
pub mod name_smell;
pub(crate) mod reexport;
pub mod team;
#[cfg(test)]
pub mod testutil;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::snapshot::{AuthorId, BlameLine, UNKNOWN_AUTHOR};

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

/// Median of a slice; 0.0 for an empty slice. Does not mutate the input —
/// sorts an internal copy.
pub(crate) fn median(values: &[usize]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let len = sorted.len();
    #[allow(clippy::manual_is_multiple_of)]
    if len % 2 == 0 {
        (sorted[len / 2 - 1] + sorted[len / 2]) as f64 / 2.0
    } else {
        sorted[len / 2] as f64
    }
}

/// Count how many times each file appears as an import target (incoming
/// edges) across the whole import graph. Shared by every metric that needs
/// afferent-style degree — a file's own key in `import_graph` counts as an
/// outgoing edge, so it never touches this map.
pub(crate) fn incoming_import_counts(
    import_graph: &HashMap<PathBuf, Vec<PathBuf>>,
) -> HashMap<&Path, usize> {
    let mut incoming: HashMap<&Path, usize> = HashMap::new();
    for targets in import_graph.values() {
        for target in targets {
            *incoming.entry(target.as_path()).or_insert(0) += 1;
        }
    }
    incoming
}

/// A file's outgoing edge count in the import graph — zero if it imports
/// nothing (or isn't a key at all). Shared by every metric that needs
/// efferent-style degree.
pub(crate) fn outgoing_degree(import_graph: &HashMap<PathBuf, Vec<PathBuf>>, path: &Path) -> usize {
    import_graph.get(path).map(|v| v.len()).unwrap_or(0)
}

/// The author holding a *strict* majority (> 50%) of a file's blamed
/// lines — the "main developer" proxy (org-coupling design, Decision 1).
/// `None` when blame is empty, no author clears the majority, or the
/// majority belongs to [`UNKNOWN_AUTHOR`] (out-of-window/departed authors
/// are not a nameable owner; the unknown mass still counts toward the
/// total, so a current author cannot inherit a majority they don't have).
/// Single source of the dominance rule — `bus_factor` and
/// `churn_ownership` wrap this instead of re-deriving it.
pub(crate) fn primary_author(lines: &[BlameLine]) -> Option<AuthorId> {
    let counts = author_line_counts(lines);
    let total: usize = counts.values().sum();
    counts
        .into_iter()
        .find(|&(_, count)| count * 2 > total)
        .map(|(author, _)| author)
        .filter(|&author| author != UNKNOWN_AUTHOR)
}

/// Whether a co-change count clears `min_ratio` against the smaller of
/// the two per-entity activity counts — the single definition of "these
/// two move together often enough, relative to how often each moves at
/// all", shared by the Coupling category's change-coupling smells and
/// Team's cross-team coupling so one `change_coupling_min_ratio` knob
/// cannot mean two different things.
pub(crate) fn meets_coupling_ratio(
    co: usize,
    denom_a: usize,
    denom_b: usize,
    min_ratio: f64,
) -> bool {
    let denom = denom_a.min(denom_b);
    denom > 0 && (co as f64 / denom as f64) >= min_ratio
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
mod primary_author_sentinel_tests {
    use super::{primary_author, UNKNOWN_AUTHOR};
    use crate::snapshot::BlameLine;
    use chrono::Utc;

    fn lines(counts: &[(usize, usize)]) -> Vec<BlameLine> {
        counts
            .iter()
            .map(|&(author_id, line_count)| {
                let mut l = BlameLine::new(author_id, Utc::now());
                l.line_count = line_count;
                l
            })
            .collect()
    }

    #[test]
    fn unknown_author_majority_is_not_a_primary_author() {
        // 80 legacy lines + 20 alice lines: the majority is unknown —
        // no one nameable owns this file.
        assert_eq!(
            primary_author(&lines(&[(UNKNOWN_AUTHOR, 80), (1, 20)])),
            None
        );
    }

    #[test]
    fn known_majority_over_unknown_minority_is_kept() {
        assert_eq!(
            primary_author(&lines(&[(UNKNOWN_AUTHOR, 20), (1, 80)])),
            Some(1)
        );
    }

    #[test]
    fn unknown_mass_still_counts_toward_the_total() {
        // alice has 40 of 100 lines — a majority of the *known* lines but
        // not of the file; unknown mass must not be excluded from totals.
        assert_eq!(
            primary_author(&lines(&[(UNKNOWN_AUTHOR, 60), (1, 40)])),
            None
        );
    }
}

#[cfg(test)]
mod meets_coupling_ratio_tests {
    use super::meets_coupling_ratio;

    #[test]
    fn zero_denominator_never_qualifies() {
        assert!(!meets_coupling_ratio(1, 0, 5, 0.30));
        assert!(!meets_coupling_ratio(1, 5, 0, 0.30));
    }

    #[test]
    fn ratio_exactly_at_threshold_qualifies() {
        // 3 / min(10, 20) = 0.30 == threshold -> >= keeps it.
        assert!(meets_coupling_ratio(3, 10, 20, 0.30));
    }

    #[test]
    fn ratio_below_threshold_does_not_qualify() {
        assert!(!meets_coupling_ratio(1, 4, 4, 0.30));
    }

    #[test]
    fn smaller_denominator_side_is_used() {
        // min(3, 9) = 3 -> 1/3 qualifies; max would give 1/9 and fail.
        assert!(meets_coupling_ratio(1, 9, 3, 0.30));
    }
}

#[cfg(test)]
mod incoming_import_counts_tests {
    use super::incoming_import_counts;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    #[test]
    fn empty_graph_yields_empty_map() {
        let graph: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        let counts = incoming_import_counts(&graph);
        assert!(counts.is_empty());
    }

    #[test]
    fn counts_each_target_occurrence() {
        let mut graph: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        graph.insert(
            PathBuf::from("a.rs"),
            vec![PathBuf::from("core.rs"), PathBuf::from("util.rs")],
        );
        graph.insert(PathBuf::from("b.rs"), vec![PathBuf::from("core.rs")]);
        let counts = incoming_import_counts(&graph);
        assert_eq!(counts.get(Path::new("core.rs")), Some(&2));
        assert_eq!(counts.get(Path::new("util.rs")), Some(&1));
        assert_eq!(
            counts.get(Path::new("a.rs")),
            None,
            "a source-only file has no incoming edges"
        );
    }
}

#[cfg(test)]
mod outgoing_degree_tests {
    use super::outgoing_degree;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    #[test]
    fn counts_a_files_own_targets() {
        let mut graph: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        graph.insert(
            PathBuf::from("a.rs"),
            vec![PathBuf::from("b.rs"), PathBuf::from("c.rs")],
        );
        assert_eq!(outgoing_degree(&graph, Path::new("a.rs")), 2);
    }

    #[test]
    fn zero_for_a_file_with_no_outgoing_imports() {
        let graph: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        assert_eq!(outgoing_degree(&graph, Path::new("a.rs")), 0);
    }
}

#[cfg(test)]
mod median_tests {
    use super::median;

    #[test]
    fn empty_slice_is_zero() {
        assert_eq!(median(&[]), 0.0);
    }

    #[test]
    fn odd_count_returns_middle_value() {
        assert_eq!(median(&[5, 1, 3]), 3.0);
    }

    #[test]
    fn even_count_averages_the_two_middle_values() {
        assert_eq!(median(&[1, 2, 3, 4]), 2.5);
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
