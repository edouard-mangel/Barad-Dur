# Pressman Coupling M5 — History-Corroborated Findings — Design

**Date:** 2026-07-09
**Status:** Approved design, pending implementation plan
**Parent design:** `2026-07-02-pressman-coupling-design.md` (§ Milestones → M5)

## Context

M5 was intentionally deferred in the parent design as a *checkpoint*: "the
corroboration heuristic … should not be designed blind" — its detailed
design was to be revisited against real finding data once M1–M3 shipped.
M1 (detectors + metrics), M2 (trend counts), and M3 (gate ratchet) are
merged. This document resolves the checkpoint.

### Real finding data (barad-dur, 2026-07-09)

Analyzing barad-dur itself produced:

| Kind    | Findings |
|---------|----------|
| Content | 0        |
| Common  | 0        |
| Control | 6        |

Plus 1204 co-change pairs to join against, of which `change_coupling_smells`
already qualifies a cross-boundary, ratio-thresholded subset.

**The signal that shaped this design:** a clean, well-architected FP codebase
has *zero findings on the severe rungs* (Content, Common) by construction. As
originally scoped (corroborate Common/Content only), M5 would be invisible in
barad-dur's own report and untestable on the dogfood repo — every join empty.
The 6 Control findings, by contrast, sit in high-co-change files
(`renderer/json.rs`, `exclude.rs`, `snapshot_builder.rs`). This directly drove
the scope decision below.

## Decisions

Four decisions were made during design review (2026-07-09):

1. **Scope: all three kinds.** Corroboration applies to Content, Common, *and*
   Control — not just the two severe rungs. Rationale: makes the feature
   dogfoodable and testable on barad-dur's own control findings; a flag
   argument's co-change is a weaker theoretical signal than a mutable global's,
   but corroboration is informational-plus-nudge, not a hard claim, so the
   broader scope is safe.
2. **Criterion: reuse the change-coupling smell rule.** A finding is
   corroborated iff its file participates in ≥1 cross-boundary co-change pair
   that already qualifies as a change-coupling smell
   (`co_changes / min_commits ≥ change_coupling_min_ratio`, cross-component).
   One definition of "real coupling" in the codebase; inherits the smell rule's
   existing bulk-commit resistance. Rejected: a new raw-degree threshold (second
   divergent knob) and a single-strongest-pair strength test (breadth matters
   more than one tight pair for ripple risk).
3. **Score impact: a nudge, not informational-only.** Corroborated findings
   weigh more in their metric's score. This folds in the parent design's
   "corroboration-weighted scoring" future-work item. Kept safe by the
   weighted-count mechanism (below) and the fixed "corroborated, never
   confirmed" language rule.
4. **Mechanism: weighted effective count.** The nudge is expressed in the
   bands' own currency (finding count), so it flows *through* the maintainer-
   authored `score_pressman` band SSOT rather than around it. Rejected: a fixed
   band-drop penalty (creates a second severity ladder divorced from the count
   bands; ad-hoc cap interaction) and cap-tightening-only (invisible on Control
   findings — exactly what barad-dur has).

## Architecture

Pure computation in `metrics/coupling/mod.rs`. **No collector change**: the
co-change pairs (`snapshot.file_change_pairs`) and findings
(`snapshot.coupling_findings`) are already in the snapshot. Corroboration is a
pure join, honoring the pipeline invariant (detection in the collector,
classification/scoring pure in `metrics`).

```
snapshot.file_change_pairs ─┐
                            ├─► corroboration_degree() ─► per-file partner count
snapshot.commits_by_file ───┘                                    │
snapshot.coupling_findings ─────────────────────────────────────┤
                                                                 ▼
                                              pressman_metric(): annotate + weight
```

### The shared "qualifying pair" predicate

The smell predicate currently lives inline in `change_coupling_smells`
(`mod.rs` lines ~175–188). Extract it so there is a **single definition of a
meaningful co-change**:

```rust
/// File → number of *distinct* cross-boundary partners it shares a
/// qualifying co-change pair with (cross-component, min_commits > 0,
/// co_changes / min_commits ≥ change_coupling_min_ratio).
fn corroboration_degree(
    snapshot: &RepoSnapshot,
    thresholds: &CouplingThresholds,
) -> HashMap<PathBuf, usize>
```

`change_coupling_smells` is refactored to consume the same predicate — a pure
refactor, behavior identical, guarded by its existing tests. A finding on file
`F` is **corroborated** iff `F` is a key in this map; `N` = its value.

## Scoring — weighted effective count

`pressman_metric` changes its scored count from `findings.len()` to a weighted
sum:

```
effective = round(dormant_count * 1.0 + corroborated_count * corroboration_weight)
score     = score_pressman(kind, effective)     // band SSOT unchanged
```

- `corroboration_weight`: new `CouplingThresholds` field, **default 2.0**,
  serde-defaulted (no config migration).
- **The displayed count stays truthful.** The metric description reports the
  real number of findings and how many are corroborated; only the *scored*
  count is weighted. Documented in the description so the arithmetic is never
  mysterious — the same transparency contract as the severity-cap note.
- Monotonic and floored at 25 like all bands. Because `effective ≥ real`,
  corroboration can only *lower* a metric score, hence can only *trip* the
  severity cap, never un-trip it — consistent with the ordinal-severity
  semantics of the cap.
- `corroboration_weight = 1.0` reproduces M1 scores exactly (regression guard).

## Surfacing

Corroborated findings get an annotated evidence string in the existing
`RawValue::List` (top 10, same channel as M1):

```
src/collector/exclude.rs:88 — pub fn is_excluded( — corroborated (co-changes with 4 files)
```

Metric description:

```
6 finding(s) (3 corroborated by change history) — flag parameters steering callee logic
```

CLI, JSON, and HTML renderers pick these up generically — **no renderer
changes** (same as M1).

## Interactions (deliberately untouched)

- **Gate ratchet (M3):** diffs per-kind finding *counts*, which corroboration
  does not change → the ratchet is unaffected. Noted in code.
- **History / trend (M2):** the category score already trends, so the nudge
  flows through for free. A dedicated `corroborated_*` history field is
  **deferred future work**, not M5.
- **Backfill (ADR-005):** historical snapshots skip the AST pass → no findings
  → nothing to corroborate → metrics render unscored, consistent with M2.

## Configuration

`CouplingThresholds` gains `corroboration_weight` (f64, default 2.0). It reuses
the existing `change_coupling_min_ratio` and `component_depth`. All defaulted;
no config migration needed.

## Language rule (fixed)

The report says **"corroborated"**, never "confirmed" — co-change correlation
has non-coupling causes (shared feature areas, formatting sweeps, lockstep
version bumps). Overclaiming is how analysis tools lose user trust.

## Testing strategy (TDD throughout)

- **Predicate parity:** `corroboration_degree` and the refactored
  `change_coupling_smells` agree on which pairs qualify (the refactor changes no
  behavior).
- **Join unit tests:** a finding whose file is in a qualifying pair →
  corroborated with correct `N`; a dormant file → not corroborated; a
  within-component or below-ratio pair → not corroborating.
- **Scoring unit tests:** weighted effective count crosses a band on a
  synthetic snapshot; `corroboration_weight = 1.0` reproduces M1 scores exactly;
  corroboration can trip but never un-trip the severity cap.
- **Integration (`pressman_coupling_milestone_5.rs`):** a fixture repo with a
  real finding whose file co-changes across a component boundary → assert the
  evidence string is annotated "corroborated" **and** the metric score drops a
  band relative to the dormant case.
- **Dogfood:** barad-dur's control findings show corroboration annotations
  where their files co-change cross-boundary. With the default weight the
  Control band may stay at 70 (6 findings, 6–15 band) — honest and expected;
  the fixture test is what proves the band-crossing effect.

## Risks & mitigations

- **Overclaiming from spurious co-change:** mitigated by reusing the smell
  rule's cross-boundary + ratio filtering, by the modest default weight, and by
  the fixed "corroborated" (never "confirmed") language.
- **Nudge invisible on the dogfood repo:** accepted — with default weight the
  Control band does not move on barad-dur; the annotation is still visible and
  the fixture test proves the score effect. Teams wanting a sharper nudge raise
  `corroboration_weight`.
- **Refactor regression:** the `change_coupling_smells` extraction is guarded by
  its existing unit tests plus an explicit predicate-parity test.

## Future work (explicitly deferred)

- `corroborated_*` per-kind counts in history / trend.
- Corroboration-aware action ordering in M6 (M6 does not depend on M5; if both
  ship, corroborated findings sort ahead of dormant ones within a rung).
