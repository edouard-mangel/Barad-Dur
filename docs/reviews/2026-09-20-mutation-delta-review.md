# Mutation evidence for the structural responsibility branch

This branch carries more in-diff mutants than the six per-MR shards can test
inside their wall clock, so `mutation-gate` accepts a documented local campaign
instead (`scripts/mutation-campaign.sh`, see CLAUDE.md). This is that document:
what was run, against which commit, and what happened to every survivor.

`docs/reviews/2026-09-15-mutation-review.md` covers the earlier, feature-scoped
run at `00fca3c` (293/301 viable, 97.3%). It is a different mutant set and does
not speak for the commits below.

## Method

`scripts/mutation-campaign.sh`, one resumable `cargo mutants` run per file, with
the same policy `scripts/mutation_gate.py` applies in CI: timeouts count as
caught, unviable mutants leave the denominator, the bar is 80%. Every run passed
`--cargo-arg=--lib`, so integration suites never killed anything; the rates below
are lower bounds.

## Runs

| Scope | Commit | Mutants | Verdict |
|---|---|---|---|
| Review-fix delta (`41f126a~2..df7bab0`) | df7bab0 | 299 listed, 228 viable | 201/228 caught = 88.2% |
| Fix delta of 2026-09-19 (`df7bab0..acb0a91`) | acb0a91 | 26 listed, 23 viable | 23/23 caught = 100.0% |
| Whole branch vs `origin/main` | acb0a91 | 987 listed; 10 of 12 files completed | 431/434 viable caught = 99.3% |

The third run is partial and is reported as partial: ten files finished
(`counters` 2, `complexity/mod` 3, `responsibility/resolution` 170, `test_context`
`managed` 89, `mod` 10, `php` 78, `python` 86, `rust` 57, `treesitter` 3,
`scorer/actions` 21 — 519 outcomes, 85 unviable), and two did not:
`responsibility/mod.rs` (~211 mutants, interrupted) and
`test_context/javascript.rs` (~256, never started). Those two files are the
branch's largest, so roughly 47% of the in-diff mutants have no completed
measurement at this head. What they do have: every line either file changed
between `df7bab0` and `acb0a91` is covered by the 100% run above, and their
earlier state was measured in the 88.2% run.

## Survivors

### Review-fix delta at df7bab0 — 27 survivors, all dispositioned

**17 killed by tests written against the mutant**, each observed red on the
mutant and green on the real code, in `739d9a0` (responsibility/mod.rs),
`2a5be1d` (responsibility/resolution.rs), `65542d5` (test_context/php.rs) and
`a9151c2` (test_context/python.rs). They covered: the unbraced-namespace
conjunction, the anonymous-class and PHP global-namespace labels, comments
inside parameter and argument lists, the six TypeScript `required_parameter`
arms (`this` and rest parameters, which the JavaScript fixtures cannot reach),
the Python generator argument, PHP `...$xs`, the Kotlin trailing-lambda-only
call, a `MISSING` node inside a `#[Test]` method, and a decorated redefinition
of an imported fixture.

**9 equivalent**, with the reason recorded here rather than left implicit:

| Mutant | Why no test can distinguish it |
|---|---|
| `responsibility/mod.rs:390` `<` → `<=` | A namespace declaration and a function have distinct start bytes, so equality never occurs. |
| `responsibility/resolution.rs:365` `\|\|` → `&&` in `parameter_count` | The `=` sibling always decides the branch first. |
| `responsibility/resolution.rs:459` `&&` → `\|\|` | Only a `call_expression` carries an annotated lambda, so the second operand cannot change the result. |
| `test_context/javascript.rs:623` and `:625`, six arithmetic mutants on the callee-chain cap | The cap is 8 and the longest callee any provider accepts is five links, so every mutant of the bound admits exactly the same chains. |
| `test_context/php.rs:80` `<` → `<=` | `start == end` holds only for an empty program, which has no functions to classify. |

**1 mutant that was equivalent as written and should not have been.**
`resolution.rs:445` showed the Kotlin spread guard testing a `*` token the
grammar never emits: the guard was dead, so mutating it changed nothing. That is
a defect, not an equivalence — `helper(*xs)` counted as one argument and could
fabricate shared-callee evidence against an inherited vararg overload. Fixed in
`acb0a91`, pinned by `a_kotlin_spread_argument_makes_arity_undecidable`.

Counting the 17 kills, the delta stands at 218/228 = 95.6%.

### Whole-branch run at acb0a91 — 3 survivors in the completed files

All three are in the equivalence table above (`resolution.rs:365`,
`resolution.rs:460`, `php.rs:80`); `:460` is the same annotated-lambda argument
as `:459` after the intervening edits. The interrupted `responsibility/mod.rs`
run had also reported six survivors before it stopped (`:149`, `:154`, `:155`,
`:376`, `:399`, `:443`); they are recorded here as observed, not dispositioned,
since that file's run never completed.
