# Fix #8: test-aware responsibility clusters

This replaces the Rust-only plan. The implementation contract comes from the
approved September 14 plan: require framework or path evidence, preserve every
function measurement, and filter only responsibility grouping before its minimum,
counts, ranking, and five-action cap.

## Detection contract

All functions in existing `FileRole::Test` paths have evidence. Content-only
analysis has no path evidence. Helpers inherit lexical containment, never calls.
A test method does not classify its siblings; a whole class requires a recognized
class marker, direct test base, or file context.

| Language | Bounded evidence |
| --- | --- |
| Rust | Bare test, exact outer/inner cfg(test), resolved tokio/async_std test attributes |
| JS/TS/JSX/TSX | Imported Jest, Vitest, Mocha, node:test test/suite/lifecycle inline callbacks; explicit provider/modifier catalog |
| Python | Direct unittest.TestCase; pytest.fixture and built-in parametrize/skip/skipif/xfail/usefixtures function marks |
| Go | Existing test-file classification, including `_test.go` helpers |
| Java | JUnit 4/Jupiter/TestNG method and lifecycle annotations; direct JUnit TestCase, Jupiter Nested, TestNG class Test |
| C# | xUnit/NUnit/MSTest method and lifecycle attributes; NUnit TestFixture/TestFixtureSource, MSTest TestClass; optional Attribute suffix |
| Kotlin/KTS | kotlin.test Test/BeforeTest/AfterTest plus Java catalog and explicit class rules |
| PHP | Direct PHPUnit TestCase; PHPUnit Test/Before/After/BeforeClass/AfterClass method attributes |

Fully qualified names, named/namespace imports and direct aliases are file-local.
JS also supports static CommonJS require bindings. Ambiguous, shadowed, reassigned,
conflicting, wildcard-only or cross-file-only names remain eligible.
Import aliases are resolved directly; subsequent value aliases such as
`const check = test` and Kotlin `typealias` declarations are outside the catalog. Names alone,
custom wrappers/annotations, indirect inheritance, dynamic imports, compound Rust
cfg/cfg_attr and macro expansion are outside the catalog.

## Implementation and invariants

1. Reuse the parsed tree; build and merge a private byte-range evidence index once.
2. Add documented, default-false `FunctionMetrics.is_test`. Keep all functions and
   numeric measurements. Existing JS extraction remains unchanged.
3. Use snapshot version 9; reject old positional payloads before decoding. Keep
   history schemas and exported report contracts unchanged. Public struct addition
   requires a breaking conventional commit and the next 0.x minor release (0.23.0).
4. Filter at `group_methods_by_prefix`, preserving prefix rules, deterministic
   tie-breaking, action formatting and Health selection.
5. Deliver all language detectors together, with executable grammar evidence and
   positive/negative tests. No placeholder language implementation is completion.

Superseded details (2026-09-16/17, MR !152 review): #9 shipped in the same
branch, so the cache is version 10, not 9. The action text is no longer
preserved: it has one ` | ` segment per owner (ordered by each owner's first
function, groups merged per owner, predicate evidence as `shared field X` or
`shared callee X`), shows at most the five owners with the most grouped names
followed by `+N more groups` (`+1 more group` for one), and names unnamed owners in plain words. Same-name
members count once, callee evidence needs a matching argument count, self-calls
are not evidence, and Go methods on a type declared in another file are grouped
under the type name (unknown under a local alias, a malformed local declaration or
a parse error). The version bump happens at release time; the crate is not
pre-bumped to 0.23.0 in the branch.

## Verification

P0 parse dumps: `reviews/issue8-{rust,go,managed,dynamic,javascript}-grammar.md`.
P1 enumerates constructors, consumers, collection paths, cache and scorer callers.
Acceptance covers file/content analysis, live/historical parity, cache reload and
cold/warm/forced CLI runs. Check nonempty extraction in all grammars, aliases,
shadows/conflicts, method vs class scope, lexical vs called helpers, malformed
lookalikes, singleton removal and ranking under fifty test functions.

Final gates: format, Clippy, full all-feature Rust suites, generated report
contract, HTML smoke, scoped mutation kill rate >=80%, full corpus regression and
determinism, and recommendation audit. Inspect pinned Rust validate/render/build
examples and every newly surfaced suggestion. Only intentional action-only
baseline changes may be accepted in a separate reviewed commit; investigate score
or measurement drift. Record actual results and unresolved limits in the review.
