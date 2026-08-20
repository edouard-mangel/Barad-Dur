# Organizational (Conway's-Law) Coupling — Design

**Date:** 2026-08-18
**Status:** Implemented 2026-08-19 (see docs/superpowers/plans/2026-08-19-organizational-coupling.md)
**Closes:** Ch. 12 gap in `docs/crime-scene-book-notes.md` ("no main-dev-per-coupled-module
cross-team analysis; no day-bucketed coupling window")

## Context

Ch. 12 of *Your Code as a Crime Scene* argues that who touches coupled code matters as
much as the coupling itself: two files that co-change but are each dominated by a
different primary author carry a coordination/communication cost on top of the plain
code-coupling risk — the number of distinct contributors to a module has been shown to
predict defects better than most code metrics. The chapter's method has two ingredients:
(1) each file's **main developer** (the author who contributed most to it), and (2)
coupling measured with a **day-bucketed window** (grouping commits by calendar day
rather than requiring the exact same commit) so that two people working on related
files independently, a few hours apart, still register as coupled.

barad-dûr already has both halves separately but never joins them:

- **Ownership**: `snapshot.blame_map` (current-state, per-line author attribution) is
  already summarized per file by the `author_line_counts` helper and consumed by
  `metrics/health/bus_factor.rs::is_file_author_dominated` and
  `metrics/health/churn_ownership.rs::is_single_author_dominated` — both ask "does one
  author own >50% of this file's blamed lines?" but neither exposes *which* author, and
  neither is joined against coupling data.
- **Coupling**: `snapshot.file_change_pairs` (built by
  `RepoSnapshot::build_file_change_pairs` → `count_co_changed_pairs` in
  `src/snapshot/mod.rs`) counts co-changes **strictly per exact commit** — two files are
  a pair only if the same commit touched both. There is no day-bucketing anywhere in the
  intra-repo path. (`src/coupling/team.rs::analyze_team_coupling` does something
  structurally similar — shared-author overlap between two *repositories* — but it is
  the cross-repo multi-repo `coupling` CLI subcommand, a separate feature with its own
  `CouplingSnapshot`/`TeamCouplingPair` types. This spec extends the intra-repo path
  (`src/metrics/coupling/` + `src/metrics/team/`, feeding the main `analyze` report),
  not `src/coupling/`. `src/coupling/team.rs`'s Jaccard-style overlap is a useful pattern
  reference, not code to be reused directly — it operates on repo-level author sets, this
  spec needs file-level primary-author identity.)

## Decisions

1. **"Main developer" = blame-dominant author, not added-lines-across-history.** The
   book defines main developer as whoever contributed the most (lines added) to a
   module over its lifetime. barad-dûr does not currently aggregate per-author
   added-line counts across commit history (only current-state blame, which reflects
   who last touched surviving lines — refactors and reverts can change blame without a
   "contribution" in the book's sense). Computing true historical added-lines would
   require new collector work (per-commit diff-stat aggregation by author). Rejected for
   this spec: the cost isn't justified when `blame_map` already gives a *usably close*
   proxy — for a file with one clearly dominant author, blame-dominance and
   history-dominance overwhelmingly agree in practice, and disagreement only matters at
   the margin (barely-dominant authors), which is exactly where the signal is weak
   anyway. **Decision: reuse `blame_map` via a new `primary_author` helper** (same
   `author_line_counts` input `bus_factor.rs`/`churn_ownership.rs` already call), adding
   *which* author dominates (currently discarded — both call sites only keep the
   boolean). True historical-contribution main-developer is noted as future work.

2. **Day-bucketed coupling window: add it, scoped to this feature only — don't touch
   the existing exact-commit pairing.** `count_co_changed_pairs` in `src/snapshot/mod.rs`
   is exact-commit and several existing metrics (`change_coupling_smells`, M5
   corroboration, the gate ratchet's diffing) depend on its precise semantics and
   `min_commits: 3` cardinality — widening it repo-wide to day-granularity would change
   scores across the whole coupling category and is out of scope (a breaking behavior
   change to a maintainer-authored, gate-ratcheted metric, not something this spec should
   destabilize). Cross-team coupling gets **its own** day-bucketed pair computation, built
   the same way (`commit.files_changed` per commit → group by `(author, day)` instead of
   by exact commit → co-occurring files across each author-day bucket are a pair),
   living beside `count_co_changed_pairs` but not replacing it. This directly answers the
   gap-analysis note: the day-bucketing gap is real (confirmed — `build_file_change_pairs`
   has no day option), and it's closed by addition, not by widening the existing metric.

3. **Signal → finding: primary-author mismatch on a qualifying day-bucketed pair.** A
   cross-team-coupling finding fires when a day-bucketed pair (A, B) qualifies by the
   *same* ratio rule the existing smell predicate uses
   (`co_changes / min_commits >= change_coupling_min_ratio`, reusing
   `CouplingThresholds.change_coupling_min_ratio` — one ratio-qualification definition,
   not a second divergent knob, mirroring how M5's corroboration reused the smell
   predicate) **and** `primary_author(A) != primary_author(B)` **and** both files have a
   primary author (files with no blame-dominant author — i.e. genuinely
   collectively-owned — are not organizational-coupling risks; skip them). Cross-boundary
   (`component_depth`) is *not* required here, unlike the plain smell predicate — Conway's
   law risk is about people, not directories; two files in the same component with
   different owners are still a coordination cost.

4. **Score impact: new Team-category metric, not a nudge to Coupling.** Unlike M5
   (which folded corroboration into existing Pressman coupling scores because it was
   additional evidence for a coupling problem already being scored), this is a
   fundamentally different signal — organizational/communication risk, not code
   structure risk — and the Team category (`src/metrics/team/mod.rs::compute_team`)
   already owns exactly this kind of people-and-process metric (`knowledge_distribution`,
   `ownership_clarity`, `collaboration_patterns`). **Decision: add a sixth Team metric,
   `cross_team_coupling`**, scored by finding count via the existing `score_count_bands`
   convention (0 → 100, matching every other count-banded metric in the codebase — no new
   banding scheme). Rejected: folding into the Coupling category's severity-cap machinery,
   which is reserved for Pressman code-structure findings and would conflate two distinct
   risk types under one cap.

5. **Granularity: individual authors, no team-mapping config.** barad-dûr has no
   org-chart/team-membership concept anywhere in config (`TeamThresholds`,
   `CouplingThresholds` — neither has one), and inventing one is a heavier, separate
   feature with real design questions (how are teams declared, kept in sync with reality,
   etc.) that this spec doesn't need to answer to deliver the book's core signal — "two
   coupled files, two different primary people" is itself informative even without team
   boundaries, matching how `bus_factor`/`churn_ownership` already operate at
   individual-author granularity. **Decision: individual-author granularity only.**
   Explicit team-mapping (grouping authors into named teams, so the finding reads "Team A
   ↔ Team B" instead of "Alice ↔ Bob") is deferred future work, consistent with how the
   architecture-conformance spec (Group C, if written) would introduce a comparable new
   declared-boundary config surface — this spec avoids duplicating that decision here.

## Architecture

Pure computation, no collector change — `blame_map`, `commits` (with `files_changed`,
`author`, `timestamp`), and `commits_by_file` are already in `RepoSnapshot`.

```
snapshot.commits ──────────────► day_bucketed_pairs() ─┐
   (author, timestamp,                                 │
    files_changed)                                     ├─► cross_team_coupling()
                                                         │      │
snapshot.blame_map ─────────────► primary_author() ─────┘      ▼
                                     (per file)          Team metric (finding list)
```

### New functions (both pure, in `src/metrics/team/`)

```rust
/// The author with the most blamed lines in a file, when that author's share
/// exceeds 50% (mirrors bus_factor.rs's dominance threshold, but returns
/// *which* author instead of a bool — the piece those call sites discard).
/// `None` when the file has no blame data or no author holds a strict majority.
fn primary_author(lines: &[BlameLine]) -> Option<usize> // author_id
```

```rust
/// Co-changed file pairs grouped by (author, calendar day) instead of exact
/// commit: two files are a pair if the same author touched both on the same
/// UTC day, even across separate commits. Distinct data source from
/// `snapshot.file_change_pairs` (exact-commit) — existing coupling metrics
/// are untouched.
fn day_bucketed_pairs(snapshot: &RepoSnapshot) -> Vec<(PathBuf, PathBuf, usize)>
```

```rust
/// Cross-team coupling findings: day-bucketed pairs meeting
/// change_coupling_min_ratio whose files have different (and known)
/// primary authors.
fn cross_team_coupling(
    snapshot: &RepoSnapshot,
    thresholds: &CouplingThresholds,
) -> MetricValue
```

`compute_team` in `src/metrics/team/mod.rs` gains this as its sixth metric; it needs
`CouplingThresholds` passed in alongside the existing `TeamThresholds` (a small signature
change — `compute_team` is called once from `scorer.rs`, so this is a single call-site
update).

**Note on day-bucketing and the existing `min_commits` semantics:** day-bucketed pairs
use the same qualifying-ratio idea but recompute `min_commits` from
`commits_by_file`-style per-file counts *within the day-bucketed universe*, not the raw
exact-commit `commits_by_file`. Concretely, `min_commits` for the ratio denominator is
`min(day_count(A), day_count(B))` — the number of distinct (author, day) buckets each
file appears in — keeping the ratio's meaning ("how often do these two files move
together, relative to how often each moves at all") consistent with the exact-commit
version's intent, just at day granularity.

## Surfacing

Findings render through the existing `RawValue::List` evidence pattern:

```
src/renderer/html.rs ↔ src/renderer/templates/report.js — coupled 4 day(s),
  primary owners: alice vs. bob
```

Metric description:

```
2 cross-team coupling pair(s) — coupled files with different primary owners
```

CLI, JSON, HTML renderers pick this up generically through the standard `MetricValue` /
`CategoryResult` shape — no renderer changes needed (same "generic surfacing" pattern
M5 and M6 relied on).

## Interactions

- **Backfill (ADR-005):** historical snapshots skip the AST pass but blame is *always*
  skipped too (`collect_snapshot_at` never populates `blame_map`) — so
  `cross_team_coupling` has no primary-author data in a backfill sample and returns the
  existing "No blame data" `N/A` shape (same pattern every blame-dependent Team metric
  already uses). Consistent with the rest of the Team category during backfill; no
  special-casing needed.
- **Coupling category / M5 corroboration:** untouched. This spec's day-bucketed pairs are
  a separate data source consumed only by the new Team metric; `qualifying_smell_pairs`,
  `corroboration_degree`, and the gate ratchet's diffed counts are exact-commit and
  unaffected.
- **Gate ratchet:** the new metric is a plain count-banded `MetricValue` like the rest of
  Team — it participates in the overall/category score the same way `knowledge_distribution`
  etc. already do; no ratchet-specific wiring needed (Team category isn't currently
  ratcheted the way Coupling's Pressman counts are, per existing `gate` behavior).

## Configuration

No new config struct. Reuses `CouplingThresholds.change_coupling_min_ratio` (already
threaded through `compute_coupling`; `compute_team` gains the same parameter). No new
serde fields, no config migration.

## Testing strategy (TDD throughout)

- **`primary_author` unit tests:** empty lines → `None`; exact 50/50 → `None` (matches
  `is_file_author_dominated`'s existing "not strictly greater" semantics, same boundary
  test style as `bus_factor.rs`'s `dominated_exact_50_50_split_is_false`); 51/49 → the
  majority author's id; single-author file → that author.
- **`day_bucketed_pairs` unit tests:** two files touched by the same author in two
  separate same-day commits → paired (proves it's day-granularity, not exact-commit);
  two files touched by the same author on different days → not paired at that day-bucket
  count; two files touched by *different* authors on the same day → not paired (pairing
  is per-author-day, not "any commits happened same day" — a repo-wide same-day
  coincidence isn't a coupling signal).
- **`cross_team_coupling` unit tests:** a qualifying day-bucketed pair with differing
  primary authors → finding; same primary author on both files → no finding (same-owner
  coupling isn't a Conway's-law risk, just... one person's normal work); a file with no
  primary author (no blame majority) on either side → no finding (nothing to compare);
  a pair below `change_coupling_min_ratio` → no finding even with differing owners
  (reuses the ratio gate, doesn't invent a looser one).
- **Integration test** (new `cross_team_coupling_walking_skeleton.rs`, following the
  project's `<feature>_walking_skeleton.rs` naming convention): a fixture repo where
  Alice and Bob each dominate a different file, and those files co-change across several
  same-day-different-commit pairs → assert the Team category surfaces the finding with
  both owner names in the evidence string.
- **Dogfood:** run against barad-dûr itself once implemented — a solo/small-team repo may
  show 0 findings (expected, not a bug — Team category already degrades gracefully below
  `MIN_TEAM_SIZE = 4` authors), so the fixture test is what proves correctness, same as
  M5's fixture-vs-dogfood split.

## Risks & mitigations

- **False signal from incidental same-day activity:** mitigated by requiring the
  *same author* to have touched both files on the same day (not just "any activity that
  day"), and by reusing the existing ratio-qualification threshold rather than a raw
  count.
- **Blame-dominance ≠ true historical main-developer:** accepted and documented (Decision
  1) — the proxy is close enough for a coordination-risk *nudge*, and getting it exactly
  right requires new collector-level history aggregation that isn't justified for this
  signal alone.
- **Double-computing pair data (existing exact-commit pairs vs. new day-bucketed pairs)
  is O(commits) twice:** acceptable — `count_co_changed_pairs` is already a single linear
  pass over commits; a second linear pass for day-bucketing is the same order of cost,
  and both only run once per `analyze` invocation (not per backfill sample, since blame
  is unavailable there anyway per the Interactions section).

## Future work (explicitly deferred)

- True historical main-developer (added-lines aggregation across all commits, not just
  current blame) — would need new collector-level per-author diff-stat accumulation.
- Explicit team-mapping config (group individual authors into named teams so findings
  read "Team A ↔ Team B").
- Trend of cross-team-coupling count over time (depends on whatever per-entity history
  infrastructure a "Group A" trend-infrastructure spec, if adopted, ends up providing).

## Estimated implementation size

**S–M.** No new module, no new config struct, no new collector work — two new pure
functions in `src/metrics/team/mod.rs` (~60–90 LOC combined) plus one new `MetricValue`
wired into `compute_team`'s existing metrics vec, ~8–10 unit tests plus one integration
fixture test. The only real complexity is the day-bucketing pass, which is a
straightforward variant of `count_co_changed_pairs`'s existing per-commit grouping logic
grouped by `(author, day)` instead of by commit.

## Post-merge review addendum (2026-08-20)

A deep review of the merged MR !90 drove these amendments:

- **Merge commits are excluded from day buckets.** A merge's `files_changed`
  is the full first-parent diff; bucketing it paired files across unrelated
  MRs under the integrator's name. Same exclusion evolution/hygiene apply.
- **Support floor `MIN_CO_DAYS = 2`.** Decision 3 said "same ratio rule as
  the existing smell predicate" but that predicate inherits a `count >= 3`
  floor from `count_co_changed_pairs`; the day-bucketed path had none, so a
  single one-off co-change day qualified at ratio 1.0. Two (not three)
  keeps the day signal more sensitive than the commit one.
- **Shared ratio helper.** `meets_coupling_ratio` in `metrics/mod.rs` is now
  the single definition consumed by both `qualifying_smell_pairs` and
  `cross_team_coupling`.
- **Unknown-author sentinel.** Blame lines whose email matches no in-window
  author previously collapsed onto author id 0, and this metric was the
  first to *print* blame-derived names — misattributing legacy code to an
  arbitrary current author. `UNKNOWN_AUTHOR` (usize::MAX) now marks such
  lines; `primary_author` never names it (the unknown mass still counts
  toward majority totals), ownership views render "(unattributed)", and
  the knowledge-distribution Gini ignores it. `CACHE_VERSION` 5 → 6.
- **Evidence label** corrected from "coupled N day(s)" to "co-changed on N
  author-day(s)" — the count is (author, day) buckets, not calendar days.
- **Partial blame coverage is disclosed**: qualifying pairs whose files
  lack blame data are counted in the metric description instead of
  silently dropped.
- Context-paragraph nit acknowledged: cross-author same-day pairing is
  promised in the Context prose but excluded by Decision 2; the Decisions
  are binding.
