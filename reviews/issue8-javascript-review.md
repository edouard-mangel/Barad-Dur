# Issue 8 JavaScript detector review

Resolution: findings 1–8 and 10 are implemented and regression-tested by the scoped-binding revision. Finding 9 is retired with the explicit accepted interpretation that “direct aliases” means import aliases; ordinary value aliases stay unknown. See the final section of `issue8-javascript-grammar.md` for implementation evidence and validation.

Reviewed the initial detector against `/tmp/issue8-contract.txt`, its grammar report, and the pinned TypeScript grammar. Read-only implementation review; executable probes copied the detector into `/tmp/issue8-js-review` and compiled it with rustc against the existing pinned tree-sitter rlibs. Every reported callback contains a named `no` or `yes` descendant, so these ranges affect existing function extraction rather than only anonymous callbacks.

## Must fix: P1 false test evidence

1. **Type-only imports establish runtime test bindings** (`collect_import`, around line 60). `import type { test }`, `import { type test }`, and `import type * as v` all return one callback range despite providing no runtime binding. All three parse without errors. Inspect anonymous `type` tokens on the import statement/specifier, not only named descendants.
2. **Bindings lack lexical identity** (`imported_bindings`, around line 47; `require_call`, around line 159). A `require` binding declared inside one function leaks to an unrelated top-level call. A parameter named `require` is accepted as the Node loader, so an arbitrary caller-supplied function creates framework evidence. Bindings need scope and declaration identity, and the loader must be unshadowed.
3. **Shadow coverage misses valid lexical forms** (`is_shadowed`, around line 362; `direct_declarations_before`, around line 375). Single-parameter arrows use the field `parameter`, catch clauses also bind `parameter`, and declarations later in a block still shadow the import (function hoisting or lexical TDZ). Named function-expression self-bindings (`const f = function test() { ... }`) are missed too. Each probe falsely produces one callback. A scope sweep must include declarations throughout the relevant scope, not only source positions preceding the call.
4. **Reassignment ignores binding patterns and object properties** (`invalidated_bindings`, around line 304). `({test} = other)` fails to invalidate a CommonJS destructured binding. Mutating the provider namespace member also leaves evidence live. Resolve assignment targets to their binding identity, including destructuring and namespace roots.
5. **Call shape validation accepts unsupported API expressions** (`callee`, around line 271; `is_modifier`, around line 287; `callback_range`, around line 228). `test('first')('x', callback)`, uncalled `test.each('x', callback)`, Jest `test.only.only(...)`, and Vitest `test(callback, 'not a callback')` each return a callback range. The current code erases every intermediate call and validates each modifier independently, then selects any function argument. Preserve call/application structure and match documented combinations and provider-specific callback positions.

6. **Malformed recognized calls still generate evidence** (`callback_range`). Both `test('x', ???, callback)` and a callback containing `broken @@@` parse with ERROR nodes but return one callback range. Reject errors in the evidence-bearing invocation and malformed ancestor contexts; a single malformed-prefix negative fixture is insufficient.
7. **CommonJS destructuring discards the actual property path** (`collect_require`, around line 106). `const {anything: {test}} = require('vitest')` and `const {test} = require('vitest').anything` both give one callback range although neither resolves to the provider's exported `test`. Only direct binding patterns on the actual namespace/API result should resolve.

All false positives can suppress production responsibility advice and therefore violate the Safe invariant.

## Must fix: P2 supported evidence lost

8. **An unrelated assignment to a shadow binding invalidates a file import** (`invalidated_bindings`). `function f(test) { test = other; }` causes a later valid top-level imported `test(...)` to lose evidence. Scope invalidation by binding identity rather than spelling.
9. **Direct value aliases are not resolved** (`imported_bindings`). `const check = test` and `const check = v.test` both produce zero ranges. Import aliases themselves work; if “direct aliases” in the contract includes these ordinary immutable value aliases, support them with the same conflict/reassignment rules. If the accepted scope only includes import aliases, record that interpretation explicitly instead of silently claiming all direct aliases.

10. **Node CommonJS namespace members are unavailable** (`collect_require`). `const node = require('node:test'); node.test('x', callback)` produces zero ranges because this binding is represented only as the callable default test function. Node's CommonJS export supports both callable and namespace-member forms; both need representation.

## Controls and API evidence

The parameter-destructuring control (`function f({test})`) correctly gives zero ranges. One malformed-prefix control also gives zero ranges, but this is not sufficient to establish rejection of errors inside otherwise recognized calls.

Official provider references verified during this review: [Jest API](https://jestjs.io/docs/api), [Vitest test API](https://vitest.dev/api/test), [Node test runner](https://nodejs.org/api/test.html). Their documented signatures distinguish test/suite callbacks from hooks and describe call-form `.each(table)(name, callback)`; the classifier must retain these distinctions. No package installation or runtime framework execution was used.

## Final re-review disposition

The scoped-binding revision fixes the original binding-identity, type-only import, reassignment, malformed-tree, nested CommonJS, `.each`, and Node callable-namespace failures. The focused JavaScript suite passes all 13 tests across JS, JSX, TS, and TSX. Two call/binding cases remain unresolved.

### High — documented callback slots are checked, but other signature slots are not

`callback_argument()` selects a callback from the provider-specific position using only argument count and whether an argument is a function. It does not validate the documented types of the name, options, or timeout slots. These syntactically valid but unsupported provider calls all classify `subject` as test context through public `analyse_file`:

```ts
import { test } from 'vitest';
test('x', 'not options', () => { function subject() {} });

import { test } from 'node:test';
test('x', 'not options', () => { function subject() {} });
test(42, () => { function subject() {} });

import { test } from '@jest/globals';
test('x', () => { function subject() {} }, 'not timeout');
```

Observed for each: `[("subject", true)]`. Vitest and Node require an options object in the middle slot; Node's leading name is a string when present; Jest's third argument is a numeric timeout. This is the remaining part of original finding 5 and can still create false test evidence. Signature matching needs to validate these non-callback slots structurally, while retaining Node's documented callback-only and options-plus-callback overloads.

### Medium — static CommonJS bindings with destructuring defaults are lost

Direct object patterns resolve only shorthand properties and `pair_pattern` values that are bare identifiers. Valid static bindings expressed with an `assignment_pattern` remain unknown:

```ts
const { test = fallback } = require('vitest');
test('x', () => { function subject() {} });

const { test: check = fallback } = require('vitest');
check('x', () => { function subject() {} });
```

Observed for each: `[("subject", false)]`. These are direct static CommonJS property bindings; the default expression does not change the property path and should not be mistaken for the intentionally unsupported `const check = test` value-alias case.

The re-review probe used the public analysis path and was removed afterward. No implementation file was changed by the reviewer.

## Executable probe output

```text
type_import: error=false ranges=1
import type { test } from 'vitest'; test('x', () => { function no() {} });
(program (import_statement (import_clause (named_imports (import_specifier name: (identifier)))) source: (string (string_fragment))) (expression_statement (call_expression function: (identifier) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))

inline_type_import: error=false ranges=1
import { type test } from 'vitest'; test('x', () => { function no() {} });
(program (import_statement (import_clause (named_imports (import_specifier name: (identifier)))) source: (string (string_fragment))) (expression_statement (call_expression function: (identifier) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))

type_namespace: error=false ranges=1
import type * as v from 'vitest'; v.test('x', () => { function no() {} });
(program (import_statement (import_clause (namespace_import (identifier))) source: (string (string_fragment))) (expression_statement (call_expression function: (member_expression object: (identifier) property: (property_identifier)) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))

require_scope_leak: error=false ranges=1
function factory() { const {test} = require('vitest'); } test('x', () => { function no() {} });
(program (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block (lexical_declaration (variable_declarator name: (object_pattern (shorthand_property_identifier_pattern)) value: (call_expression function: (identifier) arguments: (arguments (string (string_fragment)))))))) (expression_statement (call_expression function: (identifier) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))

shadow_require: error=false ranges=1
function factory(require) { const {test} = require('vitest'); test('x', () => { function no() {} }); }
(program (function_declaration name: (identifier) parameters: (formal_parameters (required_parameter pattern: (identifier))) body: (statement_block (lexical_declaration (variable_declarator name: (object_pattern (shorthand_property_identifier_pattern)) value: (call_expression function: (identifier) arguments: (arguments (string (string_fragment)))))) (expression_statement (call_expression function: (identifier) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))))

arrow_parameter: error=false ranges=1
import {test} from 'vitest'; const f = test => test('x', () => { function no() {} });
(program (import_statement (import_clause (named_imports (import_specifier name: (identifier)))) source: (string (string_fragment))) (lexical_declaration (variable_declarator name: (identifier) value: (arrow_function parameter: (identifier) body: (call_expression function: (identifier) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))))

hoisted_shadow: error=false ranges=1
import {test} from 'vitest'; function f() { test('x', () => { function no() {} }); function test() {} }
(program (import_statement (import_clause (named_imports (import_specifier name: (identifier)))) source: (string (string_fragment))) (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block (expression_statement (call_expression function: (identifier) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))) (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block)))))

tdz_shadow: error=false ranges=1
import {test} from 'vitest'; function f() { test('x', () => { function no() {} }); const test = other; }
(program (import_statement (import_clause (named_imports (import_specifier name: (identifier)))) source: (string (string_fragment))) (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block (expression_statement (call_expression function: (identifier) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))) (lexical_declaration (variable_declarator name: (identifier) value: (identifier))))))

catch_parameter: error=false ranges=1
import {test} from 'vitest'; try {} catch(test) { test('x', () => { function no() {} }); }
(program (import_statement (import_clause (named_imports (import_specifier name: (identifier)))) source: (string (string_fragment))) (try_statement body: (statement_block) handler: (catch_clause parameter: (identifier) body: (statement_block (expression_statement (call_expression function: (identifier) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block)))))))))))

destructuring_assignment: error=false ranges=1
let {test} = require('vitest'); ({test} = other); test('x', () => { function no() {} });
(program (lexical_declaration (variable_declarator name: (object_pattern (shorthand_property_identifier_pattern)) value: (call_expression function: (identifier) arguments: (arguments (string (string_fragment)))))) (expression_statement (parenthesized_expression (assignment_expression left: (object_pattern (shorthand_property_identifier_pattern)) right: (identifier)))) (expression_statement (call_expression function: (identifier) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))

namespace_assignment: error=false ranges=1
import * as v from 'vitest'; v.test = fake; v.test('x', () => { function no() {} });
(program (import_statement (import_clause (namespace_import (identifier))) source: (string (string_fragment))) (expression_statement (assignment_expression left: (member_expression object: (identifier) property: (property_identifier)) right: (identifier))) (expression_statement (call_expression function: (member_expression object: (identifier) property: (property_identifier)) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))

unsupported_call: error=false ranges=1
import {test} from 'vitest'; test('first')('x', () => { function no() {} });
(program (import_statement (import_clause (named_imports (import_specifier name: (identifier)))) source: (string (string_fragment))) (expression_statement (call_expression function: (call_expression function: (identifier) arguments: (arguments (string (string_fragment)))) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))

uncalled_each: error=false ranges=1
import {test} from 'vitest'; test.each('x', () => { function no() {} });
(program (import_statement (import_clause (named_imports (import_specifier name: (identifier)))) source: (string (string_fragment))) (expression_statement (call_expression function: (member_expression object: (identifier) property: (property_identifier)) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))

duplicate_only: error=false ranges=1
import {test} from '@jest/globals'; test.only.only('x', () => { function no() {} });
(program (import_statement (import_clause (named_imports (import_specifier name: (identifier)))) source: (string (string_fragment))) (expression_statement (call_expression function: (member_expression object: (member_expression object: (identifier) property: (property_identifier)) property: (property_identifier)) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))

callback_wrong_position: error=false ranges=1
import {test} from 'vitest'; test(() => { function no() {} }, 'not a callback');
(program (import_statement (import_clause (named_imports (import_specifier name: (identifier)))) source: (string (string_fragment))) (expression_statement (call_expression function: (identifier) arguments: (arguments (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block)))) (string (string_fragment))))))

malformed_outer: error=true ranges=0
import {test} from 'vitest'; broken test('x', () => { function no() {} });
(program (import_statement (import_clause (named_imports (import_specifier name: (identifier)))) source: (string (string_fragment))) (expression_statement (call_expression function: (identifier) (ERROR (identifier)) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))

direct_alias: error=false ranges=0
import {test} from 'vitest'; const check = test; check('x', () => { function yes() {} });
(program (import_statement (import_clause (named_imports (import_specifier name: (identifier)))) source: (string (string_fragment))) (lexical_declaration (variable_declarator name: (identifier) value: (identifier))) (expression_statement (call_expression function: (identifier) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))

namespace_direct_alias: error=false ranges=0
import * as v from 'vitest'; const check = v.test; check('x', () => { function yes() {} });
(program (import_statement (import_clause (namespace_import (identifier))) source: (string (string_fragment))) (lexical_declaration (variable_declarator name: (identifier) value: (member_expression object: (identifier) property: (property_identifier)))) (expression_statement (call_expression function: (identifier) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))

shadow_sibling_reassignment: error=false ranges=0
import {test} from 'vitest'; function f(test) { test = other; } test('x', () => { function yes() {} });
(program (import_statement (import_clause (named_imports (import_specifier name: (identifier)))) source: (string (string_fragment))) (function_declaration name: (identifier) parameters: (formal_parameters (required_parameter pattern: (identifier))) body: (statement_block (expression_statement (assignment_expression left: (identifier) right: (identifier))))) (expression_statement (call_expression function: (identifier) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))

positive_parameter_destructuring: error=false ranges=0
import {test} from 'vitest'; function f({test}) { test('x', () => { function no() {} }); }
(program (import_statement (import_clause (named_imports (import_specifier name: (identifier)))) source: (string (string_fragment))) (function_declaration name: (identifier) parameters: (formal_parameters (required_parameter pattern: (object_pattern (shorthand_property_identifier_pattern)))) body: (statement_block (expression_statement (call_expression function: (identifier) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))))

```

## Additional executable probes

```text
malformed_argument: error=true ranges=1
import {test} from 'vitest'; test('x', ???, () => { function no() {} });
(program (import_statement (import_clause (named_imports (import_specifier name: (identifier)))) source: (string (string_fragment))) (expression_statement (call_expression function: (identifier) arguments: (arguments (string (string_fragment)) (ERROR) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))

malformed_callback: error=true ranges=1
import {test} from 'vitest'; test('x', () => { function no() {} broken @@@ });
(program (import_statement (import_clause (named_imports (import_specifier name: (identifier)))) source: (string (string_fragment))) (expression_statement (call_expression function: (identifier) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block)) (expression_statement (identifier) (ERROR))))))))

nested_require_pattern: error=false ranges=1
const {anything: {test}} = require('vitest'); test('x', () => { function no() {} });
(program (lexical_declaration (variable_declarator name: (object_pattern (pair_pattern key: (property_identifier) value: (object_pattern (shorthand_property_identifier_pattern)))) value: (call_expression function: (identifier) arguments: (arguments (string (string_fragment)))))) (expression_statement (call_expression function: (identifier) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))

selected_require_pattern: error=false ranges=1
const {test} = require('vitest').anything; test('x', () => { function no() {} });
(program (lexical_declaration (variable_declarator name: (object_pattern (shorthand_property_identifier_pattern)) value: (member_expression object: (call_expression function: (identifier) arguments: (arguments (string (string_fragment)))) property: (property_identifier)))) (expression_statement (call_expression function: (identifier) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))

cjs_namespace_assignment: error=false ranges=1
const v = require('vitest'); v.test = fake; v.test('x', () => { function no() {} });
(program (lexical_declaration (variable_declarator name: (identifier) value: (call_expression function: (identifier) arguments: (arguments (string (string_fragment)))))) (expression_statement (assignment_expression left: (member_expression object: (identifier) property: (property_identifier)) right: (identifier))) (expression_statement (call_expression function: (member_expression object: (identifier) property: (property_identifier)) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))

named_function_expression_shadow: error=false ranges=1
import {test} from 'vitest'; const f = function test() { test('x', () => { function no() {} }); };
(program (import_statement (import_clause (named_imports (import_specifier name: (identifier)))) source: (string (string_fragment))) (lexical_declaration (variable_declarator name: (identifier) value: (function_expression name: (identifier) parameters: (formal_parameters) body: (statement_block (expression_statement (call_expression function: (identifier) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))))))

node_namespace_require: error=false ranges=0
const node = require('node:test'); node.test('x', () => { function yes() {} });
(program (lexical_declaration (variable_declarator name: (identifier) value: (call_expression function: (identifier) arguments: (arguments (string (string_fragment)))))) (expression_statement (call_expression function: (member_expression object: (identifier) property: (property_identifier)) arguments: (arguments (string (string_fragment)) (arrow_function parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))))))))

```

## Revised probe results

```text
type_import: error=false ranges=0
inline_type_import: error=false ranges=0
type_namespace: error=false ranges=0
require_scope_leak: error=false ranges=0
shadow_require: error=false ranges=0
arrow_parameter: error=false ranges=0
hoisted_shadow: error=false ranges=0
tdz_shadow: error=false ranges=0
catch_parameter: error=false ranges=0
destructuring_assignment: error=false ranges=0
namespace_assignment: error=false ranges=0
unsupported_call: error=false ranges=0
uncalled_each: error=false ranges=0
duplicate_only: error=false ranges=0
callback_wrong_position: error=false ranges=0
malformed_outer: error=true ranges=0
direct_alias: error=false ranges=0
namespace_direct_alias: error=false ranges=0
shadow_sibling_reassignment: error=false ranges=1
positive_parameter_destructuring: error=false ranges=0
```

```text
malformed_argument: error=true ranges=0
malformed_callback: error=true ranges=0
nested_require_pattern: error=false ranges=0
selected_require_pattern: error=false ranges=0
cjs_namespace_assignment: error=false ranges=0
named_function_expression_shadow: error=false ranges=0
node_namespace_require: error=false ranges=1
```


## Remaining signature/default findings resolved (2026-09-14)

The final two re-review findings above are now implemented. Regression tests call
`analyse_file` in JS, JSX, TS and TSX and assert the extracted `subject.is_test`
value, plus an unaffected production sibling. The four exact invalid calls
`test('x', 'not options', cb)` (Vitest and Node), `test(42, cb)` (Node), and
`test('x', cb, 'not timeout')` (Jest) now yield `subject=false`. The two direct
CommonJS patterns `{test = fallback}` and `{test: check = fallback}` on
`require('vitest')` now yield `subject=true`. Nested property paths remain unknown.

Non-callback slots reject incompatible literal strings, numbers, objects, arrays,
booleans, null, regex and inline functions, including parenthesized literals.
Dynamic expression types are deliberately not inferred. Provider hook options
and numeric timeout slots use the same structural check. Vitest's documented
function-valued name remains supported without classifying the name function.
Valid Node callback-only, options/callback and name/options/callback overloads,
Jest/Vitest numeric timeouts and direct defaulted imports are positive controls.

Rechecked official [Vitest test signatures](https://vitest.dev/api/test),
[Vitest hook signatures](https://vitest.dev/api/hooks),
[Jest API](https://jestjs.io/docs/api) and
[Node test runner API](https://nodejs.org/api/test.html).
The new regressions first failed against the previous code (13 passing, 2 failing);
the focused JavaScript suite then passed all 15 tests. Full-suite and corpus gates
are recorded by the parent implementation review.

## Final scoped follow-up disposition

The two remaining concrete findings above are resolved. `callback_argument()` rejects the incompatible literal name, options, and timeout slots from the final re-review while preserving the covered Node callback-only and options-plus-callback overloads. Direct CommonJS shorthand and renamed destructuring defaults now resolve the exported property; nested property paths and ordinary value aliases retain their existing conservative treatment.

The focused integrated-library JavaScript suite passed: `CARGO_TARGET_DIR=/home/edouard/WS/barad-dur/target cargo test --lib test_context::javascript:: -- --nocapture` — 15 passed, 0 failed. Its new positive and negative extraction fixtures run across JS, JSX, TS, and TSX. No new finding arose within these final changed paths. This disposition is scoped to the concrete outstanding findings; full-suite, corpus, and mutation evidence is recorded separately.
