# Unit Tests Specification: `src/trend.rs`

Pure function unit tests for the trend computation module. These tests live as
`#[cfg(test)]` modules within `src/trend.rs` and do NOT use `#[ignore]` — they
test pure functions and should pass immediately once `trend.rs` is implemented.

---

## Module under test: `src/trend.rs`

Expected public surface (derived from architecture-design):

```rust
pub struct TrendEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub commit: String,
    pub branch: String,
    pub overall_score: u32,
    pub category_scores: CategoryScores,
    pub schema_version: u32,
}

pub struct CategoryScores {
    pub health: u32,
    pub team: u32,
    pub evolution: u32,
    pub git_hygiene: u32,
}

pub struct TrendSummary {
    pub direction: TrendDirection,
    pub delta_vs_last: i32,
    pub delta_vs_oldest: i32,
    pub velocity_per_week: Option<f64>,
    pub sparkline: Vec<SparklinePoint>,
    pub schema_version: u32,
    pub snapshots: Vec<TrendEntry>,
}

pub enum TrendDirection {
    Improving,
    Declining,
    Stable,
}

pub struct SparklinePoint {
    pub score: u32,
    pub branch: String,
}

/// Compute trend summary from a slice of prior entries and the current score.
/// `prior` entries are ordered chronologically (oldest first).
/// `current_branch` is the branch of the in-progress run.
pub fn compute_trend(
    prior: &[TrendEntry],
    current_score: u32,
    current_category_scores: &CategoryScores,
    current_branch: &str,
) -> TrendSummary;
```

---

## Test Groups

### Group 1: `compute_trend` with 0 prior entries (first run)

```rust
#[test]
fn compute_trend_with_no_prior_entries_returns_first_run_summary() {
    // Given: no prior trend entries
    // When: compute_trend called with empty slice
    // Then:
    //   - direction is Stable (no comparison possible)
    //   - delta_vs_last is 0
    //   - delta_vs_oldest is 0
    //   - velocity_per_week is None
    //   - sparkline contains exactly 1 point (current score)
    //   - snapshots is empty (prior only)
}

#[test]
fn compute_trend_velocity_is_none_when_zero_prior_entries() {
    // Given: no prior entries
    // When: compute_trend called
    // Then: velocity_per_week is None
}
```

### Group 2: `compute_trend` with exactly 1 prior entry

```rust
#[test]
fn compute_trend_with_one_prior_entry_shows_delta_vs_last() {
    // Given: 1 prior entry with overall_score 70, same branch
    // When: compute_trend called with current_score 75
    // Then:
    //   - delta_vs_last is +5
    //   - delta_vs_oldest is +5 (same as delta_vs_last when only 1 prior)
    //   - direction is Improving
    //   - sparkline has 2 points: [70, 75]
}

#[test]
fn compute_trend_declining_direction_when_score_drops() {
    // Given: 1 prior entry with overall_score 80, same branch
    // When: compute_trend called with current_score 70
    // Then:
    //   - delta_vs_last is -10
    //   - direction is Declining
}

#[test]
fn compute_trend_stable_direction_when_score_unchanged() {
    // Given: 1 prior entry with overall_score 65, same branch
    // When: compute_trend called with current_score 65
    // Then:
    //   - delta_vs_last is 0
    //   - direction is Stable
}

#[test]
fn compute_trend_velocity_is_none_with_one_prior_entry() {
    // Given: 1 prior entry
    // When: compute_trend called
    // Then: velocity_per_week is None
    // (velocity requires at least 2 prior entries per AC-04.5)
}
```

### Group 3: `compute_trend` with N prior entries (N >= 2)

```rust
#[test]
fn compute_trend_velocity_calculated_from_first_and_last_prior_entries() {
    // Given: 2 prior entries: score 60 (4 weeks ago) and score 68 (now-ish)
    // When: compute_trend called with current_score 72
    // Then: velocity_per_week is approximately (72 - 60) / 4.0 = 3.0
    // (within ±0.1 to account for floating-point arithmetic)
}

#[test]
fn compute_trend_delta_vs_oldest_uses_first_prior_entry() {
    // Given: prior entries with overall_scores [50, 55, 60]
    // When: compute_trend called with current_score 65
    // Then:
    //   - delta_vs_oldest is +15 (65 - 50)
    //   - delta_vs_last is +5 (65 - 60)
}

#[test]
fn compute_trend_sparkline_capped_at_8_entries() {
    // Given: 10 prior entries
    // When: compute_trend called
    // Then: sparkline contains at most 8 points (most recent 7 + current)
    // Implementation note: "..." omission is a render concern, not a compute concern.
    //   The compute layer returns the capped slice; renderer decides formatting.
}

#[test]
fn compute_trend_velocity_rounded_to_2_decimal_places() {
    // Given: prior entries with scores [60, 67] separated by 3 weeks
    // When: compute_trend called with current_score 71
    // Then: velocity_per_week is (71 - 60) / 3.0 = 3.67 (rounded to 2dp)
}
```

### Group 4: Branch filtering

```rust
#[test]
fn compute_trend_uses_only_same_branch_prior_entries() {
    // Given: 3 prior entries — 2 on "main", 1 on "feature/x"
    // When: compute_trend called with current_branch "main"
    // Then: computation uses only the 2 "main" entries
    //   - delta_vs_last is relative to the most recent "main" entry
    //   - snapshots in TrendSummary contains only "main" entries
}

#[test]
fn compute_trend_branch_mismatch_returns_no_delta() {
    // Given: all prior entries are on branch "feature/refactor"
    // When: compute_trend called with current_branch "main"
    // Then:
    //   - delta_vs_last is 0
    //   - delta_vs_oldest is 0
    //   - direction is Stable (cannot compare across branches)
    //   - velocity_per_week is None
    //   - sparkline contains only the current point
}
```

### Group 5: Idempotency and boundary conditions

```rust
#[test]
fn compute_trend_same_commit_sha_as_last_entry_is_still_computed() {
    // Given: 1 prior entry with the same commit SHA as current run
    // When: compute_trend called
    // Then: summary is returned (deduplication is a storage concern, not compute)
    //   - delta_vs_last is 0 (same score if entry is identical)
}

#[test]
fn compute_trend_scores_are_clamped_at_100() {
    // Given: prior entry with overall_score 95
    // When: compute_trend called with current_score 100
    // Then: delta_vs_last is +5 (no overflow, no clamp issue)
}

#[test]
fn direction_is_stable_when_delta_is_zero_for_multiple_prior_entries() {
    // Given: prior entries all with score 70, current_score 70
    // When: compute_trend called
    // Then: direction is Stable
}
```

### Group 6: `TrendDirection` serialization (for JSON output)

```rust
#[test]
fn trend_direction_serializes_to_lowercase_string() {
    // TrendDirection must serialize to exactly: "improving" | "declining" | "stable"
    // per AC-04.6 ("direction is exactly one of...")
    assert_eq!(serde_json::to_string(&TrendDirection::Improving).unwrap(), "\"improving\"");
    assert_eq!(serde_json::to_string(&TrendDirection::Declining).unwrap(), "\"declining\"");
    assert_eq!(serde_json::to_string(&TrendDirection::Stable).unwrap(),    "\"stable\"");
}
```

---

## Implementation Notes for Software-Crafter

1. `compute_trend` is a pure function — no I/O, no side effects. All file access
   lives in `src/cache/history.rs`.
2. Velocity formula: `(last_overall - first_overall) / weeks_between(first, last)`
   where `first` and `last` are the oldest and most recent same-branch prior entries.
   Weeks are fractional (use float division on seconds/604800.0).
3. `velocity_per_week` is `None` (serializes to JSON `null`) when fewer than 2
   same-branch prior entries exist (AC-04.5). It must serialize as `null`, not omitted.
4. Sparkline cap: retain the most recent min(N, 7) prior entries plus the current point
   for up to 8 sparkline points total.
5. `schema_version` is always 1 in the current design.
