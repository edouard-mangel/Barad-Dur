# Analysis orchestration — implementation evidence

Plan: [M02](../superpowers/plans/2026-09-07-analysis-orchestration.md). Implementation base: `9c4a273` (main after M06). Branch `refactor/analysis-orchestration`, commits `c0bd19f` (plan docs), `2916097` (task 1), `9ff83ac` (task 2), `c2cca2f` (task 3), `67325d4` (task 4), plus this record.

## Task 1 — characterization

`tests/analysis_orchestration_walking_skeleton.rs`: 29 tests driving `analyze`, `gate`, and `backfill` through the binary, the JSON report, `trends.json`, stdout and exit codes. They pin category selection and order per command, the weighted-overall formula for default and configured weights, unavailable-versus-zero evidence (solo team, `skip_blame`, window without commits, no HEAD, no detectable language), history fields per command, and every gate check alone and combined.

Five initial expectations were wrong and were corrected from the code, not from the output: default weights are 35/10/20/15/20; Git Hygiene scores from its window-free gitignore metric when a window has no commits; Team is size-gated (4+ authors) before any blame check; `skip_blame` lives under `[analysis]`; `append_if_new_head` appends on every run.

Three pre-existing behaviors surfaced by the characterization and pinned as current behavior, each with the disposition the [minors policy](../review-process.md#minors-policy-and-escape-accounting) requires:

| Behavior | Disposition | Rationale |
|---|---|---|
| No `--coupling` filter exists: Coupling runs only on an unfiltered `analyze` | **Retired** | A feature gap, not a defect: every unfiltered run scores Coupling, `CategorySelection::from_filters` documents the rule, and adding a flag is outside a behavior-preserving refactor. Reopen as a feature request if a filtered Coupling run is ever wanted. |
| A category-filtered `analyze` appends a partial history entry (only the selected categories, overall renormalized over them) | **Tracker issue** | A product defect: the trend chart mixes partial and full entries on one head. Not fixed here (M02 preserves behavior); the characterization test pins it so the fix changes a test on purpose. |
| A window with zero commits scores 100 overall (Git Hygiene's window-free gitignore metric is the only scored one) | **Tracker issue** | Same class as the "unscored category scores 100" True failure already named in `docs/review-process.md` (P2 audit section). Second appearance of the class: escalated per the recurrence rule. |

## Task 2 — shared selection and weighting

`src/analysis/{mod,selection,result}.rs`: `calculate(&AnalysisInputs) -> AnalysisResult`. Inputs are the snapshot, an explicit `CategorySelection` (`GATE`, `BACKFILL`, or `from_filters`), thresholds, effective weights, already-acquired dependency evidence, the reference time, and the two command-shared derivations. `compute_overall_score_with_weights` moved here; `scorer` re-exports it. `compute_selected_metrics` and its `AnalyzeArgs` dependency are gone. Six milestone-1 tests; a grep sweep of `src/analysis/` for `cli`, `clap`, `std::fs`, `std::io`, `Utc::now`, network clients and `println` is empty.

One ordering change: `analyze` loads dependency evidence before the calculation, since it is now an input. Under `-v` the dependency progress precedes the metrics timing line; stdout and scores are unchanged.

## Task 3 — shared coupling evidence

`src/metrics/coupling/evidence.rs`: `CouplingEvidence::derive` gathers detection flags, the enabled finding set (AST, gated barrel bypass, inheritance depth; source files only) and corroboration once per snapshot and threshold set. Before, one `analyze` run derived the finding set four times (metric, counts, hotspots, actions) and corroboration twice, and `gate` twice more per snapshot; `all_coupling_findings` is now called from `derive` only. Reach was already computed once and shared, so it stayed an input. `finding_counts()` is `None` without detection; a graph-derived barrel finding is still listed for the ratchet. Three unit tests (toggle on/off, no detection, no detectable language) and two milestone-2 tests. `pressman_finding_counts` and `ratchet_finding_sets` were removed with their tests migrated.

## Task 4 — report and history from the result

`build_report` takes an `AnalysisResult` and no longer recomputes the overall score; `build_history_entry` takes the result and the snapshot. `gate` decides from the result, its evidence and the snapshot branch and builds no display section; `backfill` builds only the hotspot rows its entity-trend samples need. `analyze` constructs its history record before the report takes ownership of the categories. Three milestone-3 tests, including that a Team-filtered report carries no `[Health]` advice. The `too_many_arguments` allowance retained by M06 on `build_report` is removed.

## Final automated checks

- `RUSTFLAGS='-D warnings' cargo test`: 1671 passed, 7 ignored, 38 suites (baseline at `9c4a273`: 1630 passed, 34 suites).
- `cargo fmt -- --check`; `cargo clippy --all-targets -- -D warnings`; `cargo clippy --all-targets --features export-types -- -D warnings`: clean.
- `cargo run --features export-types --example export_report_types -- --check`: generated declarations unchanged. `cargo test --features export-types --examples` and the current-fixture serialization test: pass.
- `make report-smoke`: 11 tabs rendered without JavaScript errors.
- `make field-test`: clean across 11 repositories, two passes each, no baseline accepted. `make field-audit`: no Safe failures; [worksheet](../../field-test/audit/2026-09-12-analysis-orchestration.md).

Performance was not measured; the reduced derivation counts are structural evidence, not a speed claim.

## P1 invariant sweep

| Invariant | Consumers inspected | Evidence |
|---|---|---|
| The calculation layer reads no CLI arguments, clock, or I/O | `src/analysis/*`, `src/metrics/coupling/evidence.rs` | Import sweep empty; inputs carry the reference time and acquired evidence. |
| One selection and weighting policy; each command keeps its set | `cmd::analyze`, `cmd::gate`, `backfill::run`, `calculation_time_milestone_1`, Pressman milestones 2 and 4 | All build selections at their boundary; no remaining hand-assembled category vectors in production. |
| Coupling facts are derived once per snapshot and configuration | `compute_coupling_with_evidence`, `build_report`, `build_hotspots`, `generate_coupling_actions`, gate ratchet | Only `CouplingEvidence::derive` calls `all_coupling_findings`/`corroboration_degree` in production. |
| Unavailable evidence stays distinct from zero | `finding_counts`, `HistoryCounts` optional fields, characterization tests for no-language and no-detection repositories | Counts absent, not zero, in report and history. |
| History and gate need no display report | `build_history_entry`, `check_gate_categories`, `check_trend_gate_against_history`, `backfill::run` | `build_report` has one production caller (`analyze`); gate and backfill import no report sections. |
| Wire shape, persisted schema and corpus policy are stable | `scorer::types`, `report_contract`, history schema, generated declarations, baselines | No field, schema-version, declaration, pin or baseline change; contract `--check` exact. |
| Advice does not widen on filtered reports | `build_report` Health gate, milestone-3 test | Coupling actions on filtered reports are pre-existing behavior, preserved, not added. |

## Review of MR !145 (2026-09-12, head `86d65f5`)

MR pipeline 129997 was red: the characterization test for a repository without parseable sources committed with no author identity, which the CI runner (no `~/.gitconfig`) refuses; nextest fail-fast cancelled 161 tests and the mutation gate was skipped. Reproduced locally by running the suite with `HOME` pointed at an empty directory: exactly one failure across 38 suites. Fixed by giving the fixture an identity and running the suite's git helper with `GIT_CONFIG_GLOBAL=/dev/null` and `GIT_CONFIG_NOSYSTEM=1`, so the same omission fails on every machine.

Also fixed in the review commits: the `CLAUDE.md` metric recipe and the comments that still named `pressman_finding_counts` and `ratchet_finding_sets` (generated TypeScript declarations regenerated, `--check` exact); the dead `AnalyzeArgs::should_run`/`all_categories` helpers.

Verified without change: `CategorySelection::from_filters` reproduces the old `should_run` rule; the history entry built from the result equals the one built from the report (branch and totals already came from the snapshot); `all_coupling_findings` already restricted findings to source files, so `pressman_metric`'s kind-only filter over the evidence is equivalent; `src/analysis/` imports no CLI, clock, or I/O; `all_coupling_findings`/`corroboration_degree` are called from `CouplingEvidence::derive` only. Four by-hand mutants (Coupling selected under a filter; counts without a detectable language; Dependencies without evidence; ratchet head set replaced by the base set) were each killed by a named test.

Tracked, not fixed here: `AnalysisInputs` lets `selection.deps` and `weights` disagree, with the pre-existing 0.25 fallback weight hiding the mismatch (only `analyze` performs the `deps = 20` opt-in); and the public API breaks of this branch (`build_report`, `build_history_entry`, `compute_trend_and_update_history` re-signatured, `compute_selected_metrics` removed) join the two already pending against v0.22.0, to be settled at the next version bump, which `scripts/version-bump.sh` would otherwise derive as a patch. Retired with rationale: `build_report` re-taking thresholds, god objects and reach beside the result (every command passes the same values; carrying them on the result belongs with M03), the ratchet's base evidence deriving an unused corroboration map (Metrics stage measures 185 ms on this repository), backfill computing categories when only the entity entry is needed (inherited), `build_hotspots` widened to `pub(crate)` (M04 territory), and test plumbing repeated across call sites.

## Completion and scope

M02's five tasks are implemented and verified on this branch, which is not merged or published by this work. Task 3's reach consolidation was declined on evidence (not duplicated). Next in the recommended order: M03 (snapshot assembly), then M04 (stable identities), then M05 (HTML modules).
