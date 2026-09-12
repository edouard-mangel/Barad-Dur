# Kotlin wildcard resolution evidence

Base: `cbadbe0` (Rust/dependency refresh). Worktree: `.worktrees/kotlin-wildcard-resolution`; branch: `perf/kotlin-wildcard-resolution`.
Plan and grammar probe: [Kotlin import extraction and wildcard lookup](../plans/2026-09-12-kotlin-wildcard-resolution.md).

## Ktor experiment

Repository: https://github.com/ktorio/ktor, commit `88c0026f6fee2af55d96a20620f2e700b912b334`.
Shallow clone; this measures source import resolution, not history-dependent scores.

The benchmark reads Git-tracked paths and extracts imports from full `.kt`/`.kts` contents using the production tree-sitter extractor. File reads and parsing occur before timing. Every measured pass includes construction of the path/directory indexes, resolution of **all** imports, and deterministic target materialization. One warm-up, seven timed passes, with graph equality checked each pass. Both versions use the corrected extractor. The comparison isolates the directory lookup optimization; comparing against the original broken extractor would measure an empty workload.

| Measurement | Scan every file | Directory index |
|---|---:|---:|
| Tracked files | 3,129 | 3,129 |
| Kotlin files | 2,593 | 2,593 |
| Extracted imports | 15,190 | 15,190 |
| Wildcard imports | 9,446 | 9,446 |
| Distinct file edges | 126 | 126 |
| Median resolution time, debug build | 32382.007 ms | 227.087 ms |

Measured speedup: **142.6×**, in the debug profile on this WSL2 machine. This is an isolated resolver benchmark, not an end-to-end CLI or release-build speedup claim. Background compilation/test activity adds timing noise; the raw ranges were 29461.911–43239.439 ms before and 192.155–249.321 ms after. Before/after graphs were exactly equal. Every graph source is under `build-logic/`; these edges exercise populated conventional layouts, while most Ktor multiplatform imports remain unresolved.

Reproduce the current measurement:

```bash
git clone --depth 1 https://github.com/ktorio/ktor.git /tmp/barad-dur-ktor
git -C /tmp/barad-dur-ktor fetch --depth 1 origin 88c0026f6fee2af55d96a20620f2e700b912b334
git -C /tmp/barad-dur-ktor checkout --detach 88c0026f6fee2af55d96a20620f2e700b912b334
KOTLIN_BENCH_REPO=/tmp/barad-dur-ktor \
KOTLIN_BENCH_REPORT=/tmp/barad-dur-kotlin-after.json \
RUSTFLAGS='-D warnings' \
cargo test --locked benchmark_kotlin_import_resolution -- --ignored --nocapture
```

The manual benchmark intentionally has no wall-clock assertion: scheduler noise must not fail CI. Regression tests protect populated targets, overlapping roots, direct membership, `.kts` support, excluded/non-Kotlin files, self-edge exclusion, and single-target ambiguity.

## P1 invariant sweep

| Rule | Enumerated production paths | Evidence |
|---|---|---|
| Extract qualified path and optional wildcard, ignore alias | `lang_dispatch::import_query` → `KOTLIN_IMPORTS` → `treesitter::imports_from_tree`; called by both `extract_file_imports` and `analyse_source` | `.kt`/`.kts`, named/aliased/wildcard, comments/spacing tests; shared-parse-to-graph test |
| Build index once per resolution batch | `resolve_imports`, called from full snapshot assembly and `collect_worktree_details`; `resolve_against_files` | Both construction sites use `index_import_files` outside per-specifier mapping |
| Every single-target caller uses the same index | `resolve_reexports`, `resolve_call_records`, `resolve_class_records` → `resolve_specifier` | All three receive the batch index through `resolve_against_files` |
| Wildcards visit direct Kotlin directory members only | `resolve_import_targets` → `resolve_kotlin_wildcard` | No repository traversal inside wildcard resolution; index contains only known `.kt`/`.kts` files |
| Sorted unique targets; no self edges | `resolve_kotlin_wildcard`, `resolve_imports` | `BTreeSet` materialization; explicit source exclusion; overlapping-root and repeated-import fixture |
| Ambiguity stays unresolved | `resolve_single_import` → all three single-target consumers | Existing one-target and multi-target tests now exercise the index |
| Old collected facts and scores cannot survive an upgrade | `cache::storage::{save,load}`; `cache::history::load_history_checked` used by analyze/backfill | Snapshot version 7→8; history version 5→6; generated report fixture changes only that history version |

## Validation status

- RED: Kotlin extraction returned `["Status"]` instead of the three imported paths.
- Focused Kotlin tests passed after extraction and index changes.
- Baseline full suite: one sandbox-only failure binding a local test server; that test passed on the escalated retry.
- `cargo fmt --all -- --check` and `git diff --check` passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` passed.
- `RUSTFLAGS='-D warnings' cargo test --locked --all-features --no-fail-fast`: exit 0; 1,641 passed, zero failed, eight ignored across 36 test targets. One ignored test is the explicitly run Ktor benchmark.
- `make field-test`: exit 0; `field test clean across 11 repositories`. All 22 passes matched their committed decision surfaces and each other. The initial sandbox attempt could not create corpus worktrees; the escalated run completed.
- `make field-audit`: exit 0; five existing recommendations inspected, no Safe failures. Recurring inline-test clustering actionability failures remain explicitly recorded in the [completed worksheet](../../field-test/audit/2026-09-12-kotlin-wildcard-resolution.md). No new, changed, or withdrawn recommendations.

## Limits and dispositions

- Ktor's `*/common/src`, platform, and test roots are not inferred. The backlog records module/source-set visibility as remaining work. Guessing global package fan-out would create unsupported edges; this change preserves root-selection semantics.
- Existing wildcard semantics represent package membership, not proof of symbol use. This optimization preserves them exactly.
- Entity-history records contain temporal co-change and file complexity/churn, not static import edges, so their schema/fingerprint does not change.
- Scope and cache/history invalidation were reviewed; no corpus baseline is accepted implicitly.
