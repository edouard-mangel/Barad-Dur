# Fix #9 implementation and verification

Base: `decff37` (completed #8). Branch: `fix/structural-responsibility`.
The user-supplied Fix #9 plan is the binding specification.

## Work ledger

- AST ownership/dependency producer: delegated to structural_detector.
- Grouping/formatting and ranking tests: delegated to grouping.
- Persistence, collection parity, integration, corpus and final review: root.

## Preflight interface review

| Tasks | Shared contract | Decision |
|---|---|---|
| Producer / grouping | Optional owner provenance and typed direct dependencies | None excludes from advice; sorted IDs are file-local |
| Producer / cache | New public FunctionMetrics field | Cache 10; breaking conventional commit and next minor release |
| Grouping / tests | Existing fixtures lack owners | Give advice fixtures explicit provenance; numeric fixtures remain unknown |
| Verification / baseline acceptance | Advice changes only | Audit each change before separate baseline commit |

Ruling: #8 already prepares unreleased 0.23.0 from released 0.22.x; #9 shares
that next minor release, rather than incrementing an unreleased version again.
No release tag or publishing is part of this task.

Correction (2026-09-16, MR !152 review): the version is not bumped in this
branch. `scripts/version-bump.sh` derives the next version from the current
`Cargo.toml` one, so a pre-bumped 0.23.0 would release as 0.24.0. The crate
stays at 0.22.0 and the entries sit under `[Unreleased]`; the minor bump
happens at release time (issue #7).

Correction (2026-09-16, MR !152 review): the cache "header guard before
payload decode" was removed. Deleting it left every cache test green, and a
version-8 header followed by an impossible `commits` length is still rejected
and deleted by the existing version match, because bincode 2 returns an error
instead of allocating. Version 10 remains the invalidation mechanism.

## Verification results

Baseline command: `RUSTFLAGS='-D warnings' CARGO_TARGET_DIR=/home/edouard/WS/barad-dur/target cargo test --locked --all-features --no-fail-fast` in the unchanged #8 worktree.
Result: 1,785 passed, 8 ignored, one sandbox-denied TCP listener. The exact
`registry::client::tests::client_times_out_on_unresponsive_server` test passed
on an escalated retry (0.32 seconds): combined baseline 1,786 passed, 8 ignored.
Logs: `/tmp/structural-baseline-clean.log`, `/tmp/structural-baseline-socket-retry.log`.

Dashboard locked install initially hit sandbox DNS denial; escalated retry
completed with pnpm 12.3.4. No dependency/lockfile changes.

Task review: grouping static diff approved without findings in
[the grouping review](2026-09-15-grouping-review.md). Producer integration and full-suite checks passed.

Implementation verification results are recorded below.


## P1 constructor and collection-path sweep

| Invariant | Enumerated paths | Evidence |
|---|---|---|
| One producer, unchanged extraction | `counters::extract_functions`; sole production `FunctionMetrics` literal | Index lookup adds provenance after existing queries; name/LOC/CC/nesting/test assignment preserved |
| Consistent standalone/shared parse | `treesitter::analyse` → `analyse_tree`; `analyse_source` → same core; `analyse_file`, `analyse_content` | Extraction tests compare nonempty function vectors; no new parser invocation |
| Live/historical identical facts | `snapshot_builder.rs` live line 122, historical line 485 → `analyse_source`; `RawSourceChannels::aggregate` → resolved channels | Added repository-backed live/historical/standalone/cache equality test |
| All literal consumers compile | `scorer.rs`, `scorer/actions.rs`, `metrics/testutil.rs`, `health/long_methods.rs` | Advice fixtures supply explicit owners; metric-only fixtures use `None` |
| Provenance only affects advice | `group_methods_by_prefix` → `generate_refactoring_actions`; scorer Health selection guard | Existing god-object population and score builders untouched; report-contract check passed |
| Old positional snapshots rejected | `cache/storage.rs` header guard before payload decode, version 10 | Added decodable version-9 rejection and field/callee round trip |
| Persisted histories unchanged | `scorer/types.rs::HISTORY_SCHEMA_VERSION = 6`; cache history / report writers | No schema or history producer changes |
| Other metrics ignore provenance | health long methods/god objects/biomarkers/complex hotspots; scorer audit/hotspots; metrics callgraph and coupling | Consumers continue reading existing measurements only |

This sweep enumerates constructors and entry paths; language-rule review is
recorded separately by the detector reviewer. Full tests and corpus outcomes are recorded below; they are separate evidence
from source inspection.


## Review-driven corrections

- Independent AST review found callback, shadowing, declaration visibility and
  static/instance identity mistakes. Each accepted finding gets an executable
  probe and extraction regression; review ledgers record the specific cases.
- Full suite first integration pass found the new synthetic god-object fixture
  had no decision nodes (CC=0), so it correctly received no advice. Added one
  `if` to satisfy the existing eligibility rule. Both binary-level tests passed
  on retry; the production eligibility code is unchanged.
- First corpus run completed both passes for ten repositories with action-only
  diffs and no nondeterminism. Mautic's first pass exceeded ten minutes;
  inspection identified repeated whole-tree declaration scans for each callee.
  Interrupted this exploratory run and requested a behavior-preserving index
  optimization before the final corpus run.
- Initial mutation run was interrupted before mutant results while final review
  was still finding binder cases. It is not counted as mutation-gate evidence.


## Final check results

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: passed
  (18.07s; `/tmp/structural-clippy-final.log`).
- `RUSTFLAGS='-D warnings' cargo test --locked --all-features --no-fail-fast`:
  **1845 passed, 0 failed, 8 ignored** across 42 test targets;
  `/tmp/structural-full-final.log`.
- `make report-contract-check`: passed, generated declarations and serialized
  producer fixture agree; report/history wire shapes remain unchanged.
- `make report-smoke`: all 11 HTML tabs rendered without errors.
- Independent final review closed F1/F2 with executable rechecks and found no
  unresolved source issues. See [final review](2026-09-15-final-review.md).

Corpus comparison and recommendation audit completed; final baseline and mutation
results are recorded in the completion section below.


## Resume note

The interruption preserved the reviewed source and committed-style evidence
records but removed `/tmp` logs and background runs. Source remained unchanged
since the 1,845-test pass. Restarted the unfinished corpus/mutation gates with
logs under ignored `target/structural-verification/` in this worktree.

The mutation run covers every mutant in the source diff. Its execution filter
runs responsibility extraction, owner/grouping/ranking, repository-backed
collection parity, and cache unit tests (102 tests), using `--lib` to avoid
relinking unrelated integration binaries for every mutant. This is additional
to the successful full all-feature suite and cold/warm/forced binary tests;
it is not the sole validation of the branch.

Ruling: the requested delivery is a verified isolated implementation with a
separate baseline commit. Keep the local branch/worktree for review; merging,
pushing, tagging and publishing are outside this task's requested scope.


## Corpus comparison and recommendation review

The resumed `make field-test` completed both passes for all 11 pinned
repositories. Nine have intentional responsibility-action differences; no
nondeterminism or non-action differences were reported. The independent
numeric comparison also checked scores, unscored states, counts, thresholds,
and top-20 hotspot order for all 11 with no drift.

The [completed audit](../../field-test/audit/2026-09-15-structural-responsibility.md)
inspected 39 changed/displayed recommendation rows, 136 owner/member groups,
all 13 predicate witnesses, five withdrawals and five existing recommendations.
No structural True or Safe failures. Eleven pre-existing generated/vendor
recommendations fail Actionable; the audit records their source evidence,
local follow-up and explicit corpus-tested disposition. Baseline acceptance
is restricted to the reviewed action changes.

Evidence: `target/structural-verification/field-before.log`,
`numeric-comparison.json`, and the source/member evidence cited in the audit.

## Completion checks — 2026-09-16

After the second interruption, the final source and all nine mutation-driven
regressions passed the full all-feature suite: **1,854 passed, zero failed,
eight ignored**, across 42 targets. Formatting and all-target/all-feature
Clippy with warnings denied also passed. The HTML smoke check rendered all
11 tabs without errors. Durable local logs are under
`target/structural-verification/`: `full-final.log`, `clippy-final.log`, and
`report-smoke.log`. The refreshed `make report-contract-check` also passed
(`report-contract.log`), with no generated-file changes.

The official `make field-audit` emitted the complete worksheet before the
interruption (`field-audit.log`). The committed audit ledger supplies the
source-based judgments for every changed row and the bounded existing sample.

`make field-test-accept` completed for all 11 pins. The reviewed diff changes
39 responsibility action rows across nine baseline files; two files are
unchanged. Comparing each baseline to its previous committed version after
removing responsibility actions found no numeric or unrelated-action drift
(`accepted-comparison.json`). The baselines are committed separately from the
implementation as required by the review process.

The final `make field-test` passed: **field test clean across 11 repositories**
(`field-final.log`). Both passes match each other and the accepted baselines.
This closes P2 regression and determinism after the separate source-based
recommendation audit.

With all 20 mutation-driven regressions included, the full all-feature suite
passed again: **1,865 passed, zero failed, eight ignored**, across 42 targets
(`full-completion.log`). All-target/all-feature Clippy passed again
(`clippy-completion.log`). Later assertion strengthening is checked with the
focused regression and mutant rerun recorded in the mutation review.

The complete scoped mutation gate passed at **97.3% (293/301 viable)**:
291 caught, two proven nonterminating traversal mutations, eight reviewed
survivors, and ten unviable mutations, covering all 311 distinct cases.
Interrupted and resumed outcomes were deduplicated by exact mutant identity;
final reruns replace earlier outcomes. Timing-pressure cases were rerun, not
counted as kills. See [the mutation review](2026-09-15-mutation-review.md) for
commands, all survivor dispositions, and assertion-strengthening evidence.
The final 21 focused regressions, formatting, and all-target/all-feature
Clippy passed. Local artifacts: `mutation-gate.log`, `mutation-aggregate.json`.

All requested implementation and verification gates are complete. The existing
generated/vendor Actionable limitations are documented in the corpus audit;
no structural True or Safe failure remains. The isolated branch and worktree
are retained, and unrelated edits in the main worktree were preserved.
