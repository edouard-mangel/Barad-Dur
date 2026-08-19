# Source/Test Coupling — "Safety Net" Signal — Design

**Date:** 2026-08-18
**Status:** Proposed design
**Closes:** `docs/crime-scene-book-notes.md` → Chapter 9 gap

## Context

Chapter 9's technique treats the temporal coupling between a source file and
the test file(s) that exercise it as an early-warning signal: when a file
keeps changing but its paired test stops co-changing with it, the safety net
is eroding — tests aren't evolving alongside the code they're meant to guard.
`docs/crime-scene-book-notes.md` marks this ⬜ Not implemented.

That verdict undersells what already exists. `src/scorer/builders/coupling.rs`
already computes a per-pair `is_test_pair` flag (stem-based: `user.go` ↔
`user_test.go`, `parser.ts` ↔ `parser.spec.ts`, etc. — tested against Go,
Python, Java, C#, TS) and threads it onto every `CouplingPair` the coupling
dashboard tab renders as a badge. What's missing is entirely on the
**scoring** side: nothing aggregates those flags into a metric, so a repo
where every test file has stopped co-changing with its source produces no
signal in the report at all — only a per-row badge a human has to notice.
This spec closes that gap with a new scored metric, reusing the existing
pairing logic rather than re-deriving it.

## Decisions

1. **Extract the pairing predicate to a single source of truth.**
   `is_test_pair`/`is_test_of`/`file_stem` currently live private in
   `scorer/builders/coupling.rs`, taking `&str`. Move them to
   `src/metrics/file_role.rs` (already the file-classification module) as
   `pub fn is_test_pair(a: &Path, b: &Path) -> bool`, alongside `classify`.
   `scorer/builders/coupling.rs` calls the moved function instead of its own
   copy — a pure refactor guarded by its existing tests plus one parity test,
   mirroring the M5 precedent (`docs/superpowers/specs/2026-07-09-pressman-coupling-m5-design.md`)
   of extracting `qualifying_smell_pairs`/`corroboration_degree` so two call
   sites can't drift on what "a meaningful pair" means. Rejected: leaving a
   second copy in the new metric module (exactly the divergence risk the M5
   extraction was written to avoid).
2. **Scope: point-in-time coupling degree only, not growth-ratio trend.**
   The book chapter also tracks code-growth vs. test-growth ratio across
   iterations. That needs a historical time series barad-dûr doesn't have
   yet (`trend.rs`/`backfill/` only track the aggregate report score across
   history — see the per-entity trend design). This spec ships the
   snapshot-time coupling-degree signal only; the growth-ratio trend is
   explicitly deferred (Future work), to be picked up once per-file
   historical snapshots exist.
3. **Candidate-file requirement, not path-existence assumption.** A source
   file only enters the metric if the repo actually contains at least one
   file that `is_test_pair` matches it to. A source file with zero plausible
   test-file candidates (e.g. `main.rs`, glue/wiring code, a repo that tests
   only through integration suites with unrelated names) is **skipped**, not
   flagged — flagging it would be a false claim that a test file *should*
   exist by naming convention alone. This mirrors the project's existing
   "don't overclaim" stance (the M5 spec's "corroborated, never confirmed"
   language rule).
4. **Best-candidate matching.** If a source file matches more than one
   candidate (e.g. both `foo.test.ts` and `foo.spec.ts` exist), score against
   the candidate with the *highest* co-change ratio — the most charitable
   read, consistent with "flag only when the safety net is actually thin,"
   not "flag because one of several tests drifted while another stayed
   tight."
5. **Independent threshold, not reuse of `change_coupling_min_ratio`.** A new
   `CouplingThresholds.test_safety_net_min_ratio` (f64, default `0.30` —
   same numeric default as `change_coupling_min_ratio` for now, but a
   distinct knob) rather than reusing the existing field. The two measure
   different things (arbitrary cross-component smell vs. an expected-tight
   source↔test relationship) and teams may reasonably want them tuned
   differently. Serde-defaulted; no config migration.
6. **Surfaces inside the existing "Coupling" category**, as a new
   `MetricValue` alongside `change_coupling_smells` and the Pressman
   metrics — not a new top-level category. It consumes the same
   `file_change_pairs`/`commits_by_file` data with no new collector work, no
   new category weight, no config migration for the weights table.

## Architecture

New submodule `src/metrics/coupling/test_safety_net.rs`, following the
`community.rs`/`inheritance.rs` pattern already used in this directory
(self-contained file with its own `#[cfg(test)] mod tests`, rather than
growing the large shared `coupling/tests.rs`). Registered in
`compute_coupling()`'s `metrics` vec (`src/metrics/coupling/mod.rs`).

```
snapshot.files ──────────────┐
snapshot.file_change_pairs ──┼─► test_safety_net() ─► MetricValue
snapshot.commits_by_file ────┘        │
                                       uses file_role::is_test_pair (shared)
```

```rust
/// For every Source-role file with a plausible paired Test-role file in the
/// repo, the strongest (highest co-change ratio) candidate pairing found.
/// Files with no candidate test file are absent from the map — "no test
/// convention detected," not "test coverage is bad."
fn strongest_test_pairing(
    snapshot: &RepoSnapshot,
) -> HashMap<PathBuf, TestPairing> // { test_path, co_change_ratio }

/// Source/test pairs at or below `test_safety_net_min_ratio`: the safety
/// net is eroding for that source file. Scored on count via the standard
/// four-band scale (0→100, 1-2→75, 3-5→50, _→25), same as
/// `change_coupling_smells`.
fn test_safety_net(snapshot: &RepoSnapshot, thresholds: &CouplingThresholds) -> MetricValue
```

`strongest_test_pairing` computes, for each `FileRole::Source` file with a
nonzero commit count, the co-change ratio (`co_changes / min(commits_a,
commits_b)`, the same formula `qualifying_smell_pairs` and
`build_coupling_pairs` already use) against every candidate in
`snapshot.files` that `file_role::is_test_pair` matches it to, keeping the
highest-ratio candidate. A source file with commits but zero co-changes with
its best candidate still gets an entry (ratio `0.0`) — it's *checked*, just
failing. A source file with no candidate at all gets no entry.

`test_safety_net` filters that map to ratio `< test_safety_net_min_ratio`,
reports the count via `score_count_bands`, and lists up to 10 eroding pairs
in `RawValue::List` (same top-10 evidence convention as the Pressman
metrics), e.g.:

```
src/collector/snapshot_builder.rs ↔ tests/coupling_milestone_1.rs — 8% co-change (was 61%)
```

(The "(was 61%)" comparison is aspirational text only if/when the growth
history exists — v1 just reports the current ratio, no comparison clause.)

Metric description: `"3 of 42 source/test pairs below 30% co-change — safety net eroding"`.
When no Source file has any candidate Test file in the whole repo (e.g. a
project with no naming-convention-matched tests at all), the metric returns
`score: None` with description `"No source/test pairs detected by naming
convention"` — same "not applicable, not zero" pattern `afferent_coupling`
uses for an empty import graph.

## Configuration

`CouplingThresholds` gains `test_safety_net_min_ratio` (f64, default `0.30`,
serde-defaulted). Rejects `< 0.0` or `> 1.0` in `config::validate` (mirrors
how `change_coupling_min_ratio` is presumably already bounded — confirm and
match the existing validation pattern rather than inventing a new one).

## Interactions (deliberately untouched)

- **`scorer/builders/coupling.rs`'s `is_test_pair` badge:** unaffected in
  behavior after the extraction (parity-tested); the dashboard's per-row
  badge and this new aggregate metric are two views of the same underlying
  predicate, not two competing definitions.
- **Gate ratchet (M3, Pressman coupling):** untouched — this metric is not a
  Pressman finding kind and doesn't participate in that ratchet's per-kind
  count diff.
- **Backfill (ADR-005):** historical snapshots still collect
  `file_change_pairs`/`commits_by_file` (unlike the AST pass, that data
  doesn't require the collector's parse step), so this metric *can* run on
  backfilled snapshots — worth confirming in the integration test rather
  than assuming.

## Testing strategy (TDD throughout)

- **Predicate parity:** the moved `file_role::is_test_pair` reproduces every
  existing case in `scorer/builders/coupling.rs`'s test suite (suffix,
  `.test`/`.spec`, underscore, prefix, case-insensitive, rejects unrelated
  pairs) — run those exact assertions against the new location.
- **`strongest_test_pairing` unit tests:** a source file with one candidate
  and a qualifying co-change ratio; a source file with two candidates, picks
  the higher-ratio one; a source file with zero commits, excluded; a source
  file with a candidate but zero co-changes, present with ratio `0.0`; a
  source file with no candidate anywhere in `snapshot.files`, absent from
  the map.
- **`test_safety_net` unit tests:** count crosses each `score_count_bands`
  boundary on a synthetic snapshot; empty map → `score: None`; threshold
  configurability (a pairing below the default ratio but above a
  loosened `test_safety_net_min_ratio` is not flagged).
- **Integration test** (new `tests/source_test_coupling_*.rs`, following the
  `pressman_coupling_milestone_N.rs` naming convention): a fixture repo with
  a source file whose test file stopped co-changing in recent history →
  assert it's flagged and the Coupling category score reflects it.
- **Dogfood:** run against barad-dûr itself post-implementation — its own
  layout mixes inline `#[cfg(test)] mod tests` and separate `tests.rs`
  siblings (e.g. `src/metrics/team/mod.rs` ↔ `src/metrics/team/tests.rs`),
  which `is_test_pair`'s `_test`/`.test` stem rules already cover via the
  `tests.rs` ↔ (module dir name via `has_test_name`) path — confirm in the
  integration test that this repo's own well-maintained pairs correctly
  score as *not* eroding (a true-negative check, not just true-positive).

## Risks & mitigations

- **False positives from loose naming conventions:** mitigated by decision
  3 (skip, don't flag, when no candidate exists) and decision 4
  (best-candidate matching) — a team with inconsistent test naming sees
  fewer pairs checked, not more false flags.
- **Noise on brand-new files:** a file with 1 commit and no co-change yet
  reads as "eroding" on day one. Accepted, consistent with how
  `qualifying_smell_pairs` already treats `min_commits` without an extra
  floor — no new precedent, but worth a description caveat if it proves
  noisy in practice (not blocking v1).
- **Refactor regression on the moved predicate:** guarded by the parity
  test (Testing strategy, first bullet).

## Future work (explicitly deferred)

- Growth-ratio trend (code LOC vs. test LOC, or coupling-ratio-over-time)
  once per-entity historical snapshots exist (Group A / per-entity trend
  infrastructure design).
- The "(was N%)" delta-vs-history annotation sketched in Architecture,
  once the same trend infra lands.
- Community-aware corroboration (does the eroding pair also sit in
  different Louvain communities?) — not requested by the book chapter,
  omitted from v1 to keep scope tight.

## Estimated implementation size

**S** — one new ~120-line submodule (`test_safety_net.rs`) plus a small
extraction refactor of an existing, already-tested predicate; one new
`CouplingThresholds` field (serde-defaulted, no migration); no new collector
work, no new config surface beyond the one field, no renderer changes needed
for v1 (reuses the existing top-10 `RawValue::List` + description
convention already rendered generically by CLI/JSON/HTML). Roughly 6-8 pure
functions/tests plus one integration fixture.
