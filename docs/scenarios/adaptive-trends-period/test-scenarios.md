# Test Scenarios — adaptive-trends-period

All scenarios are `@pending` / `#[ignore]` pending implementation.

## Scenario Table

| Test Name | AC Reference | User Story | Milestone | File | Status |
|-----------|-------------|-----------|-----------|------|--------|
| `backfill_large_repo_writes_ten_entries` | AC-BF-01 | US-BF-01 | Walking Skeleton | `backfill_walking_skeleton.rs` | @pending |
| `backfill_writes_source_backfill_field` | AC-BF-10 (partial) | US-BF-02 | Walking Skeleton | `backfill_walking_skeleton.rs` | @pending |
| `backfill_exits_zero` | AC-BF-01 | US-BF-01 | Walking Skeleton | `backfill_walking_skeleton.rs` | @pending |
| `backfill_small_repo_writes_all` | AC-BF-02 | US-BF-01 | Milestone 1 | `backfill_milestone_1.rs` | @pending |
| `backfill_deduplication` | AC-BF-04 | US-BF-01 | Milestone 1 | `backfill_milestone_1.rs` | @pending |
| `backfill_no_op_guard` | AC-BF-05 | US-BF-03 | Milestone 1 | `backfill_milestone_1.rs` | @pending |
| `backfill_zero_commit_repo_exits_nonzero` | AC-BF-06 | US-BF-03 | Milestone 1 | `backfill_milestone_1.rs` | @pending |
| `backfill_non_destructive` | AC-BF-08 | US-BF-02 | Milestone 1 | `backfill_milestone_1.rs` | @pending |
| `backfill_schema_version_is_1` | AC-BF-10 | US-BF-02 | Milestone 1 | `backfill_milestone_1.rs` | @pending |
| `backfill_branch_is_current_branch` | AC-BF-11 | US-BF-02 | Milestone 1 | `backfill_milestone_1.rs` | @pending |
| `backfill_progress_output` | AC-BF-07 | US-BF-02 | Milestone 2 | `backfill_milestone_2.rs` | @pending |
| `backfill_no_blame_flag_writes_entries` | AC-BF-03 | US-BF-02 | Milestone 2 | `backfill_milestone_2.rs` | @pending |

## Deferred

| AC Reference | Reason |
|-------------|--------|
| AC-BF-03b | Performance property test (`--no-blame` runtime < 120s). Non-blocking; requires a large real repo fixture. Deferred to a separate performance test pass. |
| AC-BF-09 | Warn-and-continue on single invalid SHA. Requires injecting a corrupt SHA into the git log stream; lower priority than safety/correctness milestone. |

## Coverage by User Story

| User Story | ACs Covered | ACs Deferred |
|-----------|-------------|--------------|
| US-BF-01 (adaptive sampling) | AC-BF-01, AC-BF-02, AC-BF-04 | — |
| US-BF-02 (fast + safe + schema) | AC-BF-03, AC-BF-07, AC-BF-08, AC-BF-10, AC-BF-11 | AC-BF-03b |
| US-BF-03 (guard rails) | AC-BF-05, AC-BF-06 | AC-BF-09 |
