# Longitudinal trends — design & testing plan

Response to `2026-08-20-longitudinal-trends-planning-prompt.md`. Design +
testing strategy for the *Crime Scene* Ch. 6/8/9/14 trend family; no code.
File references verified against `main` at the org-coupling hardening
(post-!93 state).

**TL;DR:** three of the four chapters need **no new collection, no backfill,
no storage** — they are metric-time window-slicing over `Commit.files_changed`
data every snapshot already holds. Ship those first (M1–M3), annotation-first,
zero score changes. Ch. 6 is the only one needing historical AST; it becomes
an **opt-in `backfill --with-complexity`** (M4) built on `ast_pass_at`, scoped
to the current top-N hotspots, storing an additive-optional field in
`HistoryEntry`. Cut M4 first if the budget shrinks.

---

## 1. The load-bearing asymmetry

Every commit in the analysis window already carries per-file
`additions`/`deletions`/`change_type` (`Commit.files_changed`) and a
timestamp. Therefore:

| Ch. | Needs | Mechanism |
|-----|-------|-----------|
| 14 — churn timeline | nothing new | slice the window into day buckets at metric time |
| 9 — code/test growth | nothing new | sum additions per `file_role` partition per window half |
| 8 — coupling-degree trend | nothing new | count distinct co-change partners per window half |
| 6 — complexity trend | **historical AST** | multi-SHA sampling + `ast_pass_at` (opt-in) |

The prompt's risk 4 (sampling vs windowing) is resolved structurally: M1–M3
never touch `backfill/`; M4 never touches the metric-time slicers.

## 2. Milestones

### M1 — Ch. 14: churn timeline (repo-level) + coupling-growth annotation

**New report JSON section** (same `Option` + `skip_serializing_if` pattern as
`call_graph`):

```json
"churn_timeline": {
  "bucket_days": 1,
  "buckets": [ { "date": "2026-08-01", "added": 412, "deleted": 118 }, ... ],
  "merge_commits_excluded": true
}
```

- Day-bucketed lines added/deleted across the whole window, UTC calendar
  days, **merge commits excluded** (their first-parent diff double-counts
  every merged MR — the exact failure the org-coupling hardening fixed;
  reuse or mirror `team::files_by_bucket`'s exclusion, not a third policy).
  Empty days are emitted with zeros so the shape (spikes, silences) is
  visible without client-side gap-filling.
- **Ch. 14's prioritization half**: `CouplingPair` rows gain
  `growth_a`/`growth_b` (net lines added−deleted in-window per side) so a
  user can see *which* member of a temporally-coupled pair actually grew.
  Additive fields on an existing report row; no score impact.
- `None` when the snapshot has no commits.

No score anywhere. HTML/dashboard rendering is a separate follow-up; JSON
first, same as call-graph M2.

### M2 — Ch. 9: code/test growth balance (Evolution category, unscored)

New Evolution metric `"Code/test growth balance"`:

- Partition in-window non-merge commits' file changes by
  `file_role::classify` into Source and Test; sum additions per partition
  per **window half** (first half vs second half by timestamp — two slices,
  a constant `GROWTH_SLICES: usize = 2`, no config).
- Description (exact, tested):
  `source +1240 / test +310 lines this window; second half ratio 4.0:1 (first half 2.1:1)`
- Evidence list: the top source files added-to in the second half that have
  no test-role co-change partner in the window (the "death march" tell).
- **`score: None` permanently in v1** (risk 5): a growth ratio has no
  defensible universal band; `score: None` metrics are excluded from the
  category average by `compute_score` (established semantics), so this adds
  information without moving anyone's numbers. A band may graduate later
  with dogfood evidence, via the standard thresholds pattern.
- N/A shape when either partition has zero files or the window has no
  commits.

### M3 — Ch. 8: coupling-degree trend (annotation on existing surfaces)

Pure function: per file, the count of **distinct co-change partners** in the
first vs second window half (partner = shares a qualifying non-merge commit;
reuses the exact-commit pairing notion, computed windowed at metric time —
`snapshot.file_change_pairs` itself is whole-window and stays untouched).

- Files whose partner count at least doubles half-over-half (and reaches a
  floor to suppress 1→2 noise) surface on two report surfaces:
  - `HotspotFile` gains `coupling_trend: Option<CouplingTrend>` — structured
    first/second-half partner counts; renderers own the wording (a `3→9`
    badge on the Hotspots tab).
  - The Coupling category gains a dedicated **unscored** metric row,
    "Co-change reach trend" (count + top offenders) — no score change (the
    M5-corroboration precedent). *(Revised per the MR !98 post-merge
    review: the original afferent/efferent description note died behind
    those metrics' import-graph early returns; a git-derived signal now
    lives on its own row. Files with zero first-half partners are never
    flagged — new reach is not decay — and a half qualifies as a baseline
    only if it formed pairs.)*
- Thresholds: `thresholds.coupling.decay_min_partners` (default 8, was a
  borrow of `god_node_min_degree` — revised per the same review: an
  import-graph knob must not silently govern a co-change signal); the
  doubling factor is a named constant
  (`DECAY_GROWTH_FACTOR: f64 = 2.0`), not config, until someone
  needs to tune it.

### M4 — Ch. 6: per-hotspot complexity trend (`backfill --with-complexity`)

**ADR-005 revisit — the decision the prompt demands:**

- The ADR rejected per-file `git show` subprocesses (5,000 spawns). That
  objection is gone: `ast_pass_at` (built for the gate ratchet baseline)
  walks blobs in-process via libgit2 and runs the full tree-sitter pass at
  any SHA, sequentially, and is already shipped and tested.
- **Cost model:** one `ast_pass_at` ≈ the `analyze` complexity phase for the
  historical tree — on barad-dûr (~250 parseable files) roughly 1–3 s
  release-mode. Default `sample_count = 10` → **~10–30 s added, in-process,
  opt-in only**. The D-07 budget (< 120 s) governs the *default* backfill,
  which is untouched; the flag's own budget is "user asked for it". Abort
  criterion for the milestone: if dogfood measurement shows > 10 s per
  sample on this repo, the flag ships with a printed per-sample timing and
  the design revisits sampling count before M4 merges — measured, not
  assumed.
- ADR-005 is **amended, not repealed**: default backfill still skips AST and
  blame; the ADR gains an addendum documenting the flag (the ADR itself
  sketched exactly this future). Coupling category remains excluded from
  backfill entries either way (its category computation needs more than
  `file_metrics`).

**Scoping (risk 2):** only the **current HEAD's top hotspots** get a stored
series — the files `build_hotspots` ranks into the top
`health.hotspot_top_n` (existing config, default 10) at the moment backfill
runs. Tornhill's method is explicitly "trend the triaged hotspots", not the
whole tree. When the hotspot set changes between runs, old entries keep the
files they recorded; the trend view simply has shorter series for newer
hotspots — documented, not patched.

**Data model:** `HistoryEntry` gains one additive-optional field:

```rust
/// Per-hotspot complexity at this sample (backfill --with-complexity only).
#[serde(default, skip_serializing_if = "Option::is_none")]
pub hotspot_complexity: Option<Vec<HotspotComplexityPoint>>,  // {path, cyclomatic_complexity, loc}
```

`schema_version` stays 1: additive `Option` fields with serde defaults are
readable by old code (field skipped) and old entries by new code (`None`) —
the version constant is for breaking shape changes. `Vec` (not map), sorted
by path, for deterministic JSON. Snapshot cache (`CACHE_VERSION`) is
untouched — nothing new is stored in snapshots.

**Rename handling (risk 1):** reset-on-rename, documented as a bounded
limitation in the metric's doc comment and the ADR addendum. Git rename
detection at each sampled SHA would need per-sample diff similarity analysis
whose cost rivals the AST pass itself; a hotspot that was renamed mid-window
shows as a short series, which is honest. Future work: thread
`ChangeType::Renamed` old→new pairs through `files_changed` if demand
materializes.

**Surfacing:** trend direction per hotspot (`rising/flat/falling` via the
same `DIRECTION_THRESHOLD` idea as `trend.rs`, computed over the stored
points at render time) appears in the JSON report's hotspot rows as
`complexity_trend: Option<String>` when history has ≥ 3 points for that
file. No score impact.

## 3. What is deliberately not built

- No scored metric anywhere in M1–M4 (risk 5; annotation-first is now the
  third feature to follow this rule — corroboration, call-graph, trends).
- No per-file storage for M1–M3 (nothing to store — recomputed each run).
- No new config knobs except the existing `hotspot_top_n` reuse and the
  `--with-complexity` CLI flag.
- No day-bucketed backfill, no Ch. 6 for non-hotspot files, no rename
  tracking, no dashboard charts (JSON-first; charts are follow-ups per
  established pattern).

## 4. TDD / mutation-gate plan

Assertion rules as established (exact values, both-sides boundaries, exact
strings, determinism sorts). Per milestone:

1. **M1**: bucket arithmetic pinned (commit at 23:59 vs 00:01 UTC lands in
   adjacent buckets — both sides); merge exclusion (a merge's additions must
   not appear — mutant-verified like `files_by_bucket`); zero-fill (a gap
   day emits `{added:0,deleted:0}`); `None` on empty snapshot;
   `growth_a`/`growth_b` exact values on a constructed pair including a
   deletion-heavy side (net negative). Integration: fixture repo through
   `analyze --json` pinning an exact three-day series.
2. **M2**: half-split boundary (commit exactly at the midpoint timestamp —
   pin which half owns it, test both sides of the chosen rule); exact
   description string; ratio with zero test-additions (no division blowup —
   exact "no test growth" wording); N/A shapes; role classification joined
   correctly (a `_test.rs` file's additions land in the Test sum — exact).
3. **M3**: doubling boundary (3→5 partners not flagged, 3→6 flagged — both
   sides of `DECAY_GROWTH_FACTOR`); floor interplay (1→2 suppressed by
   `god_node_min_degree`); annotation string exact; absent (stable) file
   yields `None`; determinism of the flagged list.
4. **M4**: flag plumbing (default backfill entry has `hotspot_complexity:
   None` — pinned; with flag: exact points for a fixture repo whose second
   sample adds a branch to a hotspot, cc value pinned exactly);
   `schema_version` stays 1 and an old-format entry (JSON without the
   field) round-trips (serde default test); top-N scoping (file #11 by
   hotspot rank stores nothing); trend-direction both-sides boundary at the
   threshold; sequential `ast_pass_at` reuse — no new collector tests needed
   beyond one asserting the with-AST path populates `file_metrics` at a
   historical SHA (exists for the gate; extend if the flag takes a different
   entry point).

Each milestone is one MR; `--in-diff` mutation gate ≥ 80% per MR, with the
usual hand-applied-mutant verification for any test whose RED was masked by
batch compilation.

## 5. Cost vs payoff, and the cut order

M1–M3 are cheap (each S-sized, pure functions over existing data) and
deliver the book's day-to-day operational signals: crunch shape, test-lag,
decay-in-progress. M4 is M-sized, touches CLI + backfill + history schema,
and its payoff (per-hotspot complexity direction) is the most genuinely
"longitudinal" but also the most deferrable — the hotspot list itself
already tells you *where* to look; the trend only adds *which way it's
moving*.

**Cut order if budget shrinks: M4 first** (opt-in, separable, the only one
with collection cost), **then M3** (its annotation overlaps what
change-coupling smells already imply), never M1/M2 (near-free, close two
chapters outright).

**Recommendation: build M1–M3 now** as three small MRs; hold M4 until the
first three are dogfooded and the `--with-complexity` per-sample timing is
measured on a large real repo (not just barad-dûr) — the flag's design is
settled here either way, so M4 is implementation-ready when wanted.
