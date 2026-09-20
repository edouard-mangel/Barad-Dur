# Issue 8 dynamic-language scope follow-up

Executable probes use the repository-pinned grammars with `cargo run --quiet --example issue8_scope_probe`. The temporary probe is removed after recording evidence.

Python control-flow assignments are nested below a block, but do not introduce a language binding scope:

```text
(module (import_statement name: (dotted_name (identifier)))
 (function_definition name: (identifier) parameters: (parameters) body: (block
  (if_statement condition: (identifier) consequence: (block
   (expression_statement (assignment left: (identifier) right: (identifier)))))
  (for_statement left: (identifier) right: (identifier) body: (block (pass_statement)))
  (decorated_definition (decorator (attribute object: (identifier) attribute: (identifier)))
   definition: (function_definition name: (identifier) parameters: (parameters)
    body: (block (pass_statement)))))))
```

PHP absolute and relative bases both parse as `qualified_name`; their original leading separator, and namespace/import context, distinguish resolution:

```text
(program (php_tag)
 (namespace_definition name: (namespace_name (name)))
 (namespace_use_declaration (namespace_use_clause (name) alias: (name)))
 (class_declaration name: (name)
  (base_clause (qualified_name prefix: (namespace_name (name) (name)) (name)))
  body: (declaration_list)))
```

Additional Python probe nodes for binding targets:

```text
(with_item value: (as_pattern (call function: (identifier) arguments: (argument_list))
 alias: (as_pattern_target (identifier))))
(except_clause value: (as_pattern (identifier) alias: (as_pattern_target (identifier)))
 (block (pass_statement)))
(named_expression name: (identifier) value: (identifier))
```

## Corrections

- Python indexes module/function/class scopes across conditional blocks, loops, `with` and `except` aliases, and assignment expressions. It stops at nested definitions and lambdas. A lambda-local assignment expression cannot invalidate an outer provider. These bindings invalidate provider resolution throughout the containing scope.
- Python remembers declarations preceding imports, so declaration/import conflicts are order-independent. Direct class bases resolve in the surrounding environment. Class-body imports apply to method decorators; method bodies and nested-class bodies inherit the surrounding function/module environment, as Python class namespaces do not enclose them. A nested class base can still resolve an alias in its containing class body.
- PHP distinguishes absolute leading-backslash names from namespace-relative references and resolves import aliases before accepting names in the global namespace. A `use Vendor as PHPUnit` alias cannot supply PHPUnit evidence. Class/import conflicts remain unresolved regardless of order.

Initial regressions: Python 2 failed / 9 passed; PHP 3 failed / 9 passed. These failures directly reproduced the reported false classifications. The class-base regression initially passed and prevents the new recursive binding collector from incorrectly applying class-local shadows to bases.

Production functions changed (for the supplemental mutation scope): Python `ranges`, `visit`, `bindings_for`, new `collect_bindings`, and `invalidate`; PHP `ranges`, `bindings_for`, `bindings_from_nodes`, `parse_use_clause`, `bind`, `invalidate`, and `resolve`, plus the namespace-aware `Bindings` data type.

Targeted verification: the first corrected `cargo test --lib test_context:: -- --test-threads=4` run passed all 68 tests. The final rerun adds the independent reviewer’s nested-class-boundary regression and passed **69 tests, 0 failures**. `git diff --check` passed for the changed files.


## Independent bounded scope review

Reviewed the introduced Python scope indexing and PHP namespace/root-alias resolution changes without editing either implementation. Two Python boundary follow-ups were reported with exact fixtures and resolved by the implementation agent: a lambda-local walrus must not invalidate its surrounding provider import, and a nested class body must not inherit its enclosing class namespace although its base expression evaluates there.

Independent pinned-grammar probes against the final Python source parse cleanly and produce: lambda-local walrus plus outer `@pytest.fixture` → one function range; nested class referencing only an outer-class provider alias → no ranges; directly resolved `unittest.TestCase` with a same-named class-body assignment → one class range. The implementation agent also reports all 69 targeted context tests passing. PHP inspection confirms that only leading-backslash references bypass alias resolution, namespaced relative names do not become root framework evidence, and declarations before imports remain conflicts. No remaining actionable finding was identified within these changed scope paths. This is a bounded review of these fixes, not a claim of complete Python/PHP name resolution.
