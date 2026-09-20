# Issue 8 dynamic-language mutation follow-up

Production sections of `python.rs` and `php.rs` remain byte-identical to `e460f37`; this follow-up adds public-behavior regression cases only.

## Original surviving helper mutations

| Location in original run | Regression disposition |
| --- | --- |
| Python `invalidate_parameters`: removed default/typed-default arm | Provider-named default and typed-default parameters must suppress nested fixture evidence. |
| Python `invalidate_parameters`: removed typed arm | Provider-named typed parameters and typed splats suppress evidence; provider names used only as annotations retain evidence. |
| Python `invalidate_parameters`: removed splat/tuple arm | Provider-named `*args` and `**kwargs` suppress nested fixture evidence. |
| Python `parse_import`: inverted alias/root checks | Dotted unaliased imports bind the root namespace; `import unittest.case` preserves direct base evidence; repeated `import pytest` preserves fixture evidence. |
| Python `bind`: identical-import guard false | Repeated identical dotted imports retain resolution. |
| PHP `has_test_attribute`: inverted attribute-node filter | Called `#[Test()]` retains evidence for its method alone. |
| PHP `bind`: identical-import guard false | Repeated identical imports retain the same framework target. |
| PHP `bindings_from_nodes`: malformed-import guard true | An import with a trailing incomplete `as` cannot supply attribute evidence. |

The obsolete original binding-collection mutations are superseded by the production scope correction and the final-source dynamic mutation run. Its newly surviving outcomes will be audited below.

## Executable malformed-import grammar probe

Input: `<?php use PHPUnit\Framework\Attributes\Test as; class C { #[Test()] function subject() {} }`

The pinned grammar retains the resolvable import clause but also places an `ERROR` inside the import declaration. This proves the malformed-import guard has observable work to do:

```text
(program (php_tag)
 (namespace_use_declaration
  (namespace_use_clause (qualified_name prefix: (namespace_name (name) (name) (name)) (name)))
  (ERROR))
 (class_declaration name: (name) body: (declaration_list
  (method_declaration attributes: (attribute_list (attribute_group
   (attribute (name) parameters: (arguments))))
   name: (name) parameters: (formal_parameters) body: (compound_statement)))))
```

Probe/targeted regression passed before removing the temporary print. Full test-context suite: **74 passed, 0 failures**. The final exact dotted-import fixture rerun also passed. `git diff --check` passed. The final survivor verification results are recorded below.


## Final-source dynamic run

The complete bounded run evaluated 108 mutants: 87 caught, 11 missed, and 10 unviable. Every survivor has a disposition below; line numbers identify `e460f37` production source.

| Survivor | Disposition |
| --- | --- |
| PHP 95:36, `has_test_attribute`, `==` → `!=` | Called-attribute regression supplies the missing positive coverage. |
| PHP 133:44, `bindings_from_nodes`, error guard → true | Malformed-import regression supplies the missing negative coverage; the recorded executable parse retains a resolvable clause inside an erroneous import. |
| PHP 186:33, `bind`, identical-target guard → false | Duplicate identical import regression supplies positive coverage. |
| PHP 205:17, `resolve`, global-namespace guard → false | Added explicit positive global qualified references without a leading separator, covering both a direct TestCase base and an individual Test attribute. |
| Python 147:9, delete `decorated_definition` arm in `collect_bindings` | Equivalent for the bounded decorator catalog: generic traversal reaches the same underlying function/class definition, invalidates the same declaration name, and stops at its body. Recognized fixture/mark decorator arguments contain no additional binding constructs that change this result. Keep the explicit arm because it documents the scope boundary and avoids unnecessary decorator traversal. No mutant exclusion is added. |
| Python 189:31 and 191:19, invert `parse_import` root/alias choices | Real dotted `unittest.case` import regression supplies positive root-namespace coverage. |
| Python 226:13, remove default-parameter arm | Parameter matrix covers default and typed-default provider shadows. |
| Python 231:13, remove typed-parameter arm | Parameter matrix covers typed provider shadows and positive annotation-only references. |
| Python 241:13, remove splat-parameter arm | Parameter matrix covers provider-named list and dictionary splats. |
| Python 254:33, identical-target guard → false | Repeated identical import regression supplies positive coverage. |

The final survivor-only rerun will determine which added regressions kill these mutants; coverage expectations above are not represented as measured kills until that run completes.

The final global-qualified PHP fixture passed its exact targeted test after using a non-reserved class name (`Subject`). Fixtures are frozen for the survivor-only rerun; production remains unchanged.

## Final survivor verification

The final survivor-only run caught all ten non-equivalent dynamic mutations with the added fixtures. Python 147:9 remains equivalent for bounded static decorator forms, as described above. Together with the Rust follow-up and prior JavaScript/managed checks, exact reconciliation covers all 553 final-source mutants and the unchanged gate passes at 98.7%. No dynamic production code changed during this test follow-up.
