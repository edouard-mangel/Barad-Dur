# Snapshot assembly — implementation evidence

Plan: [M03](../superpowers/plans/2026-09-07-snapshot-assembly.md). Implementation base: `fa02644` (main). Branch `refactor/snapshot-assembly`: `eaac7bb` (task 1), `508c213` (task 2), `d63fde4` (task 3), `ba006f7` (task 4), `42aecac` (task 5 test), `c2bd458` (plan documents, cherry-picked from the M02 branch so this branch is self-contained), plus this record.

## Task 1 — characterization

`src/collector/assembly_parity_tests.rs` (13 tests, crate-internal because the historical readers are `pub(crate)`) and `tests/snapshot_assembly_walking_skeleton.rs` (3 tests through the binary). One fixture tree exercises every channel across Rust, TypeScript (barrel with star and named re-exports, `extends` of an imported class, same-file and external calls, two imports of one file), PHP with PSR-4 roots from a nested `composer.json`, Go (an unreliable resolver), non-UTF-8 and binary content, and a default-excluded lockfile.

Pinned: the working-tree and at-commit readers agree on every analysis channel for the same committed content; every resolved channel is emitted sorted; indexes reference only listed files, known commits and known authors; an empty tree, an unresolvable import, and AST-free collection each keep their shape. Pinned as intentional differences: historical collection never blames; manifests and source come from disk live and from the commit historically; the current ignore file filters both; an unreadable working-tree file is skipped live but read from its blob.

Two expectations were corrected from the code, not from the output, and are recorded as behavior: a Rust symbol import (`use crate::util::helper`) resolves no import edge because the resolver maps the whole path to a module file, while the call channel resolves the same symbol to `src/util.rs`; re-exports with equal `(path, target)` keys keep extraction order, so their sort must stay stable. Neither was changed.

Through the binary: `gate --no-new-coupling --baseline-ref HEAD` reports no new findings against its own head (working tree versus blobs), and `analyze` and `backfill` write the same coupling counts for one head.

## Task 2 — named intermediates

`src/collector/source_assembly.rs`: `RawSourceChannels` (collected, unresolved) and `ResolvedSourceChannels` (the snapshot's own shape) replace the six- and seven-element tuples. Both are collector-internal and unserialized; `ResolvedSourceChannels::default()` is the AST-free mode, which `coupling::detection_ran` reads as "not collected".

## Task 3 — shared aggregation and resolution

`RawSourceChannels::aggregate` is the one policy over per-file `SourceAnalysis` results; `resolve` matches raw channels against the file list with the manifest reader as an explicit argument (disk live, blob historically — the historical path has no disk fallback, unit-tested). The live reader keeps its parallel read-and-parse and the historical reader its sequential blob walk; which files are read and parsed is unchanged. The four resolvers and the unreliable-specifier count moved with their five unit tests.

## Task 4 — one construction path

`assemble(provenance, collection, files, blame_map, resolved)` in `snapshot_builder.rs` builds the snapshot and its indexes once; both readers call it through a `SnapshotProvenance` they fill. The two `RepoSnapshot` literals are gone. `created_at` is still stamped at assembly.

## Task 5 — verification

- `RUSTFLAGS='-D warnings' cargo test`: 1651 passed, 7 ignored, 36 suites at `ba006f7`; the warm-cache test adds one. No separate baseline run was made in this worktree; main's CI at `fa02644` is the baseline.
- `cargo fmt -- --check`; `cargo clippy --all-targets -- -D warnings`, with and without `export-types`: clean.
- `cargo run --features export-types --example export_report_types -- --check`: generated declarations unchanged; the current-fixture serialization test passes.
- Cache format: no change under `src/snapshot` or `src/cache`; `CACHE_VERSION` untouched. Cold (`no_cache`) and warm (`cache_only`) resolution agree channel for channel and the hit keeps its acquisition time (`a_warm_cache_replays_the_cold_collection_channel_for_channel`).
- `make report-smoke`: 11 tabs rendered without JavaScript errors (renderer untouched by this branch).
- P2: `make field-test` clean across 11 repositories, two passes each, no baseline accepted; `make field-audit` no Safe failures — [worksheet](../../field-test/audit/2026-09-12-snapshot-assembly.md).

Performance was not measured; the historical reader now owns each blob's text once (`String`) where it previously borrowed it — one allocation per parsed file on the sequential path, not benchmarked.

## P1 invariant sweep

| Invariant | Consumers inspected | Evidence |
|---|---|---|
| One aggregation and one resolution path | `collect_file_metrics_with_progress`, `ast_pass_at`, `assemble_snapshot` | Only `RawSourceChannels::aggregate` builds raw channels; only `resolve` resolves them; no `raw_*` locals or resolver calls remain in `snapshot_builder.rs`. |
| Manifest provenance is the reader's | `resolve` callers | Live passes a disk reader; historical passes the blob reader used for source; `resolve_reads_manifests_through_the_supplied_reader_only`. |
| Which files are read and parsed | both readers | Same `is_binary` filter, same skip on unreadable/non-UTF-8 content; `ast_pass_at_skips_bad_oid_missing_blob_and_non_utf8` retained. |
| Deterministic ordering | `aggregate` (findings), resolvers (records), `build_file_change_pairs` | Sort keys unchanged; `resolved_channels_are_deterministically_ordered_on_both_paths`. |
| Indexes from final core data | `assemble` | Single `build_indexes` after construction; `assemble_builds_indexes_once_from_final_core_data_and_keeps_provenance`. |
| Public collector API and serialized snapshot | `collector/mod.rs`, `snapshot/mod.rs`, `cache/` | No signature, field, or version change; `collect_file_metrics` still returns the metrics map. |
| Backfill and gate policy | `backfill::run`, `cmd::gate` | Both still call `collect_snapshot_at_with_ast`; blame stays empty historically. |

## Completion and scope

M03's five tasks are implemented and verified on this branch, which is not merged or published by this work. Next in the recommended order: M04 (stable identities), then M05 (HTML modules).

## Rebase onto main (2026-09-12, head `cd3522b` on `ba93cc7`)

Main gained M02 (`3d9c3c8`, merged as `efd4053`) and the Kotlin wildcard resolution fix (`d491c29`, merged as `ba93cc7`) after this branch was cut, and MR !146 reported a conflict. Rebased with `git rebase origin/main`; the seven commits became six:

- `c2bd458` (the plan documents cherry-picked from the M02 branch) was **dropped**: every file it added is byte-identical to main's copy except `2026-09-07-analysis-orchestration.md`, where main carries M02's ticked task boxes. Nothing of this branch was in it.
- One content conflict, in `src/collector/snapshot_builder.rs` at task 3 (`share source aggregation and resolution`): the Kotlin fix had changed `resolve_against_files` to index the file set through `index_import_files`/`ImportFileIndex` instead of a `HashSet<&PathBuf>`, and task 3 moves that helper into `src/collector/source_assembly.rs`. Resolution: the branch side in `snapshot_builder.rs` (the helper and its imports leave the file), and the Kotlin change ported verbatim into the helper's new home — signature `&ImportFileIndex<'_>` and `index_import_files(files.iter().map(|f| &f.path))`. The rebased commit is `89f949d`.
- The remaining commits applied clean.

Gates re-run on the rebased head, from the same worktree, corpus root `~/WS` (all 11 entries present at their pins):

- `RUSTFLAGS='-D warnings' cargo test --no-fail-fast`: 1695 passed, 0 failed, 8 ignored, 40 suites (the base grew by M02's and the Kotlin fix's suites).
- `cargo fmt -- --check`; `cargo clippy --all-targets -- -D warnings`, with and without `export-types`: clean.
- `cargo run --features export-types --example export_report_types -- --check`: exact.
- `make report-smoke`: 11 tabs rendered without JavaScript errors.
- P2: `make field-test` clean across 11 repositories, no baseline accepted, worktree clean afterwards. `make field-audit` was not re-run: the corpus pins and the recommendation surface are unchanged by the rebase (the field test compares full decision surfaces), so the worksheet above stands.
