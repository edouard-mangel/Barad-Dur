# Fix #8 verification record

## Scope and compatibility

The approved all-language plan replaces the prior Rust-only document. Public
`FunctionMetrics` literals gain default-false `is_test`; version 0.23.0 records
this source compatibility break. Snapshot cache 9 rejects old headers before
payload decoding. History schemas and exported report types are unchanged.

## P1 invariant sweep

| Invariant | Enumerated call sites | Evidence |
| --- | --- | --- |
| All functions/measurements preserved | complexity/counters.rs::extract_functions, treesitter.rs::analyse_tree | Adds only index lookup; same queries and CC/LOC/nesting counters |
| Every constructor supplies provenance | counters.rs production constructor; scorer.rs and scorer/actions.rs fixtures; metrics/testutil.rs; metrics/health/long_methods.rs fixtures | Explicit boolean or Default; full compilation catches missing literals |
| One evidence index per tree | treesitter.rs::analyse_tree; test_context::TestContext::build | Language detectors return ranges; merged before extraction queries |
| File context optional | complexity/mod.rs::analyse_source, analyse_file, analyse_content; treesitter.rs::analyse | First two pass actual path, content passes None |
| Live and historical context agree | collector/snapshot_builder.rs live analyse_source and historical analyse_source calls; source_assembly.rs | Existing shared source assembly preserved; cross-language parity fixture compares standalone/live/historical/cache |
| Only responsibility advice filters evidence | scorer/actions.rs::group_methods_by_prefix, generate_refactoring_actions; scorer.rs::build_report | Filter precedes prefix/minimum/count/rank/take(5); Health selector unchanged |
| Other function consumers retain tests | metrics/health/long_methods.rs; metrics/callgraph.rs | No is_test filter added; hotspot/history code unchanged |
| Current cache retains evidence, old cache rejected | cache/storage.rs::save/load, CACHE_VERSION | Roundtrip + v8 rejection + absent JSON field regression |
| Report contract unchanged | exported report types and current serialization fixture | report-contract-check result recorded below |

## Test development evidence

- Baseline all-feature suite: 1510 library tests passed, 8 ignored; one local
  socket bind was denied by sandbox. All integration suites passed. Escalated
  exact socket-test retry passed.
- CLI acceptance RED: `responsibility_advice_ignores_inline_tests_on_cold_warm_and_forced_runs`
  failed on the existing split suggestion before scorer filtering.
- Scorer RED: mixed render/build group retained test entries before filtering.
  Test fixture then corrected to use CC=1 so all seven large files actually meet
  existing god-object eligibility. Final focused scorer test passes.
- First combined detector run: 34 passed, two fixture failures. PHP `Mixed` is a
  reserved type name; Kotlin string followed by comment/function parsed as infix
  expression. Corrected fixtures; retained P0 evidence.
- Independent reviews identified Rust function-name shadowing and omitted local
  trait/union shadows, Python parameters/foreign imports, and PHP braced namespace
  imports. See separate review reports and their follow-up evidence.

## Final gate results

- Full all-feature rerun on final production: 1,784 tests passed (1,589 library), zero failures, 8 ignored across
  42 suite results. Command: `RUSTFLAGS='-D warnings' cargo test --locked
  --all-features --no-fail-fast -- --test-threads=4`, with Rayon and build jobs
  limited to four while other checks ran. The last two test-only fixtures also
  pass in the final 38-test Rust/Python/PHP mutation baseline; production remains
  byte-identical to `e460f37`.
- Formatting and all-target/all-feature Clippy with warnings denied passed.
- Generated TypeScript declarations and current Rust serialization fixture passed;
  no generated artifacts changed. HTML smoke rendered all 11 tabs cleanly.
- Initial full 11-repository corpus regression/determinism run changed only
  responsibility actions in barad-dur, ripgrep and starship. Scores, category
  metrics, finding counts and hotspot surface stayed identical. Audit is in
  `field-test/audit/2026-09-14-test-context.md`; no Safe failure. Two prefix-only
  Actionable failures are tracked by [issue #9](https://lab.frogg.it/Edouard_Mangel/barad-dur/-/work_items/9)
  because this plan preserves the production prefix heuristic.
- Final corpus audit passed. The three action-only baselines were accepted in
  commit `a2b12d5`; the accepted-baseline regression/determinism rerun passed all
  11 repositories. After the bounded dynamic-language scope fixes in `e460f37`,
  the final full corpus run also passed all 11 repositories with no further
  baseline changes.
- Final scoped mutation inventory: **553 cases**, with **460 caught, 6 equivalent
  survivors, 87 unviable, zero outer-runner timeouts**. The repository's unchanged
  `scripts/mutation_gate.py` passes at **460/466 viable = 98.7%** (required 80%).
  Exact-name reconciliation rejects missing final-source cases, replaces obsolete
  Python/PHP outcomes, and applies the latest regression-fixture result for each
  survivor. No production logic was excluded beyond the existing project policy.
  Details and equivalence arguments are in `issue8-mutation-audit.md` and
  `issue8-dynamic-mutation-audit.md`.
- Looping PHP mutants exposed nextest child processes outliving the outer runner's
  timeout. The MR and nightly mutation commands now select a dedicated mutation
  profile that terminates a test after three 60-second periods, with a one-second
  grace. All 38 Rust/Python/PHP tests pass under that profile. Local narrow runs
  used a shorter ten-second child limit; the observed nonterminating mutations
  are caught. The GitLab pipeline itself was not run from this local branch.

Local transcripts are retained under
`.superpowers/sdd/2026-09-14-production-responsibility-clusters/logs/`.
