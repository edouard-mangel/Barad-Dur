# Calculation boundaries and time — implementation evidence

Date: 2026-09-10
Implementation base: `47b6c26` (includes the pre-existing CI shard change).

## Scope and method

This is the P0 inventory before definitions or clocks move. Production definitions were traced to their direct production call sites; test-only `Utc::now()` fixtures were excluded from the production clock list. Searches were then checked against the owning functions, rather than treating grep output as behavioral evidence.

## Metric-to-scorer dependencies

| Producer / consumer | Imported item | Classification | Current owner and meaning |
|---|---|---|---|
| `scorer::build_report` | `metrics::CategoryResult` | calculation result | Metrics owns category output; scorer aggregates it and stores it in `AnalysisReport`. |
| `scorer::build_report` | `metrics::coupling::CouplingReach` | calculation result | Coupling owns the half-over-half per-file partner counts; scorer only annotates hotspots. |
| `scorer::build_report` | `metrics::coupling::pressman_finding_counts` | calculation result | Coupling detection owns the four finding counts. |
| `scorer::build_report` | `metrics::callgraph::call_graph_report` | calculation result | Call-graph calculation owns resolution accounting and hub ranking. |
| `scorer::build_report` | `metrics::churn::churn_timeline_report` | calculation result | Churn calculation owns day buckets and merge exclusion. |
| `scorer::actions` | `CategoryResult`, `RawValue` | calculation result | Score/action selection reads metric values. |
| `scorer::actions` | `coupling::{all_coupling_findings, corroboration_degree, detection_ran}` | calculation result | Coupling owns detection evidence and corroboration; actions turn it into display recommendations. |
| `scorer::builders::hotspots` | `coupling::all_coupling_findings`, `CouplingReach` | calculation result | Findings and trend counts enrich hotspot display rows. |
| `scorer::builders::hotspots` | `file_role::classify` | calculation result | File-role policy classifies a path; the resulting role is serialized. |
| `scorer::builders::coupling` | `coupling::extract_component` | score/policy helper | Configured component depth defines cross-boundary status. |
| `scorer::builders::coupling` | `file_role::is_test_pair` | score/policy helper | File-role policy marks production/test pairs. |
| `scorer::types::{HotspotFile}` / report contract | `metrics::file_role::FileRole` | display-only model dependency | The report DTO exposes the metric-owned enum in its public/serialized shape. |
| `metrics::callgraph` | `scorer::{CallGraphReport, FunctionHub}` | display-only model | Metric code constructs report DTOs, creating the reverse metrics-to-scorer edge. |
| `metrics::churn` | `scorer::{ChurnBucket, ChurnTimelineReport}` | display-only model | Metric code constructs report DTOs, also creating the reverse edge. |
| `metrics::coupling` | `scorer::CouplingFindingCounts` | display-only model | Coupling calculation constructs a report DTO, the third reverse edge. |
| `metrics::coupling` | `scorer::SCORE_GOOD_MIN` | score policy | `SEVERITY_CAP` is `SCORE_GOOD_MIN - 1`, keeping a category with critical/major Pressman evidence below the inclusive good band. This is a reverse policy edge in addition to the DTO edge. |

The category calculation entry points (`health`, `team`, `evolution`, `hygiene`, `coupling`) are invoked by command/backfill code and passed into scorer; scorer does not own their calculation. `callgraph`, `churn`, and Pressman counts are exceptional because scorer invokes them internally while those metric modules import scorer DTOs.

## Production clocks and roles

Wall-clock reads:

| Location | Role | Meaning |
|---|---|---|
| `runner::build_time_window_from_config` and `TimeWindow::default` | evidence filtering | Anchor relative `since`, implicit `until`, and the default inclusive 180-day window. Explicit date specs are midnight UTC. |
| `collector::libgit::git_time_to_chrono` fallback | evidence acquisition | Invalid git timestamps synthesize current-UTC evidence that then feeds filtering and calculations. This malformed-evidence fallback remains outside M06 and is preserved unchanged. |
| `snapshot::RepoSnapshot::new`, `collector::snapshot_builder` (normal and pinned assembly) | acquisition metadata | Stamp snapshot creation. The acquisition timestamp also serves as the reference for missing-history fallbacks. |
| `metrics::evolution::code_age` | calculation reference | Compute signed median blame age at evaluation time. |
| `scorer::builders::files`, `scorer::builders::authors`, `scorer::audit::build_dead_files` | calculation reference | Compute file age, contributor inactivity, and dead-file age. |
| `contributors::resolve_time_window` | evidence filtering | Anchor an optional relative `--since` and its inclusive `until`; absent or invalid `since` deliberately selects full history. |
| `coupling::collector` | evidence filtering | Anchor its rolling 90-day temporal-coupling window. |
| `scorer::build_history_entry` | history-point time | Timestamp persisted score/history samples. Backfill overwrites the initially generated timestamp with the sampled commit time, so sampled history remains historical. The apparent reads in `cache/history.rs` and `trend.rs` are test fixture helpers, not production clocks. |
| `registry::fetch_dep_network` | acquisition metadata | Stamp dependency registry data when fetched. |
| `registry::cache::is_fresh` | cache/acquisition policy | Compare `now - cached_at < 7 days`; future cache timestamps are fresh because the signed duration is negative. This is distinct from repository-window evidence filtering. |
| `cmd::analyze` and `collector::snapshot_builder` `Instant::now()` | operational timing | Monotonic elapsed-time/progress measurement only; no report calculation meaning. |

The field-test entry point deliberately contributes no wall clock: `field_test::runner` always invokes analysis with fixed `--since 2026-03-01`, pinned commits, fresh worktrees, and `--no-cache`. However, the analyzed binary still supplies its ordinary implicit `until = now` and generation/acquisition timestamps; decision-surface extraction is intended to omit volatile metadata. Watch installs a post-commit hook that launches a fresh analyze process, so each run obtains fresh clocks; there is no long-lived iteration reference clock.

## Signed duration, rounding, thresholds, and missing data

- Time-window bounds are inclusive: reject only `timestamp < since` or `timestamp > until`. Consequently future commits are excluded under the normal implicit `until = now`, but included by full history or a future explicit `until`.
- Relative time specs use exact integer calendar approximations: day = 1 day, month = 30 days, year = 365 days. Signed input is accepted; a negative quantity produces a future boundary. Invalid specs return `None` after a warning; if neither bound parses, `TimeWindow::default()` is used, while one valid custom bound causes the other bound to become `now`.
- Chrono `num_days()` truncates the signed duration to whole days toward zero. `code_age` selects the upper weighted median: `mid = total_lines / 2`, then the first age whose cumulative line weight is strictly greater than `mid`. It preserves negative values, divides by `30.0`, displays one decimal through 12 months and zero decimals above 12, and scores with strict `> 3`, `> 12`, `> 24` month boundaries. Thus future blame can yield negative age and receives the “very new” score (70).
- File and author ages use `(now - timestamp).num_days().max(0)`, so future evidence is reported as zero days old. File history absence falls back to `snapshot.created_at - 5 * 365 days`; author history absence falls back to `snapshot.created_at`.
- Dead files do not clamp signed age. They require churn `<= 1`, a resolvable latest commit, and `days > 180` (implemented by excluding `days <= 180`); future evidence is therefore not dead. Files absent from `commits_by_file`, empty commit-id lists, or unresolved ids are omitted.
- Commit cadence filters through the inclusive window, groups by UTC day, and uses population CV. Empty windows are unscored (`score: None`, `N/A`). Active span is `((last-first).num_days()+1).max(1)`; same-day and sub-day ranges therefore use one day. CV thresholds are strict `< 0.5` and `< 1.0`.
- Coupling reach filters to inclusive-window, non-merge commits. Its midpoint is `min + (max-min)/2`; timestamps exactly at midpoint enter the second half (`>=`). A half with no pair-forming evidence makes the result empty. A flag needs first-half partners `>= 1`, second-half partners `>= configured minimum`, and second `>= first * 2.0`. Oversized changesets are skipped.
- Call-graph hubs are hidden only when `resolution_rate < call_resolution_floor`; equality is trusted. No detection or no records returns `None`, distinct from a measured report with an empty hub list.
- Churn returns `None` when no non-merge commit touches a known file. It buckets UTC dates, zero-fills inclusively, and caps output to the final 365 active calendar days. Add/delete totals remain unsigned, while coupling-pair growth is signed `additions - deletions` and excludes merges.
- Overall category score uses floating weighted mean and `.round()` to `u32`; absence of any scored positive-weight category returns `None`. Action candidates use metric score `< 80`.
- Dependency drift uses signed seconds clamped at zero, then divides by 365.25 days/year. Missing either publication date omits that dependency age; network/parse failures are skipped.

## Public paths and compatibility attributes before moves

`scorer.rs` privately declares `types` and publicly glob-re-exports it, so every public item below is reachable as `barad_dur::scorer::<Type>`; `barad_dur::scorer::types::<Type>` is not public. Relevant calculation-output DTOs are `CallGraphReport`, `FunctionHub`, `ChurnTimelineReport`, `ChurnBucket`, and `CouplingFindingCounts`. Other public DTOs in the same module are `EntityTrendDirection`, `HotspotFile`, `CouplingTrend`, `CouplingPair`, `AuthorShare`, `FileOwnership`, `FileAge`, `AuthorCard`, `CrisisFile`, `DirConcentration`, `DeadFile`, `VelocityBucket`, `AuditReport`, `FileCouplingMetrics`, `ImportEdge`, `RemoteMeta`, `ActionItem`, `AnalysisReport`, `LongMethodThresholds`, `HistoryCounts`, `HistoryEntry`, `ScoreBand`, and `ScoreThresholds`. Public policy paths are `barad_dur::scorer::{SCORE_GOOD_MIN, SCORE_WARN_MIN, score_band}`. `metrics::coupling::CouplingReach` is a public type alias at `barad_dur::metrics::coupling::CouplingReach`.

All five calculation-output DTOs derive `Debug`, `Clone`, `PartialEq`, `Serialize`, plus conditional `ts_rs::TS` under `export-types`; `CouplingFindingCounts` additionally derives `Copy`. None derives `Deserialize`, has `#[non_exhaustive]`, or has a type-level serde rename. Their fields are serialized verbatim. On `AnalysisReport`, `coupling_finding_counts`, `call_graph`, and `churn_timeline` use `skip_serializing_if = "Option::is_none"` and conditional `ts(optional)`; preserving this distinguishes unavailable data from measured zero/empty results. Moving a DTO must preserve both its `scorer::<Type>` compatibility path (via re-export if necessary) and these derives/attributes so JSON and generated TypeScript remain stable.

Additional compatibility-sensitive attributes in the containing model: `EntityTrendDirection` uses lowercase serde names; several optional trend/action fields skip `None` (actions and report optionals also use `ts(optional)`); `HistoryCounts` optional coupling counts use `serde(default, skip_serializing_if)` plus `ts(optional)`; `HistoryEntry` retains aliases/renames (`head`/`commit`, `category_scores`/`categories`), defaults for evolved fields, and optional `source`; `HotspotFile`, `CouplingPair`, `AnalysisReport`, and `LongMethodThresholds` are `#[non_exhaustive]`.

The neutral policy surface is deliberately different from the DTOs: `ScoreBand` derives only `Debug`, `Clone`, `Copy`, `PartialEq`, and `Eq` (no serde or TypeScript generation), while `ScoreThresholds` derives `Debug`, `Clone`, and `Serialize` plus conditional `ts_rs::TS`. `score_band` uses inclusive `>= 71` for Good and `>= 41` for Warn; `ScoreThresholds::default` serializes those two public constants as `good_min` and `warn_min`.

## P0 conclusions

1. The boundary to break includes the three metric-to-scorer DTO imports and coupling's reverse dependency on scorer score policy; it does not include legitimate scorer consumption of metric calculations.
2. A single injected analysis reference time can govern filtering and calculation, but acquisition timestamps, history-point timestamps, registry-cache time, monotonic operational timing, and pinned field-test boundaries have distinct owners and must not be conflated.
3. Current future-date behavior is intentionally/non-uniformly observable: normal window filtering excludes it, code age preserves negative values, file/author display clamps to zero, dead-file classification naturally excludes it, dependency drift clamps to zero, and cache freshness accepts it.
4. `None` is semantic in category scores and the three optional report sections. Refactoring must preserve it rather than manufacturing zero/empty measured results.
5. Clock-taking function signatures may change and their callers may be updated mechanically; compatibility is required for public type paths and serialized/generated contracts, not for retaining hidden-clock convenience wrappers. The default boundary must still represent the same inclusive 180-day filtering window.

## Type ownership verification

The five calculation outputs now belong to `metrics::callgraph`, `metrics::churn`, and `metrics::coupling`. Band constants, classification, and serialized thresholds belong to `scoring`. Original `scorer::*` type and policy paths are public re-exports. Display structures remain in scorer.

At `df29c25`, focused tests passed: scoring (2), public-path compatibility (1), callgraph (13), churn (8), coupling (102), current serialized fixture (1), and declaration generation (4). `cargo run --features export-types --example export_report_types -- --check` confirmed exact agreement with committed declarations. Formatting and whitespace checks passed. The independent task review checked fields, derives, serde/TypeScript annotations, threshold values, and import ownership against the prior definitions. It identified only two stale source-location comments, to be corrected in the next slice.

`pnpm -C dashboard check` passed 38 tests across 8 files and the TypeScript/Vite production build. No frontend files or generated declarations changed.

## Baseline verification

Before source edits, `cargo test --no-fail-fast` ran all targets. All integration targets passed. The library reported 1477 passed, 7 ignored, and one failure: the sandbox denied the local TCP listener used by `registry::client::tests::client_times_out_on_unresponsive_server`. Re-running that exact test with the required permission passed (1 test, 0.31 seconds). No production fix or test relaxation was needed.

## Time semantics retained by this refactor

The analysis reference is invocation time, including cached snapshots and date-filtered analyses. Snapshot acquisition timestamps remain metadata and preserve their existing missing-history fallback roles. Backfill age scoring uses the backfill invocation reference; history points still belong at their selected commit timestamps. Watch launches a new analyze process after each commit, so each run captures a fresh reference. Scoring historical code as of the selected commit would be a separate behavioral change.

Capturing before collection can change values at whole-day thresholds compared with the former independent later clock reads. For example, a newly collected snapshot can have `created_at` just after the invocation reference; the unchanged missing-file fallback of `created_at - 1825 days` then truncates to 1824 elapsed days. This is the consequence of the chosen single-invocation reference and preserved fallback, not a new age formula or use of acquisition time as the calculation clock. Fixed-reference tests document the retained rounding behavior.

The pinned corpus keeps its existing commit pins and `--since 2026-03-01` lower boundary. That bounds evidence consistently, but does not freeze wall-clock-based code age across different invocations. No baseline acceptance or history/cache schema change is part of this work.

## Final automated checks

- `RUSTFLAGS='-D warnings' cargo test --all-features --no-fail-fast`: exit 0. Library: 1483 passed, 7 ignored; all integration, feature-gated binary, and documentation targets passed. This includes the current serialized fixture and report declaration-generation tests.
- `cargo fmt -- --check && cargo clippy --all-targets --all-features -- -D warnings`: exit 0, no issues.
- `target/release/barad-dur analyze . --html -o /tmp/barad-dur-calculation-time-smoke.html && node scripts/report-smoke.mjs /tmp/barad-dur-calculation-time-smoke.html`: exit 0; all 11 tabs rendered without JavaScript errors.
- Full-branch source review of `47b6c26..3b33f14`: no required fixes. Individual ownership and time-plumbing reviews also approved; both stale threshold-source comments were corrected.

`make field-test` exited 0: **field test clean across 11 repositories**, with two passes per repository and no regression, nondeterminism, or baseline acceptance. `make field-audit` exited 0 and emitted five unchanged rotation recommendations. The [completed worksheet](../../field-test/audit/2026-09-10-calculation-boundaries-time.md) records no Safe failures and explicitly tracks the recurring inline-test responsibility-clustering Actionable defect, including an additional affected `analyze.rs` row identified by source inspection.

## Explicit-time implementation checks

At `3b33f14`, focused tests passed: 4 fixed-reference tests, 3 `calculation_time_milestone_1` integration tests, 18 evolution tests, and 125 scorer tests. These checks exercise whole-day and score/format thresholds, UTC midnight, future timestamps, unavailable evidence, acquisition-based fallbacks, repeated report output, real fresh/cache-only resolution, a later reference advancing age, and real backfill history timestamps. They are implementation evidence, not substitutes for the full final suite.

## P1 invariant sweep

| Invariant | Consumers inspected | Evidence |
|---|---|---|
| Metrics depend on their own calculation types and neutral score policy | `metrics::callgraph`, `metrics::churn`, `metrics::coupling`; scorer types, report contract exporter, gate, CLI formatting | No `scorer` references remain in metrics. Old scorer paths remain explicit compatibility re-exports and are compile-tested. |
| Calculations receive their reference; they do not read the clock | `evolution::compute_evolution` → `code_age`; `scorer::build_report` → file ages, author cards, audit → dead files | Production clock reads removed from these functions; signed/clamped arithmetic and thresholds unchanged. |
| One invocation supplies filtering, metrics, report details and current history | `cmd::analyze::run_analyze` → runner window, selected metrics, report, history; `cmd::gate::run_gate` → default window, evolution, report, trend gate | Each command captures once before collection. `TimeWindow::at` preserves the inclusive 180-day default; custom date windows still use existing parsing. |
| Cache acquisition time is not reused as the scoring clock | `runner::resolve_snapshot` cache-hit and collection branches; analyze/gate callers; `calculation_time_milestone_1` | Same explicit reference produces equal full reports from actual fresh/cache-only snapshots; advancing it changes ages while keeping the cached snapshot. Existing `created_at` fallback semantics are retained. |
| Backfill scoring reference differs from history-point placement | `backfill::run` sample loop; `scorer::build_history_entry`; score and entity-history append paths | One reference captured before the loop feeds all calculations. Selected commit timestamp feeds both history representations; absent-commit fallback is explicit invocation time. Integration test reads persisted history dates. |
| Watch obtains fresh invocation time | `cmd::watch::install_hook` script → new `barad-dur analyze` process | No long-lived analysis loop or process-start scoring reference exists. |
| All changed signatures are supplied by their consumers | Production paths above; scorer/audit tests; Pressman milestones 2 and 4; new time integration suite | Focused compilation covered all targets. Full all-feature execution is recorded separately. |
| Wire shape, persisted versions and corpus policy remain stable | `scorer::types`, `report_contract`, snapshot/cache/history definitions, field-test driver/runner, generated declarations | No field, cache-version, history-schema, committed fixture, generated declaration, corpus pin or baseline change. |

Legitimate wall clocks remain in invocation boundaries; `TimeWindow::default` compatibility construction; snapshot acquisition; malformed Git timestamp fallback; independent contributors/coupling window construction; registry acquisition/freshness; and test fixtures. Monotonic timing reads remain diagnostic only. The malformed Git timestamp fallback is synthesized evidence, not a scoring-clock service; changing it is outside this refactor.

## Completion and scope

M06's five tasks are implemented and verified. The next roadmap item is M02 (shared analysis orchestration). This branch preserves its original base and is not merged or published by this work.

The plan-execution workflow supplied the isolated branch, task reviews, final source review, and evidence ledger. Implementation decisions were to treat the new user request as authorization beyond the former planning-only phase; preserve acquisition-based fallbacks; combine signature changes with their callers in one coherent time slice; and retain a narrow argument-count lint allowance on the existing report builder until orchestration is addressed separately. No calculation or historical scoring policy was changed to simplify these choices.
