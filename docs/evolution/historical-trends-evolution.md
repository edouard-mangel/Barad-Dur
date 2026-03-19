# historical-trends — Evolution Log

**Feature**: Historical Trend Snapshots
**DELIVER wave completed**: 2026-03-18
**Commits**: 85bd0e7..8e43457 (18 steps + refactor)
**Tests added**: 42 (18 acceptance integration + 21 unit + 3 walking skeleton)
**Total test count**: 221 (was 166+)

## What was built

`barad-dur analyze` now:
1. **Auto-records** every run to `.repository-analysis/trends.json` (NDJSON, one object per line)
2. **Shows inline delta** vs last run on same branch: `72/100  (+3 vs last run)`
3. **Per-category deltas** — each of the 4 category rows shows `(+N)` / `(-N)`
4. **Direction indicator** — `→ stable` / `↑ improving` / `↓ declining` after the score line
5. **Branch mismatch warning** — suppresses delta and warns when prior history is on a different branch
6. **`--trend --json`** — injects a `"trend"` JSON object with snapshots, delta, velocity, direction
7. **Backward compatible** — `--json` without `--trend` is structurally identical to pre-feature output
8. **Resilient** — corrupt `trends.json` is archived to `.bak` and recreated; tool never aborts

## New files

- `src/trend.rs` — pure computation module (no I/O, no cache/renderer imports)
- `tests/trend_walking_skeleton.rs` — 3 walking skeleton acceptance tests
- `tests/trend_milestone_1.rs` — 15 milestone acceptance tests
- `tests/common/mod.rs` — shared test helpers (extracted during L1-L4 refactor)

## Modified files

- `src/scorer.rs` — `HistoryEntry` + `branch: String` + `schema_version: u32` (both `#[serde(default)]`)
- `src/cache/history.rs` — renamed to `trends.json`; added `archive_and_replace()`, `load_history_checked()`
- `src/cli.rs` — added `--trend` flag
- `src/main.rs` — pipeline: `load_history → compute_trend → record → render`
- `src/renderer/cli.rs` — delta/sparkline/direction rendering
- `src/renderer/json.rs` — trend JSON object injection when `--trend` flag present
- `src/lib.rs` — `pub mod trend;`

## Key design decisions (from DESIGN wave)

- **NDJSON** for trends store — O(1) append, streaming reads, resilient to partial writes (ADR-002)
- **Pure computation** in `trend.rs` — no I/O, fully unit testable, enforced by Rust module system
- **Renderer injection** — `TrendSummary` passed through signatures, not embedded in `AnalysisReport` (NFR-03)
- **Every run recorded** — SHA dedup removed; each invocation appends an entry (acceptance test drove this decision)
- **Branch isolation** — delta only compared against same-branch history

## Wave decisions revisited

- **D-01**: Rust native tests + assert_cmd (no Gherkin runner)
- **D-02**: US-01 + US-02 + US-04 in Release 1; US-03 (CLI trend table) deferred
- **D-03**: NDJSON format confirmed
- **D-05**: Timing test (AC-01.6) uses delta approach, not absolute bound

## Upstream deviations from DESIGN

- `append_if_new_head` SHA dedup **removed** — the acceptance test `second_run_appends_to_trend_store` requires every run to be recorded, including same-SHA runs (e.g., re-running on same commit). The design doc called for dedup but the acceptance test was authoritative.
- `HistoryCounts` added `#[derive(Default)]` and `HistoryEntry.metrics`/`.counts` gained `#[serde(default)]` to handle seeded NDJSON entries in tests without those fields.
