# Evolution: html-trend-graph-readability

**Date**: 2026-03-28
**Feature ID**: html-trend-graph-readability
**Status**: DELIVERED

---

## Feature Summary

Enhanced the Trends tab in the self-contained HTML report (`barad-dur analyze --html`) to visually distinguish backfill history entries from live analysis entries.

**Problem**: After running `barad-dur backfill .`, the trend graph shows historical reconstruction data points with the same visual treatment as real measured-quality data points. Engineering leads could not tell "is this improvement real or just backfill showing a steady historical baseline?" — destroying trust in the trend graph.

**Solution**: Three visual layers that work together:
1. **Hollow/filled circle encoding** — backfill entries render as hollow SVG circles (`fill="none"`, stroke = score color); live entries remain filled. Shape distinction is colorblind-safe.
2. **Conditional legend** — shown only when backfill entries exist in history; absent for all-live configs (zero visual regression for existing users).
3. **Tooltip source label** — hover shows "Source: Backfill" or "Source: Live analysis" as second line.

**Scope**: Single file — `src/renderer/html.rs`. No Rust backend changes (the `source` field was already serialized by `HistoryEntry`).

---

## Business Context

**Who**: Engineering leads who run both `barad-dur backfill .` (to seed history) and `barad-dur analyze --html` (ongoing monitoring).

**Why it matters**: A backfill-seeded trend graph with 10+ historical data points is indistinguishable from a graph of real measurements. Without visual distinction, the trend graph's core value proposition — tracking real quality improvement — is undermined.

**Outcome KPIs target**: At-a-glance differentiation of backfill vs live data; no visual regression for all-live configurations.

---

## Steps Completed

| Phase | Step | Description | Result |
|-------|------|-------------|--------|
| 01 — Walking Skeleton | 01-01 | Wire backfill source field through to JS guard and window.R | PASS |
| 02 — Circle Visual Encoding | 02-01 | Hollow circle JS uses fill="none" with pointer-events:all | PASS |
| 02 — Circle Visual Encoding | 02-02 | Live circle JS assigns scoreColor fill | PASS (immediate — implementation covered it) |
| 02 — Circle Visual Encoding | 02-03 | Backfill circle JS sets stroke to scoreColor | PASS (immediate — covered by 02-01) |
| 02 — Circle Visual Encoding | 02-04 | Legacy entry with no source absent from window.R source field | PASS (immediate — serde skip_serializing_if enforces this) |
| 03 — Legend | 03-01 | Legend labels present in JS when backfill entries exist | PASS |
| 03 — Legend | 03-02 | Legend uses DOM APIs — no innerHTML | PASS |
| 03 — Legend | 03-03 | Legend CSS class .tr-legend present for styling | PASS |
| 04 — Tooltip / Edge Cases | 04-01 | Tooltip JS mouseenter contains "Source: Backfill" literal | PASS |
| 04 — Tooltip / Edge Cases | 04-02 | Tooltip JS mouseenter contains "Source: Live analysis" literal | PASS |
| 04 — Tooltip / Edge Cases | 04-03 | Tooltip uses textContent — no innerHTML | PASS |
| 04 — Tooltip / Edge Cases | 04-04 | Zero-backfill history — all window.R entries lack source field | PASS |
| 04 — Tooltip / Edge Cases | 04-05 | All-backfill history — all window.R entries carry source "backfill" | PASS |
| 04 — Tooltip / Edge Cases | 04-06 | pointer-events:all present on hollow circle JS string | PASS |

All 14 steps DONE. Mutation testing: 100% kill rate.

---

## Key Wave Decisions

### DISCUSS Wave

**D-01: Visual Encoding — Hollow vs Filled**
Shape distinction selected over color-only (fails WCAG) and tooltip-only (invisible at a glance). SVG `fill="none"` for backfill, `fill=scoreColor` for live.

**D-02: Legend Visibility Rule**
Show legend if and only if `window.R.history.some(e => e.source === 'backfill')`. Zero-backfill users see no change.

**D-05: Legacy Entry Handling**
Guard: `entry.source === 'backfill'` (strict equality). Any other value including `undefined` → treated as live. Safe default.

**D-06: No Backend Changes**
`entry.source` was already serialized in `HistoryEntry` via serde. No changes to `scorer.rs`, `backfill/mod.rs`, or `cache/history.rs`.

**D-07: No innerHTML**
Existing security constraint. All new DOM construction uses `el()` / `svgEl()` / `txt()` helpers.

### DESIGN Wave

**D-DESIGN-01: Circle encoding via string template, not DOM replacement**
The existing `renderChart()` builds SVG via string concatenation. Hollow/filled distinction is implemented by conditionalizing `fill` and `stroke` within the existing template. Consistent with existing pattern; the no-innerHTML constraint applies to new DOM construction, not modification of an existing string template.

**D-DESIGN-02: pointer-events:all on hollow circles**
SVG circles with `fill="none"` do not receive pointer events in their interior by default. Inline `style="pointer-events:all"` added to hollow circle elements to restore full hover area.

**D-DESIGN-03/04: Legend uses el()/svgEl() DOM APIs, placed right-aligned in .tr-controls**
New DOM element — must comply with no-innerHTML constraint. `margin-left: auto` pushes it to the right of the flex controls row.

**D-DESIGN-05: Legend not re-rendered on metric change**
Legend reflects history composition, not selected metric. Created once in `buildTrendsTab()`.

**D-DESIGN-06: No ADRs needed**
All significant decisions made in DISCUSS wave (D-01 through D-07). DESIGN wave adds only implementation-level decisions.

### DISTILL Wave

**D-DISTILL-01: Structural tests (JS code + window.R data), not behavioral**
SVG circles are JS-rendered at browser runtime, not Rust-rendered. Tests verify JS code patterns in the HTML string and `window.R` data correctness. Browser behavioral tests are a manual checklist.

**D-DISTILL-02: Tests co-located in src/renderer/html.rs test module**
Follows existing pattern. No separate `tests/` integration file — driving port is `render(&report)`, a library function.

**D-DISTILL-03: make_history_entry(score, source) test helper**
Test-only helper constructing a `HistoryEntry` with specified score and optional source. Not production code.

---

## Quality Metrics

| Metric | Result |
|--------|--------|
| Tests added | 14 new tests (structural JS code + data correctness) |
| Total lib tests | 254 passing |
| Mutation kill rate | 100% |
| Files modified | 1 (`src/renderer/html.rs`) |
| New dependencies | None |
| No-innerHTML constraint | Met (legend uses el()/svgEl(); tooltip uses textContent) |

---

## Lessons Learned

**What went well**: The `source` field was already serialized by `HistoryEntry` / serde, so the walking skeleton (step 01-01) confirmed end-to-end wiring without any backend work. Four of the 14 RED phases were skipped because the implementation from an earlier step had already satisfied the test — a sign that the decomposition was tightly clustered.

**Pattern discovered**: When decomposing features for a JS-in-Rust renderer, the unit-test granularity (one test per JS string pattern) is coarser than for pure Rust logic. Several planned "RED" steps passed immediately because a single implementation change satisfied multiple acceptance criteria simultaneously. Future decompositions for this renderer should treat circle-encoding, legend, and tooltip as three independent implementation passes rather than fine-grained per-attribute tests.

**Deferred**: Aggregation/thinning of dense point clusters (D-03) — complex UX tradeoffs (mixed-source smoothed lines, threshold definition). Deferred to a follow-up feature if users report cluster density as a remaining pain point.

---

## Migrated Artifacts

| Source | Destination |
|--------|-------------|
| `design/architecture-design.md` | `docs/architecture/html-trend-graph-readability/architecture-design.md` |
| `design/component-boundaries.md` | `docs/architecture/html-trend-graph-readability/component-boundaries.md` |
| `distill/test-scenarios.md` | `docs/scenarios/html-trend-graph-readability/test-scenarios.md` |
| `distill/walking-skeleton.md` | `docs/scenarios/html-trend-graph-readability/walking-skeleton.md` |
| `discuss/journey-trend-graph.yaml` | `docs/ux/html-trend-graph-readability/journey-trend-graph.yaml` |
| `discuss/journey-trend-graph-visual.md` | `docs/ux/html-trend-graph-readability/journey-trend-graph-visual.md` |
