# Per-Entity Trend History — Design

**Date:** 2026-08-18
**Status:** Proposed
**Parent design:** `2026-03-13-blame-cache-and-trends-design.md` (historical trend tracking) + `docs/crime-scene-book-notes.md` (Ch. 6, Ch. 8, Ch. 14 gap analysis)

## Context

`docs/crime-scene-book-notes.md` flags three chapters as unimplemented or
partial because barad-dûr's trend machinery only tracks the **aggregate
report score** across historical commits (`trends.json` / `HistoryEntry`,
`src/trend.rs`), never any metric at file or pair granularity:

- **Ch. 6** — whether a hotspot's complexity is growing or shrinking
  commit-over-commit. Today complexity is a snapshot at HEAD only.
- **Ch. 8** (partial) — whether a module/file pair's temporal-coupling
  degree is trending up (architecture eroding) or down. Today coupling
  smells are detected only at HEAD, no trend.
- **Ch. 14** (partial) — a churn timeline across history, not just the
  current run's `HotspotFile.churn_timeline` (which already exists but is
  a *within-one-run* breakdown of commits-per-1/12-of-window, not a
  *cross-run* series).

**Key finding that shapes this design:** `src/backfill/mod.rs::run()`
already walks a representative sample of historical commits and, at each
one, calls `scorer::build_report(...)`. That report **already contains**
`file_hotspots: Vec<HotspotFile>` (with `cyclomatic_complexity` and
`churn_count`) and `coupling_pairs: Vec<CouplingPair>` (with `co_changes`),
computed unconditionally by `build_hotspots`/`build_coupling_pairs`
regardless of which `CategoryResult`s were passed in. **The data this
feature needs is already computed at every backfill sample — nothing new
to collect, only something new to persist.**

One real gap exists: backfill currently calls
`Collector::collect_snapshot_at` (see `src/collector/snapshot_builder.rs:386`),
the AST-free variant — `AstParts::default()` — so `cyclomatic_complexity`
is `0` for every file in every backfilled sample today. This is resolved
explicitly in Decision 5 below.

## Decisions

1. **Scope: three signals, one mechanism.** Per-file complexity (Ch. 6),
   per-pair coupling degree (Ch. 8), and per-file churn count (Ch. 14) are
   recorded together at every backfill sample, through one new module and
   one new trend-direction classifier shared by all three. Rejected:
   building three separate one-off mechanisms — the data shape (a bounded
   set of entities, each with a numeric series over samples) is identical
   across all three, so one mechanism serves all.

2. **Storage: a new sidecar file, not an extension of `trends.json`.**
   New file `.repository-analysis/entity_trends.json` (JSONL, one line per
   backfill sample), new module `src/cache/entity_history.rs` mirroring
   `src/cache/history.rs`'s API (`load_entity_history`,
   `append_entity_entry`, `load_entity_history_checked` +
   `archive_and_replace`-style corruption handling — same pattern,
   separate file). Rejected: adding fields to `HistoryEntry`/`trends.json`.
   That struct's `metrics: HashMap<String, u32>` is a closed set of ~17
   named aggregate metrics with an established comparability contract
   (`old_trends_json_without_coupling_counts_still_loads`-style
   backward-compat tests); jamming a variable, per-run set of file/pair
   keys into it would blow up entry size, break the "same keys every run"
   assumption those tests protect, and couple two unrelated schemas'
   evolution together.

3. **Bounding: top-20 hotspots by `hotspot_score`, plus qualifying
   change-coupling-smell pairs.** `build_hotspots` returns *every* file in
   the snapshot (verified: no existing cap) — persisting all of them every
   sample forever is unbounded growth. Only the top 20 files by
   `hotspot_score` (already computed, just sorted-and-truncated at
   persistence time) get a complexity + churn entry per sample. Coupling
   degree reuses `qualifying_smell_pairs` (the same cross-boundary +
   ratio-thresholded predicate `corroboration_degree` and
   `change_coupling_smells` already share, per the M5 design's "single
   definition of a meaningful co-change") instead of the raw
   `coupling_pairs` list — bounded by construction, and reusing rather
   than inventing a second "interesting pair" definition. 20 is a new
   `BackfillConfig` field (`entity_trend_top_n`, default 20,
   serde-defaulted, no config migration), matching the existing
   `sample_count` field's style and doc-comment convention.

4. **Entity identity: path string / sorted pair key, no rename
   tracking.** A file's identity across samples is its path string; a
   pair's identity is `"{min(a,b)}|{max(a,b)}"` (lexicographic, so the
   same unordered pair always produces the same key regardless of which
   side `file_change_pairs` happened to put first). A rename produces a
   timeline gap (old path's series ends, new path's series starts) — an
   accepted, documented limitation. Solving rename-tracking is out of
   scope (git's rename detection is itself heuristic and would need
   threading through every historical sample's diff, a materially bigger
   feature).

5. **ADR-005 resolution: backfill switches to
   `collect_snapshot_at_with_ast` unconditionally.** The AST-free skip
   exists purely for backfill's original walk-many-commits performance
   goal, before any consumer needed AST data. `collect_snapshot_at_with_ast`
   already exists and is already paid for once per historical sample by
   the gate ratchet's baseline collection (`src/cmd/gate.rs` or
   equivalent) — this is the same cost, just paid at every backfill sample
   instead of once. Cost: backfill's default `sample_count` is 10, so this
   adds ~10 AST parses of the whole repo tree over the *entire* backfill
   run — the same per-commit AST cost `analyze`/`gate` already pay on
   every normal invocation, just amortized across 10 historical points
   instead of 1. Blame stays skipped (ADR-005's actual expensive part,
   85s of a 90s run per the parent trend design) — this decision narrows
   ADR-005 to "blame is skipped," not "AST is skipped," and the doc
   comment at `src/backfill/mod.rs:15-16` is updated accordingly. Rejected:
   a `--with-complexity-trend` opt-in flag — adds a footgun (silently
   empty complexity trend by default) for a cost increase that's small
   relative to backfill's existing multi-minute runtime for large repos.

6. **Trend classification: relative delta, not `trend.rs`'s absolute
   threshold.** `trend.rs::DIRECTION_THRESHOLD = 0.5` is tuned for 0–100
   integer scores. Complexity, coupling-degree, and churn counts have
   unrelated natural scales (a hotspot might run cyclomatic complexity
   40 or 400; a coupling pair might co-change 3 times or 30). A single
   absolute threshold would misclassify low-magnitude entities as
   perpetually "stable" and high-magnitude ones as perpetually volatile.
   `compute_entity_trend(series: &[f64]) -> EntityTrendDirection`
   (`Growing | Shrinking | Stable`) classifies on **percent change from
   the oldest to the newest point in the available window**
   (`(last - first) / first`), with a ±15% relative threshold — mirroring
   `trend.rs`'s velocity-window-then-classify shape but swapping the
   metric from absolute points/run to relative change, since these values
   have no natural common unit. Fewer than 2 points → `Stable` (no signal
   yet), matching `trend.rs`'s `is_first` handling. `first == 0.0` is
   treated as `Growing` if `last > 0`, `Stable` otherwise (percent change
   is undefined from a zero baseline).

## Architecture

```
backfill::run() per sampled commit
  │
  ├─ Collector::collect_snapshot_at_with_ast(...)   [was collect_snapshot_at]
  ├─ scorer::build_report(...)                       [unchanged call]
  │     → report.file_hotspots: Vec<HotspotFile>      (already computed)
  │     → report.coupling_pairs: Vec<CouplingPair>     (already computed)
  │
  ├─ history::append_if_new_head(...)                [unchanged]
  └─ entity_history::append_entity_entry(...)         [NEW]
        - select_top_hotspots(&report.file_hotspots, top_n)
        - select_qualifying_pairs(&snapshot, &coupling_thresholds)
        - build EntityTrendEntry { head, timestamp, branch,
            complexity: HashMap<String, u32>,
            churn: HashMap<String, u32>,
            coupling_degree: HashMap<String, usize> }
        - append as one JSONL line to entity_trends.json
```

Report rendering (CLI/JSON/HTML), separately:

```
entity_history::load_entity_history(repo_path)
  → for each of top-N current hotspots / current smell pairs:
      trend::compute_entity_trend(series_for(entity)) → EntityTrendDirection
  → attached to HotspotFile / CouplingPair as an optional field, same
    pattern as trend.rs's TrendSummary being attached to the overall report
```

## Schema

New struct in `src/scorer/types.rs` (or a new `src/cache/entity_history.rs`
if kept private to the cache layer — cache-layer, since renderers only need
the derived direction, not the raw series, mirroring how `trend.rs` — not
`cache::history` — owns `TrendSummary`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityTrendEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub head: String,
    pub branch: String,
    /// path → cyclomatic_complexity, top-N hotspots only
    pub complexity: HashMap<String, u32>,
    /// path → churn_count, same key set as `complexity`
    pub churn: HashMap<String, u32>,
    /// "{path_a}|{path_b}" (sorted) → co_changes, qualifying smell pairs only
    pub coupling_degree: HashMap<String, usize>,
    pub schema_version: u32,
}
```

Stored as JSONL at `.repository-analysis/entity_trends.json`, append-only,
same `archive_and_replace`-on-corruption behavior as `trends.json` (code
reused via a small shared helper extracted from `cache/history.rs`, since
the corruption-handling logic is identical byte-for-byte across both
files — this is the one intentional shared-code moment in the design).

## Consumption

`src/trend.rs` gains:

```rust
pub enum EntityTrendDirection { Growing, Shrinking, Stable }

pub fn compute_entity_trend(series: &[f64]) -> EntityTrendDirection
```

`AnalysisReport` build path (`build_hotspots`/`build_coupling_pairs`)
optionally receives the loaded `Vec<EntityTrendEntry>` and attaches a
direction to matching `HotspotFile`/`CouplingPair` entries — `None` when
no history exists yet (fresh repo, or entity not in top-N in any prior
sample), same "empty state" contract as `TrendSummary`'s
"fewer than 2 history entries" case in the parent trend design. CLI/JSON
renderers surface the direction as a new field; HTML report adds a small
trend arrow/badge next to complexity and coupling-pct columns in the
existing hotspots/coupling tabs — no new tab, no new UI paradigm, reusing
the score-color convention (`score_band`) is *not* applicable here since
this isn't a score, so a neutral up/down/flat glyph is used instead.

## Configuration

`BackfillConfig` gains one field:

```rust
#[serde(default = "default_entity_trend_top_n")]
pub entity_trend_top_n: usize,   // default 20
```

No other new config surface. `CouplingThresholds` (component_depth,
change_coupling_min_ratio) is reused as-is for the qualifying-pairs
selection — no new coupling-specific knobs.

## Testing strategy (TDD throughout)

- **`select_top_hotspots`**: given N hotspots with distinct scores, returns
  exactly `top_n` sorted descending by `hotspot_score`; returns all when
  fewer than `top_n` exist; empty input → empty output.
- **`select_qualifying_pairs`**: reuses `qualifying_smell_pairs` — a
  parity test asserting the same pair set `change_coupling_smells` counts
  is what gets persisted (guards against drift between the two call
  sites).
- **`entity_history::append_entity_entry` / `load_entity_history`**:
  round-trip test (write then read back byte-identical data); corruption
  test mirroring `load_history_checked_corrupt_file_triggers_archive_and_returns_warning`.
- **`compute_entity_trend`**: `[10.0, 15.0]` (50% up) → `Growing`;
  `[10.0, 8.0]` (20% down) → `Shrinking`; `[10.0, 10.5]` (5% up, under
  threshold) → `Stable`; `[]` and `[10.0]` → `Stable`; `[0.0, 5.0]` →
  `Growing`; `[0.0, 0.0]` → `Stable`.
- **Integration (`backfill_entity_trend_walking_skeleton.rs`)**: a
  fixture repo with a file whose cyclomatic complexity provably increases
  across 3 synthetic commits (e.g. adding nested `if`s), backfilled with
  `sample_count = 3` → assert `entity_trends.json` has 3 lines, the file's
  complexity series is monotonically increasing, and
  `compute_entity_trend` on that series returns `Growing`.
- **Backfill AST-pass regression**: assert `report.file_hotspots` entries
  have non-zero `cyclomatic_complexity` for a backfilled sample containing
  a non-trivial source file — the concrete regression test for Decision 5
  (today this would fail; it's the change that proves the fix).
- **Dogfood**: run `backfill` on barad-dûr itself, confirm
  `entity_trends.json` is produced and `cargo run -- analyze .` renders at
  least one non-`Stable` direction somewhere in the report (a real repo
  this size has *some* file whose complexity has moved >15% over its
  sampled history).

## Risks & mitigations

- **AST pass cost increase makes backfill noticeably slower**: mitigated
  by reusing the exact same `collect_snapshot_at_with_ast` path the gate
  ratchet already exercises (known, bounded cost), and by `sample_count`
  already being the user's existing dial for backfill runtime — no new
  dial needed, the existing one now controls "how many AST passes" too,
  which is the honest cost model.
- **Top-20 churn from run to run**: a file entering/leaving the top-20
  between samples creates a sparse, gap-y series. Accepted — `trend.rs`'s
  own `TrendVelocity` already tolerates sparse/short windows
  (`take_velocity_window` clamps to whatever exists); `compute_entity_trend`
  does the same (needs only oldest+newest of *available* points, not a
  contiguous run).
- **Relative-threshold instability near zero**: a pair going from 1
  co-change to 2 is a 100% increase but noise at that scale. Mitigated by
  the top-N/qualifying-pairs bounding already filtering to
  non-trivially-active entities before any trend math runs (a file with
  near-zero complexity or a pair with 1 co-change is unlikely to make the
  cut at all).
- **JSONL file grows unboundedly over very long project histories**:
  same growth profile as `trends.json` today (~one line per backfill
  sample, not per commit) — accepted per the parent trend design's "no
  pruning needed" call at ~300 bytes/entry; this entry is larger
  (bounded by `entity_trend_top_n`) but still O(samples), not O(commits).

## Future work (explicitly deferred)

- Rename-aware entity identity (Decision 4).
- Surfacing entity trends in the `backfill` CLI's own progress output
  (currently only prints `[n/total] Analyzing <sha>...`).
- A configurable relative-threshold override (currently a fixed 15%
  constant, mirroring `trend.rs::DIRECTION_THRESHOLD`'s own
  currently-fixed 0.5).

---

**Estimated implementation size: S/M.** No new collector work (the AST
pass already exists, just needs to be switched on for backfill — one call
site change) and no new metric computation (complexity/churn/coupling
data is already produced by every backfill sample today). New surface:
one new module (`src/cache/entity_history.rs`, ~mirrors existing
`history.rs`), 2-3 new pure functions in `src/trend.rs`
(`compute_entity_trend` + selection helpers), one new config field, and
renderer wiring to attach an existing-shape direction badge to two
existing report sections. No new subsystem, no new CLI flag, no new UI
paradigm — the risk is almost entirely in testing the bounding/relative-
threshold edge cases, not in building new machinery.
