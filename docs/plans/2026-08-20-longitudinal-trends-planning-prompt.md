# Longitudinal trends — planning prompt

Not a design doc — a self-contained prompt for handing to another agent (e.g.
Fable) to produce the actual design/implementation plan for per-entity
longitudinal analysis. This is the "biggest structural gap" named at the
bottom of `docs/crime-scene-book-notes.md`: four *Your Code as a Crime Scene*
chapters (6, 8, 9, 14) all need trend series at per-file or per-pair
granularity, and barad-dûr's history machinery only tracks the aggregate
report score.

Paste the block below as-is into a fresh agent session — it assumes no prior
context.

---

```
I need a detailed implementation plan (design + testing strategy, not code yet) for adding per-entity longitudinal analysis to barad-dûr, a Rust CLI repository-health analyzer. This is a real feature for an existing, mature codebase — study the architecture below before proposing anything.

## What the feature is

barad-dûr scores a repo's current state and tracks ONE number over time: the aggregate report score (`src/trend.rs` — velocity, sparkline — fed by `src/backfill/`, which samples historical commits). Four techniques from Adam Tornhill's *Your Code as a Crime Scene* all require trends at finer granularity, and none exist:

1. **Ch. 6 — per-file complexity trend**: is a specific hotspot's complexity rising, flat, or falling across its history? (Lehman's law made actionable per file.)
2. **Ch. 8 — per-module coupling-degree trend**: is one file's coupling-partner count growing across successive time windows? (Architectural decay caught in progress.)
3. **Ch. 9 — code/test growth-ratio trend**: are Source and Test partitions growing in step, or is one running away? (`src/metrics/file_role.rs` already classifies Source/Test/Config/Docs.)
4. **Ch. 14 — churn timeline**: day-bucketed lines-added/deleted shape over the window (crunch spikes, deadline patterns). Note `scorer/builders/hotspots.rs::churn_timeline` already buckets per-file COMMIT COUNTS into 12 slices for a sparkline — the gap is lines added/deleted and repo-level shape, not the bucketing idea.

Your plan should decide which of these four ship in which milestone, and which (if any) should be cut — they share infrastructure but have very different cost profiles.

## Architecture you must work within

Pipeline: `CLI (clap) → Collector (git2 + git CLI) → RepoSnapshot → Metrics → Scorer → Renderer`. Read these files before designing anything:

- `src/backfill/mod.rs` + `src/backfill/sampling.rs` — the historical sampler: adaptive sampling picks N commits (config `backfill.sample_count`, default 10), and for each calls `collect_snapshot_at()` then `build_report()`, appending a `HistoryEntry` per sample.
- `src/scorer/types.rs::HistoryEntry` — what a history point currently stores: timestamp, head SHA, overall score, per-category and per-metric score maps, `HistoryCounts` (commits/files/authors + optional per-kind coupling-finding counts), `schema_version: u32` (currently 1), `source` tag (ADR-006: backfill entries render distinctly). Stored via `src/cache/history.rs`. Any shape change is a schema-version decision.
- **`docs/adrs/ADR-005-backfill-skips-complexity-metrics.md`** — backfill skips ALL AST work and blame at historical SHAs; complexity sub-scores are 0 in backfill entries. CRITICAL: the ADR's cost argument is partly obsolete. It rejected "5,000 `git show` subprocesses" — but `src/collector/snapshot_builder.rs::ast_pass_at()` now exists (built for the gate ratchet's baseline): an in-process libgit2 blob walk running the full AST pass at any SHA, no subprocesses, already used in production for single-SHA baselines. Ch. 6 hinges on whether running `ast_pass_at` per sampled commit (N samples × repo files, tree-sitter parse each) fits a sane budget — your plan must actually reason about this cost (barad-dûr itself: ~200 source files; the ADR's D-07 target was < 120 s for a backfill run) and decide: revisit ADR-005 with an opt-in flag (the ADR itself sketches `--with-complexity`), or keep Ch. 6 out. Do not hand-wave this.
- `src/trend.rs` — aggregate trend computation over history entries (deltas, velocity); the pattern any per-entity trend would extend or parallel.
- `src/cmd/gate.rs` — the gate ratchet already collects a full AST snapshot at ONE baseline SHA per run via `collect_snapshot_at_with_ast`. Precedent for "historical AST is affordable at small N".
- `src/metrics/team/mod.rs::files_by_bucket` — (author, UTC day) bucketing shipped 2026-08-20 for cross-team coupling (merge commits excluded). Ch. 14's day-bucketed churn should share or mirror this, not invent a third bucketing.
- `src/snapshot/mod.rs::Commit.files_changed` — per-commit additions/deletions per file are ALREADY collected for every commit in the window. Ch. 9 and Ch. 14 need NO new collection and NO backfill: they are pure metric-time computations over data in every current snapshot. Ch. 8 likewise (co-change pairs can be windowed by commit timestamp at metric time). Only Ch. 6 needs historical AST. Your milestone split should exploit this asymmetry.
- `src/cache/storage.rs` (`CACHE_VERSION`, currently 6) and `src/cache/history.rs` — snapshot cache vs history cache are separate; know which one any new data touches.

## Project conventions (non-negotiable, from CLAUDE.md)

- Functional paradigm: pure `(snapshot) → MetricValue` metric functions, no I/O in metrics.
- TDD mandatory; per-MR mutation gate `cargo mutants --in-diff` ≥ 80% kill rate — the testing strategy must name both-sides boundary tests and exact-value assertions per metric.
- Score-band thresholds live once in `scorer/types.rs`; new tunables follow the `#[serde(default)]` + `validate()` + default-pinning-test pattern in `src/config/thresholds.rs`.
- Spec → plan → TDD → MR workflow; specs in `docs/superpowers/specs/`, plans in `docs/superpowers/plans/`.

## Known risks you must address head-on

1. **Entity identity across history.** Files get renamed; a per-file trend that resets on rename is noise. `Commit.files_changed` records `ChangeType::Renamed` but no old→new mapping is stored. Decide: follow renames (how, at what cost), or document reset-on-rename as a bounded limitation.
2. **Storage growth.** Per-entity series × N samples × M files can bloat `history.json`. Tornhill's own practice is the scoping lever: trends are computed for TRIAGED entities (current hotspots), not every file. Decide top-N-only vs all-files, and what happens when the hotspot set changes between runs.
3. **Ch. 6 cost honesty** (see ADR-005 bullet above): give a concrete cost model, an opt-in mechanism, and a stated abort criterion, or cut it.
4. **Sampling vs windowing confusion.** Ch. 8/9/14 can be computed from the CURRENT snapshot by slicing the analysis window (commits carry timestamps) — no backfill involvement. Ch. 6 genuinely needs multi-SHA sampling. Keep these two mechanisms distinct in the design; conflating them was the trap the gap-analysis note warned about.
5. **Trend → score temptation.** A trend is a derivative; scoring it (e.g. "complexity rising → penalize") doubles down on noisy small-sample slopes. The corroboration/annotation precedent (M5 coupling, call-graph): surface trends as evidence/annotation first, score only after dogfooding. State this stance explicitly per chapter.

## What I want back

A written design document covering: which chapters ship in which milestone and why (exploit the "Ch. 8/9/14 need no new collection" asymmetry); data model for any stored series (HistoryEntry schema-version impact, or why nothing new is stored); the ADR-005 revisit decision for Ch. 6 with a real cost model and opt-in design; rename handling; top-N scoping; how each trend is surfaced (annotation vs scored metric, per risk 5); a TDD/mutation-testing plan naming the boundary tests; and a clear-eyed final section weighing cost against the four chapters' payoff — including which chapter you would cut first if the budget shrinks.
```
