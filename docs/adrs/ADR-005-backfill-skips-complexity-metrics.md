# ADR-005: Backfill skips working-tree complexity metrics

**Status**: Accepted
**Date**: 2026-03-19
**Feature**: adaptive-trends-period
**Deciders**: Morgan (Solution Architect)

---

## Context

The existing `collect_file_metrics()` function reads source files from the working tree using `std::fs::read_to_string(&abs_path)` to compute complexity-derived sub-scores: cyclomatic complexity, public method count, and lines of code (LOC).

When `barad-dur backfill` analyzes a historical commit SHA, the following conditions hold:

1. **Working tree reflects current HEAD, not the historical SHA.** The `collect_file_metrics` function would read current-HEAD file content and attribute it to a historical commit's score — producing incorrect results, not historical results.

2. **Files present in a historical tree may not exist on disk.** Files renamed or deleted between the historical commit and current HEAD are absent from the working tree. `std::fs::read_to_string` would return `Err`, requiring skip logic that would silently produce incomplete data.

3. **Checking out historical commits is prohibited.** DISCUSS decision D-04 (non-destructive) explicitly forbids modifying the working tree or git state during backfill.

4. **Reading historical file content via `git show <sha>:<file>` adds per-file subprocess overhead.** For a repository with 500 files and 10 sample commits, this is 5,000 subprocess invocations, which conflicts with the < 120 s performance target (DISCUSS decision D-07).

---

## Decision

`collect_snapshot_at()` passes `file_metrics: HashMap::new()` to the constructed `RepoSnapshot`. Complexity-derived sub-scores (cyclomatic complexity, public method count, LOC) are 0 in all backfill `HistoryEntry` records.

Scores derived from commit history — health (commit frequency, churn), team (author diversity, ownership), evolution (hotspots, file age), and hygiene (branch, tag, stale indicators) — remain fully accurate because they draw from git metadata, not file content.

---

## Alternatives Considered

### Alternative 1: `git show <sha>:<file>` per file at each historical SHA

Read historical file content via one `git show` subprocess per file per sample commit.

**Rejected**: For 500 files and 10 sample commits, this is 5,000 subprocesses, adding 10–50 s of I/O before any analysis begins. Conflicts with the D-07 performance target. Also adds error-handling complexity for files not present at the historical SHA (deleted files, renames).

### Alternative 2: Temporary worktree via `git worktree add --detach <sha>`

Create a detached worktree at the historical SHA, run `collect_file_metrics` against it, then remove the worktree.

**Rejected**: DISCUSS decision D-04 explicitly prohibits working tree modification. A worktree creation is a git state change. Also introduces cleanup risk: if backfill crashes mid-run, stale worktrees remain. The implementation complexity is disproportionate to the benefit (complexity scores in historical entries).

### Alternative 3: Include partial complexity scores (only files that still exist on disk)

Run `collect_file_metrics` against the working tree, skip files not found, and include whatever complexity data is available.

**Rejected**: This produces scores that mix current-HEAD file content with historical commit metadata. A file heavily refactored since the historical commit would report its current complexity, not its historical complexity. This is worse than 0 — it is misleading data presented as historical fact.

### Alternative 4: Accept 0 complexity scores with provenance tagging (this ADR)

Set `file_metrics = HashMap::new()`, which results in complexity sub-scores of 0. Tag entries with `source = "backfill"` so users and the dashboard know these entries have absent complexity data.

**Accepted**: Non-destructive, correct (no fabricated historical data), performant (no file I/O), and honest (0 is distinguishable from a real score via the `source` tag).

---

## Consequences

### Positive

- Working tree and git state are unmodified throughout backfill (D-04 satisfied)
- No per-file subprocess overhead; performance target D-07 is achievable
- Implementation is simpler: `collect_snapshot_at` constructs `RepoSnapshot` with an empty map
- Complexity scores in backfill entries are predictably 0, not randomly partial

### Negative

- Complexity sub-scores (cyclomatic complexity, LOC, public methods) are 0 in all backfill entries, not historical values
- Users viewing the trends dashboard will see flat complexity trend lines for the backfill period
- If the overall scoring formula weights complexity heavily, backfill `overall_score` values will be lower than they would be with real historical complexity data

### Mitigation

- The `source = "backfill"` field (ADR-006) allows the dashboard to render backfill entries distinctly (e.g., hollow dots, tooltip noting absent complexity data)
- A future ADR-006+ could add opt-in `git show <sha>:<file>` complexity collection behind a `--with-complexity` flag, accepting the performance trade-off for users who need it
- Documentation for the `backfill` subcommand must clearly state that complexity sub-scores are unavailable for historical entries
