# Walking Skeleton — adaptive-trends-period

## First Test to Enable

`backfill_large_repo_writes_ten_entries` in `tests/backfill_walking_skeleton.rs`

## What the Walking Skeleton Validates End-to-End

The skeleton answers: "Can a developer run `barad-dur backfill` on a real repository and get a populated trends history?"

It exercises the full path in a single assertion:

1. Binary compiles and the `backfill` subcommand is registered in the CLI parser
2. The command accepts a `<path>` positional argument and locates the git repository
3. Adaptive sampling logic fires: a 15-commit repo produces exactly 10 evenly-spaced samples
4. Each sampled commit is analysed and a score is computed
5. Results are written to `<path>/.repository-analysis/trends.json` as NDJSON
6. The command reports progress ("10 entries written") and exits 0

A stakeholder can verify this skeleton by pointing `barad-dur backfill` at any repository with more than 10 commits and seeing a populated trends history appear, ready to visualise in the dashboard.

## Why 15 Commits

The adaptive sampling threshold is >10 commits. A 10-commit repo is boundary-ambiguous — it could trigger either path. 15 commits unambiguously exercises the sampling path (10 samples from 15) while remaining fast to set up in a TempDir fixture.

## TDD Enabling Order (Recommended)

Enable tests one at a time. Remove `#[ignore]` from each, implement until it passes, restore `#[ignore]` to prior tests (or leave passing), then move to the next.

| Order | Test | AC | Rationale |
|-------|------|----|-----------|
| 1 | `backfill_large_repo_writes_ten_entries` | AC-BF-01 | Skeleton — proves entire pipe from CLI to file |
| 2 | `backfill_exits_zero` | AC-BF-01 | Isolates crash from wrong-output failures |
| 3 | `backfill_writes_source_backfill_field` | AC-BF-10 | Validates provenance tag wired (ADR-006) |
| 4 | `backfill_small_repo_writes_all` | AC-BF-02 | Sampling boundary: ≤10 commits → write all |
| 5 | `backfill_schema_version_is_1` | AC-BF-10 | Schema completeness: head/branch/timestamp non-empty |
| 6 | `backfill_branch_is_current_branch` | AC-BF-11 | Branch recorded correctly for all entries |
| 7 | `backfill_non_destructive` | AC-BF-08 | Safety: working tree untouched |
| 8 | `backfill_deduplication` | AC-BF-04 | Idempotent re-run: pre-existing SHAs skipped |
| 9 | `backfill_no_op_guard` | AC-BF-05 | Full no-op: "Backfill already complete" message |
| 10 | `backfill_zero_commit_repo_exits_nonzero` | AC-BF-06 | Error path: empty repo rejected cleanly |
| 11 | `backfill_progress_output` | AC-BF-07 | UX: [1/N] … [N/N] progress markers |
| 12 | `backfill_no_blame_flag_writes_entries` | AC-BF-03 | Fast mode: --no-blame still produces valid output |
