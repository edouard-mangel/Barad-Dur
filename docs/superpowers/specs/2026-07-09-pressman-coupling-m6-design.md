# Pressman Coupling M6 — Refactoring Actions — Design

**Date:** 2026-07-09
**Status:** Approved design, pending implementation plan
**Parent design:** `2026-07-02-pressman-coupling-design.md` (§ Milestones → M6)

## Goal

Turn the coupling findings that M1–M5 already detect into concrete, prioritized
**refactoring suggestions**: a capped, ordered list of per-file actions
(`coupling_actions`), each naming a file, its worst coupling rung, and
kind-specific advice — surfaced in the Coupling tab.

M6 does not add detectors and does not depend on M5's scoring; it consumes the
existing findings and (optionally) the corroboration signal M5 exposes.

## Decisions

Locked during design review (2026-07-09):

1. **Separate list, not folded into `top_actions`.** A new
   `coupling_actions: Vec<ActionItem>` on `AnalysisReport`, surfaced in the
   Coupling tab where the findings already live. The 3-worst-metric
   `top_actions` overview is untouched. Rejected: folding per-finding actions
   into `top_actions` (mixes per-metric and per-file granularity in one
   summary) and per-metric-only advice (under-delivers the design's per-file
   specificity).
2. **One action per file, capped at 10.** A file with several findings
   collapses to one action; the cap matches the coupling metric evidence-list
   convention. Rejected: one-per-finding (noisy on files with many flag
   functions) and uncapped (unbounded lists).
3. **Worst-rung-wins rank inheritance.** When a file has findings of multiple
   kinds, it inherits its **most severe** kind on the ordinal ladder
   content ≻ common ≻ control — for both ordering and advice text. Corroboration
   and finding-count are tiebreakers *within* a rung; they never lift a file
   across a rung boundary. This mirrors M1's severity cap: Pressman's ladder is
   ordinal, so the worst rung present dominates and quantity of a milder rung
   never outweighs a severe one. Rejected: corroboration-crosses-one-rung and a
   fully blended numeric score (both abandon the ordinal guarantee the rest of
   the feature commits to).
4. **Corroborated-first within a rung.** M5 has shipped, so the corroboration
   signal is available at action time. Within a rung, corroborated files
   (their file co-changes cross-boundary) sort ahead of dormant ones — acting
   on the highest actual-risk items first. Rejected: rung-only ordering (leaves
   an available, cheap risk signal on the table).

## Architecture

Pure computation in `src/scorer/actions.rs`, invoked from `build_report`
(`src/scorer.rs`), which already has `snapshot` and the `CouplingThresholds`
in scope. No collector, cache, or scoring changes.

```
snapshot.coupling_findings ─┐
gated barrel findings ──────┼─► all_coupling_findings() ─► group by file
                            │                                    │
corroboration_degree() (M5) ┴──────────────────────────────────►│
                                                                 ▼
                              per file: worst rung, count, corroborated?
                                                                 │
                                        sort (rung ≻, corrob-first, count desc,
                                              path asc) → take 10 → ActionItem
```

### Finding aggregation — the DRY fix

Content findings come from two sources: AST findings in
`snapshot.coupling_findings` (`#[path]`, common, control) **plus** the
barrel-bypass content findings computed at metric time by
`barrel_bypass_findings` (gated by `content_barrel_rule`). M4 left this
barrel-gating snippet duplicated across four sites (`compute_coupling`,
`pressman_finding_counts`, `build_hotspots`, and the gate's
`ratchet_finding_sets`) — a recorded M4 follow-up.

M6 extracts one helper and reuses it — closing that follow-up:

```rust
/// All coupling findings for a snapshot: the AST findings plus the
/// config-gated barrel-bypass content findings. Single source of the
/// "complete finding set" the metric, counts, hotspots, gate, and M6
/// actions all need.
pub(crate) fn all_coupling_findings(
    snapshot: &RepoSnapshot,
    thresholds: &CouplingThresholds,
) -> Vec<CouplingFinding>
```

The four existing sites are refactored to call it (pure refactor, guarded by
their existing tests — the M5 Task-1 playbook). The action generator consumes
the same helper, so it can never see a different finding set than the metrics
report.

### Corroboration exposure

M5's `corroboration_degree(snapshot, thresholds) -> HashMap<PathBuf, usize>` is
made `pub(crate)` so `actions.rs` can mark a file corroborated
(`map.contains_key(&file)`). No behavior change; visibility only.

### Ordering key

Per file, build `(worst_rung, corroborated, count, path)` and sort by:

1. **worst rung** ascending on the severity index `Content=0, Common=1,
   Control=2` (content first)
2. **corroborated** — `true` before `false`
3. **count** descending (more findings in a file → higher within its tier)
4. **path** ascending (deterministic final tiebreak)

Take the first 10.

### Output

Reuse the existing `ActionItem { text, target_tab, sort_by }`. For each file
action: `target_tab = Some("coupling")`, `sort_by = None`. Text format:

```
[Coupling] <file> — <N> finding(s) (worst: <kind>)[, corroborated by change history] — <advice>
```

Example:

```
[Coupling] src/config.rs — 2 finding(s) (worst: common), corroborated by change history — Shared mutable global state: replace it with explicitly passed or injected state.
```

### Advice text (worst-rung keyed) — maintainer-authored decision point

One advice string per rung, keyed on the file's worst rung. Authored during
implementation (like M1's banding); the proposed defaults, editable:

- **content:** "Reaches into another module's internals — import through the module's public interface instead."
- **common:** "Shared mutable global state — replace it with explicitly passed or injected state."
- **control:** "A flag parameter steers this function's control flow — split it into two intent-revealing functions."

## Surfacing (renderers)

The new field flows to every output:

- **JSON** — automatic via serde (`AnalysisReport` derives `Serialize`).
- **CLI** (`renderer/cli/`) — a "Coupling actions" section listing the items.
- **HTML Coupling tab** (`renderer/templates/coupling.js` or equivalent) — a
  panel rendering the list; **no `innerHTML`** (security hook), matching the
  existing template idiom.
- **Dashboard** (`dashboard/src/pages/Report.tsx` Coupling tab +
  `dashboard/src/types.ts`) — a panel + the type field.

Follows M4's precedent for Coupling-tab additions (the coupling badge work).

## What M6 does not touch

- `top_actions` (the 3-worst-metric overview) — unchanged.
- Gate ratchet (diffs finding counts), M2 history, M5 metric scoring — unchanged.
- No new detectors (inheritance coupling is recorded as **future work** in the
  parent design; it would be a new rung, not part of M6).

## Configuration

None. The cap (10) is a constant matching the evidence-list convention;
advice strings are code. No new `CouplingThresholds` field, no config
migration. (If a future need arises to tune the cap, it can become a threshold
then — YAGNI now.)

## Testing strategy (TDD throughout)

- **`all_coupling_findings` parity:** the helper returns exactly what the four
  refactored sites computed inline before (the refactor changes no behavior).
- **Grouping / worst-rung:** a file with mixed kinds inherits its most severe
  kind; a file with only control findings inherits control.
- **Ordering:** content-before-common-before-control across files; within a
  rung, corroborated-before-dormant, then higher-count-first, then path;
  cap-at-10 drops the lowest-priority overflow.
- **Advice text:** each rung produces its kind-specific string; the
  corroboration note appears only for corroborated files; `target_tab` is
  `"coupling"`.
- **Empty / unscored:** no findings → empty `coupling_actions`; a
  detection-did-not-run snapshot (backfill, ADR-005) → empty, never fabricated.
- **Integration (`pressman_coupling_milestone_6.rs`):** a fixture repo with
  content, common, and control findings across several files (one file
  co-changing cross-boundary to corroborate) → assert the ordered action list
  and the annotated text through the real binary + JSON.
- **Dogfood:** barad-dur's 6 control findings yield control-advice actions in
  the Coupling tab.

## Risks & mitigations

- **DRY-refactor regression:** the `all_coupling_findings` extraction touches
  four existing sites; guarded by their existing tests plus an explicit parity
  test, and each site's diff is a mechanical substitution.
- **Renderer surface:** the new list touches CLI + HTML + dashboard; mitigated
  by reusing the existing `ActionItem` shape and following M4's Coupling-tab
  precedent, so each renderer change is additive.
- **Advice text bikeshedding:** strings are a marked maintainer decision point,
  editable without touching logic or tests (tests assert per-kind *mapping*,
  not exact prose — assert a stable substring, not the whole sentence).

## Future work (explicitly deferred)

- **Inheritance coupling detector** (recorded in the parent design's Future
  work): once it lands as a new rung, it slots into this action generator's
  ordinal ordering automatically.
- Tunable action cap as a `CouplingThresholds` field, if a real need appears.
