# Issue 8 mutation survivor audit

Scope: the final production source at `e460f37` has **553 mutants** under the repository's existing exclusions. Exact-name reconciliation accounts for every final-source mutant, replaces the old Python/PHP outcomes, and retains completed results for unchanged code. No new production logic is excluded. The final reconciled gate passes at **460/466 viable (98.7%)**, with six equivalent survivors and 87 unviable cases. The last survivor-only run caught 15 of 16 native-language cases; the remaining Python case is equivalent within the static catalog.

The disposable mutation clone uses nextest's documented [per-test termination configuration](https://nexte.st/docs/configuration/reference/) (10-second period, one period before termination, one-second grace). This prevents an infinite-loop mutant's child process from indefinitely delaying the outer runner's timeout. Application behavior is unchanged. The committed mutation-only CI profile uses three 60-second periods and the same one-second grace so the MR and nightly runners also terminate looping test children. Baselines pass; inventory reconciliation rejects missing cases.

The notes below preserve the initial survivor review. Final outcomes:

| Surviving condition | Final disposition |
| --- | --- |
| JavaScript 98:20 | Declared loop bindings already have no provider value; additional invalidation is redundant. |
| JavaScript 700:18, 707:50, 708:32 (true) | Safe indexed access and exhaustive signature-slot matching enforce the same accepted arities. |
| JavaScript 791:32 | Independent declaration validity and exact quoted provider matching reject the same invalid references. |
| Python 147:9 | Generic traversal reaches the same nested definition for bounded static decorator forms; arbitrary side-effecting decorators are outside this equivalence. |

All other originally surviving conditions were caught after the regression fixtures. The unchanged gate script accepts the reconciled 553-case result; all inventories and per-case provenance are retained with the local transcripts.

## Current survivor dispositions

The entries below inspect generated diff artifacts against existing scoped behavior. They identify missing regressions or redundant checks; no original-implementation contract gap has been established from these survivors. All 19 focused JavaScript tests passed against unchanged production code after adding four regression tests; the test file was formatted with rustfmt. Final supplemental outcomes are recorded above.

### `javascript.rs:105:20` — replace match guard node .child_by_field_name("operator") .is_some_and(|op| op.kind() == "delete") with true in ranges

Missing positive regression: `typeof test` reads the provider binding and must not invalidate it. The mutant conservatively loses supported evidence. Added read-versus-write fixture.

### `javascript.rs:98:20` — replace match guard !children(*node) .iter() .any(|n| matches!(n.kind(), "const" | "let" | "var")) with true in ranges

Equivalent for covered declaration semantics: a declared for-in/of binding is collected with `value: None` and already shadows outer provider bindings. Adding that same declaration identity to the invalid set cannot remove live API evidence; conflicting same-scope declarations already resolve to no binding.

### `javascript.rs:111:13` — delete match arm "update_expression" in ranges

Missing negative regression: `test++` changes a resolved local API binding. Deleting this arm leaves false test evidence. Added update-expression fixture.

### `javascript.rs:124:45` — replace && with || in ranges

Missing positive regressions: assigning an unrelated unresolved global or a locally shadowed `require` must not invalidate an unrelated CommonJS import. Added both controls; mutant loses supported evidence.

### `javascript.rs:193:5` — replace function -> bool with false

Missing negative regression: function-local `var test = require(...).test` must not leak to a top-level call. Without function boundaries, var scope expands to program. Added fixture.

### `javascript.rs:209:21` — delete ! in lexical_scope

Missing positive regression: var declared inside an inner block remains visible through its containing function. Inverting the lexical/block scope selector loses evidence outside that block. Added fixture.

### `javascript.rs:242:9` — delete match arm "import_alias" in declarations

Missing TS negative regression: an internal import alias shadows an outer provider (`namespace N { import test = custom.test; ... }`). Deleting this declaration arm falsely resolves the outer API. Added TS/TSX fixture.

### `javascript.rs:244:34` — replace == with != in declarations

Same missing internal-import-alias regression as 242:9: rejecting its identifier drops the shadow declaration. Added TS/TSX fixture.

### `javascript.rs:307:67` — replace == with != in declarations

Missing positive regression: a `let` for-of binding ends at the loop and must not shadow a later imported test call. The mutant treats non-var loop declarations as var and expands their scope. Added fixture.

### `javascript.rs:434:39` — replace == with != in variable_declarations

Missing positive regression: inner-block var declarations must remain visible after their block. Inverting variable_declaration detection changes var/lexical scope. Added fixture shared with 209:21.

### `javascript.rs:447:33` — replace match guard p == Provider::Node with true in variable_declarations

Missing negative regression: `require("vitest")` is a namespace, not the callable Node default. Treating every provider as NodeDefault accepts unsupported direct namespace calls. Added fixture.

### `javascript.rs:456:40` — replace match guard text(item, source) == name with true in variable_declarations

Missing alias-selection regression: `{it, test: beforeEach}` must resolve `beforeEach` to exported test, not the shorthand iteration name. Unguarded shorthand selection sees the wrong export before the matching renamed pair. Added positive fixture.

### `javascript.rs:474:66` — replace && with || in variable_declarations

Missing negative regression: `{test: check, expect: verify}` must not bind verify to test from the first identifier-valued pair. Added multiple renamed-property fixture.

### `javascript.rs:515:75` — replace || with && in required_provider

Missing negative regression: `other("vitest")` is not the CommonJS loader even with a recognized literal module argument. The changed disjunction admits it. Added fixture.

### `javascript.rs:515:38` — replace || with && in required_provider

Same missing loader-identity regression as 515:75: an arbitrary identifier callee passes the weakened conjunction. Added fixture.

### `javascript.rs:565:9` — delete match arm "assignment_pattern" | "object_assignment_pattern" in assignment_names

Missing negative regression: destructuring default assignment to a namespace member (`{handler: v.test = fake}`) changes the provider root. Generic pattern_names does not recurse through member_expression, unlike assignment_names. Added fixture.

### `javascript.rs:607:17` — replace || with && in callee

Missing negative regressions: call-form each requires exactly one table argument. The mutant admits zero/two table arguments when the final member is each. Added both fixtures.

### `javascript.rs:656:17` — replace && with || in modifiers_valid

Missing negative regressions: unknown/duplicate/conflicting Vitest modifiers must remain unclassified. The weakened disjunction bypasses the allowed/unique checks or permits only+skip. Added fixtures.

### `javascript.rs:652:17` — replace && with || in modifiers_valid

Same modifier coverage gap as 656:17: allowed and unique checks must both hold. Added unknown, repeated-only, and only-plus-skip fixtures.

### `javascript.rs:700:18` — replace match guard (1..=2).contains(&args.len()) with true in callback_argument

Equivalent final classification: after selecting index zero, args.get rejects no arguments and the later exhaustive hook slots reject more than two. Removing this preliminary arity guard changes no accepted callback.

### `javascript.rs:699:48` — replace && with || in callback_argument

Missing no-callback robustness regression: a zero-argument Mocha hook reaches args[0] after changing && to || and panics. For nonempty invalid signatures the later slot check still rejects them. Added empty provider-call controls.

### `javascript.rs:705:31` — replace match guard (1..=3).contains(&args.len()) with true in callback_argument

Missing no-callback robustness regression: zero-argument Node test computes args.len()-1 after removing the guard (debug underflow panic). Later slot checks otherwise reject excess arguments. Added empty provider-call controls.

### `javascript.rs:707:50` — replace match guard (2..=3).contains(&args.len()) with true in callback_argument

Equivalent final classification: args.get(1) rejects fewer than two arguments; exhaustive slots reject more than three. Removing this preliminary arity guard cannot add an accepted callback.

### `javascript.rs:708:32` — replace match guard args.len() == 2 with false in callback_argument

When replaced with true: equivalent final classification, since args.get(1) rejects short calls and the Mocha slot match accepts exactly two. When replaced with false: missing positive Mocha test regression; every Mocha test callback is lost. Added named Mocha test fixture.

### `javascript.rs:708:32` — replace match guard args.len() == 2 with true in callback_argument

When replaced with true: equivalent final classification, since args.get(1) rejects short calls and the Mocha slot match accepts exactly two. When replaced with false: missing positive Mocha test regression; every Mocha test callback is lost. Added named Mocha test fixture.

### `javascript.rs:708:43` — replace == with != in callback_argument

Missing positive Mocha test regression: changing ==2 to !=2 rejects the supported two-argument call; later slots reject all other arities. Added named Mocha test fixture.

### `javascript.rs:742:13` — delete Mocha two-argument signature arm

Missing positive Mocha test regression: this removes every accepted Mocha named test callback. The new positive named Mocha test fixture covers the behavior; the final supplemental run catches this mutant.

### `javascript.rs:769:9` — delete spread-element signature rejection

Missing negative regression: an argument spread can shift the callback to an unknown runtime position, so `test(...names, callback)` cannot establish the required signature. Added a spread-name fixture across all four grammars. The subsequent final-module run caught this mutant.

### `javascript.rs:791:32` — weaken the string-node validity guard

Equivalent at current call sites: imports and require declarations independently require `valid(node)`, so malformed child nodes already lose evidence. A valid non-string expression cannot have exactly a quoted catalog provider as its complete source text; quote stripping followed by exact provider matching still rejects those expressions. The preliminary kind/error guard therefore adds no accepted binding after this mutation.

## Managed detector follow-up

### `managed.rs:112:32` — replace `==` with `!=` in `shadow`

Missing scope-isolation regression. A generic type parameter must shadow annotation names within its declaring class or method, without suppressing recognized annotations on neighboring classes or methods. The mutant sends type parameters through the ordinary lexical-scope search and expands their shadows. Added `generic_parameter_shadows_are_limited_to_their_declaration` with Java, C#, and Kotlin generic-class sibling controls and a Java generic-method sibling control. This targets existing type-parameter binding behavior; it adds no detector feature. `CARGO_TARGET_DIR=/home/edouard/WS/barad-dur/target cargo test --lib test_context::managed:: -- --nocapture` passed all 13 tests against unchanged production code. The final-module run caught this mutant.

### `managed.rs:255:30` — replace Kotlin language guard with true

Missing negative language-boundary regression: `kotlin.test.Test`, `BeforeTest`, and `AfterTest` are within the accepted Kotlin detector catalog but must not establish Java framework evidence. Removing the guard wrongly classifies those Java annotations. Added `kotlin_test_markers_do_not_establish_java_framework_evidence` for all three markers. Targeted baseline verification passed (now 16 managed tests); the final-module run caught this mutant.

### `managed.rs:263:9`, `264:9`, `265:22` — weaken targeted-marker rejection

Missing negative target-boundary regressions. 263:9 changes `has_error || use_site_target` to a conjunction and permits an otherwise clean Kotlin use-site-target annotation. 264:9 joins the Kotlin target check and C# target check by conjunction, permitting each target in its own grammar. 265:22 stops recognizing C# attribute lists, so return-targeted attributes are treated as method markers. Added paired negative/positive fixtures for `@get:kotlin.test.Test` versus the ordinary Kotlin annotation and `[return: Xunit.Fact]` versus the ordinary C# method attribute. These preserve the existing explicit target exclusions. The focused managed suite with these fixtures passed all 15 tests.

### `managed.rs:289:28` — change declaration/ancestor validity conjunction to disjunction

Missing negative malformed-body regression: an intact `@org.junit.Test` annotation on a method containing `broken ???;` must not establish evidence. The mutant accepts an erroneous method whenever its ancestors are not ERROR nodes. Added that exact parser-validity negative fixture.

### `managed.rs:293:27` — change ancestor validity accumulation from `&=` to `|=`

Missing negative malformed-ancestor regression: the accumulator starts true, so OR assignment can never reject an ERROR ancestor. Concrete Java `class C { @org.junit.Test void no() {} ???`, C# `class C { [Xunit.Fact] void no() {}`, and Kotlin `class C { @kotlin.test.Test fun no() {}` parse as an otherwise intact marked function under an ERROR node. Independent pinned-grammar probes confirmed that original code returns no ranges for all three. Added those fixtures; final managed verification passed all 16 tests; the final supplemental run catches this mutant.

## Rust survivor regression fixtures

- `rust.rs:88:30` and `93:39`: malformed `use` and `extern crate` declarations can retain the framework path and alias in parser recovery. New fixtures assert the declaration has an error and its alias supplies no test evidence. Both fixtures pass against unchanged production code.
- `rust.rs:179:14` and `179:48`: weakening the function-item guard accepts `#[test] mod checks { fn prod() {} }`. The prior struct-only negative had no extracted descendant and could not detect this. Added a module containing an extracted function.
- `rust.rs:216:25`: changing the inner-attribute conjunction accepts arbitrary leading inner attributes. Added `#![allow(dead_code)]` and module-local `#![cfg(not(test))]` controls with extracted production functions.

These are test coverage gaps, with no production change. The final survivor-only run caught all five Rust mutations with these fixtures.
