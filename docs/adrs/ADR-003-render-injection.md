# ADR-003: Render Injection Strategy

**Status**: Accepted
**Date**: 2026-03-18
**Feature**: historical-trends
**Deciders**: Morgan (Solution Architect)

---

## Context

The `historical-trends` feature must deliver trend data to three renderers (CLI, JSON, HTML) without violating:
- **NFR-03**: `--json` without `--trend` must be structurally identical to the current output (backward compatibility)
- **D-05**: Trend data in JSON only when `--trend` is explicitly specified
- **DA-03**: HTML always shows trend tab when history data exists

The renderers currently receive only `&AnalysisReport` (CLI also receives `verbosity: u8`). The question is how to get `TrendSummary` into the renderers without breaking the existing contract.

**Four strategies were evaluated**:
1. Add `trend: Option<TrendSummary>` field to `AnalysisReport`
2. Pass `TrendSummary` as an additional function parameter to renderers
3. Create a `RenderContext` envelope struct wrapping report + trend
4. Use a separate post-processing step that patches the JSON string

---

## Decision

**Strategy 2: Pass `TrendSummary` as an additional parameter to each renderer function.**

New signatures:
- `renderer::cli::render(report: &AnalysisReport, verbosity: u8, trend: Option<&TrendSummary>, show_full_history: bool) -> String`
- `renderer::json::render(report: &AnalysisReport, pretty: bool, trend: Option<&TrendSummary>) -> Result<String>`
- `renderer::html::render(report: &AnalysisReport) -> Result<String>` — unchanged (DA-03: HTML reads `report.history`)

Callers in `main.rs` pass `Some(&trend_summary)` or `None` based on the `--trend` flag and the output mode.

Justification:
- `AnalysisReport` is a pure data struct serialised to JSON by `renderer::json`. Adding `trend` to it would include the field in all JSON output unconditionally (serde serialises all fields unless annotated `#[serde(skip_serializing_if)]`), violating NFR-03.
- An explicit `Option<&TrendSummary>` parameter makes the injection point visible in the function signature. Callers know exactly when trend data is present without reading the renderer implementation.
- The change is localised: only `main.rs` call sites and the three renderer function signatures change. No struct definitions change except `HistoryEntry` (which is pre-existing in scorer.rs with two new fields).
- This is consistent with the existing pattern: `renderer::cli::render` already receives `verbosity` as an extra parameter separate from the report.
- `renderer::html` remains unchanged, which is the simplest possible outcome for that renderer.

---

## Alternatives Considered

### Strategy 1: Add `trend: Option<TrendSummary>` to `AnalysisReport`

**Rejected.**

This would embed trend data in the primary report struct. The JSON renderer serialises `AnalysisReport` directly via `serde_json::to_string(report)`. With a non-null `trend` field, the key would appear in JSON output even without `--trend`, violating NFR-03. Conditionally suppressing it requires `#[serde(skip_serializing_if = "Option::is_none")]`, which works but still leaks the concern into the data model. More importantly, `AnalysisReport` is the core domain type — it should contain analysis results, not render-time view metadata. Mixing them creates a violation of the Scorer → Renderer layering.

### Strategy 3: `RenderContext` envelope struct

**Rejected as over-engineering.**

A `RenderContext<'a> { report: &'a AnalysisReport, trend: Option<&'a TrendSummary>, verbosity: u8, show_full_history: bool }` struct would unify the renderer parameters. This is a reasonable pattern when renderers have many shared parameters that evolve frequently. With two or three renderers and a small parameter set, it adds indirection without benefit. If the renderer parameter set grows significantly in future, a `RenderContext` can be introduced at that point via a straightforward refactor.

### Strategy 4: Post-processing JSON string to inject `trend` key

**Rejected.**

Parsing and re-serialising a JSON string is fragile (string manipulation), slow (double parse), and loses type safety. It also makes the injection invisible to callers. This strategy has no advantages over Strategy 2.

---

## Consequences

**Positive**:
- `AnalysisReport` struct remains unchanged — all existing JSON consumers are unaffected
- Injection is explicit and compile-time visible
- `renderer::html` requires no changes
- NFR-03 verified: `renderer::json::render(report, pretty, None)` produces identical output to the pre-feature version
- Existing tests for `renderer::json` and `renderer::cli` remain valid with `None` passed for the new parameter

**Negative**:
- Renderer function signatures change — any external code calling these functions directly will need updating. This is acceptable: the renderers are internal to the binary, not part of a public library API.
- `main.rs` render dispatch block requires updating the three call sites to pass the new parameter
