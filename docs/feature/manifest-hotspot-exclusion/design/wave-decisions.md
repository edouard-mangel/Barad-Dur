# DESIGN Decisions — manifest-hotspot-exclusion

## Key Decisions
- [D1] **Presentation-layer dismiss, not snapshot exclusion** — mirror coupling-pair
  dismissal; filter rows at render time, leave analysis data untouched.
  (see: ADR-009, architecture-design.md)
- [D2] **Manual dismiss only** — no manifest auto-exclude heuristic; manifests are the
  motivating example, the mechanism is generic per-row dismissal. (user decision)
- [D3] **Both surfaces** — HTML report (`hotspots.js`) and React dashboard
  (`HotspotsView.tsx`). (user decision)
- [D4] **Dismiss by file path, ephemeral state** — no persistence in v1; mirrors
  coupling.js. (see: ADR-009)
- [D5] **Scatter plot left full in v1** (HTML report) — table-only dismiss matches
  the coupling pattern (which has no plot). (see: architecture-design.md C1)
- [D6] **No paradigm/CLAUDE.md change** — feature lives in presentation templates,
  not the pure-function metric core.

## Architecture Summary
- Pattern: presentation-layer view filter over unchanged analysis data; an isolated
  client-side dismissal unit added to each of two existing view containers.
- Paradigm: N/A (presentation templates).
- Key components: C1 `hotspots.js` dismissal; C2 `HotspotsView.tsx` dismissal.

## Technology Stack
- Unchanged. Vanilla JS (HTML report, `include_str!`) + React 19/d3/TS (dashboard).
  No new dependencies.

## Constraints Established
- No `innerHTML` (security hook) — use existing `el`/`txt`/`append` builders.
- Dismiss keyed by path (not index) for sort/filter correctness.
- Behavior partly outside Rust test coverage — assert control presence in
  `html/tests.rs`; dashboard verified manually (no test harness present).

## Upstream Changes
- Major pivot from the DISCUSS snapshot-exclusion plan → see `upstream-changes.md`.
  DISCUSS FR-1..3 / NFR-1 / AC-1..5 superseded; replacement AC-D1..6 proposed for
  product-owner ratification. Feature-id rename optional (now generic dismissal).

## Handoff
- Next: DEVOPS (nw-platform-architect) — minimal; no infra/CI impact (presentation
  change, no new deps, no new commands). Or proceed to DISTILL/DELIVER.
- Open for product owner: ratify pivot + AC-D1..6 (upstream-changes.md).
