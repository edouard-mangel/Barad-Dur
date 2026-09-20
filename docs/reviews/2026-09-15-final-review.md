# Fix #9 final independent review — 2026-09-15

## Scope and evidence

Reviewed the complete diff against `decff37`, explicitly including untracked
`src/metrics/complexity/responsibility/{mod.rs,resolution.rs,tests.rs,resolution/tests.rs}`.
Read the implementation ledger, P0 probes, grouping review, detector review,
and repository review-process requirements. No production files were edited by
this reviewer and no full suite was duplicated.

Standalone executable probes rebuild an exact copy of the current producer and
use the repository's pinned tree-sitter libraries. The harness substitutes only
the snapshot value types and a false test-context lookup. Thus these probes
establish structural resolution behavior; repository tests establish recognized
test exclusion. The harness source is `/tmp/detector-review-harness.py`.

## Findings caught at P1 final review

### F1 — shadow binders could manufacture shared dependencies

The initial producer omitted these valid lexical binding forms:

| Language | Binding | Incorrect initial evidence |
|---|---|---|
| Rust | `for helper in ...` | outer `helper` callee |
| Rust | `match value { helper => helper() }` | outer `helper` callee |
| Rust | `if let Some(helper) = value` | outer `helper` callee |
| Java | `catch (Exception ready)` | receiver field `ready` |
| Java | `value instanceof String ready` | receiver field `ready` |
| C# | `foreach (bool ready in values)` | receiver field `ready` |
| C# | `value is string ready` | receiver field `ready` |
| Kotlin | `for (ready in values)` | receiver field `ready` |
| Kotlin | `val (ready, other) = value` | receiver field `ready` |

Two same-owner predicates using these forms could falsely satisfy the required
shared dependency minimum. This violates the explicit shadow-name exclusion.
Initial AST/output: `/tmp/final-review-probes.log` and
`/tmp/final-review-more.log`; each source is saved as
`/tmp/final-review-<case>.<extension>`. Python exception aliases were a passing
control. Disposition: fixed. The frozen producer collects all nine binding forms;
`loop_pattern_catch_and_destructuring_bindings_cannot_supply_outer_dependencies`
contains their exact sources. Independently rebuilt probes now show empty
dependencies for every case (`/tmp/final-review-after.log`).

### F2 — unindexed named functions failed to shadow outer callees

A C# `local_function_statement helper` and a JavaScript
`generator_function_declaration helper` already stop nested body traversal, but
were absent from function indexing and local shadow checks. Calling `helper()`
in their enclosing scope incorrectly selected the same-name class/module
function. Initial executable evidence is in `/tmp/final-review-more.log`.
Disposition: fixed. The producer records both names as local binding blockers
without changing extraction or treating their bodies as caller evidence.
`unindexed_local_callable_declarations_block_outer_callee_names` covers both
cases; independently rebuilt probes now show empty dependencies.

## P1 invariant and consumer sweep

| Contract | Enumerated consumers / tests inspected | Inspection result |
|---|---|---|
| Optional provenance only changes advice | sole production `FunctionMetrics` literal in `counters::extract_functions`; metric consumers in health, audit, hotspots, callgraph/coupling | existing extraction queries, name/LOC/CC/nesting and test assignment unchanged |
| All owners partition every prefix | `group_methods_by_prefix`, `every_prefix_respects_owner_identity_including_separate_declarations` | bucket key is owner ID plus prefix; unknown owners and tests removed first |
| Predicates require identical direct evidence | `predicate_groups`; identity/kind, duplicate, transitive-chain, overlap and tie tests | typed identity keys; largest remaining set; assigned indexes removed from all candidates; singleton sets discarded |
| Existing ranking and advice routing | `generate_refactoring_actions`, scorer Health guard, ranking/path/cap tests | eligible-member count, display-path tie, cap five, Health category and hotspots routing retained |
| Advice exposes evidence | formatter and exact-string assertion | owner label always present; predicate field/callee label present |
| Cache rejects old positional payload | cache header guard, version-nine-decodable-payload test | version 10 checked before snapshot deserialization |
| Cache preserves both dependency variants | `direct_responsibility_provenance_survives_cache_reload` | source-derived owner, field and callee round trip asserted |
| Collection paths share implementation | `analyse_file`/`analyse_content`/`analyse_source`; live/historical assembly equality test | shared parser and extractor; cached values equal original snapshot |
| Report/history schemas unchanged | scorer/report contract/history modules | history version remains 6; report types untouched |
| Cold/warm/forced behavior agrees | structural responsibility walking skeleton | actual CLI advice and categories compared; cross-impl singleton and unrelated predicates excluded |
| Breaking public literal API documented | snapshot types and changelog | optional Serde input defaults to None; literal break, cache 10 and next 0.x minor release documented |

## Conservative limitations

The detector intentionally omits evidence it cannot establish locally, including
unknown receivers, imports, ambiguous or inherited callees, dynamic members,
aliases, and Kotlin extension ownership. The initially omitted JS private `#field` syntax was fixed during review:
private names now participate in the same direct field/callee resolution, with
a regression covering JS/TS variants and unchanged function extraction counts.
The independent private-field probe now yields the same `#ready` dependency for
both predicates. No minor finding is silently deferred.

## Verification status

The frozen F1/F2 fixes were independently re-read and the exact-code harness
rebuilt. All 13 final-review cases matched their expected outcomes: eleven
previous false-positive cases now have empty dependencies, the Python exception
alias control remains empty, and private JS field evidence is present.
`/tmp/final-review-after.log` records the ASTs and values. The 22 earlier
review probes were rerun and their output inspected, including positive lexical
helper resolution, Kotlin properties/companions, Python typed receivers, and
negative callback/type-shadowing/receiver-alias cases
(`/tmp/final-review-controls-after.log`). `git diff --check` passed.

The performance patch was also reviewed: `ResolutionIndex` materializes the
same declaration-scope/name partitions and source-ordered semicolon namespace
lookup previously recalculated per call. Callee uniqueness, local body/test
checks, receiver mode, and lexical stopping rules remain at their existing
decision points. No new unresolved source finding remains.

Full-suite, formatting/Clippy, report contract, mutation and pinned-corpus
results are owned by the root agent and must be recorded before completion.
This source review does not imply those commands passed or that the corpus
recommendations are safe.
