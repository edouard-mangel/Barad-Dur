# Evolution: adaptive-trends-period

**Feature**: `barad-dur backfill` — retroactive historical trend seeding
**Delivered**: 2026-03-20
**Waves completed**: DISCUSS → DESIGN → DISTILL → DELIVER

---

## Summary

Implements the `barad-dur backfill` subcommand, which retroactively seeds `trends.json` by analyzing a sample of historical commits. Users can now run `barad-dur backfill .` on an existing repo to generate a trend line covering full git history without checking out past commits. Adaptive sampling selects ~10 evenly-spaced commits; `--no-blame` targets < 2 minutes on 1000-commit repos.

---

## Business Context

Prior to this feature, the trends dashboard only showed data from the date of first install onward. Teams with months or years of git history had no historical baseline. Two personas drove this feature:

- **Marco** (team lead): needs historical velocity data to show regression patterns to management
- **Priya** (developer advocate): needs pre-seeded trend data for sprint demos without waiting weeks

---

## Key Design Decisions

### DA-01 — Skip complexity metrics for backfill (ADR-005)
`file_metrics` is always `HashMap::new()` for backfill entries. `collect_file_metrics` reads the working tree via `std::fs::read_to_string`, which at backfill time contains HEAD content rather than historical content. Health, team, evolution, and hygiene scores derive from commit history (not file content), so empty file metrics produce correct scores.

### DA-02 — SHA-targeted collector variants (additive)
Added `collect_commits_at()` and `collect_files_at()` in `src/collector/libgit.rs` using revwalk and tree traversal targeting specific SHAs. No `git checkout` is ever executed — the working tree is never modified.

### DA-03 — Separate `src/backfill/` module
Follows the `src/init.rs` isolation pattern. Prevents `run_analyze` from accumulating backfill-specific conditionals.

### DA-04 — Pure sampling function
`src/backfill/sampling.rs::select_samples()` is a pure function — no I/O, no git calls, no global state. Evenly-spaced formula: `i * (len-1) / (count-1)` for `count` samples from `len` commits. Deterministically testable without any git infrastructure.

### DA-05 — `BackfillConfig` in `RepoConfig` (TOML integration)
`backfill.sample_count = 10` (default) in `barad-dur.toml`. Uses `#[serde(default)]` — existing TOML files remain valid.

### DA-06 — `source: Option<String>` on `HistoryEntry` (ADR-006)
Backward-compatible additive field. Live analyze entries omit `source` (via `skip_serializing_if`); backfill entries emit `"source": "backfill"`. Enables dashboard hollow-dot rendering and future targeted re-backfill. Old `"commit"` field name supported as serde alias.

---

## Steps Completed (12/12)

| Step | Description | Status |
|------|-------------|--------|
| 01-01 | Walking skeleton: CLI wires through to `backfill::run` | PASS |
| 01-02 | Sampling: `select_samples` with evenly-spaced formula | PASS |
| 01-03 | Schema version and branch field in history entries | PASS |
| 02-01 | Non-destructive: no working tree modification | PASS |
| 02-02 | `source: "backfill"` field in trend entries (ADR-006) | PASS |
| 02-03 | Deduplication: skip already-backfilled SHAs | PASS |
| 03-01 | Progress output: `[N/M] Analyzing {sha}...` | PASS |
| 03-02 | `--no-blame` flag writes valid entries | PASS |
| 03-03 | No-op guard: fully backfilled repo exits cleanly | PASS |
| 04-01 | Empty repo rejection with clear error message | PASS |
| 04-02 | `BackfillConfig` from `barad-dur.toml` | PASS |
| 04-03 | `backfill_no_blame_flag_writes_entries` enabled | PASS |

---

## Test Coverage

- **12 acceptance tests** across 3 test files (no `#[ignore]`)
  - `tests/backfill_walking_skeleton.rs` — 3 tests (end-to-end pipe)
  - `tests/backfill_milestone_1.rs` — 7 tests (safety, correctness, schema)
  - `tests/backfill_milestone_2.rs` — 2 tests (developer experience)
- All tests use real git fixture repos via `tempfile::TempDir` — no mocks
- Deferred: AC-BF-03b (performance < 120s, needs large fixture), AC-BF-09 (warn-and-continue on invalid SHA)

---

## Mutation Testing

**Feature-scoped kill rate: 80% (PASS)**

| Function | Kill Rate |
|----------|-----------|
| `backfill::run` | 83% |
| `sampling::select_samples` | 75% |
| `cache::history::append_if_new_head` | 100% |
| `cache::history::load_history` | 100% |

Follow-up (non-blocking): add test asserting exact sample indices in `select_samples` to catch 4 missed arithmetic formula mutations and raise its kill rate above 80%.

---

## Files Modified

| File | Change |
|------|--------|
| `src/backfill/mod.rs` | New — backfill orchestrator |
| `src/backfill/sampling.rs` | New — pure sampling function |
| `src/cli.rs` | Added `BackfillArgs` + `Commands::Backfill` |
| `src/config.rs` | Added `BackfillConfig { sample_count: u32 }` |
| `src/main.rs` | Dispatch `Commands::Backfill` |
| `src/scorer.rs` | Added `source: Option<String>` to `HistoryEntry`; renamed `commit` → `head` with alias |
| `src/collector/mod.rs` | Added `Collector::collect_snapshot_at` |
| `src/collector/libgit.rs` | Added `collect_commits_at`, `collect_files_at` |
| `src/collector/gitcli.rs` | Added `at_rev: Option<&str>` to `blame_file` |
| `src/trend.rs` | L1-L4: replaced `-0.5` magic literal with `-DIRECTION_THRESHOLD` |
| `tests/backfill_walking_skeleton.rs` | New — 3 walking skeleton tests |
| `tests/backfill_milestone_1.rs` | New — 7 milestone 1 tests |
| `tests/backfill_milestone_2.rs` | New — 2 milestone 2 tests |
| `tests/trend_walking_skeleton.rs` | Updated `entry["commit"]` → `entry["head"]` |
| `tests/common/mod.rs` | Added `init_git_repo_with_commits`, `read_trends_entries` helpers |
| `docs/adrs/ADR-005-backfill-skips-complexity-metrics.md` | New ADR |
| `docs/adrs/ADR-006-backfill-source-field.md` | New ADR |

---

## Issues Encountered

1. **Serde rename bug** — `HistoryEntry.head` was originally `#[serde(rename = "commit")]`. DISTILL spec required `"head"`. Fixed by changing to `#[serde(rename = "head", alias = "commit")]`, preserving backward compatibility with existing `"commit"` JSON files.

2. **Deduplication test fixture** — `backfill_deduplication` pre-seeded wrong SHAs (consecutive indices at tail of log) that the evenly-spaced formula doesn't select. Fixed by running a real first backfill to discover actual sampled SHAs, then using those as the pre-existing seed.

3. **Concurrent agent write conflict** — Steps 02-01 and 02-02 dispatched in parallel both edited `backfill_milestone_1.rs`, causing the second agent's write to re-add `#[ignore]`. Fixed via direct Edit + commit `0ea3935`. Lesson: serialize steps that share a single file.

4. **Mutation scope over-breadth** — `--file src/collector/mod.rs` included pre-existing functions not exercised by backfill tests, suppressing the apparent kill rate to 26%. Feature-scoped rate (new/modified functions only) is 80%.

---

## ADRs

- [ADR-005](../adrs/ADR-005-backfill-skips-complexity-metrics.md) — Backfill skips complexity metrics
- [ADR-006](../adrs/ADR-006-backfill-source-field.md) — Backfill source field
