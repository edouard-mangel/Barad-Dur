# P2 decision-surface audit — test-context responsibility filtering

Corpus: all 11 repositories in `field-test/corpus.toml`, inspected at their
pins. The current `field-test/archive/*-0.json` reports were compared with
`field-test/baselines/*.surface.json`; baselines were not modified. The two
completed passes were deterministic. Only responsibility suggestions changed,
and only in the three Rust repositories below. Mautic and the seven other
non-Rust repositories had no responsibility-action changes.

For each changed row, function names were independently extracted from the
pinned source with the current analyzer and checked against `git show
<pin>:<path>`. `false` test-context flags were required before treating a name
as production evidence.

## barad-dur (`dc23fbd6`)

| Change | True? | Safe? | Actionable? | Pinned evidence |
|---|---|---|---|---|
| `src/renderer/cli/mod.rs`: `render_*` 22 -> 8 | yes | yes | yes | The remaining names are `render_repo_info`, `render_trend_line`, `render_score_and_trend`, `render_single_category`, `render_dep_section`, `render_top_unstable_files`, `render_categories`, and `render_actions_and_footer`. They are the production report sections called by `render`; fourteen `render_*` unit tests are gone. Extracting section renderers is a concrete split. |
| `src/scorer/audit.rs`: `build_*` (5), unchanged text but retained after reranking | yes | yes | yes | `build_audit_report` composes `build_crisis_files`, `build_dir_concentration`, `build_dead_files`, and `build_velocity_buckets`. All five are production functions and form the audit-building pipeline; the four facets can become focused modules. |
| `src/snapshot/mod.rs`: new `build_*` (4) | yes | yes | yes | `build_indexes` orchestrates `build_commits_by_author`, `build_commits_by_file`, and `build_file_change_pairs`. All are production functions over derived snapshot indexes. Moving index construction behind a focused component is concrete. |
| `src/metrics/coupling/mod.rs`: new `has_*` (3) | yes | yes | **no** | The names are `has_edge`, `has_import_extractable_files`, and `has_detectable_files`. The first is graph adjacency; the other two determine metric availability. They share only a generic predicate prefix, so `has_*` does not identify one extractable responsibility. This is a newly surfaced Actionable failure, not a Safe failure. |
| `src/cmd/analyze.rs`: `build_*` (6) removed; `compute_*` (2) retained | yes | yes | partial | Five `build_ecosystem_reports_*` tests were removed, leaving the production `build_ecosystem_reports` singleton, which correctly fails the two-member minimum. `compute_trend_and_update_history` and `compute_selected_metrics` are both production orchestration functions, but their common verb alone gives only a weak split boundary. |
| `src/config/mod.rs`: `validate_*` (22) withdrawn | yes | yes | yes | All 22 names are inside the pinned test module (starting near line 332). Removing this false production responsibility is the intended correction. |
| `src/scorer.rs`: `build_*` 14 withdrawn from the top five | yes | yes | yes | Twelve test functions were removed. Only production `build_history_entry` and `build_report` remain; their count of two now ranks below the five displayed candidates. The withdrawal follows the documented pre-ranking filter and cap. |

The explicit acceptance details hold: `validate_*` is gone; renderer is exactly
eight production `render_*` functions; the `build_*` group in `cmd/analyze.rs`
is a singleton and therefore gone; both production `compute_*` functions
remain; and the newly visible audit/snapshot/coupling groups have counts 5/4/3.

## ripgrep (`3fce3b5b`)

| Change | True? | Safe? | Actionable? | Pinned evidence |
|---|---|---|---|---|
| `crates/globset/src/lib.rs`: `set_*` (2) removed, `is_*` (12) retained | yes | yes | partial | `set_works` and `set_does_not_remember` occur after the file's `#[cfg(test)]` boundary. The twelve remaining production predicates cover the public `GlobSet` API, match-strategy dispatch, and individual strategy implementations. They describe matching behavior, but collecting methods solely by `is_*` cuts across intentionally separate strategy types. |
| `crates/matcher/src/lib.rs`: new `is_*` (8) | yes | yes | **no** | The eight production methods are `Match::is_empty`, `LineTerminator::is_crlf`, `LineTerminator::is_suffix`, `Captures::is_empty`, `Matcher::{is_match,is_match_at}`, and the two forwarding implementations on `&M`. They span unrelated types and forwarding methods; the prefix is not a responsibility boundary. This is a newly surfaced Actionable failure. |
| `crates/ignore/src/gitignore.rs`: prior `is_*` (3), `parse_*` (6) row withdrawn | yes | yes | yes | Five `parse_excludes_file*` tests were removed, leaving only production `parse_excludes_file`; the production `is_*` count is three. Its total rank falls below the newly eligible matcher row, so the five-action cap withdraws it. |

The other three displayed ripgrep rows are byte-for-byte unchanged. Their
functions were nevertheless checked for test leakage: `defs.rs` retains 108
production `is_switch` implementations; `walk.rs` retains production groups
3/2/10; and `literal.rs` retains production groups 2/2/7.

## starship (`e939a19a`)

| Change | True? | Safe? | Actionable? | Pinned evidence |
|---|---|---|---|---|
| `src/context/mod.rs`: `set_*` 8 -> 6; other groups unchanged | yes | yes | partial | The six production setters are `set_config`, `ScanDir::set_files`, `set_extensions`, `set_folders`, and the two forwarding `DirContents::set_files/set_folders` methods. Two test setters were removed. All displayed groups are production, though generic `get/has/is/set` verbs span environment, repository, and directory-scanning concerns rather than defining a single extraction. |
| `src/config.rs`: new `get_*` (6), `parse_*` (2) | yes | yes | yes | The getters cover module/custom-module/environment configuration plus `get_palette`; the parsers are `parse_style_string` and `parse_color_string`. All eight precede the file's `#[cfg(test)]` module. Separating configuration lookup from style/color parsing is concrete. |
| `src/utils/mod.rs`: `render_*` (9) withdrawn | yes | yes | yes | The only production function is `render_time`; the eight `render_time_test_*` functions begin after `#[cfg(test)]`. A production singleton correctly yields no group. |

The remaining three displayed starship rows (`aws.rs`, `git_status.rs`, and
`package.rs`) are unchanged.

## Decision

- **Safe failures: none.** No recommendation directs a destructive or
  unbounded change, so this audit does not block the merge.
- **True failures: none.** Every retained count matches production functions
  at the configured pin, and every removed test-inflated group was confirmed.
- **Actionable failures: two newly surfaced rows:** barad-dur
  `metrics/coupling/mod.rs has_*` and ripgrep `matcher/src/lib.rs is_*`. Both
  are truthful counts but generic predicate prefixes across unrelated types or
  concerns. They are tracked by [issue #9](https://lab.frogg.it/Edouard_Mangel/barad-dur/-/work_items/9).
  The #8 contract explicitly preserves production prefix matching, so changing
  that heuristic here would exceed the approved scope. Baseline acceptance
  records the known limitation and its owner rather than silently deferring it.

The temporary function-classification probe was removed after collecting this
evidence.
