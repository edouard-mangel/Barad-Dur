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
}
