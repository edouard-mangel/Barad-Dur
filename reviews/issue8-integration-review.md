# Issue 8 integration and Rust review

## Findings

### High — a test function named `test` loses bare `#[test]` evidence

`src/metrics/complexity/test_context/rust.rs:103-104` records every
`function_item` name as an empty shadow binding. Resolution is file-wide and
does not distinguish the item being annotated, so the declaration of
`fn test` shadows its own built-in attribute. This violates the contract's
explicit bare-`#[test]` case and causes responsibility advice to include a
real test whenever its name also matches a grouping prefix.

An executable probe through `analyse_content` produced:

```text
#[test] fn test() {} fn prod() {}
[("test", false), ("prod", false)]

use tokio::test; #[test] fn test() {}
[("test", false)]
```

The first case must classify `test` as test evidence. The second must also do
so because its resolved imported attribute and the function declaration are
in different Rust namespaces; the declaration is not conflicting attribute
evidence. Add a regression test whose annotated function is literally named
`test`, then avoid treating ordinary function declarations as attribute-macro
shadow bindings (while retaining real macro-name conflicts).

### High — trait and union declarations do not shadow qualified providers

The same binding catalog includes `mod`, `struct`, `enum`, and `type`, but
omits `trait_item` and `union_item`. Both occupy the namespace used by the head
of a qualified attribute path. As a result, a local lookalike is treated as a
fully-qualified external provider even though the contract requires shadowed
bindings to stay unclassified.

The probe produced:

```text
trait tokio {} #[tokio::test] fn prod() {}
[("prod", true)]

union tokio { x: u8 } #[tokio::test] fn prod() {}
[("prod", true)]
```

Both flags must be false. Add `trait_item` and `union_item` to the declaration
shadow catalog and cover each with a negative test.

## Verified integration behavior

The shared integration keeps one parse in `analyse_source`, builds one merged
range index, and annotates every existing extracted function without changing
the extraction query or numeric measurements. Test paths remain authoritative,
including Go `_test.go`, while `analyse_content` intentionally has no path
evidence. `FunctionMetrics.is_test` defaults to false for legacy JSON, and the
cache version is incremented before decoding the changed positional payload.

The scorer filters `is_test` functions before group minimums, ranking, and the
five-action cap. The cold/warm/forced test checks the user-visible advice and
cache determinism. The assembly parity fixture now uses `class Fixture` in PHP;
the earlier `class Mixed` form was invalid because `mixed` is a reserved PHP
type name in the pinned grammar.

Focused validation after that fixture correction:

```text
cargo test --lib test_context -- --nocapture
36 passed; 0 failed
```

This passing suite does not cover the two Rust cases above. The temporary
review probe was removed after recording its output.

## Fix disposition and final integration sweep

Both Rust findings are resolved. Ordinary `function_item` declarations are no
longer entered as type/macro namespace shadows, while `trait_item` and
`union_item` declarations are. The regression covers both original positive
reproductions and both local-type negative reproductions:

```text
cargo test --lib function_value_names_do_not_shadow_attributes_but_local_types_do -- --nocapture
1 passed; 0 failed
```

The final shared-integration sweep found no further actionable defects in the
reviewed scope:

- `analyse_source` still parses once and passes the same tree to metrics,
  imports, coupling, inheritance/re-export, and call-edge extraction.
- `TestContext` merges overlapping lexical ranges and requires full function
  containment. Test-role paths contribute the root range; Go source files get
  no name/import heuristic.
- Function extraction and every numeric measurement remain intact. `is_test`
  is an annotation on each existing `FunctionMetrics` value and defaults to
  false during JSON deserialization.
- Responsibility grouping removes test functions before the two-member group
  minimum, ranking count, deterministic ordering, and five-result cap.
- Live, historical, standalone, and cached collection paths compare the same
  annotated `FileComplexity` values. Cache version 9 rejects version 8 before
  decoding the changed positional snapshot shape.
- The walking-skeleton test verifies the externally rendered advice on cold,
  warm, and forced collection while preserving category output.

Focused cache, collection-parity, and scorer regressions were also rerun; each
passed (`function_evidence_roundtrips_and_missing_json_evidence_defaults_to_unknown`,
`live_and_historical_ast_preserve_function_test_context`, and
`test_functions_are_removed_before_group_minimum_counts_and_ranking`).

JavaScript was excluded because its detector is being rewritten, and managed
language resolution was outside this re-review assignment.
