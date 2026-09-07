//! Pure trend analytics — no I/O, no imports from cache:: or renderer::.
//! Dependency direction: renderer → trend → scorer → snapshot.

use std::collections::HashMap;

use serde::Serialize;

use crate::scorer::HistoryEntry;

/// Number of same-branch history entries used to compute velocity and sparkline.
/// 8 entries covers ~2 months at weekly cadence — enough signal to detect
/// short-term trends without being dominated by very old history (AC-01.6, DA-04).
const VELOCITY_WINDOW: usize = 8;

/// Minimum points-per-run delta to classify a trend as improving or declining.
/// Changes below ±0.5 points/run are considered noise and classified as "stable"
/// (AC-04.6). Smaller than 0.5 = effectively zero for integer scoring.
const DIRECTION_THRESHOLD: f64 = 0.5;

/// Direction of the score trend over recent runs.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VelocityDirection {
    Improving,
    Declining,
    Stable,
}

/// Relative threshold for classifying an entity's series as growing or
/// shrinking — percent change from the oldest to the newest available
/// point. Unlike `DIRECTION_THRESHOLD` (tuned for 0-100 integer scores),
/// entity series (complexity, coupling degree, churn) have unrelated
/// natural scales, so classification uses percent change, not an absolute
/// delta (Decision 6).
const ENTITY_TREND_THRESHOLD_PCT: f64 = 0.15;

/// Direction of a per-entity metric series (complexity, coupling degree,
/// churn) across backfill samples — distinct from `VelocityDirection`,
/// which classifies the aggregate report score on an absolute scale.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityTrendDirection {
    Growing,
    Shrinking,
    Stable,
}

/// Classify a per-entity metric series by percent change from its oldest
/// to its newest point. Fewer than 2 points, or a zero baseline with no
/// growth, classifies as `Stable` (no signal yet / undefined percent
/// change). A zero baseline with growth classifies as `Growing`.
pub fn compute_entity_trend(series: &[f64]) -> EntityTrendDirection {
    let (Some(&first), Some(&last)) = (series.first(), series.last()) else {
        return EntityTrendDirection::Stable;
    };
    if series.len() < 2 {
        return EntityTrendDirection::Stable;
    }
    if first == 0.0 {
        return if last > 0.0 {
            EntityTrendDirection::Growing
        } else {
            EntityTrendDirection::Stable
        };
    }
    let pct_change = (last - first) / first;
    if pct_change > ENTITY_TREND_THRESHOLD_PCT {
        EntityTrendDirection::Growing
    } else if pct_change < -ENTITY_TREND_THRESHOLD_PCT {
        EntityTrendDirection::Shrinking
    } else {
        EntityTrendDirection::Stable
    }
}

/// Per-period rates from a running total. `churn_count` and `co_changes` are
/// recorded as all-time totals up to each sampled commit (backfill collects
/// every sample over `TimeWindow::full_history()`), so those series are
/// monotonically non-decreasing by construction: classifying them directly
/// makes `Shrinking` unreachable and turns `Growing` into "this file is still
/// alive". Differencing recovers the quantity a reader actually means by a
/// churn or coupling trend — how much activity each period carried — and with
/// it the ability to report a genuine slowdown.
///
/// A gap in the series (an entity that dropped out of a sample's top-N) makes
/// the next delta span two periods; that is the same sparse-window tolerance
/// `take_velocity_window` already accepts, not a special case.
fn period_rates(cumulative: &[f64]) -> Vec<f64> {
    cumulative
        .windows(2)
        .map(|w| (w[1] - w[0]).max(0.0))
        .collect()
}

/// Attach a `Growing`/`Shrinking`/`Stable` direction to every hotspot and
/// coupling pair that has at least 2 points of history — matched by path
/// (hotspots) or sorted pair key (coupling pairs). Entities absent from
/// `history` (never in a backfill sample's top-N, or no history exists yet)
/// are left as `None`, same "empty state" contract as `TrendSummary`.
pub fn attach_entity_trends(
    hotspots: &mut [crate::scorer::HotspotFile],
    pairs: &mut [crate::scorer::CouplingPair],
    history: &[crate::cache::entity_history::EntityTrendEntry],
) {
    // `history`'s write order is not guaranteed chronological (see
    // `select_samples`'s `count >= len` shortcut and multi-run appends), so
    // sort a local copy by timestamp once, up front, and have every loop
    // below read from it — `compute_entity_trend` relies on `series.first()`
    // being the oldest point and `series.last()` being the newest.
    let mut ordered: Vec<&crate::cache::entity_history::EntityTrendEntry> =
        history.iter().collect();
    ordered.sort_by_key(|e| e.timestamp);

    for h in hotspots.iter_mut() {
        let complexity_series: Vec<f64> = ordered
            .iter()
            .filter_map(|e| e.complexity.get(&h.path).copied())
            .map(|c| c as f64)
            .collect();
        if complexity_series.len() >= 2 {
            h.complexity_trend = Some(compute_entity_trend(&complexity_series));
        }

        // Cumulative: classify the per-period rate, not the running total.
        let churn_rates = period_rates(
            &ordered
                .iter()
                .filter_map(|e| e.churn.get(&h.path).copied())
                .map(|c| c as f64)
                .collect::<Vec<f64>>(),
        );
        if churn_rates.len() >= 2 {
            h.churn_trend = Some(compute_entity_trend(&churn_rates));
        }
    }

    for p in pairs.iter_mut() {
        let key = crate::cache::entity_history::entity_pair_key(&p.file_a, &p.file_b);
        // Cumulative, like churn — see `period_rates`.
        let rates = period_rates(
            &ordered
                .iter()
                .filter_map(|e| e.coupling_degree.get(&key).copied())
                .map(|c| c as f64)
                .collect::<Vec<f64>>(),
        );
        if rates.len() >= 2 {
            p.coupling_trend = Some(compute_entity_trend(&rates));
        }
    }
}

/// One point on the sparkline: score at a given commit.
#[derive(Debug, Clone, Serialize)]
pub struct SparklinePoint {
    pub score: u32,
    /// First 7 characters of the commit SHA.
    pub head_short: String,
}

/// Trend velocity computed from a window of recent same-branch runs.
#[derive(Debug, Clone, Serialize)]
pub struct TrendVelocity {
    pub direction: VelocityDirection,
    pub points_per_run: f64,
    pub window_size: usize,
}

/// Delta between the current run and the previous run on the same branch.
#[derive(Debug, Clone, Serialize)]
pub struct TrendDelta {
    /// Change in overall score vs the most recent prior run (positive =
    /// improving); `None` when either run had no measurable overall.
    pub overall: Option<i32>,
    /// Change in overall score vs the oldest entry in the history window;
    /// `None` on the same condition.
    pub delta_vs_oldest: Option<i32>,
    /// Per-category score deltas.
    pub categories: HashMap<String, i32>,
    /// True when there is no prior run on this branch to compare against.
    pub is_first: bool,
}

/// Full trend summary passed to renderers.
#[derive(Debug, Clone, Serialize)]
pub struct TrendSummary {
    pub delta: TrendDelta,
    pub sparkline: Vec<SparklinePoint>,
    pub velocity: Option<TrendVelocity>,
    /// True when the last recorded entry's branch differs from `current_branch`.
    pub branch_mismatch_warning: bool,
    pub history: Vec<HistoryEntry>,
}

/// Compute trend analytics from prior history.
///
/// `history` must NOT include `current_entry` — it is passed separately so the
/// sparkline can incorporate the current run's score.
/// Both `history` and `current_entry` are compared only within `current_branch`.
pub fn compute_trend(
    history: &[HistoryEntry],
    current_branch: &str,
    current_entry: &HistoryEntry,
) -> TrendSummary {
    // Filter history to same-branch entries only.
    let same_branch: Vec<&HistoryEntry> = history
        .iter()
        .filter(|e| e.branch == current_branch)
        .collect();

    if same_branch.is_empty() {
        // If history is non-empty but contains no same-branch entries, the prior
        // runs were on a different branch — emit a mismatch warning.
        let branch_mismatch_warning = !history.is_empty();
        return TrendSummary {
            delta: TrendDelta {
                overall: None,
                delta_vs_oldest: None,
                categories: HashMap::new(),
                is_first: true,
            },
            sparkline: build_sparkline(&same_branch, current_entry),
            velocity: None,
            branch_mismatch_warning,
            history: history.to_vec(),
        };
    }

    let last = *same_branch.last().unwrap();
    let oldest = *same_branch.first().unwrap();

    // Check for mismatch: if the very last entry in the full history (not just
    // same-branch) belongs to a different branch, warn the caller.
    let branch_mismatch_warning = history
        .last()
        .map(|e| e.branch != current_branch)
        .unwrap_or(false);

    let delta_overall = score_delta(last.overall_score, current_entry.overall_score);
    let delta_vs_oldest = score_delta(oldest.overall_score, current_entry.overall_score);

    let delta_categories = compute_category_deltas(&last.categories, &current_entry.categories);

    let sparkline = build_sparkline(&same_branch, current_entry);

    let velocity = compute_velocity(&same_branch, current_entry);

    TrendSummary {
        delta: TrendDelta {
            overall: delta_overall,
            delta_vs_oldest,
            categories: delta_categories,
            is_first: false,
        },
        sparkline,
        velocity,
        branch_mismatch_warning,
        history: history.to_vec(),
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// `current - previous`, only when both runs were measurable.
fn score_delta(previous: Option<u32>, current: Option<u32>) -> Option<i32> {
    Some(current? as i32 - previous? as i32)
}

/// Per-category deltas for the categories scored in *both* runs. A category
/// that just became measurable (or just stopped being) has no delta rather
/// than a misleading `+0`.
fn compute_category_deltas(
    previous: &HashMap<String, Option<u32>>,
    current: &HashMap<String, Option<u32>>,
) -> HashMap<String, i32> {
    current
        .iter()
        .filter_map(|(key, current_score)| {
            let current_score = (*current_score)?;
            let previous_score = previous.get(key).copied().flatten()?;
            Some((key.clone(), current_score as i32 - previous_score as i32))
        })
        .collect()
}

/// Return the most recent `VELOCITY_WINDOW` entries from `same_branch`, or all
/// entries if there are fewer than `VELOCITY_WINDOW`.
fn take_velocity_window<'a>(same_branch: &[&'a HistoryEntry]) -> Vec<&'a HistoryEntry> {
    if same_branch.len() > VELOCITY_WINDOW {
        same_branch[same_branch.len() - VELOCITY_WINDOW..].to_vec()
    } else {
        same_branch.to_vec()
    }
}

fn build_sparkline(
    same_branch: &[&HistoryEntry],
    current_entry: &HistoryEntry,
) -> Vec<SparklinePoint> {
    let window_entries = take_velocity_window(same_branch);

    // Runs with no measurable overall have no point: a gap, not a zero.
    window_entries
        .iter()
        .copied()
        .chain(std::iter::once(current_entry))
        .filter_map(|e| {
            Some(SparklinePoint {
                score: e.overall_score?,
                head_short: e.head[..e.head.len().min(7)].to_string(),
            })
        })
        .collect()
}

/// `None` when the current run or the oldest run in the window has no
/// measurable overall: velocity needs two real endpoints.
fn compute_velocity(
    same_branch: &[&HistoryEntry],
    current_entry: &HistoryEntry,
) -> Option<TrendVelocity> {
    let window = take_velocity_window(same_branch);

    let window_size = window.len() + 1; // +1 for current entry

    let last_score = current_entry.overall_score?;
    let first_score = window
        .first()
        .map(|e| e.overall_score)
        .unwrap_or(current_entry.overall_score)?;

    let total_change = last_score as i32 - first_score as i32;
    let runs = (window_size - 1).max(1) as f64;
    let points_per_run = total_change as f64 / runs;

    let direction = if points_per_run > DIRECTION_THRESHOLD {
        VelocityDirection::Improving
    } else if points_per_run < -DIRECTION_THRESHOLD {
        VelocityDirection::Declining
    } else {
        VelocityDirection::Stable
    };

    Some(TrendVelocity {
        direction,
        points_per_run,
        window_size,
    })
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scorer::HistoryCounts;
    use chrono::Utc;

    #[test]
    fn compute_entity_trend_50_percent_increase_is_growing() {
        assert_eq!(
            compute_entity_trend(&[10.0, 15.0]),
            EntityTrendDirection::Growing
        );
    }

    #[test]
    fn compute_entity_trend_20_percent_decrease_is_shrinking() {
        assert_eq!(
            compute_entity_trend(&[10.0, 8.0]),
            EntityTrendDirection::Shrinking
        );
    }

    #[test]
    fn compute_entity_trend_5_percent_increase_is_stable() {
        // Under the ±15% threshold.
        assert_eq!(
            compute_entity_trend(&[10.0, 10.5]),
            EntityTrendDirection::Stable
        );
    }

    #[test]
    fn compute_entity_trend_empty_series_is_stable() {
        assert_eq!(compute_entity_trend(&[]), EntityTrendDirection::Stable);
    }

    #[test]
    fn compute_entity_trend_single_point_is_stable() {
        assert_eq!(compute_entity_trend(&[10.0]), EntityTrendDirection::Stable);
    }

    #[test]
    fn compute_entity_trend_zero_baseline_with_growth_is_growing() {
        assert_eq!(
            compute_entity_trend(&[0.0, 5.0]),
            EntityTrendDirection::Growing
        );
    }

    #[test]
    fn compute_entity_trend_zero_baseline_no_growth_is_stable() {
        assert_eq!(
            compute_entity_trend(&[0.0, 0.0]),
            EntityTrendDirection::Stable
        );
    }

    #[test]
    fn compute_entity_trend_uses_oldest_and_newest_only() {
        // A dip in the middle must not affect the classification — only the
        // first and last points of the available window matter (Decision 6).
        assert_eq!(
            compute_entity_trend(&[10.0, 2.0, 14.0]),
            EntityTrendDirection::Growing
        );
    }

    #[test]
    fn compute_entity_trend_exactly_at_threshold_boundary_is_stable() {
        // 15.0% change is NOT > 15% — boundary is exclusive, matching
        // trend.rs's own DIRECTION_THRESHOLD comparison style (`>`, not `>=`).
        assert_eq!(
            compute_entity_trend(&[100.0, 115.0]),
            EntityTrendDirection::Stable
        );
    }

    fn make_entry(branch: &str, overall_score: u32, head: &str) -> HistoryEntry {
        let mut categories = HashMap::new();
        categories.insert("Health".to_string(), Some(overall_score));
        categories.insert("Team".to_string(), Some(overall_score));

        HistoryEntry {
            timestamp: Utc::now(),
            head: head.to_string(),
            overall_score: Some(overall_score),
            categories,
            metrics: HashMap::new(),
            counts: HistoryCounts {
                commits: 1,
                files: 1,
                authors: 1,
                ..Default::default()
            },
            branch: branch.to_string(),
            schema_version: 1,
            source: None,
        }
    }

    #[test]
    fn category_deltas_skip_categories_unscored_on_either_side() {
        let previous: HashMap<String, Option<u32>> = [
            ("Health".to_string(), Some(70)),
            ("Team".to_string(), None),
            ("Coupling".to_string(), Some(50)),
        ]
        .into_iter()
        .collect();
        let current: HashMap<String, Option<u32>> = [
            ("Health".to_string(), Some(75)),
            ("Team".to_string(), Some(38)),
            ("Coupling".to_string(), None),
        ]
        .into_iter()
        .collect();
        let deltas = compute_category_deltas(&previous, &current);
        assert_eq!(deltas.get("Health"), Some(&5));
        assert!(
            !deltas.contains_key("Team"),
            "a category that just became measurable has no delta, not +0"
        );
        assert!(!deltas.contains_key("Coupling"));
    }

    #[test]
    fn compute_trend_with_3_entries_has_non_null_velocity_and_correct_delta_vs_oldest() {
        let entry1 = make_entry("main", 60, "aaa0001");
        let entry2 = make_entry("main", 65, "aaa0002");
        let entry3 = make_entry("main", 68, "aaa0003");
        let current = make_entry("main", 72, "bbb1111");

        let history = vec![entry1, entry2, entry3];
        let summary = compute_trend(&history, "main", &current);

        // velocity must be non-null with 3+ same-branch prior entries
        assert!(
            summary.velocity.is_some(),
            "velocity should be Some when 3+ same-branch prior entries exist"
        );

        // delta_vs_oldest should be current - oldest (72 - 60 = 12), not current - last (72 - 68 = 4)
        assert_eq!(
            summary.delta.delta_vs_oldest,
            Some(12),
            "delta_vs_oldest should be current_score - oldest_score = 72 - 60 = 12"
        );
        assert_eq!(
            summary.delta.overall,
            Some(4),
            "delta.overall (delta_vs_last) should be current_score - last_score = 72 - 68 = 4"
        );
    }

    #[test]
    fn compute_trend_with_no_prior_entries_returns_is_first_true() {
        let current = make_entry("main", 75, "abc1234");
        let summary = compute_trend(&[], "main", &current);

        assert!(
            summary.delta.is_first,
            "is_first should be true when history is empty"
        );
        assert_eq!(summary.delta.overall, None, "no prior run, no delta");
    }

    #[test]
    fn compute_trend_with_one_prior_entry_computes_positive_delta() {
        let prior = make_entry("main", 70, "aaa0000");
        let current = make_entry("main", 75, "bbb1111");

        let summary = compute_trend(&[prior], "main", &current);

        assert!(
            !summary.delta.is_first,
            "is_first should be false when prior history exists"
        );
        assert_eq!(
            summary.delta.overall,
            Some(5),
            "delta should be 75 - 70 = +5"
        );
    }

    #[test]
    fn compute_trend_branch_mismatch_sets_warning() {
        // History exists only on "feature/refactor"; current branch is "main".
        // Expected: branch_mismatch_warning = true, delta.is_first = true.
        let prior = make_entry("feature/refactor", 70, "aaa0000");
        let current = make_entry("main", 75, "bbb1111");

        let summary = compute_trend(&[prior], "main", &current);

        assert!(
            summary.branch_mismatch_warning,
            "branch_mismatch_warning should be true when history is non-empty but no same-branch entries exist"
        );
        assert!(
            summary.delta.is_first,
            "is_first should be true when no same-branch prior entries exist"
        );
    }

    #[test]
    fn compute_trend_direction_improving_when_score_increases() {
        // Prior entries with ascending scores; current score exceeds all prior.
        // With 4 prior entries [60,65,68,72] and current 80:
        // velocity window: points_per_run = (80 - 60) / 4 = 5.0 > 0.5 → Improving
        let entry1 = make_entry("main", 60, "aaa0001");
        let entry2 = make_entry("main", 65, "aaa0002");
        let entry3 = make_entry("main", 68, "aaa0003");
        let entry4 = make_entry("main", 72, "aaa0004");
        let current = make_entry("main", 80, "bbb1111");

        let history = vec![entry1, entry2, entry3, entry4];
        let summary = compute_trend(&history, "main", &current);

        let velocity = summary
            .velocity
            .expect("velocity should be Some with prior entries");
        assert_eq!(
            velocity.direction,
            VelocityDirection::Improving,
            "direction should be Improving when current score exceeds prior scores"
        );
        assert!(
            summary.delta.overall > Some(0),
            "delta_vs_last should be positive when current score exceeds last score"
        );
    }

    #[test]
    fn compute_trend_direction_declining_when_score_drops() {
        // Prior entry with score 99; current score much lower (40).
        // velocity window: points_per_run = (40 - 99) / 1 = -59.0 < -0.5 → Declining
        let prior = make_entry("main", 99, "aaa0001");
        let current = make_entry("main", 40, "bbb1111");

        let history = vec![prior];
        let summary = compute_trend(&history, "main", &current);

        let velocity = summary
            .velocity
            .expect("velocity should be Some with prior entries");
        assert_eq!(
            velocity.direction,
            VelocityDirection::Declining,
            "direction should be Declining when current score is below last score"
        );
        assert!(
            summary.delta.overall.is_some_and(|delta| delta < 0),
            "delta_vs_last should be negative when current score drops, got: {:?}",
            summary.delta.overall
        );
    }

    #[test]
    fn compute_trend_filters_to_current_branch_only() {
        let on_other_branch = make_entry("feature", 90, "fff0000");
        let on_main = make_entry("main", 60, "aaa0000");
        let current = make_entry("main", 65, "bbb1111");

        let history = vec![on_other_branch, on_main];
        let summary = compute_trend(&history, "main", &current);

        assert!(
            !summary.delta.is_first,
            "should find prior entry on main branch"
        );
        assert_eq!(
            summary.delta.overall,
            Some(5),
            "delta should compare against main branch entry (65 - 60 = +5), not feature branch"
        );
    }

    /// Build a hotspot with only the fields these tests care about.
    fn trend_hotspot(path: &str) -> crate::scorer::HotspotFile {
        crate::scorer::HotspotFile {
            path: path.to_string(),
            role: crate::metrics::file_role::FileRole::Source,
            churn_count: 0,
            bug_commit_count: 0,
            loc: 100,
            total_lines: 100,
            cyclomatic_complexity: 10,
            public_methods: 0,
            properties: 0,
            hotspot_score: 50.0,
            coupling_trend: None,
            content_findings: 0,
            common_findings: 0,
            control_findings: 0,
            inheritance_findings: 0,
            churn_timeline: vec![],
            complexity_trend: None,
            churn_trend: None,
        }
    }

    /// One entry per sample, spaced a day apart, carrying a churn count for
    /// `path`. Churn is recorded as an all-time cumulative total, so these
    /// are totals, not per-period counts.
    fn churn_history(
        path: &str,
        totals: &[u32],
    ) -> Vec<crate::cache::entity_history::EntityTrendEntry> {
        use crate::cache::entity_history::EntityTrendEntry;
        use std::collections::HashMap;
        let base = chrono::Utc::now() - chrono::Duration::days(totals.len() as i64);
        totals
            .iter()
            .enumerate()
            .map(|(i, total)| {
                let mut churn = HashMap::new();
                churn.insert(path.to_string(), *total);
                EntityTrendEntry {
                    timestamp: base + chrono::Duration::days(i as i64),
                    head: format!("sha{i}"),
                    branch: "main".into(),
                    complexity: HashMap::new(),
                    churn,
                    coupling_degree: HashMap::new(),
                    schema_version: 1,
                }
            })
            .collect()
    }

    #[test]
    fn attach_entity_trends_reports_steady_churn_rate_as_stable() {
        // Backfill records churn as an all-time total up to each sampled
        // commit, so the series only ever climbs. A file touched at a
        // perfectly steady rate yields totals 10 -> 20 -> 30; classifying
        // the totals directly gives (30-10)/10 = +200% and calls a file
        // whose habits never changed "Growing". The direction has to be
        // read from the per-period rate, not the running total.
        let mut hotspots = vec![trend_hotspot("src/steady.rs")];
        let mut pairs: Vec<crate::scorer::CouplingPair> = vec![];

        attach_entity_trends(
            &mut hotspots,
            &mut pairs,
            &churn_history("src/steady.rs", &[10, 20, 30]),
        );

        assert_eq!(
            hotspots[0].churn_trend,
            Some(EntityTrendDirection::Stable),
            "a steady 10-commits-per-period rate is not a rising churn trend"
        );
    }

    #[test]
    fn attach_entity_trends_reports_decelerating_churn_as_shrinking() {
        // 10 commits in the first period, then only 2 in the second: the
        // file is calming down. `Shrinking` must be reachable at all — on a
        // monotonically climbing cumulative total it never is.
        let mut hotspots = vec![trend_hotspot("src/calming.rs")];
        let mut pairs: Vec<crate::scorer::CouplingPair> = vec![];

        attach_entity_trends(
            &mut hotspots,
            &mut pairs,
            &churn_history("src/calming.rs", &[10, 20, 22]),
        );

        assert_eq!(
            hotspots[0].churn_trend,
            Some(EntityTrendDirection::Shrinking),
            "churn dropping from 10 to 2 per period is a shrinking churn trend"
        );
    }

    #[test]
    fn attach_entity_trends_sets_direction_on_matching_hotspot() {
        use crate::cache::entity_history::EntityTrendEntry;
        use crate::scorer::HotspotFile;
        use std::collections::HashMap;

        let mut hotspots = vec![HotspotFile {
            path: "src/big.rs".to_string(),
            role: crate::metrics::file_role::FileRole::Source,
            churn_count: 10,
            bug_commit_count: 0,
            loc: 500,
            total_lines: 500,
            cyclomatic_complexity: 50,
            public_methods: 0,
            properties: 0,
            hotspot_score: 90.0,
            coupling_trend: None,
            content_findings: 0,
            common_findings: 0,
            control_findings: 0,
            inheritance_findings: 0,
            churn_timeline: vec![],
            complexity_trend: None,
            churn_trend: None,
        }];
        let mut pairs: Vec<crate::scorer::CouplingPair> = vec![];

        let mut complexity_a = HashMap::new();
        complexity_a.insert("src/big.rs".to_string(), 10u32);
        let mut complexity_b = HashMap::new();
        complexity_b.insert("src/big.rs".to_string(), 50u32);
        // Three samples, because churn is a running total and is classified
        // on its per-period rate (`period_rates`): N totals give N-1 rates,
        // and a direction needs two of them.
        let mut complexity_c = HashMap::new();
        complexity_c.insert("src/big.rs".to_string(), 90u32);
        // Steady 20 commits per period — a running total that climbs, but a
        // rate that does not move.
        let churn_totals = [20u32, 40u32, 60u32];
        let mut churn_maps = churn_totals.iter().map(|total| {
            let mut m = HashMap::new();
            m.insert("src/big.rs".to_string(), *total);
            m
        });

        let now = chrono::Utc::now();
        let history = vec![
            EntityTrendEntry {
                timestamp: now - chrono::Duration::days(2),
                head: "aaa".into(),
                branch: "main".into(),
                complexity: complexity_a,
                churn: churn_maps.next().unwrap(),
                coupling_degree: HashMap::new(),
                schema_version: 1,
            },
            EntityTrendEntry {
                timestamp: now - chrono::Duration::days(1),
                head: "bbb".into(),
                branch: "main".into(),
                complexity: complexity_b,
                churn: churn_maps.next().unwrap(),
                coupling_degree: HashMap::new(),
                schema_version: 1,
            },
            EntityTrendEntry {
                timestamp: now,
                head: "ccc".into(),
                branch: "main".into(),
                complexity: complexity_c,
                churn: churn_maps.next().unwrap(),
                coupling_degree: HashMap::new(),
                schema_version: 1,
            },
        ];

        attach_entity_trends(&mut hotspots, &mut pairs, &history);

        assert_eq!(
            hotspots[0].complexity_trend,
            Some(EntityTrendDirection::Growing),
            "10 -> 90 is a 800% increase, well past the 15% threshold"
        );
        assert_eq!(
            hotspots[0].churn_trend,
            Some(EntityTrendDirection::Stable),
            "a steady 20-per-period churn rate is Stable even while complexity \
             climbs — complexity and churn must classify independently"
        );
    }

    #[test]
    fn attach_entity_trends_leaves_none_for_unmatched_hotspot() {
        use crate::scorer::HotspotFile;

        let mut hotspots = vec![HotspotFile {
            path: "src/never_seen.rs".to_string(),
            role: crate::metrics::file_role::FileRole::Source,
            churn_count: 0,
            bug_commit_count: 0,
            loc: 10,
            total_lines: 10,
            cyclomatic_complexity: 1,
            public_methods: 0,
            properties: 0,
            hotspot_score: 5.0,
            coupling_trend: None,
            content_findings: 0,
            common_findings: 0,
            control_findings: 0,
            inheritance_findings: 0,
            churn_timeline: vec![],
            complexity_trend: None,
            churn_trend: None,
        }];
        let mut pairs: Vec<crate::scorer::CouplingPair> = vec![];

        attach_entity_trends(&mut hotspots, &mut pairs, &[]);

        assert_eq!(hotspots[0].complexity_trend, None);
        assert_eq!(hotspots[0].churn_trend, None);
    }

    #[test]
    fn attach_entity_trends_sorts_history_by_timestamp_before_classifying() {
        use crate::cache::entity_history::EntityTrendEntry;
        use crate::scorer::HotspotFile;
        use chrono::{Duration, Utc};
        use std::collections::HashMap;

        let mut hotspots = vec![HotspotFile {
            path: "src/big.rs".to_string(),
            role: crate::metrics::file_role::FileRole::Source,
            churn_count: 10,
            bug_commit_count: 0,
            loc: 500,
            total_lines: 500,
            cyclomatic_complexity: 50,
            public_methods: 0,
            properties: 0,
            hotspot_score: 90.0,
            coupling_trend: None,
            content_findings: 0,
            common_findings: 0,
            control_findings: 0,
            inheritance_findings: 0,
            churn_timeline: vec![],
            complexity_trend: None,
            churn_trend: None,
        }];
        let mut pairs: Vec<crate::scorer::CouplingPair> = vec![];

        let now = Utc::now();
        let older = now - Duration::days(10);
        let newer = now;

        let mut complexity_older = HashMap::new();
        complexity_older.insert("src/big.rs".to_string(), 10u32);
        let mut complexity_newer = HashMap::new();
        complexity_newer.insert("src/big.rs".to_string(), 50u32);

        // History vec is in REVERSE chronological order (newest first, oldest
        // last) — exactly what `select_samples`'s `count >= len` shortcut
        // produces, since `collect_commits` yields commits newest-first.
        let history = vec![
            EntityTrendEntry {
                timestamp: newer,
                head: "bbb".into(),
                branch: "main".into(),
                complexity: complexity_newer,
                churn: HashMap::new(),
                coupling_degree: HashMap::new(),
                schema_version: 1,
            },
            EntityTrendEntry {
                timestamp: older,
                head: "aaa".into(),
                branch: "main".into(),
                complexity: complexity_older,
                churn: HashMap::new(),
                coupling_degree: HashMap::new(),
                schema_version: 1,
            },
        ];

        attach_entity_trends(&mut hotspots, &mut pairs, &history);

        // Complexity genuinely grew from 10 (older) to 50 (newer). Without
        // sorting by timestamp first, the pre-fix code would treat the vec's
        // first entry (newer, 50) as "oldest" and the last entry (older, 10)
        // as "newest", computing a -80% change and misclassifying this as
        // Shrinking instead of Growing.
        assert_eq!(
            hotspots[0].complexity_trend,
            Some(EntityTrendDirection::Growing),
            "history must be sorted by timestamp before classifying, regardless of vec order"
        );
    }

    #[test]
    fn attach_entity_trends_classifies_coupling_pair_trend_with_key_normalization() {
        use crate::cache::entity_history::EntityTrendEntry;
        use chrono::{Duration, Utc};
        use std::collections::HashMap;

        let mut hotspots: Vec<crate::scorer::HotspotFile> = vec![];
        let mut pairs = vec![crate::scorer::CouplingPair {
            // Reversed order from the stored key ("src/a.rs|src/b.rs") to
            // prove `entity_pair_key`'s normalization is applied at the
            // consumption site, not just at construction.
            file_a: "src/b.rs".to_string(),
            file_b: "src/a.rs".to_string(),
            co_changes: 9,
            coupling_pct: 90.0,
            growth_a: 0,
            growth_b: 0,
            cross_boundary: true,
            is_test_pair: false,
            coupling_trend: None,
        }];

        let now = Utc::now();
        let older = now - Duration::days(10);
        let newer = now;

        // Coupling degree is a running total too, so three samples are needed
        // for two rates: 3 -> 6 -> 21 gives rates 3 then 15, a real
        // acceleration rather than a merely climbing total.
        let mut coupling_newer = HashMap::new();
        coupling_newer.insert("src/a.rs|src/b.rs".to_string(), 21usize);
        let mut coupling_mid = HashMap::new();
        coupling_mid.insert("src/a.rs|src/b.rs".to_string(), 6usize);
        let mut coupling_older = HashMap::new();
        coupling_older.insert("src/a.rs|src/b.rs".to_string(), 3usize);

        // Non-chronological vec order (newest first) to also exercise the
        // Fix 1 sort.
        let history = vec![
            EntityTrendEntry {
                timestamp: newer,
                head: "bbb".into(),
                branch: "main".into(),
                complexity: HashMap::new(),
                churn: HashMap::new(),
                coupling_degree: coupling_newer,
                schema_version: 1,
            },
            EntityTrendEntry {
                timestamp: now - chrono::Duration::days(5),
                head: "mid".into(),
                branch: "main".into(),
                complexity: HashMap::new(),
                churn: HashMap::new(),
                coupling_degree: coupling_mid,
                schema_version: 1,
            },
            EntityTrendEntry {
                timestamp: older,
                head: "aaa".into(),
                branch: "main".into(),
                complexity: HashMap::new(),
                churn: HashMap::new(),
                coupling_degree: coupling_older,
                schema_version: 1,
            },
        ];

        attach_entity_trends(&mut hotspots, &mut pairs, &history);

        assert_eq!(
            pairs[0].coupling_trend,
            Some(EntityTrendDirection::Growing),
            "3 -> 9 is a 200% increase; key normalization must match \
             file_a/file_b regardless of their order vs. the stored key"
        );
    }

    #[test]
    fn attach_entity_trends_leaves_none_with_only_one_history_point() {
        use crate::cache::entity_history::EntityTrendEntry;
        use crate::scorer::HotspotFile;
        use std::collections::HashMap;

        let mut hotspots = vec![HotspotFile {
            path: "src/big.rs".to_string(),
            role: crate::metrics::file_role::FileRole::Source,
            churn_count: 10,
            bug_commit_count: 0,
            loc: 500,
            total_lines: 500,
            cyclomatic_complexity: 50,
            public_methods: 0,
            properties: 0,
            hotspot_score: 90.0,
            coupling_trend: None,
            content_findings: 0,
            common_findings: 0,
            control_findings: 0,
            inheritance_findings: 0,
            churn_timeline: vec![],
            complexity_trend: None,
            churn_trend: None,
        }];
        let mut pairs: Vec<crate::scorer::CouplingPair> = vec![];

        let mut complexity = HashMap::new();
        complexity.insert("src/big.rs".to_string(), 10u32);

        let history = vec![EntityTrendEntry {
            timestamp: chrono::Utc::now(),
            head: "aaa".into(),
            branch: "main".into(),
            complexity,
            churn: HashMap::new(),
            coupling_degree: HashMap::new(),
            schema_version: 1,
        }];

        attach_entity_trends(&mut hotspots, &mut pairs, &history);

        assert_eq!(
            hotspots[0].complexity_trend, None,
            "a single history point must leave the trend as None (no signal), not Some(Stable)"
        );
    }
}
