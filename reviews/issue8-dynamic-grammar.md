# Issue 8 dynamic-language grammar evidence

## Catalog provenance

Python's `unittest` documentation defines test cases as subclasses of
`unittest.TestCase` and shows test methods inside those subclasses:
<https://docs.python.org/3/library/unittest.html#basic-example>.

The pytest documentation defines `@pytest.fixture` and the built-in marker
catalog used here (`parametrize`, `skip`, `skipif`, `xfail`, and
`usefixtures`):

- <https://docs.pytest.org/en/stable/reference/reference.html#pytest-fixture>
- <https://docs.pytest.org/en/latest/how-to/mark.html>

PHPUnit documents `PHPUnit\Framework\TestCase` as the test-case base and the
method attributes `Test`, `Before`, `After`, `BeforeClass`, and `AfterClass`:

- <https://docs.phpunit.de/en/12.5/writing-tests-for-phpunit.html>
- <https://docs.phpunit.de/en/12.5/attributes.html#test>
- <https://docs.phpunit.de/en/12.5/fixtures.html#attributes>

The detectors intentionally use only this bounded catalog. Names such as
`test_*`, Pest functions, arbitrary pytest marks, and PHPUnit convention-only
method names are not framework evidence.

## Executable grammar probe

The probe used the repository-pinned `tree-sitter` crates and printed
`Tree::root_node().to_sexp()`. It was run with:

```text
CARGO_TARGET_DIR=/home/edouard/WS/barad-dur/target cargo run --quiet --example issue8_dynamic_probe
```

The Python input covered module aliases, imported aliases, a called decorator,
an attribute base, and nested methods. Relevant output:

```text
(module
  (import_statement name: (aliased_import name: (dotted_name (identifier)) alias: (identifier)))
  (import_from_statement module_name: (dotted_name (identifier))
    name: (aliased_import name: (dotted_name (identifier)) alias: (identifier)))
  (decorated_definition
    (decorator (call function: (identifier) arguments: (argument_list)))
    definition: (function_definition name: (identifier) parameters: (parameters) body: (block (pass_statement))))
  (class_definition name: (identifier)
    superclasses: (argument_list (attribute object: (identifier) attribute: (identifier)))
    body: (block (function_definition name: (identifier) parameters: (parameters (identifier)) body: (block (pass_statement))))))
```

The PHP input covered aliased class and attribute imports, a direct base, an
attribute list, and marked/unmarked sibling methods. Relevant output:

```text
(program (php_tag)
  (namespace_use_declaration
    (namespace_use_clause
      (qualified_name prefix: (namespace_name (name) (name)) (name)) alias: (name)))
  (namespace_use_declaration
    (namespace_use_clause
      (qualified_name prefix: (namespace_name (name) (name) (name)) (name)) alias: (name)))
  (class_declaration name: (name) (base_clause (name))
    body: (declaration_list
      (method_declaration
        attributes: (attribute_list (attribute_group (attribute (name))))
        (visibility_modifier) name: (name) parameters: (formal_parameters)
        return_type: (primitive_type) body: (compound_statement))
      (method_declaration (visibility_modifier) name: (name)
        parameters: (formal_parameters) return_type: (primitive_type)
        body: (compound_statement)))))
```

PHP grouped imports expose their shared prefix separately from their clauses:

```text
(namespace_use_declaration
  (namespace_name (name) (name) (name))
  body: (namespace_use_group
    (namespace_use_clause (name))
    (namespace_use_clause (name) alias: (name))))
```

These shapes justify resolving imports and aliases from import nodes, bases
from `superclasses` / `base_clause`, and decorators or attributes only from
their attachment nodes. Comments and strings cannot produce those shapes.

## Classification boundary

A directly resolved `unittest.TestCase` or `PHPUnit\Framework\TestCase` base
class contributes the whole class range, including explicit helper methods.
A resolved pytest decorator or PHPUnit attribute contributes only the attached
function or method range, so an unmarked sibling stays production code. Nested
functions inherit evidence through containment in the marked range. Bindings
that are shadowed, reassigned, conflicting, wildcard-only, or resolved only in
another file remain unclassified.

The temporary probe was removed after recording this evidence.
