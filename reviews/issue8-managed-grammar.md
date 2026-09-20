# Issue 8 managed grammar evidence

P0: executable probe compiled with rustc against the pinned, compiled tree-sitter crates in target/debug/deps (Java 0.23.5, C# 0.23.5, Kotlin NG 1.1.0).

```text
java
import org.junit.Test; import org.junit.*; @org.junit.jupiter.api.Nested class C extends junit.framework.TestCase { @Test void yes() {} void no() {} @interface Test {} }
(program (import_declaration (scoped_identifier scope: (scoped_identifier scope: (identifier) name: (identifier)) name: (identifier))) (import_declaration (scoped_identifier scope: (identifier) name: (identifier)) (asterisk)) (class_declaration (modifiers (marker_annotation name: (scoped_identifier scope: (scoped_identifier scope: (scoped_identifier scope: (scoped_identifier scope: (identifier) name: (identifier)) name: (identifier)) name: (identifier)) name: (identifier)))) name: (identifier) superclass: (superclass (scoped_type_identifier (scoped_type_identifier (type_identifier) (type_identifier)) (type_identifier))) body: (class_body (method_declaration (modifiers (marker_annotation name: (identifier))) type: (void_type) name: (identifier) parameters: (formal_parameters) body: (block)) (method_declaration type: (void_type) name: (identifier) parameters: (formal_parameters) body: (block)) (annotation_type_declaration name: (identifier) body: (annotation_type_body)))))
csharp
using NUnit.Framework; using N = NUnit.Framework; using T = Xunit.FactAttribute; namespace A { [N.TestFixture] class C : B { [T, NUnit.Framework.Test] void yes() {} void no() {} class FactAttribute {} } }
(compilation_unit (using_directive (qualified_name qualifier: (identifier) name: (identifier))) (using_directive name: (identifier) (qualified_name qualifier: (identifier) name: (identifier))) (using_directive name: (identifier) (qualified_name qualifier: (identifier) name: (identifier))) (namespace_declaration name: (identifier) body: (declaration_list (class_declaration (attribute_list (attribute name: (qualified_name qualifier: (identifier) name: (identifier)))) name: (identifier) (base_list (identifier)) body: (declaration_list (method_declaration (attribute_list (attribute name: (identifier)) (attribute name: (qualified_name qualifier: (qualified_name qualifier: (identifier) name: (identifier)) name: (identifier)))) returns: (predefined_type) name: (identifier) parameters: (parameter_list) body: (block)) (method_declaration returns: (predefined_type) name: (identifier) parameters: (parameter_list) body: (block)) (class_declaration name: (identifier) body: (declaration_list)))))))
kotlin
import kotlin.test.Test as Check
import org.junit.*
@org.junit.jupiter.api.Nested class C : junit.framework.TestCase() { @Check fun yes() { fun nested() {} }
fun no() {}
}
annotation class Check {}
(source_file (import (qualified_identifier (identifier) (identifier) (identifier)) (identifier)) (import (qualified_identifier (identifier) (identifier))) (class_declaration (modifiers (annotation (user_type (identifier) (identifier) (identifier) (identifier) (identifier)))) name: (identifier) (delegation_specifiers (delegation_specifier (constructor_invocation (user_type (identifier) (identifier) (identifier)) (value_arguments)))) (class_body (function_declaration (modifiers (annotation (user_type (identifier)))) name: (identifier) (function_value_parameters) (function_body (block (function_declaration name: (identifier) (function_value_parameters) (function_body (block)))))) (function_declaration name: (identifier) (function_value_parameters) (function_body (block))))) (class_declaration (modifiers (class_modifier)) name: (identifier) (class_body)))
```

Annotations and attributes attach inside declaration modifiers/attribute lists; imports expose typed path nodes and aliases; bases are superclass/delegation-specifiers nodes. Kotlin declarations require statement separators: the initial same-line adjacent-function sample parsed as ERROR, so malformed scopes must be rejected. Comments and strings are not marker nodes. Method markers do not authorize sibling functions; explicit class markers and direct bases do.

Provider catalog checked against official sources:
- JUnit 4 Test and lifecycle: https://junit.org/junit4/javadoc/latest/org/junit/package-summary.html
- Jupiter method annotations, lifecycle and Nested: https://docs.junit.org/5.14.1/writing-tests/annotations.html
- TestNG Test (class or method) and ten Before/After lifecycle markers: https://testng.org/annotations.html
- NUnit method/lifecycle and TestFixture/TestFixtureSource: https://docs.nunit.org/articles/nunit/writing-tests/attributes.html
- MSTest methods, class and lifecycle: https://learn.microsoft.com/en-us/dotnet/core/testing/unit-testing-mstest-writing-tests
- xUnit Fact/Theory: https://xunit.net/docs/getting-started/v2/getting-started
- Kotlin Test/BeforeTest/AfterTest: https://kotlinlang.org/api/latest/kotlin.test/kotlin.test/

The classifier intentionally requires explicit imports for Java/Kotlin; star imports are insufficient evidence. C# namespace usings are explicit namespace bindings, with conflicts and local type shadowing rejected. No runtime inference, inheritance traversal, or class promotion from a single method.

Kotlin grammar caveat observed in negative-fixture execution: a string property followed by a block comment and `fun` can be parsed as an `infix_expression`, with no extracted function and no ERROR node. The fixture therefore terminates the property with `;`; tests explicitly require nonempty function extraction to prevent vacuous negatives. Kotlin `typealias` declarations remain unrecognized type aliases; `import ... as ...` aliases are supported.

Validation: `CARGO_TARGET_DIR=/home/edouard/WS/barad-dur/target cargo test --lib test_context::managed -- --nocapture` passed 12 tests. These bounded detector checks include public `analyse_file` extraction for `.java`, `.cs`, `.kt`, and `.kts`, nonempty extraction in negative fixtures, production siblings, retained complexity, lexical aliases/conflicts, method-vs-class markers, direct bases, lifecycle annotations, malformed syntax, and comment/string lookalikes. Full-suite and field gates are handled by the integrating parent task.
