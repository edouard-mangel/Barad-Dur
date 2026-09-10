# Calculation boundaries and time — Task 1 inventory

Date: 2026-09-10
Baseline: `ff64353` (inventory only; the controller owns the baseline test run)

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

The category calculation entry points (`health`, `team`, `evolution`, `hygiene`, `coupling`) are invoked by command/backfill code and passed into scorer; scorer does not own their calculation. `callgraph`, `churn`, and Pressman counts are exceptional because scorer invokes them internally while those metric modules import scorer DTOs.

## Production clocks and roles

Wall-clock reads:

| Location | Role | Meaning |
|---|---|---|
| `runner::build_time_window_from_config` and `TimeWindow::default` | evidence filtering | Anchor relative `since`, implicit `until`, and the default inclusive 180-day window. Explicit date specs are midnight UTC. |
| `collector::libgit::git_time_to_chrono` fallback | evidence acquisition | Invalid git timestamps fall back to current UTC, so malformed evidence can appear current. |
| `snapshot::RepoSnapshot::new`, `collector::snapshot_builder` (normal and pinned assembly) | acquisition metadata | Stamp snapshot creation. This is also the fallback reference stored for authors/files without history. |
| `metrics::evolution::code_age` | calculation reference | Compute signed median blame age at evaluation time. |
| `scorer::builders::files`, `scorer::builders::authors`, `scorer::audit::build_dead_files` | calculation reference | Compute file age, contributor inactivity, and dead-file age. |
| `contributors::build_contributor_report` | calculation reference | Compute contributor recency for the standalone contributor view. |
| `coupling::collector` | evidence filtering | Anchor its rolling 90-day temporal-coupling window. |
| `scorer::build_history_entry`, `cache::history::append` fallback, `trend` history creation | history-point time | Timestamp persisted score/history samples. Backfill overwrites the initially generated history timestamp with the sampled commit time, so sampled history remains historical. |
| `registry::{fetch_dep_network, resolve test-independent fetch path}` | acquisition metadata | Stamp dependency registry data when fetched. |
| `registry::cache::is_fresh` | evidence filtering | Compare `now - cached_at < 7 days`; future cache timestamps are fresh because the signed duration is negative. |
| `cmd::analyze` and `collector::snapshot_builder` `Instant::now()` | operational timing | Monotonic elapsed-time/progress measurement only; no report calculation meaning. |
| `registry::client` `Instant::now()` | operational timing | Request/diagnostic duration only. |

The field-test entry point deliberately contributes no wall clock: `field_test::runner` always invokes analysis with fixed `--since 2026-03-01`, pinned commits, fresh worktrees, and `--no-cache`. However, the analyzed binary still supplies its ordinary implicit `until = now` and generation/acquisition timestamps; decision-surface extraction is intended to omit volatile metadata. Watch installs a post-commit hook that launches a fresh analyze process, so each run obtains fresh clocks; there is no long-lived iteration reference clock.

## Signed duration, rounding, thresholds, and missing data

- Time-window bounds are inclusive: reject only `timestamp < since` or `timestamp > until`. Consequently future commits are excluded under the normal implicit `until = now`, but included by full history or a future explicit `until`.
- Relative time specs use exact integer calendar approximations: day = 1 day, month = 30 days, year = 365 days. Signed input is accepted; a negative quantity produces a future boundary. Invalid specs return `None` after a warning; if neither bound parses, `TimeWindow::default()` is used, while one valid custom bound causes the other bound to become `now`.
- Chrono `num_days()` truncates the signed duration to whole days toward zero. `code_age` preserves negative values, divides by `30.0`, displays one decimal through 12 months and zero decimals above 12, and scores with strict `> 3`, `> 12`, `> 24` month boundaries. Thus future blame can yield negative age and receives the “very new” score (70).
- File and author ages use `(now - timestamp).num_days().max(0)`, so future evidence is reported as zero days old. File history absence falls back to `snapshot.created_at - 5 * 365 days`; author history absence falls back to `snapshot.created_at`.
- Dead files do not clamp signed age. They require churn `<= 1`, a resolvable latest commit, and `days > 180` (implemented by excluding `days <= 180`); future evidence is therefore not dead. Files absent from `commits_by_file`, empty commit-id lists, or unresolved ids are omitted.
- Commit cadence filters through the inclusive window, groups by UTC day, and uses population CV. Empty windows are unscored (`score: None`, `N/A`). Active span is `((last-first).num_days()+1).max(1)`; same-day and sub-day ranges therefore use one day. CV thresholds are strict `< 0.5` and `< 1.0`.
- Coupling reach filters to inclusive-window, non-merge commits. Its midpoint is `min + (max-min)/2`; timestamps exactly at midpoint enter the second half (`>=`). A half with no pair-forming evidence makes the result empty. A flag needs first-half partners `>= 1`, second-half partners `>= configured minimum`, and second `>= first * 2.0`. Oversized changesets are skipped.
- Call-graph hubs are hidden only when `resolution_rate < call_resolution_floor`; equality is trusted. No detection or no records returns `None`, distinct from a measured report with an empty hub list.
- Churn returns `None` when no non-merge commit touches a known file. It buckets UTC dates, zero-fills inclusively, and caps output to the final 365 active calendar days. Add/delete totals remain unsigned, while coupling-pair growth is signed `additions - deletions` and excludes merges.
- Overall category score uses floating weighted mean and `.round()` to `u32`; absence of any scored positive-weight category returns `None`. Action candidates use metric score `< 80`.
- Dependency drift uses signed seconds clamped at zero, then divides by 365.25 days/year. Missing either publication date omits that dependency age; network/parse failures are skipped.

## Public paths and compatibility attributes before moves

`scorer.rs` privately declares `types` and publicly glob-re-exports it, so every public item below is reachable as `barad_dur::scorer::<Type>`; `barad_dur::scorer::types::<Type>` is not public. Relevant calculation-output DTOs are `CallGraphReport`, `FunctionHub`, `ChurnTimelineReport`, `ChurnBucket`, and `CouplingFindingCounts`. Other public DTOs in the same module are `EntityTrendDirection`, `HotspotFile`, `CouplingTrend`, `CouplingPair`, `AuthorShare`, `FileOwnership`, `FileAge`, `AuthorCard`, `CrisisFile`, `DirConcentration`, `DeadFile`, `VelocityBucket`, `AuditReport`, `FileCouplingMetrics`, `ImportEdge`, `RemoteMeta`, `ActionItem`, `AnalysisReport`, `LongMethodThresholds`, `HistoryCounts`, `HistoryEntry`, `ScoreBand`, and `ScoreThresholds`. `metrics::coupling::CouplingReach` is a public type alias at `barad_dur::metrics::coupling::CouplingReach`.

All five calculation-output DTOs derive `Debug`, `Clone`, `PartialEq`, `Serialize`, plus conditional `ts_rs::TS` under `export-types`; `CouplingFindingCounts` additionally derives `Copy`. None derives `Deserialize`, has `#[non_exhaustive]`, or has a type-level serde rename. Their fields are serialized verbatim. On `AnalysisReport`, `coupling_finding_counts`, `call_graph`, and `churn_timeline` use `skip_serializing_if = "Option::is_none"` and conditional `ts(optional)`; preserving this distinguishes unavailable data from measured zero/empty results. Moving a DTO must preserve both its `scorer::<Type>` compatibility path (via re-export if necessary) and these derives/attributes so JSON and generated TypeScript remain stable.

Additional compatibility-sensitive attributes in the containing model: `EntityTrendDirection` uses lowercase serde names; several optional trend/action fields skip `None` (actions and report optionals also use `ts(optional)`); `HistoryCounts` optional coupling counts use `serde(default, skip_serializing_if)` plus `ts(optional)`; `HistoryEntry` retains aliases/renames (`head`/`commit`, `category_scores`/`categories`), defaults for evolved fields, and optional `source`; `HotspotFile`, `CouplingPair`, `AnalysisReport`, and `LongMethodThresholds` are `#[non_exhaustive]`.

## P0 conclusions

1. The boundary to break is specifically the three metric-to-scorer DTO imports, not the legitimate scorer consumption of metric calculations.
2. A single injected analysis reference time can govern filtering and calculation, but acquisition timestamps, history-point timestamps, registry-cache time, monotonic operational timing, and pinned field-test boundaries have distinct owners and must not be conflated.
3. Current future-date behavior is intentionally/non-uniformly observable: normal window filtering excludes it, code age preserves negative values, file/author display clamps to zero, dead-file classification naturally excludes it, dependency drift clamps to zero, and cache freshness accepts it.
4. `None` is semantic in category scores and the three optional report sections. Refactoring must preserve it rather than manufacturing zero/empty measured results.
5. Clock-taking function signatures may change and their callers may be updated mechanically; compatibility is required for public type paths and serialized/generated contracts, not for retaining hidden-clock convenience wrappers. The default boundary must still represent the same inclusive 180-day filtering window.
