# Issue 8 JavaScript/TypeScript grammar evidence

Pinned grammars: `tree-sitter-javascript 0.25.0`, `tree-sitter-typescript 0.23.2` (through tree-sitter 0.27). The probe parsed the samples with `LANGUAGE_TYPESCRIPT`; TSX uses the same relevant node shapes. Command:

```sh
CARGO_TARGET_DIR=/home/edouard/WS/barad-dur/target cargo run --example issue8_js_probe
```

## Imports and static bindings

Input:

```ts
import def, { test as check, describe } from 'vitest';
import * as node from 'node:test';
const { it: spec, beforeEach } = require('@jest/globals');
const mocha = require('mocha');
```

Relevant parse output:

```text
(import_statement (import_clause (identifier) (named_imports
  (import_specifier name: (identifier) alias: (identifier))
  (import_specifier name: (identifier)))) source: (string (string_fragment)))
(import_statement (import_clause (namespace_import (identifier)))
  source: (string (string_fragment)))
(variable_declarator name: (object_pattern
  (pair_pattern key: (property_identifier) value: (identifier))
  (shorthand_property_identifier_pattern))
  value: (call_expression function: (identifier)
    arguments: (arguments (string (string_fragment)))))
(variable_declarator name: (identifier)
  value: (call_expression function: (identifier)
    arguments: (arguments (string (string_fragment)))))
```

This proves named aliases, namespace imports, CommonJS destructuring, and namespace/default CommonJS bindings can be resolved structurally without scanning source text.

## Chained and call-form APIs, callbacks, descendants

Input:

```ts
check.each([[1]])('x', async function named() { function helper() {} });
node.describe.only('s', () => { node.it('x', function () {}); });
```

Relevant parse output:

```text
(call_expression
  function: (call_expression
    function: (member_expression object: (identifier) property: (property_identifier))
    arguments: (arguments (array (array (number)))))
  arguments: (arguments (string (string_fragment))
    (function_expression name: (identifier) parameters: (formal_parameters)
      body: (statement_block (function_declaration ...)))))
(call_expression
  function: (member_expression
    object: (member_expression object: (identifier) property: (property_identifier))
    property: (property_identifier))
  arguments: (arguments (string (string_fragment)) (arrow_function ...)))
```

The outer call owns the callback for `.each(table)(name, callback)`. Returning the callback's byte range also makes nested named functions inherit the context while a separately declared helper passed by identifier has no callback range.

## Shadowing and negative syntax

Input:

```ts
function f(test) { test('x', () => {}); }
const test = fake;
test('x', () => {});
// test('x', () => {})
const s = "describe('x', () => {})";
broken test('x', () => {}
```

Relevant parse output:

```text
(function_declaration name: (identifier)
  parameters: (formal_parameters (required_parameter pattern: (identifier))) ...)
(lexical_declaration (variable_declarator name: (identifier) value: (identifier)))
(comment)
(lexical_declaration (variable_declarator name: (identifier) value: (string (string_fragment))))
(ERROR (identifier) (identifier) (string (string_fragment)) (arrow_function ...))
```

Parameters and declaration patterns provide structural shadow evidence. Comments and strings are distinct nodes, and the malformed invocation is under `ERROR`; none is treated as a valid imported API call.

## Provider catalog references

- Jest documents explicit imports from `@jest/globals`, hooks, `test`/`it`/`describe`, and the supported `.only`, `.skip`, `.concurrent`, and call-form `.each` combinations: https://jestjs.io/docs/api
- Vitest documents explicit imports and chained `skip`, `only`, `concurrent`, and `each` options: https://vitest.dev/api/test and https://vitest.dev/guide/parallelism.html
- Mocha documents the BDD callbacks and hooks (`describe`, `it`, `before`, `after`, `beforeEach`, `afterEach`): https://mochajs.org/interfaces/bdd/ and https://mochajs.org/features/hooks/
- Node documents ESM/CommonJS imports, `test`/`it`, `suite`/`describe`, hooks, and `.only`/`.skip`: https://nodejs.org/api/test.html


## P1 scoped-binding and call-shape revision

The review in `issue8-javascript-review.md` found false evidence in seven families and recorded complete executable AST probes. The revised implementation builds declaration identities once and indexes them by lexical scope. Unknown locals and type-only imports participate in lookup, preventing fallback to an imported spelling. Reassignment invalidates the resolved declaration, including destructuring and namespace property writes; assignments to shadow parameters leave the outer import intact. Static CommonJS binding initialization additionally requires an unshadowed, unreassigned loader. `var` uses function scope, lexical declarations use block/loop scope, and function/arrow/catch parameters and function-expression self-names establish local identities.

The pinned grammar distinguishes single arrow/catch `parameter` from plural `parameters`; TS `type` modifiers are anonymous tokens on import statements/specifiers; destructuring patterns use `pair_pattern`/`object_pattern`; default values are excluded from bound names; `for_in_statement` contains a direct `left` binding instead of `variable_declarator`. These distinctions are covered through actual extraction in JS, JSX, TS and TSX.

Calls retain the `.each` application boundary. Only the single documented table call followed by the test-definition call is recognized; arbitrary returned calls, repeated applications, and uncalled `.each` stay unknown. Jest uses an explicit order-sensitive combination catalog; Vitest allows distinct only/skip/concurrent options before a final applied each, with only/skip conflict rejected; Node/Mocha allow only/skip on test/suite APIs. Hook modifiers remain unsupported. Callback positions differ between hooks, test/suite definitions, Node optional arguments, Vitest options-before-body, and Mocha named hooks. Error-bearing evidence calls and ERROR ancestors are rejected.

Scope clarification from the accepted implementation contract: direct **import** aliases and static CommonJS property/destructuring bindings are supported. Ordinary value aliases (`const check = test`, `const check = ns.test`), alias chains, computed-property calls and runtime wrapper inference stay unknown. Node's default CommonJS export is represented as both callable test and a namespace of its exported APIs.

Validation: `cargo test --lib test_context::javascript -- --nocapture` passed 13 tests, including the seven review failure categories across `.js`, `.jsx`, `.ts`, `.tsx`, positive Node/Jest/Vitest/Mocha forms, production siblings, named descendant versus referenced helper behavior, and numeric metric parity between known and unknown providers. The full-suite and field gates remain integration-task responsibilities.

Additional scope checks cover named class expressions, catch/loop bindings, function-scoped `var` inside loops, destructured namespace-member writes, namespace-member deletion, and dynamic `with` scopes (unknown). TypeScript runtime enum/namespace declarations shadow imports, and `var` inside a namespace stays in that namespace. Static TypeScript `import name = require('provider')` resolves the documented CommonJS namespace/callable forms; its type-only counterpart remains unknown. Plain TypeScript `import alias = identifier.path` is an unresolved alias rather than a new provider proof.

The final bounded JavaScript suite passed 13 tests. `cargo clippy --lib -- -D warnings` passed after the final TS-specific additions; full-workspace validation is performed by the integrating task.
