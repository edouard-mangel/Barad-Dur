# Issue 8 managed detector review

Review scope: `test_context/managed.rs` and `managed_tests.rs` against `/tmp/issue8-contract.txt`, covering Java, C#, Kotlin, and KTS. I found no contract violation in the reviewed implementation.

Validation command:

```text
CARGO_TARGET_DIR=/home/edouard/WS/barad-dur/target cargo test --lib test_context::managed -- --nocapture
```

Result: all 12 focused tests passed. The tests exercise the detector directly and also exercise existing `FunctionMetrics` extraction through `analyse_file` for `.java`, `.cs`, `.kt`, and `.kts`, with nonempty extraction assertions.

## Resolution audit

- Java imports bind the final imported type name to its complete path. Star imports contribute no binding. Fully qualified annotations resolve only when the resulting complete name belongs to the fixed catalog.
- Kotlin imports use the same mechanism, including `import ... as ...` aliases. Star imports contribute no binding. Kotlin's `kotlin.test` catalog is enabled only for Kotlin, while its supported Java annotations and direct `junit.framework.TestCase` base remain available.
- C# distinguishes namespace usings (`name: None`) from aliases (`name: Some`). Namespace lookup accepts a bare attribute only when exactly one visible namespace produces a catalog member. Aliases, alias-qualified references, `global::` references, and the optional `Attribute` suffix resolve as intended. Static usings do not establish provider bindings.
- Bindings carry syntax-derived lexical ranges. Local types, annotation declarations, type aliases, type parameters, and namespaces create empty-target shadow bindings. Multiple matching bindings are rejected, so conflicting aliases do not fabricate evidence. Namespace-local C# usings stay within their declaration list and do not leak to neighboring namespaces.
- Unqualified references without explicit evidence do not resolve. Qualified references still pass through the exact class/method catalogs, preventing similarly named providers from being accepted.

## Scope audit

- Java and Kotlin method annotations contribute only the attached method/function range. Nested functions inherit through range containment; separate helpers and sibling methods do not.
- Direct `junit.framework.TestCase` bases, Jupiter `Nested`, and class-level TestNG `Test` contribute the whole class range, including explicit helpers. Inheritance is direct only.
- C# `NUnit` fixture and `MSTest` class attributes contribute the whole class range. xUnit, NUnit method/lifecycle, and MSTest method/lifecycle attributes contribute only the attached method range.
- Applying a method-only marker to a class does not promote that class. Applying a class-only marker to a method does not mark that method.

## Catalog and syntax audit

- The Java catalog matches the contract's JUnit 4, Jupiter, parameterized-test, and TestNG markers.
- The C# catalog matches xUnit `Fact`/`Theory`, NUnit test/lifecycle and fixture markers, and MSTest method/lifecycle and class markers. Attribute-suffixed and unsuffixed spellings normalize to the same catalog entry.
- Kotlin adds exactly `kotlin.test.Test`, `BeforeTest`, and `AfterTest` to the Java catalog.
- Marker nodes with parse errors, declarations under error ancestors, use-site/attribute-target annotations, comments, strings, and name-only conventions are excluded.

The existing grammar evidence in `reviews/issue8-managed-grammar.md` supports the import, annotation, attribute, base-class, and malformed-input node shapes used by the implementation. No temporary probe or implementation change was needed for this review.
