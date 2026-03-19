# Component Boundaries — Historical Trends

Feature: `historical-trends`
Wave: DESIGN
Date: 2026-03-18

---

## Pre-existing infrastructure (critical discovery)

Before designing new components, the existing codebase already provides:

| Existing artefact | Location | Status |
|---|---|---|
| `HistoryEntry` struct | `src/scorer.rs` lines 85–100 | EXISTS — reuse as-is |
| `HistoryCounts` struct | `src/scorer.rs` lines 86–90 | EXISTS — reuse as-is |
| `build_history_entry()` | `src/scorer.rs` lines 102–125 | EXISTS — reuse as-is |
| `cache::history::append_if_new_head()` | `src/cache/history.rs` | EXISTS — reuse as-is |
| `cache::history::load_history()` | `src/cache/history.rs` | EXISTS — reuse as-is |
| History recording in `run_analyze()` | `src/main.rs` lines 154–157 | EXISTS — branch-isolation modification needed |
| History load for HTML | `src/main.rs` line 160 | EXISTS — extend for `--trend` |

The NDJSON append pattern (one JSON object per line, deduplication by HEAD SHA) is **already implemented**. The `historical-trends` feature builds on top of this foundation; it does not replace it.

---

## New components

### `src/trend.rs` (NEW)

**Responsibility**: Pure computation of trend analytics from a slice of `HistoryEntry`. No I/O.

**Boundary rules**:
- Imports: `crate::scorer::{HistoryEntry, HistoryCounts}`, standard library, `chrono`
- No imports from `cache`, `renderer`, `cli`, or `main`
- All public functions are pure (no side effects, no `mut` on inputs)

**Public surface** (types and function signatures — not implementation):

```
pub struct TrendDelta {
    pub overall: i32,           // score points vs previous entry (same branch)
    pub categories: HashMap<String, i32>,
    pub is_first: bool,         // true when no prior same-branch entry exists
}

pub struct SparklinePoint {
    pub score: u32,
    pub head_short: String,     // first 7 chars of HEAD SHA
}

pub struct TrendVelocity {
    pub points_per_run: f64,    // rolling window velocity (DA-04)
    pub window_size: usize,     // actual entries used (may be < VELOCITY_WINDOW)
    pub direction: VelocityDirection,
}

pub enum VelocityDirection { Improving, Declining, Stable }

pub struct TrendSummary {
    pub delta: TrendDelta,
    pub sparkline: Vec<SparklinePoint>,   // ≤ VELOCITY_WINDOW points
    pub velocity: Option<TrendVelocity>,  // None when < 2 entries
    pub branch_mismatch_warning: bool,
    pub history: Vec<HistoryEntry>,        // full branch-filtered history
}

pub const VELOCITY_WINDOW: usize = 8;

// Compute full TrendSummary for current branch
pub fn compute_trend(
    history: &[HistoryEntry],
    current_branch: &str,
    current_entry: &HistoryEntry,
) -> TrendSummary

// Format sparkline as unicode block chars for CLI (e.g. "▁▂▄▆█")
pub fn format_sparkline(points: &[SparklinePoint]) -> String

// Format velocity as "+N.N pts/run" or "-N.N pts/run" or "stable"
pub fn format_velocity(velocity: &TrendVelocity) -> String
```

---

### `src/cache/trend_store.rs` (NEW — replaces role of current `cache/history.rs`)

**Decision**: Do NOT create a new file. The existing `src/cache/history.rs` already implements the required persistence contract (NDJSON append, deduplication, load). This new module is NOT needed.

The only modification needed to `src/cache/history.rs` is: **rename `history.json` to `trends.json`** (matching FR-01). This is a one-line constant change.

---

## Modified files

### `src/cache/history.rs` — MODIFIED

Changes:
1. Rename `HISTORY_FILE` constant from `"history.json"` to `"trends.json"` (FR-01)
2. Add `branch` parameter to `append_if_new_head()` — store is already branch-agnostic but D-04 requires branch isolation for delta computation; the branch field already exists on `HistoryEntry` via `report.branch` passed through `build_history_entry()`
3. Add `maybe_migrate_history(repo_path: &Path) -> Result<()>` function: if `history.json` exists and `trends.json` does not, copy `history.json` → `trends.json`. Called once at startup in `main.rs` before the history load step. Non-destructive: `history.json` is left in place.

Note: `HistoryEntry` does not currently store `branch`. This field must be added (see `data-models.md`).

---

### `src/scorer.rs` — MODIFIED (minimal)

Changes:
1. Add `branch: String` field to `HistoryEntry` (required for D-04 branch isolation)
2. Add `schema_version: u32` field to `HistoryEntry` with `#[serde(default)]` (required for DA-05)
3. Update `build_history_entry()` to populate both new fields from the report

No changes to `AnalysisReport` struct (NFR-03: backward compat preserved).

---

### `src/cli.rs` — MODIFIED (minimal)

Changes:
1. Add `--trend` flag to `AnalyzeArgs`:
   ```
   /// Show full trend history table with velocity and category insights
   #[arg(long, help_heading = "Output Format")]
   pub trend: bool,
   ```

No other changes.

---

### `src/main.rs` — MODIFIED (minimal)

Changes:
1. After building the report (line 148), before the history recording block, compute `TrendSummary` by calling `trend::compute_trend(&report.history, &report.branch, &history_entry)`
2. Pass `trend_summary: Option<&TrendSummary>` to CLI and JSON renderers
   - CLI renderer always receives it (shows delta/sparkline when `!trend_summary.is_none()`)
   - JSON renderer receives it only when `args.trend` is true (NFR-03)
   - HTML renderer does not receive it (HTML tab reads `report.history` directly, DA-03)
3. Load history before computing trend summary (already done on line 160 — move earlier)

Pipeline order after modification:
```
collect → snapshot → metrics → score → load_history → compute_trend → record_entry → render
```

Note: `compute_trend` uses the history *before* appending the current entry. This means the delta is computed against the previous run, not the current. The current entry is passed separately as `current_entry` so the sparkline can include the current run's score.

---

### `src/renderer/cli.rs` — MODIFIED

Changes:
1. Add `trend_summary: Option<&TrendSummary>` parameter to `render()` function
2. After the "Overall Score" line, inject the trend delta block:
   - When `trend_summary.is_none()` or `trend_summary.delta.is_first`: emit `"  Trend: first snapshot recorded\n"` (FR-03)
   - When `trend_summary` present and `!delta.is_first`: emit delta line with sparkline (FR-02)
   - When `args.trend` (determined by caller): emit full history table section (FR-04)

The `trend_summary` data is computed in `main.rs` and passed in; the renderer does not perform I/O or computation.

---

### `src/renderer/json.rs` — MODIFIED

Changes:
1. Add `trend_data: Option<&TrendSummary>` parameter to `render()` function
2. When `trend_data` is `Some(summary)`, serialize the report then inject a `"trend"` key via `serde_json::Value` manipulation before final serialization
3. When `trend_data` is `None`, output is structurally identical to current output (NFR-03)

The `"trend"` JSON key shape is defined in `data-models.md`.

---

### `src/renderer/html.rs` — MODIFIED

Changes:
1. The HTML renderer reads `report.history` (already present) — no new parameter needed (DA-03)
2. Add Trends tab to the tab navigation
3. Embed trend chart and table in the new tab using the `history` data already embedded via `window.R`
4. The frontend JavaScript computes sparklines and velocity client-side from the `window.R.history` array

---

## Untouched files

The following files require no changes for this feature:

- `src/collector/` — all collector modules
- `src/snapshot.rs`
- `src/metrics/` — all metric modules
- `src/config.rs` — (optional: add `max_trend_entries` as commented-out reserved key; not required)
- `src/cache/storage.rs`
- `src/cache/staleness.rs`
- `src/cache/blame.rs`
- `src/init.rs`
- `src/remote/`

---

## Architecture enforcement (Rust)

Rust's module visibility system enforces component boundaries at compile time. Additional rules:

- `src/trend.rs` must import nothing from `src/cache/` or `src/renderer/` — enforced by compiler if violated
- `src/renderer/*.rs` must not import from `src/cache/` — enforced by compiler
- `src/cache/history.rs` must not import from `src/trend.rs` — the dependency arrow must point outward (cache is a port, trend is a computation layer above it)

Recommended enforcement tool: **cargo-deny** (for dependency graph) + custom `#[deny(unused_imports)]` and module-level doc comments stating the allowed import boundary. For strict architectural tests, the software-crafter may add integration tests that verify the compiled dependency graph using `cargo-tree --prefix none` in CI.
