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
        → scorer: hotspot badges, corroboration join, actions
        → history: per-kind counts → trend
        → gate: HEAD vs --baseline-ref snapshot diff → ratchet
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
| **Content** | `#[path = "..."]` attribute imports (bypassing the module tree). Deep `use` paths are NOT flagged — rustc privacy already prevents most content coupling; flagging paths would be noise. | **Cross-component barrel bypass**: relative import whose target lies in a *different component* (per the existing `component_depth` boundary definition) **and** whose target directory contains an `index.ts`/`index.js` barrel, yet the import resolves to a different file in that directory. Within-component deep imports are normal internal structure and never flagged. Toggle: `content_barrel_rule = false` disables the rule for deep-import-culture teams. Known accepted false-positive class: deep imports of files the barrel does not actually re-export (re-export verification deferred to future work). |
| **Common** | **Truly mutable globals only (look-through rule)**: `static mut`, or a `static` whose type tree contains an interior-mutability type (`Mutex`, `RwLock`, `RefCell`, `Cell`, `Atomic*`) at **any nesting depth** — including inside write-once wrappers (`OnceLock<Mutex<…>>`, `LazyLock<Mutex<…>>` are flagged). Pure write-once statics (`LazyLock<Regex>`, `OnceLock<Config>`) are NOT flagged: no hidden write paths after init, so no ripple-effect risk — which is what puts common coupling near the top of Pressman's ladder. Immutable `static X: i32` is a constant — not flagged. | Top-level `export let` / `export var`; assignments to `globalThis.x` / `window.x`; singleton pattern (class with a static field holding its own instance and/or a `getInstance()`-style static accessor). |
| **Control** | **Branched-on flag in a public API**: `pub` function with ≥1 `bool` parameter that is *used in a branch condition* (`if`, `match`, ternary-like, `&&`/`||` guard) within the function body. Bool-as-data (stored or forwarded, never branched on) is not control coupling and not flagged. Private functions are never flagged — coupling is an inter-module relationship. | Same rule for exported functions: `boolean`-annotated parameter (TS) or default-value `= true/false` parameter (JS) that is branched on in the body. Future work: call-site bare-literal detection (Fowler's flag-argument smell, `render(true)`). |

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

**Severity cap (weakest-link rule).** A flat 7-metric average would let one
catastrophic rung hide behind six healthy metrics (worst case: 6×100 + 25 all
divided by 7 = 89 — green). Because metric scores are floored at 25, no
weighted average can fix this. Instead, `compute_coupling()` post-processes
the category score: **it is capped at 70 (top of the warn band) while the
Content metric scores ≤ 50, or while the Common metric scores ≤ 25.** The
coupling category is thereby the one category whose score is not a plain
average; the report documentation states this, and when the cap fires the
category carries a note (e.g. "score capped by content coupling findings") so
the arithmetic is never mysterious. Rationale: Pressman's scale is ordinal by
severity — aggregation must respect the worst rung present, the way a
security scanner never averages a critical against passing checks.

Findings surface in each metric's `RawValue::List` (top 10,
`file:line — evidence`), same pattern as circular dependencies. CLI/JSON/HTML
renderers pick metrics up generically — no renderer changes.

### M2 — Trend counts

`HistoryCounts` (already serde-defaulted on `HistoryEntry`) gains
`content_coupling`, `common_coupling`, `control_coupling`. Every `analyze`
run records them; trend deltas and the dashboard history
view surface them. Backfill entries carry **no** Pressman data: ADR-005's historical snapshots
skip the AST pass (empty `file_metrics`), so their metrics render unscored
and their counts serialize as absent (`None`) — never as fake zeros or
perfect scores. Retroactive coupling trends would require historical AST
collection and are explicitly deferred.

### M3 — Gate ratchet

New gate flags:

- `--no-new-coupling --baseline-ref <ref>` — fail if any per-kind finding
  count at HEAD exceeds the count at `<ref>`
- `--max-new-coupling <n>` — allow up to `n` new findings in total, summed
  across all three kinds (teams mid-cleanup)

**Baseline strategy: explicit ref, always.** The gate collects a second
snapshot at `<ref>` via the same `collect_snapshot_at` machinery backfill
uses, but with the AST pass opted back in (`run_ast = true`) — backfill's
historical sweep skips it per ADR-005, but the ratchet needs coupling
findings, which only the AST pass produces. This second collection reads
file blobs straight from git's object database (no working-tree checkout)
and is **not cached**: there is no per-commit snapshot cache for baseline
collection, so every gate invocation re-parses the baseline tree from
scratch. Backfill's own cache-free, blob-reading behavior (ADR-005) is
unaffected by this — the ratchet only adds the AST pass on top of the same
uncached machinery. History-file baselines were considered and **rejected**:
`.repository-analysis/` is gitignored, so fresh CI clones have no history and
the ratchet would pass vacuously — a gate whose broken state is
indistinguishable from passing. A hybrid (ref in CI, history locally) was
also rejected: two baseline semantics behind one flag produce
local-passes/CI-fails surprises.

Design properties:

- **Fail loud, fail closed**: `--no-new-coupling` without `--baseline-ref`
  is a usage error; an unresolvable ref (typo, shallow clone) is a hard
  error with a hint about `GIT_DEPTH: 0` / `git fetch`.
- **Right baseline**: the recommended ref is the MR merge base
  (`$CI_MERGE_REQUEST_DIFF_BASE_SHA` on GitLab), so the ratchet measures
  exactly what *this branch* adds — immune to main moving underneath.
- Failure output prints the per-kind delta and the offending `file:line`
  findings so the developer can act without re-running anything.

**Documentation deliverables (required for this milestone):**

1. CLI `--help` text for both flags, including the merge-base
   recommendation.
2. A docs page (`docs/` + README section) covering: what the ratchet
   guarantees, why the baseline is explicit, GitLab CI job example using
   `$CI_MERGE_REQUEST_DIFF_BASE_SHA` with `GIT_DEPTH: 0`, and a local
   usage example (`--baseline-ref origin/main`).
3. A Makefile target (e.g. `make gate-coupling`) wrapping the local
   invocation.
4. Error-message copy for: missing ref flag, unresolvable ref, shallow
   clone.

### M4 — Hotspot cross-referencing

`HotspotFile` gains per-kind finding counts. Hotspot score gets a multiplier
when Content/Common findings are present (weight configurable in
`barad-dur.toml`, default modest, e.g. 1.25×). HTML report and dashboard
hotspot rows show a coupling badge. Rationale: severity × change frequency =
actual risk; a `static mut` in a high-churn file outranks a dormant one.

### M5 — History-corroborated findings ✅ shipped

**Status: resolved and shipped.** The checkpoint was revisited against real
finding data (barad-dur: 0 content/0 common/6 control) on 2026-07-09; the
detailed design lives in `2026-07-09-pressman-coupling-m5-design.md`.
Decisions: corroboration covers all three kinds; the criterion reuses the
change-coupling smell rule; corroborated findings weigh `corroboration_weight`
(default 2.0) toward the severity band; report language is "corroborated".

Original concept (superseded in detail by the shipped design, which covers
all three kinds): join each finding's file against `file_change_pairs`; a file
with findings **and** high co-change degree gets `corroborated: true` and a
report note, e.g. "global state in `x.rs` is **corroborated by change
history**: co-changes with N files". Dormant findings stay flagged but
uncorroborated.

Language rule (fixed now, regardless of the later design): the report says
**"corroborated"**, never "confirmed" — co-change correlation has
non-coupling causes (shared feature areas, formatting sweeps, lockstep
version bumps), and overclaiming is how analysis tools lose user trust.

### M6 — Refactoring actions

`scorer/actions.rs` emits per-finding suggestions with kind-specific advice
text ("replace this global with injected state", "split this flag-argument
function", "import via the module's public barrel"). Priority order:
**rung-only** — content > common > control. (Corroboration-aware ordering can
be added later, now that M5 has shipped; M6 does not depend on M5.)

### Milestone order

Implementation order: **M1 → M2 → M3 → M4 → M6**, with **M5 revisited after
M1** produces real-world findings. M3 does not depend on M2 (the ratchet
baseline comes from a second snapshot, not from history), but M2 is cheap
and keeping it early gets trend data accumulating as soon as possible.

## Configuration

- `CouplingThresholds` (in `barad-dur.toml`) gains:
  - the per-kind band thresholds (maintainer-authored during M1)
  - `content_barrel_rule` (bool, default `true`) — disables the TS/JS
    barrel-bypass detector for teams whose culture prefers deep imports
  - the M4 hotspot multiplier
- All have defaults; no config migration needed.

## Resolved design questions

Decisions made during design review (2026-07-02), recorded so future
changes don't relitigate them blind:

1. **Rust write-once statics** (`OnceLock`/`LazyLock` with pure contents)
   are NOT common coupling — no post-init write paths. The look-through
   rule flags only type trees containing interior mutability. Rejected
   alternatives: flag-all (would flag idiomatic Rust and barad-dur itself);
   a milder informational tier (machinery without actionable value in v1).
2. **Control coupling requires branched-on + pub/exported.** A bool
   parameter alone is bool-as-data half the time; a private function's flag
   is intra-module. Rejected: naive ≥1-bool rule (noise), dropping the rule
   (the precise version is cheap enough).
3. **Gate baseline is an explicit `--baseline-ref`, never the history
   file.** History is gitignored → vacuous passes in CI; hybrids create
   dual semantics. Fail-loud beats convenient. See M3.
4. **Barrel bypass fires cross-component only, without re-export
   verification.** `component_depth` defines the boundary, consistent with
   change-coupling smells. Re-export verification (only flag bypasses the
   barrel actually covers) was evaluated and deferred as future work; the
   `content_barrel_rule` toggle is the interim escape hatch.
5. **Category score uses a severity cap**, not a flat or weighted average —
   averages hide minima and the 25-floor makes weights mathematically
   toothless. See M1.
6. **M5 is a checkpoint, not a commitment.** Corroboration heuristics get
   designed against real finding data after M1; report language is
   "corroborated", never "confirmed".
   Resolved 2026-07-09 (see 2026-07-09-pressman-coupling-m5-design.md): all
   three kinds, smell-rule criterion, weighted-count score nudge (default 2.0×),
   "corroborated" never "confirmed".
7. **`pub(crate)` counts as public** (recorded 2026-07-06, pre-M4 hygiene).
   `rust_control` treats any `visibility_modifier` — including `pub(crate)` —
   as exported. Rationale: control coupling is inter-module, and
   `pub(crate)` items are exactly the cross-module surface inside a crate.
   Only truly private (no modifier) functions are exempt.
8. **Exact `boolean` only for TS flag params** (recorded 2026-07-06).
   `boolean[]`, unions (`boolean | undefined`), and look-alike named types
   are data shapes, not flags. Optional params (`flag?: boolean`) still
   qualify — the annotation itself is exactly `boolean`.

## Testing strategy (TDD throughout)

- **Detector unit tests:** inline Rust/TS/JS source snippets → parse →
  assert findings, including negative cases (immutable `static` not flagged,
  barrel-respecting import not flagged, non-boolean params not flagged).
- **Metric unit tests:** synthetic snapshots via `metrics/testutil.rs`;
  band scores; unscored-dash when no parseable files.
- **Gate/trend/hotspot/action tests:** unit tests against synthetic reports
  and history entries, following existing patterns in `cmd/gate.rs` tests.
  The ratchet additionally gets an integration test using a fixture repo
  with two commits whose finding counts differ, exercising
  `--baseline-ref` end-to-end (including the unresolvable-ref error path).
- **Integration:** walking-skeleton test analyzes the repo itself
  end-to-end and asserts the three metrics exist; milestone tests per
  feature. Dogfood check: barad-dur's own code should score cleanly.

## Risks & mitigations

- **Control-coupling noise:** largely designed out (branched-on + pub-only
  rule); residual noise mitigated by the most lenient bands and lowest
  action priority.
- **Barrel-bypass false positives:** deep imports of files the barrel never
  re-exports are still flagged (accepted, see resolved question 4);
  mitigated by cross-component-only scope and the `content_barrel_rule`
  toggle. Barrel-free projects produce zero findings by construction.
- **Gate runtime:** `--baseline-ref` collects a second snapshot, including
  an uncached blob-based AST pass at the ref, roughly doubling gate time on
  every run (there is no per-commit cache for baseline collection —
  accepted cost, not mitigated).
- **Snapshot format change:** cache version bump forces one-time
  re-collection; acceptable (same cost as a HEAD change).

## Future work (explicitly deferred)

- Detectors for Python, Go, Java, C#, Kotlin.
- `export const` mutable-object-literal detection (needs cross-file
  mutation analysis).
- Barrel re-export verification (only flag bypasses the barrel actually
  covers).
- Call-site flag-argument detection (`render(true)` — Fowler's smell,
  complements the definition-side control rule).
- Corroboration-weighted scoring (corroborated findings weigh more than
  dormant ones in the metric score itself) — **shipped in M5**
  (`corroboration_weight`, default 2.0).
- **Inheritance coupling detector** — a new rung detecting class inheritance
  (`class B extends A` in TS/JS; trait/impl relationships where relevant).
  Not one of the original Pressman six, but a widely-recognized OO coupling
  form. Today inheritance surfaces only indirectly as an import edge in the
  afferent/efferent *quantity* metrics, never as a severity-ranked finding.
  Adding it means a new `CouplingKind`, detector queries, banding, and (once
  it lands) participation in M6 refactoring actions.
