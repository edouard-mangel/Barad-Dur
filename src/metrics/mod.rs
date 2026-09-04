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
    /// Average of the scored metrics; `None` when no metric had enough
    /// evidence to score (e.g. Team on a solo repo). An unscored category
    /// stays in the report with its metrics' explanations and contributes
    /// nothing to the overall score.
    pub score: Option<u32>,
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

/// Score findings by their prevalence in the population being assessed.
/// Absolute-count bands make every sufficiently large repository fail even
/// when only a tiny fraction of its code is affected.
///
/// Below `MIN_PREVALENCE_POPULATION` the denominator is too small to trust,
/// so the count bands govern. At or above `TRUSTED_POPULATION` prevalence
/// governs alone. Between them the two are *blended* in proportion to how
/// far along the range the population sits, rather than one abruptly
/// replacing the other.
///
/// The blend is what keeps the boundaries honest. A hard switch at either
/// end let a repository jump a whole band by gaining one unrelated file —
/// 1 finding in 299 files scored 75, the same finding in 300 scored 90.
/// Weighting removes the cliff without subordinating prevalence to the
/// count bands, which would collapse this function into `score_count_bands`
/// and reinstate the very pathology it exists to remove.
///
/// A larger denominator can still *raise* a score, and must: that is what
/// prevalence means. The direction that is a bug is a score *falling* as
/// the repository grows while its findings stay put — pinned by
/// `a_growing_denominator_never_lowers_a_score`.
///
/// MAINTAINER-AUTHORED THRESHOLDS, like `score_pressman`'s bands: 100 and
/// 300 are judgement calls about when a denominator becomes meaningful,
/// not values derived from a corpus. Retune the numbers if evidence
/// warrants; keep the continuity and the no-fall invariant the tests pin.
pub(crate) fn score_prevalence(flagged: usize, total: usize) -> u32 {
    const MIN_PREVALENCE_POPULATION: usize = 100;
    const TRUSTED_POPULATION: usize = 300;

    if flagged == 0 {
        return 100;
    }
    // total == 0 (no recognized source population) falls through to the
    // count bands: findings are real even when the denominator is unknown.
    let by_count = score_count_bands(flagged);
    if total < MIN_PREVALENCE_POPULATION {
        return by_count;
    }
    let pct = flagged as f64 / total as f64 * 100.0;
    let by_prevalence = if pct <= 1.0 {
        90
    } else if pct <= 5.0 {
        75
    } else if pct <= 20.0 {
        50
    } else {
        25
    };
    let span = (TRUSTED_POPULATION - MIN_PREVALENCE_POPULATION) as f64;
    let weight = ((total - MIN_PREVALENCE_POPULATION) as f64 / span).min(1.0);
    (by_count as f64 + (by_prevalence as f64 - by_count as f64) * weight).round() as u32
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
    /// Whether at least one metric had enough evidence to score.
    pub fn is_scored(&self) -> bool {
        self.score.is_some()
    }

    /// Compute category score as the average of scored metrics. Unscored
    /// metrics (`score: None`, insufficient data) don't drag the average.
    /// When *no* metric could be scored the category is unscored too: the
    /// report keeps it, with each metric's "not applicable" explanation, and
    /// the overall score is taken over the measurable categories only.
    pub fn compute_score(self) -> Self {
        let scored: Vec<u32> = self.metrics.iter().filter_map(|m| m.score).collect();
        let score = (!scored.is_empty()).then(|| scored.iter().sum::<u32>() / scored.len() as u32);
        Self { score, ..self }
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

    // Strict-majority boundaries. These moved here from a `#[cfg(test)]`
    // wrapper in bus_factor.rs that production code no longer called; the
    // rule they pin (`count * 2 > total`) lives here.

    #[test]
    fn empty_blame_has_no_primary_author() {
        assert_eq!(primary_author(&[]), None);
    }

    #[test]
    fn single_author_owning_every_line_is_primary() {
        assert_eq!(primary_author(&lines(&[(0, 10)])), Some(0));
    }

    #[test]
    fn exact_fifty_fifty_split_has_no_primary_author() {
        // max * 2 == total (not strictly greater) → nobody dominates.
        assert_eq!(primary_author(&lines(&[(0, 50), (1, 50)])), None);
    }

    #[test]
    fn fifty_one_percent_is_a_primary_author() {
        assert_eq!(primary_author(&lines(&[(0, 51), (1, 49)])), Some(0));
    }

    #[test]
    fn eighty_twenty_split_is_a_primary_author() {
        assert_eq!(primary_author(&lines(&[(0, 80), (1, 20)])), Some(0));
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
mod prevalence_score_tests {
    use super::{score_count_bands, score_prevalence};

    #[test]
    fn large_repositories_are_scored_by_rate_not_raw_count() {
        // Above TRUSTED_POPULATION the denominator is trustworthy on its
        // own, so a large repo is judged by the fraction of itself that is
        // affected — not by an absolute count every big repo would fail.
        assert_eq!(score_prevalence(1, 1_000), 90);
        assert_eq!(score_prevalence(40, 1_000), 75);
        assert_eq!(score_prevalence(100, 1_000), 50);
        assert_eq!(score_prevalence(200, 1_000), 50);
        assert_eq!(score_prevalence(201, 1_000), 25);
    }

    #[test]
    fn small_repositories_keep_minimum_support_bands() {
        assert_eq!(score_prevalence(0, 0), 100);
        assert_eq!(score_prevalence(1, 20), 75);
        assert_eq!(score_prevalence(3, 20), 50);
    }

    #[test]
    fn the_transition_range_blends_count_bands_into_prevalence() {
        // Halfway between MIN_PREVALENCE_POPULATION and TRUSTED_POPULATION
        // the two contribute equally: the count band says 75, prevalence
        // says 90, so the blend lands midway rather than snapping to either.
        assert_eq!(score_prevalence(1, 200), 83);
        assert_eq!(score_prevalence(6, 150), 38);
    }

    #[test]
    fn crossing_the_minimum_support_boundary_is_continuous() {
        // The artifact this replaced: one unrelated file used to buy a
        // whole band. At the boundary the blend weight is zero, so the
        // count band still governs and nothing jumps.
        for flagged in [1, 3, 6, 30] {
            assert_eq!(
                score_prevalence(flagged, 99),
                score_prevalence(flagged, 100),
                "{flagged} findings jumped a band at the support boundary"
            );
            assert_eq!(
                score_prevalence(flagged, 100),
                score_count_bands(flagged),
                "the count band must still govern at the boundary"
            );
        }
    }

    #[test]
    fn crossing_the_trusted_population_boundary_is_continuous() {
        // At TRUSTED_POPULATION the weight reaches one, so the blend has
        // already converged on prevalence — no step at the boundary itself.
        // 3/300 is the exception, and only because it lands exactly on the
        // 1% prevalence band edge, a step that exists at every percentage.
        for flagged in [1, 6, 30] {
            assert_eq!(
                score_prevalence(flagged, 299),
                score_prevalence(flagged, 300),
                "{flagged} findings jumped a band at the trusted boundary"
            );
        }
    }

    #[test]
    fn a_growing_denominator_never_lowers_a_score() {
        // Prevalence exists so a large repo is not condemned by absolute
        // count; the direction that must never happen is a score *falling*
        // as the repository grows while the findings stay put.
        for flagged in 0..60 {
            for total in 2..3_000 {
                assert!(
                    score_prevalence(flagged, total) >= score_prevalence(flagged, total - 1),
                    "score fell from total={} to total={total} at {flagged} findings",
                    total - 1
                );
            }
        }
    }

    #[test]
    fn prevalence_is_not_subordinate_to_the_count_bands() {
        // The regression this guards: capping by count at every population
        // made score_prevalence identical to score_count_bands, so a
        // 10,000-file repo with six affected files scored the worst band.
        assert_eq!(score_prevalence(6, 10_000), 90);
        assert_ne!(score_prevalence(6, 10_000), score_count_bands(6));
    }

    #[test]
    fn transition_range_still_respects_the_prevalence_band() {
        // 2 findings in 100 files is 2%: both inputs say 75, so does the
        // blend. 30 in 150 is 20% (band 50) against a count band of 25 —
        // a quarter of the way along the ramp, so still close to the count.
        assert_eq!(score_prevalence(2, 100), 75);
        assert_eq!(score_prevalence(30, 150), 31);
    }

    #[test]
    fn findings_without_a_recognized_source_population_score_by_count() {
        // A repo whose language has no entry in has_source_extension (Vue,
        // Elixir, …) has an empty source population; real findings must
        // fall back to the count bands, not score a perfect 100.
        assert_eq!(score_prevalence(12, 0), 25);
        assert_eq!(score_prevalence(1, 0), 75);
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
            score: None,
            metrics: vec![metric(Some(80)), metric(None), metric(Some(40))],
        }
        .compute_score();
        assert_eq!(
            cat.score,
            Some(60),
            "None metrics must not drag the average"
        );
    }

    #[test]
    fn category_with_no_scored_metrics_is_unscored() {
        let cat = CategoryResult {
            name: "Test".into(),
            score: None,
            metrics: vec![metric(None), metric(None)],
        }
        .compute_score();
        assert_eq!(cat.score, None, "no evidence must not become a perfect 100");
        assert!(!cat.is_scored());
    }

    #[test]
    fn category_with_no_metrics_is_unscored() {
        let cat = CategoryResult {
            name: "Test".into(),
            score: None,
            metrics: vec![],
        }
        .compute_score();
        assert_eq!(cat.score, None);
    }

    #[test]
    fn category_is_scored_only_when_at_least_one_metric_is_scored() {
        let unscored = CategoryResult {
            name: "Test".into(),
            score: None,
            metrics: vec![metric(None), metric(None)],
        };
        let partially_scored = CategoryResult {
            name: "Test".into(),
            score: Some(80),
            metrics: vec![metric(None), metric(Some(80))],
        };

        assert!(!unscored.is_scored());
        assert!(partially_scored.is_scored());
    }
}
