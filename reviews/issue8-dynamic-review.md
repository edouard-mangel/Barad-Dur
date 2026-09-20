# Issue 8 dynamic detector review

Review scope: `test_context/python.rs` and `test_context/php.rs` against `/tmp/issue8-contract.txt`. The findings below were reproduced through the public `analyse_content` path with the pinned grammars. A temporary probe printed `(function name, is_test)` and was removed after the run.

## Findings

### High — Python parameter shadowing produces false test evidence

`bindings_for()` handles assignments and declarations but never invalidates inherited bindings from function parameters. A nested decorated function therefore resolves a parameter named `pytest` to the module import even though the parameter shadows it lexically.

```python
import pytest
def outer(pytest):
    @pytest.fixture
    def nested(): pass
```

Observed:

```text
[("outer", false), ("nested", true)]
```

Expected: `nested` is production because `pytest` refers to the parameter. This violates “shadowed bindings stay unclassified.” Parameter patterns, including typed/defaulted and destructured parameters, need to invalidate inherited names before visiting the function body.

### High — Python ignores conflicting imports from unsupported modules

`parse_import()` and `parse_from_import()` return early for modules other than `pytest` and `unittest`. Consequently, an unsupported import that rebinds a recognized local alias never conflicts with or invalidates the recognized binding.

Decorator reproduction:

```python
from pytest import fixture as mark
from fake import other as mark
@mark
def case(): pass
```

Observed: `[("case", true)]`; expected `case == false`.

Base-class reproduction:

```python
from unittest import TestCase as Base
from fake import Base
class C(Base):
    def helper(self): pass
```

Observed: `[("helper", true)]`; expected `helper == false`.

This violates “conflicting bindings stay unclassified” for both supported Python evidence forms. All import statements must contribute their local bound names to conflict resolution, even when their targets are outside the provider catalog.

### High — PHP misses imports inside braced namespaces

For a `namespace_definition`, `bindings_for()` scans only direct named children. In the braced form, `use` declarations live inside the namespace body node, so neither the TestCase alias nor the attribute alias is collected.

```php
<?php
namespace App {
    use PHPUnit\Framework\TestCase as Base;
    use PHPUnit\Framework\Attributes\Test as Check;
    class C extends Base {
        #[Check] function marked() {}
        function sibling() {}
    }
}
```

Observed:

```text
[("marked", false), ("sibling", false)]
```

Expected: both functions are test context because the directly resolved `TestCase` base class marks the whole class. The same aliases work in an unbraced namespace, which isolates the fault to namespace-body traversal. `bindings_for(namespace_definition)` should collect namespace-level imports from the namespace body without leaking bindings into nested scopes or neighboring namespaces.

## Invariants confirmed by inspection and existing tests

- Python bounds whole classes only for directly resolved `unittest.TestCase` bases; pytest decorators mark only their attached function, so class markers do not classify siblings.
- Python accepts only the specified fixture and built-in marker catalog; plain `test_*`, wildcard-only imports, comments, strings, and the covered malformed decorator remain unclassified.
- PHP accepts direct/aliased/fully-qualified `PHPUnit\Framework\TestCase`, resolves namespace attribute aliases, and limits method evidence to the five specified PHPUnit attributes.
- PHP foreign alias conflicts are represented as ambiguous bindings, method-only evidence excludes siblings, whole-class evidence includes explicit helpers, and errored classes/methods are rejected.

## Fix evidence

All three High findings were fixed with focused regressions.

- Python function parameters now invalidate inherited provider bindings before
  visiting the function body. Plain, typed, defaulted, variadic, and
  destructured parameter patterns are handled without treating type
  annotations or default expressions as bound names.
- Every Python import contributes its actual local binding, including foreign
  module aliases and foreign `from` imports. A foreign binding therefore makes
  a reused pytest or unittest alias conflicting or resolves it outside the
  provider catalog for both decorators and class bases.
- PHP braced namespaces collect imports and declarations from their body.
  Bindings are kept within each braced namespace, and consecutive unbraced
  namespace segments are resolved independently so aliases neither leak nor
  conflict across neighbors.

Focused verification:

```text
cargo test --lib test_context::python:: -- --nocapture
8 passed; 0 failed

cargo test --lib test_context::php:: -- --nocapture
8 passed; 0 failed
```

## Final review disposition

Accepted. The re-review confirmed that all three High findings are resolved:

- Python invalidates inherited provider bindings from parameter patterns while leaving type annotations and default expressions out of the bound-name set.
- Python records foreign imports under their actual local names, so reused pytest and unittest aliases become ambiguous or resolve outside the provider catalog.
- PHP reads imports from braced namespace bodies and segments consecutive unbraced namespaces, preserving alias visibility without cross-namespace leakage.

I reran both focused suites through the current integrated library: 8 Python tests and 8 PHP tests passed. Inspection of the changed paths found no new contract gap introduced by these fixes.

## Final cleanup follow-up disposition

The subsequent cleanup preserves the accepted scoped behavior. Python and PHP now borrow inherited bindings when traversing nodes that do not create binding scopes; scope collection still constructs local bindings. Removing Python's dead `class_is_test` parameter leaves the whole-class early return and decorated-function boundaries intact. PHP now descends through an unmarked, error-free method to discover independently marked nested classes without marking the enclosing method or its sibling.

Focused integrated-library verification with `CARGO_TARGET_DIR=/home/edouard/WS/barad-dur/target`:

- `cargo test --lib test_context::python:: -- --nocapture`: 8 passed, 0 failed.
- `cargo test --lib test_context::php:: -- --nocapture`: 9 passed, 0 failed, including `recognized_nested_class_inside_unmarked_method_keeps_its_own_boundary`.

No new finding arose in these cleanup paths. The prior review acceptance remains supported within this scope; full-suite and corpus verification is recorded separately.
