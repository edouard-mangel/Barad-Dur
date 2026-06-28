# ADR-009: Dismiss hotspot rows at the presentation layer, not by snapshot exclusion

- **Status**: Accepted (implemented)
- **Date**: 2026-06-26
- **Feature**: manifest-hotspot-exclusion
- **Deciders**: Edouard (product), Morgan (architecture)

## Context

`package.json` and similar config files surface in the hotspots view despite not
being actionable code. The DISCUSS wave specified removing manifests at snapshot
construction (`DEFAULT_EXCLUDE_PATTERNS`). During DESIGN the product owner chose
instead to mirror the existing **coupling-pair dismissal** UX: let the reader remove
any file from the hotspots view at render time.

The coupling report (`src/renderer/templates/coupling.js`) already implements
client-side dismissal (per-row `×`, "Reset dismissed", ephemeral state) plus an
auto-exclude heuristic. The chosen scope is **manual dismiss only** (no manifest
heuristic), on **both** the HTML report and the React dashboard.

## Decision

Implement hotspot-row dismissal purely in the presentation layer:
- HTML report: extend `hotspots.js` `buildTable` with path-keyed `dismissed` state,
  a `×` control, a "Reset dismissed" control, and a status line — a near-copy of the
  `coupling.js` pattern, using the existing `el`/`txt`/`append` builders (no
  `innerHTML`).
- React dashboard: add `useState<Set<string>>` dismissal keyed by path to
  `HotspotsView.tsx`, filtering rows (and the d3 scatter deps) before render.

No change to the collector, snapshot, scorer, metrics, or `report.json` schema.
State is ephemeral (lost on reload) in v1.

## Alternatives considered

1. **Snapshot-level default exclusion (original DISCUSS plan)** — add manifest globs
   to `DEFAULT_EXCLUDE_PATTERNS`. *Rejected*: removes files from *all* analysis
   surfaces globally and irreversibly per-view; cannot express "hide this one file
   from hotspots only"; the only opt-out is the all-or-nothing `use_defaults=false`
   (the C-1 dilemma). Does not match the requested coupling-parity UX.
2. **Manifest auto-exclude heuristic** (hide manifests by default + toggle, like
   coupling's `isAutoExcluded`) — *Deferred*: the product owner chose manual-only.
   Could be added later as a second layer; the richer-parsing idea in `Ideas.md` is
   the better long-term home for manifest-specific signal.
3. **Config-based per-file ignore list** (persisted in `barad-dur.toml`) — *Rejected
   for v1*: heavier (config plumbing + tests) and changes analysis inputs; the
   ephemeral client-side model matches the existing coupling behavior and ships
   faster. Revisit if persistence is requested.

## Consequences

**Positive**
- Zero risk to analysis correctness — deps/coupling/metrics untouched (former NFR-1
  is trivially satisfied).
- Reader controls relevance per-view; reversible via Reset.
- Consistent UX with coupling dismissal.

**Negative / trade-offs**
- Dismissal logic lives in JS/TS. The HTML-report side is verified by asserting
  control presence in `html/tests.rs` plus manual checks; the dashboard side is now
  behavior-tested via a newly-added vitest + RTL harness. Net: only the HTML-report
  JS click behavior remains outside automated coverage.
- Two implementations (vanilla JS + React) risk divergence — mitigated by identical
  semantics (dismiss by path, ephemeral, Reset).
- Ephemeral state means manifests reappear on every reload; acceptable for v1, and
  the natural prompt for a future auto-exclude layer (alternative 2) if the repeated
  dismissal becomes annoying.

## Follow-ups (not in this feature)
- Optional manifest auto-exclude layer (alternative 2).
- Optional persistence (localStorage) if requested.
- Optional scatter-plot sync on dismiss in the HTML report.
- ~~Dashboard test harness (vitest + RTL)~~ — **done** as part of this feature; the
  dashboard scatter already syncs on dismiss.
