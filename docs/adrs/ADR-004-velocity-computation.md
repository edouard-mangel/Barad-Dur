# ADR-004: Velocity Computation Algorithm

**Status**: Accepted
**Date**: 2026-03-18
**Feature**: historical-trends
**Deciders**: Morgan (Solution Architect)

---

## Context

The `historical-trends` feature exposes a "velocity" metric (FR-04: full history table with "velocity and category insights"). Velocity answers: "is this repository's health score trending up or down, and how fast?"

Key constraints:
- DA-04 (DESIGN decision): rolling 8-entry window
- D-04: only same-branch entries are included
- Scores are integers 0–100 (u32), but velocity is a floating-point rate
- Velocity must be stable enough to be meaningful (not dominated by a single outlier commit)
- Velocity must be computed from the trend store without additional git calls (D-02)

---

## Decision

**Linear regression slope over the most recent min(N, VELOCITY_WINDOW) same-branch entries, where VELOCITY_WINDOW = 8.**

The velocity in "score points per run" is the slope β₁ of the ordinary least-squares linear fit over the index sequence x = [0, 1, …, n-1] and scores y = [score₀, score₁, …, scoreₙ₋₁].

**Formula** (closed-form for evenly-spaced integer x):

```
n = number of entries in window (2 ≤ n ≤ 8)
x_mean = (n - 1) / 2.0
y_mean = sum(scores) / n
numerator   = sum((i - x_mean) * (score_i - y_mean)) for i in 0..n
denominator = sum((i - x_mean)^2) for i in 0..n
velocity    = numerator / denominator
```

**Direction thresholds** (defined as constants in `src/trend.rs`):
- `velocity > 0.5`: `VelocityDirection::Improving`
- `velocity < -0.5`: `VelocityDirection::Declining`
- `-0.5 ≤ velocity ≤ 0.5`: `VelocityDirection::Stable`

The ±0.5 threshold corresponds to less than 1 score point change per run, which is within the rounding noise of the weighted scoring formula.

**Edge cases**:
- Fewer than 2 entries: `velocity = None` (not enough data to compute a rate)
- All entries have the same score: denominator is non-zero (since x values are distinct integers); velocity = 0.0 exactly → `Stable`
- Window uses only same-branch entries; entries from other branches are excluded before windowing

---

## Why linear regression over simple delta

A simple "last - previous" delta is already provided by `TrendDelta.overall`. Velocity is a different metric: it characterises the *trajectory* over time, not the last step. Linear regression over the window is more resistant to outliers than a point-to-point average because all points contribute to the slope estimate with equal weight. For 8 entries, the computation cost is identical to a simple mean (8 multiplications and additions).

---

## Alternatives Considered

### Simple delta (last entry minus first entry in window, divided by window size)

Formula: `(score[n-1] - score[0]) / (n - 1)`

**Rejected as the primary velocity metric** (though this formula is close to the OLS slope for evenly-spaced data with low variance). The OLS slope is preferred because:
- It uses all intermediate points, not just the endpoints
- It is more robust when a single intermediate entry is an outlier (e.g., a run on a branch with an accidental regression quickly reverted)
- The numerical difference between the two formulas is small in practice; OLS is not more complex to implement

This formula is still valid as a quick sanity check in tests.

### Exponentially weighted moving average (EWMA)

**Rejected.** EWMA requires a smoothing factor α (a hyperparameter) and produces a dimensionless smoothed value, not a rate. Explaining "EWMA = 72.4" to a user is not actionable. A slope in "points per run" is directly interpretable.

### Rolling mean of per-step deltas

Formula: `mean([score[i] - score[i-1] for i in 1..n])`

This is mathematically equivalent to `(score[n-1] - score[0]) / (n - 1)` (the deltas telescope), so it reduces to the same formula as the simple endpoint delta. No advantage over OLS.

### Full-history regression

**Rejected** per DA-04. Full history distorts current trajectory because early runs (when the score first stabilised from an initial high-variance phase) drag the baseline. An 8-run window keeps the velocity signal current and actionable.

---

## Sparkline generation (related)

The sparkline uses the same window of entries as velocity. The 8 scores are mapped to 8 Unicode block levels `▁▂▃▄▅▆▇█` using linear interpolation between the min and max score in the window:

```
level = floor((score - min_score) / (max_score - min_score + ε) * 7)
char  = BLOCKS[level]
where BLOCKS = ['▁','▂','▃','▄','▅','▆','▇','█']
```

When `max_score == min_score` (flat line), all characters are `▄` (mid-block, level 3). The small ε = 1e-6 prevents division by zero.

---

## Consequences

**Positive**:
- Linear regression is a well-understood algorithm with no edge-case surprises for this data range
- Produces an interpretable, signed, real-valued rate in "score points per run"
- Resistant to single-run outliers
- Pure arithmetic, no external crate required

**Negative**:
- OLS slope is slightly less intuitive to explain than "it went from 68 to 74 in 6 runs = +1 per run". Mitigated by also showing the explicit sparkline and the simple delta, so velocity is supplementary context, not the primary number.
- The ±0.5 stability threshold is a design choice; users who make many small changes may always see `Stable`. Threshold is a named constant and can be tuned.
