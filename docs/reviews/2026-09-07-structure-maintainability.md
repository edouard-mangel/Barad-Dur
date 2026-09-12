# Structure and maintainability review

Date: 2026-09-07

Reviewed revision: `21cefcf` (`feat/bd-006-long-methods`)

Scope: repository structure, dependency boundaries, shared rules, and test organization.

Disposition: findings recorded; planning requested; production changes not implemented.

During documentation the shared checkout moved to `main` at `a068611`. A diff
against the reviewed revision showed no changes in `src/`, `dashboard/`,
`Cargo.toml`, or `.gitlab-ci.yml`; the evidence below still applies.

The collector → snapshot → metrics → scorer → renderer pipeline is a useful
foundation. The priorities below concern change friction and hidden dependencies.
They are ordered by expected maintainability benefit, with observed defects
distinguished from structural risks. Each item has a corresponding brief in the
[planning roadmap](../plans/2026-09-07-maintainability-refactors.md).

## M01 — Establish a reliable dashboard report boundary

### Evidence

- [dashboard/src/types.ts](../../dashboard/src/types.ts) combines handwritten
  report types, runtime validation, formatting, and mutable score thresholds.
- `AnalysisReport.overall_score` permits `number | null`, but
  `isAnalysisReport` requires a number. Both
  [Landing](../../dashboard/src/pages/Landing.tsx) and
  [Report](../../dashboard/src/pages/Report.tsx) use this guard.
- Rust's [RawValue](../../src/metrics/mod.rs) serializes tagged variants, while
  the dashboard declares `number | string | string[]`.
  [MetricRow](../../dashboard/src/components/MetricRow.tsx) passes the value to
  `formatRawValue`, which converts an object to `[object Object]`.
- `applyScoreThresholds(undefined)` leaves the previous module-level `bands`
  value in place. Loading a report without thresholds after one with custom
  thresholds therefore retains the earlier report's rules.

Read-only probes transpiled the actual `types.ts` with the installed TypeScript
compiler and invoked its exports in Node. The raw-value probe used the committed
`dashboard/report.json`; the score probes varied that fixture's score and called
the threshold setter in sequence. Observed output:

```json
{"numericScoreAccepted":true,"nullScoreAccepted":false}
{"metric":"Bus factor","serializedRawValue":{"Count":1},"dashboardFormattedValue":"[object Object]"}
{"customReportScore80":"score-yellow","followingLegacyReportScore80":"score-yellow","expectedLegacyScore80":"score-green"}
```

These are reproduced contract/state defects, not just potential problems.

### Proposed refactor and tradeoff

Introduce one decoder from untrusted report input to a normalized dashboard
model, separate display formatting, and give each report its own effective
thresholds. Validate compatibility against deterministic fixtures emitted by
Rust. This makes schema drift observable, at the cost of a small fixture and
compatibility workflow. Generated types or JSON Schema are alternative approaches
to decide during planning.

Planning decision, 2026-09-07: the user selected **TypeScript types generated from
Rust, with a separate runtime decoder**. Fixtures remain verification evidence;
handwritten wire declarations are not the selected source of truth. See the
[M01 design](../superpowers/specs/2026-09-07-report-contract-design.md).

Subsequent user decision, 2026-09-07: **no backward compatibility for older report
JSON**. The plan will reject incompatible files with regeneration guidance and
omit legacy adapters/defaulting. Rust snapshot caches and trend history remain
outside that decision. The original findings and probes above describe observed
behavior, not the selected compatibility policy.

### Verification target

Current-shape reports load through both entry points; incompatible older shapes
are rejected with regeneration guidance. Null scores
stay unscored; every raw-value variant displays correctly; report order cannot
affect colors; malformed fields used by components are rejected before rendering.

## M02 — Share analysis orchestration and derived findings

### Evidence

- [cmd/analyze.rs](../../src/cmd/analyze.rs),
  [cmd/gate.rs](../../src/cmd/gate.rs), and
  [backfill/mod.rs](../../src/backfill/mod.rs) separately assemble categories and
  inputs to `scorer::build_report`.
- `compute_selected_metrics` accepts CLI-specific `AnalyzeArgs`, so it is not
  directly reusable as an independent calculation boundary.
- Coupling metrics compute barrel findings, inheritance findings, and
  corroboration. [Action generation](../../src/scorer/actions.rs),
  [hotspots](../../src/scorer/builders/hotspots.rs), and finding totals separately
  derive overlapping evidence from the same snapshot and thresholds.
- Backfill calls the full [report builder](../../src/scorer.rs), then retains a
  history entry. The report builder also constructs ownership, ages, audit,
  import, and other display sections.

This is a coordination and redundant-work risk. No performance improvement was
benchmarked. Backfill's omitted AST/blame collection and category differences
are intentional behavior to preserve, not duplication to remove blindly.

### Proposed refactor and tradeoff

Extract pure analysis orchestration with explicit category selection and derived
findings scoped to one snapshot/configuration. Let scoring and report sections
consume that evidence. Separate a score summary from full report enrichment so
history and gates can request the data they need. This adds named types and
interfaces but reduces the number of callers that must understand each rule.

### Verification target

Equivalent command inputs produce equivalent scores and evidence. Category
filters, dependency opt-in behavior, configured weights, backfill modes,
coupling ratchets, finding ordering, and recommendation text remain consistent.

## M03 — Unify snapshot assembly with named intermediate structures

### Evidence

[collector/snapshot_builder.rs](../../src/collector/snapshot_builder.rs) contains:

- `RawAstOutput`, a six-element tuple, and `AstParts`, a seven-element tuple.
- Similar aggregation of `SourceAnalysis` fields in
  `collect_file_metrics_with_progress` and `ast_pass_at`.
- Raw import/class/re-export/call resolution in both paths.
- Separate `RepoSnapshot` literals in `assemble_snapshot` and
  `collect_snapshot_at_inner`, each followed by `build_indexes`.

An added analysis channel must be threaded through positional structures and
both assembly paths. The existing single-parse `analyse_source` function and
`resolve_against_files` helper are useful extractions to retain.

### Proposed refactor and tradeoff

Replace tuples with named raw/resolved structures; share aggregation, resolution,
and final snapshot construction. Keep working-tree and historical blob readers
explicit, including manifest provenance. Retain live parallelism and historical
collection policy. The extra types improve visibility; unnecessary generic
reader frameworks would add indirection without helping this change.

### Verification target

Live and historical collection agree for equivalent committed content;
historical manifests come from their commit; exclusions and ordering remain
stable; indexes are built from final core data; backfill still skips AST/blame.
Review cache compatibility if a change reaches the serialized snapshot shape.

## M04 — Give metrics stable identities independent of display names

### Evidence

- [scorer/actions.rs](../../src/scorer/actions.rs) dispatches advice and target
  tabs by metric display-name strings, with generic fallbacks.
- [renderer/templates/chrome.js](../../src/renderer/templates/chrome.js) maintains
  another name-keyed tooltip catalog, consumed by overview widgets.
- [scorer.rs](../../src/scorer.rs) selects Health advice using the category
  display name and keys historical metric/category maps by names.

A wording change can silently lose advice/navigation/tooltips or alter history
keys. This is a structural risk; the review did not reproduce a particular
renamed-metric failure.

### Proposed refactor and tradeoff

Introduce stable `MetricId` and `CategoryId` values with explicit metadata
mappings. Keep presentation labels separate. Prefer exhaustive Rust mappings
and catalog coverage checks. The migration affects several consumers and needs
an explicit report/history compatibility policy; afterward label edits become
local presentation changes.

### Verification target

Changing a display label does not change advice, navigation, weighting, or
historical identity. Every built-in metric has an explicit metadata disposition;
older report/history compatibility is tested or deliberately versioned.

## M05 — Make HTML script dependencies explicit

### Evidence

[renderer/html.rs](../../src/renderer/html.rs) concatenates 16 JavaScript fragments.
`shared.js` opens the common closure; `authors.js` closes it. Helpers, report
data, navigation state, tooltip data, and tab functions share that scope despite
living in separate files. File separation does not establish module boundaries.

### Proposed refactor and tradeoff

Introduce explicit modules for formatting, navigation, tabs, and initialization,
then bundle a single entry point into the existing self-contained HTML output.
Independent modules become easier to test and their dependencies become visible.
The cost is a bundle/reproducibility workflow; planning must preserve ordinary
Cargo builds, crate packaging, and installation from source.

### Verification target

The exported report works offline with embedded assets, preserves all tabs,
cross-tab navigation, hash state, sorting, quick-open, and theme behavior, and
continues to use script-safe data escaping and DOM APIs without `innerHTML`.

## M06 — Clarify calculation/report dependencies and analysis time

### Evidence

- [metrics/callgraph.rs](../../src/metrics/callgraph.rs),
  [metrics/churn.rs](../../src/metrics/churn.rs), and
  [metrics/coupling/mod.rs](../../src/metrics/coupling/mod.rs) import types or
  score-band constants from `scorer`, which in turn invokes metric calculations.
- `code_age` in [metrics/evolution/mod.rs](../../src/metrics/evolution/mod.rs)
  reads `Utc::now()` directly. Wall-clock reads also occur in the scorer's file,
  author, and audit builders.

Shared types create dependencies in both directions. An unchanged snapshot is
not by itself sufficient to reproduce all time-dependent outputs. No
clock-controlled runtime reproduction or corpus failure was claimed.

### Proposed refactor and tradeoff

Move shared calculation results and score-band definitions to appropriately
owned dependency-neutral modules; let the report builder compose them. Pass one
explicit analysis timestamp into time-dependent calculations. Type moves and
timestamp plumbing add work, but make ownership and reproducibility explicit.
Choose live, cached, historical, and corpus time semantics before freezing the
plan; changing the meaning of age is a behavior change, not a mechanical move.

### Verification target

Metric calculations no longer depend on the report orchestrator. Fixed inputs
and a fixed analysis timestamp yield fixed outputs. Date-boundary behavior,
historical semantics, and compatibility of public type paths are covered.

## File size and test organization

Do not prioritize splits using total line count alone. At the reviewed revision,
`src/scorer.rs` has 886 lines but its main test module starts at line 124;
`src/metrics/hygiene.rs` has 1,455 lines with tests starting at line 470.
`src/metrics/coupling/mod.rs` has about 897 lines before its test declaration,
making it a stronger responsibility-splitting candidate as M02/M06 touch it.

Keep behavioral tests near the responsibility they protect. Avoid moving tests
or renaming integration suites solely to reduce a size metric.

## Review evidence and limits

- `pnpm test` in `dashboard`: 3 test files, 19 tests passed.
- The three M01 probes above ran against the actual dashboard helpers.
- Rust tests, Clippy, HTML smoke, and field corpus gates were not run for this
  structural review. Their results are not implied by the dashboard run.
- The review changed no source files. Pre-existing untracked files were left
  untouched. This document and the planning roadmap record subsequent planning.
- The review is not a complete correctness/security audit or merge approval.

Future implementations follow [the review process](../review-process.md): record
P0 probe output at plan freeze, enumerate all invariant consumers at P1, and run
the required full-suite and P2 corpus/audit gates before merge. A baseline change
must be explained and reviewed explicitly.
