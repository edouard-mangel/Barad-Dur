# Roadmap

This file is the single versioned record of what the project intends to
build and where each item stands. It records **state**, not estimates and
not intended order:

- Effort estimates live in the dated
  [difficulty ranking of 2026-09-11](plans/2026-09-11-roadmap-difficulty.md);
  the `Rank` column below is that ranking (1 = easiest). Re-rank by writing
  a new dated document, not by editing the old one.
- Ideas that are not planned, and the record of what was tried and
  rejected, live in [BACKLOG.md](BACKLOG.md).
- Bugs and small follow-ups live in the
  [GitLab issue tracker](https://lab.frogg.it/Edouard_Mangel/barad-dur/-/issues)
  and are not duplicated here.

No execution order is asserted. The only dependency worth recording is
noted in the `Notes` column.

**Update rule:** any merge request that starts, finishes, or abandons an
item updates its row in the same MR. `Evidence` must name something
verifiable on `main`: an MR, a squash commit, a tag, or a document.

Statuses: `todo`, `in progress`, `done`, `dropped` (with a reason).

## Features

| Rank | Item | Status | Evidence | Notes |
|---|---|---|---|---|
| 1 | Rust toolchain and dependency refresh | done | !144, [verification](reviews/2026-09-12-rust-dependency-refresh.md) | Rust 1.98.1 pinned; tree-sitter 0.27 |
| 2 | File exclusion from the dashboard, with the matching config change suggested | todo | — | Generates `.baraddurignore` entries; scores update on re-run, no live recalculation |
| 3 | macOS support | todo | — | Needs a macOS runner for native build and packaging |
| 4 | Per-commit score gate in CI | todo | [backlog entry](BACKLOG.md#per-commit-score-gate-against-a-baseline-ref) | Extends the coupling ratchet (`--baseline-ref`) to scores |
| 5 | Interactive config editor (TUI wizard) | todo | [backlog entry](BACKLOG.md#interactive-config-editor) | |
| 6 | Analysis data stored in a dedicated repository | todo | [backlog entry](BACKLOG.md#analysis-data-in-a-dedicated-repository) | Split cache (`snapshot.bin`, `blame_cache.bin`) from history (`trends.json`) first |
| 7 | GitHub/GitLab API integration for PR data | todo | — | Prerequisite of the PR/MR analysis below |
| 8 | Multi-repo dashboard (aggregate scores across repositories) | todo | — | Easier once analysis data has a dedicated home |
| 9 | PR/merge request analysis (review turnaround, approval patterns) | todo | — | Depends on the API integration (rank 7) |
| 10 | End-of-life map for dependencies, language runtime and framework | todo | [backlog entry](BACKLOG.md#end-of-life-map--dependencies-language-runtime-framework) | Needs an external support-window dataset with the 7-day cache pattern of `src/registry/` |

## Maintainability refactors

Six refactors from the
[structure review of 2026-09-07](reviews/2026-09-07-structure-maintainability.md),
planned in [maintainability-refactors.md](plans/2026-09-07-maintainability-refactors.md).
That document's own "Status" line predates implementation; this table is
authoritative.

| Id | Item | Status | Evidence | Notes |
|---|---|---|---|---|
| M01 | Report boundary: generated dashboard contract | done | !137 | |
| M06 | Calculation ownership and explicit reference time | done | !141 | |
| M02 | Shared analysis orchestration (analyze, gate, backfill) | done | !145, squash `3d9c3c8` | Breaking public API; covered by the next minor bump (issue #7) |
| M03 | Snapshot assembly behind named source channels | done | !146, squash `2dbe837`; review follow-up !149 | !149 also fixed Rust symbol-import resolution |
| M04 | Stable metric and category identity | in progress | branch `refactor/stable-metric-identity` | Started 2026-09-14 from `a66ea41` |
| M05 | Embedded HTML report modules and committed bundle | todo | — | Do after M04 settles identity keys |
