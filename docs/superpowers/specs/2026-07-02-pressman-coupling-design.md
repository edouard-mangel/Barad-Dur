# Pressman Coupling Detection — Design

**Date:** 2026-07-02
**Status:** Approved design, pending implementation plan

## Motivation

The within-repo coupling category (`src/metrics/coupling/`) measures coupling
*quantity*: afferent/efferent edge counts, circular dependencies, co-change
smells. It cannot distinguish a file importing a pure data type from a file
mutating another module's global — both count as one edge.

Pressman's coupling taxonomy (*Software Engineering: A Practitioner's
Approach*, popularizing Stevens/Myers/Constantine structured design) is an
ordinal severity ladder, best to worst:

> data → stamp → control → external → common → content

This feature detects the dangerous rungs (**content**, **common**,
**control**) as concrete, evidenced findings, scores them by severity, and
feeds them into barad-dur's existing muscles: trend tracking, the CI gate,
hotspots, co-change history, and action suggestions.

## Scope

- **In scope:** within-repo metrics category; Rust + TypeScript/JavaScript
  detectors; trend counts; gate ratchet; hotspot cross-referencing;
  history-confirmation; refactoring actions.
- **Out of scope:** the cross-repo `coupling` command; stamp coupling (not
  reliably detectable statically); external coupling; other 5 languages
  (Python/Go/Java/C#/Kotlin — added later, zero findings until then);
  cross-file mutation analysis of `export const` object literals (future
  candidate).

## Architecture

Follows the pipeline invariant: detection (I/O + AST) happens in the
**collector**, classification/scoring stays **pure** in `metrics/coupling`.

```
Collector (existing tree-sitter pass, zero extra parses)
    → RepoSnapshot.coupling_findings: Vec<CouplingFinding>
        → metrics/coupling: 3 new pure metrics (severity-banded)
        → scorer: hotspot badges, confirmation join, actions
        → history: per-kind counts → trend + gate ratchet
```

### Data model (`src/snapshot/`)

```rust
/// Ordered worst → least severe.
pub enum CouplingKind { Content, Common, Control }

pub struct CouplingFinding {
    pub path: PathBuf,
    pub line: usize,
    pub kind: CouplingKind,
    pub evidence: String, // e.g. "static mut CACHE" or the import path
}

// RepoSnapshot gains:
pub coupling_findings: Vec<CouplingFinding>,
```

The snapshot cache version is bumped; stale caches re-collect (existing
HEAD-invalidation path handles this).

### Detection rules (v1: Rust, TS/JS)

Run as additional tree-sitter queries inside the collector's existing
per-file parse (the pass that extracts complexity + raw imports today).

| Kind | Rust | TS/JS |
|------|------|-------|
| **Content** | `#[path = "..."]` attribute imports (bypassing the module tree). Deep `use` paths are NOT flagged — rustc privacy already prevents most content coupling; flagging paths would be noise. | Relative import that crosses into another component **and** bypasses a barrel: target directory has `index.ts`/`index.js` but the import resolves to a different file in it, from outside that directory. |
| **Common** | `static mut`; `static` with interior-mutability type (`Mutex`, `RwLock`, `RefCell`, `Cell`, `Atomic*`, `OnceLock`/`OnceCell`/`LazyLock`, `lazy_static!`). Immutable `static X: i32` is a constant — not flagged. | Top-level `export let` / `export var`; assignments to `globalThis.x` / `window.x`; singleton pattern (class with a static field holding its own instance and/or a `getInstance()`-style static accessor). |
| **Control** | Function with ≥1 `bool` parameter (flag argument). | Function with ≥1 `boolean`-annotated parameter (TS); default-value `= true/false` parameter (JS). |

Unsupported languages produce zero findings; the metrics render as unscored
("not detected" dash), reusing existing behavior.

## Milestones

Each milestone is independently shippable with its own integration test
(`pressman_coupling_walking_skeleton.rs`, then
`pressman_coupling_milestone_N.rs`).

### M1 — Walking skeleton

Detectors + `RepoSnapshot.coupling_findings` + three new metrics appended to
`compute_coupling()` (existing four metrics untouched):

- **Content coupling** — count of Content findings
- **Common coupling** — count of Common findings
- **Control coupling** — count of Control findings

Severity lives in the band thresholds, not a shared weight: Content gets the
strictest bands (one finding already drops the score), Control the most
lenient. The per-kind banding function is left as a marked decision point for
the maintainer to author during implementation.

Findings surface in each metric's `RawValue::List` (top 10,
`file:line — evidence`), same pattern as circular dependencies. CLI/JSON/HTML
renderers pick metrics up generically — no renderer changes.

### M2 — Trend counts

`HistoryCounts` (already serde-defaulted on `HistoryEntry`) gains
`content_coupling`, `common_coupling`, `control_coupling`. Every
`analyze`/`backfill` run records them; trend deltas and the dashboard history
view surface them. Backfill makes history retroactive automatically since it
reuses the same pipeline.

### M3 — Gate ratchet

New gate flags:

- `--no-new-coupling` — fail if any per-kind count exceeds the last history
  entry on the branch
- `--max-new-coupling <n>` — allow up to `n` new findings in total, summed
  across all three kinds (teams mid-cleanup)

Baseline = last `HistoryEntry` on the current branch (reuses the history
cache the trend gate already loads; no new baseline file). Failure output
prints the per-kind delta and the offending `file:line` findings. First run
on a branch passes (no baseline), matching trend-gate behavior.

### M4 — Hotspot cross-referencing

`HotspotFile` gains per-kind finding counts. Hotspot score gets a multiplier
when Content/Common findings are present (weight configurable in
`barad-dur.toml`, default modest, e.g. 1.25×). HTML report and dashboard
hotspot rows show a coupling badge. Rationale: severity × change frequency =
actual risk; a `static mut` in a high-churn file outranks a dormant one.

### M5 — History-confirmed findings

Join each Common/Content finding's file against `file_change_pairs`: a file
with findings **and** high co-change degree (partners above the existing
`change_coupling_min_ratio` threshold) gets `confirmed: true` and a report
warning, e.g. "global state in `x.rs` is confirmed coupling: co-changes with
N files". Dormant findings stay flagged but unconfirmed. This empirically
validates Pressman's ripple-effect claim per repo: static analysis predicts,
git history confirms.

### M6 — Refactoring actions

`scorer/actions.rs` emits per-finding suggestions with kind-specific advice
text ("replace this global with injected state", "split this flag-argument
function", "import via the module's public barrel"). Priority order:
confirmed content > confirmed common > unconfirmed content > unconfirmed
common > control.

## Configuration

- `CouplingThresholds` (in `barad-dur.toml`) gains the per-kind band
  thresholds and the M4 hotspot multiplier. All have defaults; no config
  migration needed.

## Testing strategy (TDD throughout)

- **Detector unit tests:** inline Rust/TS/JS source snippets → parse →
  assert findings, including negative cases (immutable `static` not flagged,
  barrel-respecting import not flagged, non-boolean params not flagged).
- **Metric unit tests:** synthetic snapshots via `metrics/testutil.rs`;
  band scores; unscored-dash when no parseable files.
- **Gate/trend/hotspot/action tests:** unit tests against synthetic reports
  and history entries, following existing patterns in `cmd/gate.rs` tests.
- **Integration:** walking-skeleton test analyzes the repo itself
  end-to-end and asserts the three metrics exist; milestone tests per
  feature. Dogfood check: barad-dur's own code should score cleanly.

## Risks & mitigations

- **Control-coupling noise:** boolean-flag detection is the noisiest rule;
  mitigated by the most lenient bands and lowest action priority.
- **Barrel-bypass false positives** in TS projects that don't use barrels:
  the rule only fires when an `index.ts` actually exists in the target
  directory, so barrel-free projects produce no findings.
- **Snapshot format change:** cache version bump forces one-time
  re-collection; acceptable (same cost as a HEAD change).

## Future work (explicitly deferred)

- Detectors for Python, Go, Java, C#, Kotlin.
- `export const` mutable-object-literal detection (needs cross-file
  mutation analysis).
- Confirmation-weighted scoring (confirmed findings weigh more than dormant
  ones in the metric score itself).
