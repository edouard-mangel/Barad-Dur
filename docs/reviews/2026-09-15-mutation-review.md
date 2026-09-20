# Fix #9 scoped mutation review — completed 2026-09-16

## Result

**PASS: 293/301 viable mutants detected (97.3%), above the 80% gate.**
All 311 original scoped mutants have a completed outcome: 291 caught, two
proven infinite-loop timeouts, eight reviewed survivors, and ten unviable.
No mutant is omitted from the final aggregate.

## Scope

This review classifies surviving mutants from the feature-scoped run under
`target/structural-verification/mutation/mutants.out`. The first run was interrupted after 227 of 311 mutants had completed. Its
immutable results are retained in
`target/structural-verification/mutation-initial-interrupted/mutants.out`.
A resumed `--iterate` run skips its 199 caught and seven unviable mutants and
examines the other 105, including earlier survivors and deterministic timeouts.
The aggregate uses the latest outcome for each exact mutant name; interrupted
logs are not presented as completed shards.

No production behavior was changed to satisfy a mutant. Focused regressions
live in `src/metrics/complexity/responsibility/mutation_tests.rs`.

## Dispositions

| Mutant | Disposition | Evidence and rationale |
|---|---|---|
| `mod.rs:76` delete `field_expression \| field_access \| selector_expression` modified-member arm | Retired as a conservative guard | These are ordinary Rust, Java, and Go member writes. Rust field-call syntax requires parentheses around a callable field and does not replace a same-name method. Java method lookup is separate from fields. Go rejects a field and method with the same name on one type. Because `modified_members` is file-global and name-only, the arm can only suppress otherwise resolvable method evidence after an unrelated field write. A test would freeze a deliberate false negative rather than protect reachable method rebinding. |
| `mod.rs:80` delete Python `attribute` modified-member arm | Missing behavior test, added | Python permits runtime replacement such as `A.helper = external`. A replacement inside a separate `reset` body is deliberately invisible to the caller's lexical bindings, so the file-global scan is the only blocker. `python_method_replacement_outside_the_class_blocks_static_callee_evidence` asserts that `self.helper()` supplies no static callee evidence. A direct Python execution returned `external`, confirming that the assignment changes dispatch. |
| `mod.rs:81` delete `member_access_expression` modified-member arm | Retired as a conservative guard | In C#, a field/property and method cannot provide a mutable replacement for the statically selected same-name method. PHP permits a property and method with the same name, but `$this->helper()` still dispatches to the method after assigning a closure to `$this->helper`; an executable PHP probe printed `method`. As with line 76, the file-global name-only guard merely suppresses evidence after unrelated writes. |
| `mod.rs:82` delete Kotlin `navigation_expression` modified-member arm | Retired as a conservative guard | Kotlin assignment resolves a property setter, while ordinary call overload resolution gives function-like members priority over function-valued properties. Mutating a property therefore does not replace a resolved same-name member function. The file-global guard is conservative and receiver-insensitive; pinning it would preserve a false negative. |
| `mod.rs:250` replace malformed-ancestor `is_error() \|\| is_missing()` with `&&` | Equivalent defensive guard | `owner_of` first rejects a function that has or is a parse error. Missing nodes are zero-width recovery leaves and cannot contain a function. Probes of malformed Rust owners placed errors or missing tokens beside a recognized `function_item`, not as its ancestor; an error contained by the function is already caught by the entry check. No valid-language behavior distinguishes this mutation, and a malformed parser-shape fixture would be brittle even if one grammar happened to expose it. |
| `mod.rs:271` replace preceding namespace `<` with `>` | Missing behavior test, added | Semicolon namespace declarations apply to following PHP functions. With two namespaces, `>` assigns the first function to the later namespace and the final function to the file. `php_functions_belong_to_the_preceding_semicolon_namespace` pins both user-visible owner labels. |
| `mod.rs:271` replace preceding namespace `<` with `<=` | Equivalent | A namespace declaration node and a function node have distinct starts, so equality is impossible. `<=` and `<` select the same preceding declaration for every parse tree. |
| `mod.rs:287` invert the root-node fallback label condition | Missing behavior test, corrected after rerun | The free-function test passed under this mutation because a later branch independently sets the root label to `file`. The observable change is an unnamed non-root owner: a JavaScript object literal becomes `file (line 1)`. `an_anonymous_object_owner_reports_its_kind_and_source_line` pins `object (line 1)`; the rerun records this correction rather than claiming the initial test killed it. |
| `mod.rs:320` invert the Rust `self_parameter` predicate | Equivalent for valid Rust | `descendants(parameters)` includes the `parameters` node itself, so the mutant reports a receiver for every Rust function. Instance methods already have `self`; associated functions cannot legally use instance `self`, and `self::helper()` remains lexical module qualification handled before receiver matching. Observable divergence requires semantically invalid Rust using instance `self` without a receiver. Grammar validity alone is not evidence that such source could execute; no valid-Rust behavior is lost by this mutation. |
| `mod.rs:463` invert the anonymous-class-body kind check in `own_nodes` | Missing behavior test, added | The mutation traverses a Java anonymous class body and can leak a nested field initializer into the enclosing caller's evidence. `anonymous_class_arguments_belong_to_the_caller_but_its_body_does_not` asserts the caller keeps `this.ready` from the constructor argument and excludes `this.nested` from the anonymous body. |
| `mod.rs:464` replace the anonymous-class-body parent conjunction with disjunction | Missing behavior test, added | The mutation stops at every direct child of an object creation, including its argument list, and loses caller-owned argument evidence. The same anonymous-class test pins `ready` exactly, killing this mutation while also covering the line 463 leak. |
| `mod.rs:466` invert the anonymous-class parent-kind check in `own_nodes` | Missing behavior test, covered | Like the line 463 mutation, this traverses the nested body when its parent is an `object_creation_expression`. The exact `ready`-only assertion in `anonymous_class_arguments_belong_to_the_caller_but_its_body_does_not` detects the leaked `nested` field. |
| `mod.rs:537` delete import binding handling | Missing behavior test, added | Python executes imports as bindings. In `def helper(); from external import helper; def is_a(): helper()`, the import replaces the same-file function before `is_a` executes. `a_later_python_import_replaces_a_same_name_local_callee` asserts that the imported call is not attributed to the earlier local declaration. |
| `mod.rs:619` replace the tuple-field `&&` with `\|\|` in `member` | Grammar-equivalent | The mutation broadens the accepted name to any non-identifier child of a Rust `field_expression`, or an integer name on another member node. The pinned Rust grammar declares the `field` child as exactly `field_identifier` or `integer_literal`; the former passes `is_identifier` and the latter is the existing tuple-field exception. Other matched grammars expose identifier-shaped names and represent computed/index access with different node kinds. No producible supported-language AST distinguishes the mutation. |
| `mod.rs:686` replace nested function/type boundary `\|\|` with `&&` in `declared_fields` | Missing behavior test, added | The mutation traverses every nested owner and can attribute a Kotlin companion object's property to the enclosing instance owner. `a_kotlin_companion_property_does_not_become_an_instance_field` uses a valid unqualified companion-property access and asserts that it supplies no direct instance-field evidence. |
| `mod.rs:706` invert the Kotlin `variable_declaration` filter in `declared_fields` | Missing behavior test, added | Kotlin properties declared in a class body place their name under `variable_declaration`. `a_kotlin_body_property_supplies_implicit_field_evidence` pins a direct `ready` dependency for an ordinary body property, independently of constructor-property handling. |
| `mod.rs:725` replace the named-argument `&&` with `\|\|` in `is_bare_value` | Semantically redundant | The mutation additionally rejects identifiers followed by `=` outside a `value_argument`. In the implicit-receiver languages, that shape is an assignment left-hand side and `bindings` already records its name as local/reassigned before field evidence is considered. Within `value_argument`, the named-argument label already satisfies both sides. No dependency changes for valid source. |
| `resolution.rs:42` invert the `type_spec` kind check during receiver indexing | Missing behavior test, added | Go method expressions such as `A.helper(a)` resolve through the file's `type_spec`; the mutation omits that declaration. `a_go_type_method_expression_resolves_its_value_receiver_method` pins the local value-receiver callee. |
| `resolution.rs:43` replace scope/function-boundary `\|\|` with `&&` during receiver indexing | Missing behavior test, added | A nested Python function named `A` shadows an outer class `A` at `A.helper()`. Function boundaries are deliberately indexed as blocking receiver declarations; the mutation omits them and resolves the outer static method. `a_nested_python_function_shadows_a_same_name_type` asserts empty evidence. |
| `resolution.rs:144` replace Go receiver conjunction with disjunction | Missing behavior test, added | In valid Go, different receiver types can each declare `helper`. The mutation considers both methods during an `a.helper()` call on `A`, making an otherwise unique callee ambiguous. `same_named_go_methods_on_other_types_do_not_make_a_receiver_call_ambiguous` pins the direct local callee with both declarations present. |
| `resolution.rs:149–150` seven mutations of the Rust `Self` / PHP `self` receiver condition | Missing behavior tests, added | Positive `Self::helper()` and `self::helper()` calls must resolve only the current owner, even when another owner has a same-named helper. Explicit `B::helper()` calls must instead resolve `B`. `self_type_calls_resolve_only_methods_on_the_current_owner` and `explicit_other_type_calls_do_not_resolve_on_the_current_owner` cover both languages and both lookup directions. |
| `resolution.rs:153–154` owner equality / static-callability conjunction | Missing behavior test, added | The same current-owner test includes another type with a static same-named helper. The first test compared only callee labels and missed selecting the other owner's identically named helper. The corrected assertion pins the current owner's helper by exact source identity, and an isolated rerun verifies this correction. Inverting owner equality selects that other declaration; broadening the conjunction admits both helpers and makes the call ambiguous. |
| `resolution.rs:174` invert root-scope stop condition | Missing behavior test, added | Stopping before walking local scopes loses a nested PHP function called inside a semicolon namespace. `a_php_nested_function_resolves_inside_a_semicolon_namespace` pins that locally resolved callee. |
| `resolution.rs:217` invert namespace-owner equality | Missing behavior test, added | `a_bare_php_call_resolves_the_current_semicolon_namespace` pins an ordinary same-namespace helper. The mutation selects other namespaces and cannot resolve this callee. |
| `resolution.rs:261–262` broaden receiver-declaration eligibility conjunctions | Missing behavior test, added | Separate C# partial type declarations are valid source but deliberately remain distinct declarations in this bounded index. `separate_partial_type_declarations_make_type_qualified_calls_unknown` ensures a type-qualified call cannot silently pick the first declaration from an ambiguous local receiver lookup. |
| `resolution.rs:276` broaden block-to-parent scope mapping | Missing behavior test, added | A Rust function declared inside an `if` branch is invisible in that `if` condition. The mutation moves its scope to the enclosing `if_expression`, shadowing a valid outer helper while evaluating the condition. `rust_branch_local_functions_do_not_shadow_calls_in_the_condition` pins the outer callee's exact source identity. |
| `resolution.rs:281` narrow lexical/function scope boundary condition | Missing behavior test, corrected after rerun | Operator precedence preserves `type_scope` in this mutation, so the first Java-class fixture could not distinguish it. The mutation drops non-type lexical scopes such as a Rust inline module, indexing its helper at the root while call resolution still stops at the module boundary. `a_rust_module_keeps_its_free_functions_in_the_local_call_scope` pins the local module callee. The additional Java same-name-class fixture remains a useful positive boundary regression. |
| `resolution.rs:298` broaden anonymous-type scope condition | Missing behavior test, added | An anonymous class's argument list belongs to its caller. Treating every direct child of object creation as a type scope prematurely stops implicit lookup. `an_anonymous_class_constructor_argument_resolves_the_callers_implicit_method` pins a valid constructor argument call to its enclosing class's helper. |
| `resolution.rs:315` broaden static modifier condition | Missing behavior test, added | `public` does not make a Java or C# instance method static. `visibility_modifiers_do_not_make_an_instance_method_callable_through_a_type` verifies that a grammatically parseable but semantically invalid type-qualified instance call remains unknown rather than supplying local-callee evidence. |

## Verification

Nine focused regressions were initially added for the meaningful gaps above. The resumed
mutation baseline includes them and passed in 163 seconds build plus nine seconds
test. The earlier five-test run is retained as
`target/structural-verification/mutation-tests.log`; final all-feature verification
is recorded separately in the main review ledger.

The resumed invocation is:

```sh
NEXTEST_PROFILE=mutation BARAD_DUR_TEST_REPO="$PWD" cargo mutants \
  --in-diff target/structural-verification/final-resume.diff --iterate \
  --test-tool nextest --cargo-arg=--lib --cargo-test-arg=-E \
  --cargo-test-arg='test(responsibility) | test(scorer::actions) | test(cache::storage)' \
  --timeout-multiplier 3 --jobs 4 --gitignore true \
  --output target/structural-verification/mutation
```

The diff was refreshed solely because registering the new test module moved the
last line of `mod.rs`; its production mutation set remains exactly the original
311 mutants. The durable `aggregate_mutations.py` checks every original mutant
is present, rejects unexpected names or incomplete/error outcomes, and uses
`scripts/mutation_gate.py::Verdict` for the unchanged 80% policy. No exclusions
were added to improve the score. After reviewing the complete resumed run,
eleven further regressions initially brought the dedicated mutation module to 20 tests.
All 20 passed in the final confirmation baseline (83 seconds build, seven
seconds test). That confirmation selects the 19 meaningful survivors plus the
two deadline-sensitive outcomes, using a 60-second minimum timeout. An exact-identity assertion correction and a 21st Rust-module regression
subsequently close the two remaining meaningful gaps, each verified with its
own isolated mutant rerun. Seven
already-reviewed equivalent/conservative survivors and two proven infinite-loop
timeouts retain their completed outcomes; they are not removed from the
aggregate denominator. Library-only mutation execution avoids linking
unrelated integration binaries for each mutation; the complete all-feature
suite and real-binary integration tests are separate required gates.

The initial run's two timeouts change `index += 1` to `index *= 1` in
`descendants` (`mod.rs:449`) and `own_nodes` (`mod.rs:471`).
Both loops start at index zero, so multiplication leaves the cursor at zero
while the node vector remains nonempty. These are deterministic infinite
traversals detected by the timeout, rather than resource-pressure timeouts or
surviving mutants; the aggregate retains its normal timeout policy.

## Completed batches and final evidence

| Batch | Completion | Outcomes |
|---|---|---|
| Initial run | Interrupted after 227/311 | 199 caught, 19 missed, seven unviable, two timeouts |
| Resumed `--iterate` | 105/105, 24 minutes | 72 caught, 26 missed, three unviable, four timeouts |
| Stronger-test / deadline confirmation | 21/21, eight minutes | 18 caught, three missed |
| Exact current-owner identity correction | 1/1, two minutes | One caught |
| Rust lexical-module correction | 1/1, two minutes | One caught |
| Deduplicated latest outcome | **311/311** | **291 caught, eight missed, ten unviable, two timeouts** |

The `mod.rs:619` resumed deadline was resource pressure: the log recorded all
111 tests passing in 30.496 seconds just past its 30-second deadline. The
60-second confirmation completed and retained it as a reviewed survivor.
`mod.rs:686` already produced the expected Kotlin assertion failure, but its
runner reached the deadline; confirmation recorded an ordinary caught mutant.
Neither deadline artifact is counted as a timeout in the final result.

The final eight survivors are the three conservative modified-member guards
(`mod.rs:76`, `81`, `82`), malformed-ancestor guard (`250`), distinct-start
comparison (`271`, `<` to `<=`), valid-Rust receiver equivalence (`320`),
grammar-equivalent tuple-field predicate (`619`), and redundant named-argument
filter (`725`). Their individual rationales are in the disposition table above.
All meaningful gaps identified by mutation testing now have verified kills.

Durable artifacts under `target/structural-verification/`:

- `mutation-initial-interrupted/mutants.out/`: immutable partial first batch.
- `mutation/mutants.out/` and `mutation-resumed.log`: completed resumed batch.
- `mutation-final/mutants.out/` and `mutation-final.log`: 21-mutant confirmation.
- `mutation-identity/mutants.out/`, `mutation-module/mutants.out/`: isolated corrections.
- `mutation-aggregate.json`: each mutant's latest outcome provenance and survivors.
- `aggregate_mutations.py`, `mutation-gate.log`: completeness checks and unchanged gate policy.
- `mutation-final-tests.log`: all 21 dedicated regressions passed with all features.
- `mutation-final-clippy.log`: all-target/all-feature Clippy passed with warnings denied.

Final `cargo fmt --all -- --check` also passed. The last corrections only add or
strengthen test assertions; production source was unchanged throughout triage.
