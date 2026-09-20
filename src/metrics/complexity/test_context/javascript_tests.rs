use super::*;

fn detected(source: &str) -> Vec<String> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_typescript::LANGUAGE_TSX.into())
        .unwrap();
    let tree = parser.parse(source, None).unwrap();
    ranges(tree.root_node(), source.as_bytes())
        .into_iter()
        .map(|range| source[range].to_string())
        .collect()
}

#[test]
fn detects_named_alias_namespace_require_and_documented_chains() {
    let source = r#"
import { test as check, describe } from 'vitest';
import * as node from 'node:test';
const { beforeEach: setup } = require('@jest/globals');
const mocha = require('mocha');
check.concurrent.skip.each([[1]])('case', async () => {});
describe.only('suite', function suiteBody() {});
node.it.skip('node', () => {});
setup(() => {});
mocha.before(function () {});
"#;
    let found = detected(source);
    assert_eq!(found.len(), 5, "{found:#?}");
    assert!(found.iter().all(|callback| !callback.is_empty()));
}

#[test]
fn ignores_names_unknown_providers_shadowing_reassignment_and_conflicts() {
    let source = r#"
import { test } from 'vitest';
import { test } from './lookalike';
import * as wildcard from './helpers';
function shadow(test) { test('shadowed', () => {}); }
test = fake;
test('reassigned', () => {});
describe('global', () => {});
wildcard.test('wrong provider', () => {});
// test('comment', () => {});
const text = "test('string', () => {})";
"#;
    assert!(detected(source).is_empty());

    let conflicting = r#"
import { test as check } from 'vitest';
import { test as check } from '@jest/globals';
check('ambiguous', () => {});
"#;
    assert!(detected(conflicting).is_empty());
}

#[test]
fn enclosing_callback_inherits_nested_functions_but_not_referenced_helpers() {
    let source = r#"
import { test } from 'vitest';
import nodeDefault from 'node:test';
function outside() {}
test('case', () => { function nested() {} });
test('reference', outside);
nodeDefault('default', () => {});
"#;
    let found = detected(source);
    assert_eq!(found.len(), 2);
    assert!(found[0].contains("function nested"));
    assert!(!found[0].contains("outside"));
}

#[test]
fn supports_static_commonjs_property_and_node_default_bindings() {
    let source = r#"
const check = require('vitest').test;
const nodeTest = require('node:test');
check('vitest', () => {});
nodeTest.only('node', function () {});
"#;
    let found = detected(source);
    assert_eq!(found.len(), 2, "{found:#?}");
}

#[test]
fn malformed_comments_and_strings_do_not_create_evidence() {
    let source = "import { test } from 'vitest';\nconst s = `test('string', () => {})`; // describe('comment', () => {})\nbroken test('x', () => {}";
    assert!(detected(source).is_empty());
}

fn extracted(source: &str, extension: &str, expected: bool) {
    use crate::metrics::complexity::analyse_file;
    use std::path::Path;
    let content = format!("{source}\nfunction productionSibling() {{ if (true) {{}} }}");
    let metrics = analyse_file(Path::new(&format!("src/production.{extension}")), &content);
    assert!(!metrics.functions.is_empty(), "{extension}: {source}");
    let subject = metrics
        .functions
        .iter()
        .find(|f| f.name == "subject")
        .unwrap_or_else(|| panic!("subject not extracted: {extension}: {source}"));
    assert_eq!(subject.is_test, expected, "{extension}: {source}");
    let sibling = metrics
        .functions
        .iter()
        .find(|f| f.name == "productionSibling")
        .unwrap();
    assert!(
        !sibling.is_test,
        "production sibling: {extension}: {source}"
    );
    assert_eq!(sibling.cyclomatic_complexity, 1);
}

#[test]
fn runtime_scopes_reject_every_reviewed_shadow_and_reassignment() {
    for extension in ["js", "jsx", "ts", "tsx"] {
        for source in [
            "import {test} from 'vitest'; const C = class test { method() { test('x', () => { function subject() {} }); } };",
            "const v = require('vitest'); delete v.test; v.test('x', () => { function subject() {} });",
            "const {test} = require('vitest'); with (other) { test('x', () => { function subject() {} }); }",
            "import {test} from 'vitest'; function f() { for (var test of values) {} test('x', () => { function subject() {} }); }",
            "let {test} = require('vitest'); for (test of values) {} test('x', () => { function subject() {} });",
            "const v = require('vitest'); ({handler: v.test} = other); v.test('x', () => { function subject() {} });",
            "function factory() { const {test} = require('vitest'); } test('x', () => { function subject() {} });",
            "function factory(require) { const {test} = require('vitest'); test('x', () => { function subject() {} }); }",
            "import {test} from 'vitest'; const f = test => test('x', () => { function subject() {} });",
            "import {test} from 'vitest'; function f() { test('x', () => { function subject() {} }); function test() {} }",
            "import {test} from 'vitest'; function f() { test('x', () => { function subject() {} }); const test = other; }",
            "import {test} from 'vitest'; try {} catch(test) { test('x', () => { function subject() {} }); }",
            "let {test} = require('vitest'); ({test} = other); test('x', () => { function subject() {} });",
            "const v = require('vitest'); v.test = fake; v.test('x', () => { function subject() {} });",
            "import {test} from 'vitest'; const f = function test() { test('x', () => { function subject() {} }); };",
            "const {anything: {test}} = require('vitest'); test('x', () => { function subject() {} });",
            "const {test} = require('vitest').anything; test('x', () => { function subject() {} });",
            "import {test} from 'vitest'; function f({test}) { test('x', () => { function subject() {} }); }",
            "import {test} from 'vitest'; function f({name: test}) { test('x', () => { function subject() {} }); }",
            "import {test} from 'vitest'; function f(test = fake) { test('x', () => { function subject() {} }); }",
            "import {test} from 'vitest'; for (const test of values) { test('x', () => { function subject() {} }); }",
            "function f() { const {test} = require('vitest'); } function g() { const test = fake; test('x', () => { function subject() {} }); }",
            "const {test} = require('vitest'); require = fake; test('x', () => { function subject() {} });",
        ] {
            extracted(source, extension, false);
        }
    }
}

#[test]
fn type_only_imports_do_not_establish_runtime_evidence() {
    for extension in ["ts", "tsx"] {
        for source in [
            "import type {test} from 'vitest'; test('x', () => { function subject() {} });",
            "import {type test} from 'vitest'; test('x', () => { function subject() {} });",
            "import type * as v from 'vitest'; v.test('x', () => { function subject() {} });",
            "import type test from 'node:test'; test('x', () => { function subject() {} });",
        ] {
            extracted(source, extension, false);
        }
    }
}

#[test]
fn callbacks_require_documented_call_shapes() {
    for extension in ["js", "jsx", "ts", "tsx"] {
        for source in [
            "import {test} from 'vitest'; test('first')('x', () => { function subject() {} });",
            "import {test} from 'vitest'; test.each('x', () => { function subject() {} });",
            "import {test} from '@jest/globals'; test.only.only('x', () => { function subject() {} });",
            "import {test} from '@jest/globals'; test.only.concurrent('x', () => { function subject() {} });",
            "import {test} from 'vitest'; test(() => { function subject() {} }, 'not a callback');",
            "import {test} from 'vitest'; test.each([]).only('x', () => { function subject() {} });",
            "import {test} from 'vitest'; test.each([])('x')('again', () => { function subject() {} });",
            "import {beforeEach} from '@jest/globals'; beforeEach.only(() => { function subject() {} });",
            "import {test} from 'node:test'; test.concurrent('x', () => { function subject() {} });",
        ] {
            extracted(source, extension, false);
        }
    }
}

#[test]
fn malformed_call_evidence_is_rejected_without_losing_extraction() {
    for extension in ["js", "jsx", "ts", "tsx"] {
        for source in [
            "import {test} from 'vitest'; test('x', ???, () => { function subject() {} });",
            "import {test} from 'vitest'; test('x', () => { function subject() {} broken @@@ });",
        ] {
            extracted(source, extension, false);
        }
    }
}

#[test]
fn valid_evidence_preserves_binding_identity_and_provider_forms() {
    for extension in ["js", "jsx", "ts", "tsx"] {
        for source in [
            "import {test} from 'vitest'; function f(test) { test = other; } test('x', () => { function subject() {} });",
            "function f() { const {test} = require('vitest'); test('x', () => { function subject() {} }); } function g(test) { test = other; }",
            "const node = require('node:test'); node.test('x', () => { function subject() {} });",
            "const node = require('node:test'); node('x', () => { function subject() {} });",
            "const node = require('node:test'); node.before(() => { function subject() {} });",
            "import * as node from 'node:test'; node.test('x', {}, () => { function subject() {} });",
            "import {test} from 'node:test'; test(() => { function subject() {} });",
            "import {test} from 'node:test'; test({}, () => { function subject() {} });",
            "import {test as check} from '@jest/globals'; check.concurrent.only.each([[1]])('x', () => { function subject() {} });",
            "import * as v from 'vitest'; v.test.skip.concurrent.each([[1]])('x', () => { function subject() {} });",
            "const {before: setup} = require('mocha'); setup('named hook', () => { function subject() {} });",
            "const check = require('vitest').test; check('x', {timeout: 1}, () => { function subject() {} });",
            "import {test} from 'vitest'; function f({other = test}) {} test('x', () => { function subject() {} });",
        ] {
            extracted(source, extension, true);
        }
    }
}

#[test]
fn only_lexical_descendants_inherit_the_callback_scope() {
    for extension in ["js", "jsx", "ts", "tsx"] {
        extracted(
            "import {test} from 'vitest'; function subject() {} test('x', subject);",
            extension,
            false,
        );
        extracted(
            "import {test} from 'vitest'; const alias = test; alias('x', () => { function subject() {} });",
            extension,
            false,
        );
        extracted(
            "import {test} from 'vitest'; test('x', () => { function subject() {} subject(); });",
            extension,
            true,
        );
    }
}

#[test]
fn recognizing_provider_preserves_all_extracted_measurements() {
    use crate::metrics::complexity::analyse_file;
    use std::path::Path;
    for extension in ["js", "jsx", "ts", "tsx"] {
        let source = "import {test} from 'vitest'; test('x', () => { function subject() { if (true) {} } }); function sibling() {}";
        let path = format!("src/production.{extension}");
        let recognized = analyse_file(Path::new(&path), source);
        let unknown = analyse_file(Path::new(&path), &source.replace("vitest", "custom"));
        assert_eq!(recognized.functions.len(), 2);
        assert_eq!(recognized.functions.len(), unknown.functions.len());
        for (left, right) in recognized.functions.iter().zip(&unknown.functions) {
            assert_eq!(
                (
                    &left.name,
                    left.loc,
                    left.cyclomatic_complexity,
                    left.max_nesting_depth
                ),
                (
                    &right.name,
                    right.loc,
                    right.cyclomatic_complexity,
                    right.max_nesting_depth
                )
            );
        }
        assert!(
            recognized
                .functions
                .iter()
                .find(|f| f.name == "subject")
                .unwrap()
                .is_test
        );
        assert!(unknown.functions.iter().all(|f| !f.is_test));
    }
}

#[test]
fn typescript_runtime_names_and_namespace_var_scope_are_resolved() {
    for extension in ["ts", "tsx"] {
        for source in [
            "import {test} from 'vitest'; function f() { enum test {A} test('x', () => { function subject() {} }); }",
            "namespace N { var test = require('vitest').test; } test('x', () => { function subject() {} });",
            "import {test} from 'vitest'; function f() { namespace test { export const value = 1; } test('x', () => { function subject() {} }); }",
            "import type v = require('vitest'); v.test('x', () => { function subject() {} });",
        ] {
            extracted(source, extension, false);
        }
        extracted(
            "import v = require('vitest'); v.test('x', () => { function subject() {} });",
            extension,
            true,
        );
        extracted(
            "import node = require('node:test'); node('x', () => { function subject() {} });",
            extension,
            true,
        );
    }
}

#[test]
fn invalid_literal_signature_slots_do_not_classify_extracted_functions() {
    for extension in ["js", "jsx", "ts", "tsx"] {
        for (provider, call) in [
            ("vitest", "test('x', 'not options', BODY)"),
            ("node:test", "test('x', 'not options', BODY)"),
            ("node:test", "test(42, BODY)"),
            ("@jest/globals", "test('x', BODY, 'not timeout')"),
            ("vitest", "test('x', BODY, {})"),
            ("node:test", "test('x', [], BODY)"),
        ] {
            let source = format!(
                "import {{test}} from '{provider}'; {};",
                call.replace("BODY", "() => { function subject() {} }")
            );
            extracted(&source, extension, false);
        }
    }
}

#[test]
fn valid_signature_slots_and_defaulted_commonjs_bindings_classify_extraction() {
    for extension in ["js", "jsx", "ts", "tsx"] {
        for source in [
            "const {test = fallback} = require('vitest'); test('x', () => { function subject() {} });",
            "const {test: check = fallback} = require('vitest'); check('x', () => { function subject() {} });",
            "import {test} from '@jest/globals'; test('x', () => { function subject() {} }, 1000);",
            "import {test} from 'vitest'; test('x', () => { function subject() {} }, 1000);",
            "import {test} from 'vitest'; test('x', {timeout: 1000}, () => { function subject() {} });",
            "import {test} from 'node:test'; test('x', {timeout: 1000}, () => { function subject() {} });",
            "import {test} from 'node:test'; test({timeout: 1000}, () => { function subject() {} });",
            "import {test} from 'node:test'; test(() => { function subject() {} });",
            "import {test} from 'vitest'; test(function named() {}, () => { function subject() {} });",
            "import {beforeEach} from '@jest/globals'; beforeEach(() => { function subject() {} }, 1000);",
            "import {beforeEach} from 'node:test'; beforeEach(() => { function subject() {} }, {timeout: 1000});",
        ] {
            extracted(source, extension, true);
        }
    }
}

#[test]
fn provider_bindings_survive_reads_and_unrelated_scopes() {
    for extension in ["js", "jsx", "ts", "tsx"] {
        for source in [
            "const {test} = require('vitest'); typeof test; test('x', () => { function subject() {} });",
            "const {test} = require('vitest'); unrelated = 1; test('x', () => { function subject() {} });",
            "const {test} = require('vitest'); function f(require) { require = other; } test('x', () => { function subject() {} });",
            "function f() { { var test = require('vitest').test; } test('x', () => { function subject() {} }); }",
            "import {test} from 'vitest'; for (let test of values) {} test('x', () => { function subject() {} });",
            "const {it, test: beforeEach} = require('vitest'); beforeEach('x', () => { function subject() {} });",
            "import {test} from 'mocha'; test('x', () => { function subject() {} });",
        ] {
            extracted(source, extension, true);
        }
    }
}

#[test]
fn changed_bindings_and_nonprovider_loaders_remain_eligible() {
    for extension in ["js", "jsx", "ts", "tsx"] {
        for source in [
            "let test = require('vitest').test; test++; test('x', () => { function subject() {} });",
            "function f() { var test = require('vitest').test; } test('x', () => { function subject() {} });",
            "const v = require('vitest'); v('x', () => { function subject() {} });",
            "const {test: check, expect: verify} = require('vitest'); verify('x', () => { function subject() {} });",
            "const {test} = other('vitest'); test('x', () => { function subject() {} });",
            "const v = require('vitest'); ({handler: v.test = fake} = other); v.test('x', () => { function subject() {} });",
        ] {
            extracted(source, extension, false);
        }
    }
    for extension in ["ts", "tsx"] {
        extracted(
            "import {test} from 'vitest'; namespace N { import test = custom.test; test('x', () => { function subject() {} }); }",
            extension,
            false,
        );
    }
}

#[test]
fn tagged_template_each_tables_are_test_callbacks() {
    for extension in ["js", "jsx", "ts", "tsx"] {
        for source in [
            "import {test} from 'vitest'; test.each`\n a | b\n ${1} | ${2}\n`('x', () => { function subject() {} });",
            "import {describe} from 'vitest'; describe.each`\n a\n ${1}\n`('x', () => { function subject() {} });",
            "import {it} from '@jest/globals'; it.only.each`a\n${1}`('x', () => { function subject() {} });",
            "import {test as check} from '@jest/globals'; check.concurrent.each`a\n${1}`('x', () => { function subject() {} });",
        ] {
            extracted(source, extension, true);
        }
    }
}

#[test]
fn tagged_template_each_tables_keep_the_single_table_rule() {
    for extension in ["js", "jsx", "ts", "tsx"] {
        extracted(
            "import {test} from 'vitest'; test.each`a\n${1}`('x')('again', () => { function subject() {} });",
            extension,
            false,
        );
    }
}

#[test]
fn invalid_each_arity_and_modifier_combinations_remain_eligible() {
    for extension in ["js", "jsx", "ts", "tsx"] {
        for source in [
            "import {test} from 'vitest'; test(...names, () => { function subject() {} });",
            "import {test} from 'vitest'; test.each()('x', () => { function subject() {} });",
            "import {test} from 'vitest'; test.each([], [])('x', () => { function subject() {} });",
            "import {test} from 'vitest'; test.unknown('x', () => { function subject() {} });",
            "import {test} from 'vitest'; test.only.only('x', () => { function subject() {} });",
            "import {test} from 'vitest'; test.only.skip('x', () => { function subject() {} });",
        ] {
            extracted(source, extension, false);
        }
    }
}

#[test]
fn provider_calls_without_callbacks_do_not_prevent_later_evidence() {
    for extension in ["js", "jsx", "ts", "tsx"] {
        for provider in ["node:test", "mocha", "vitest", "@jest/globals"] {
            let source = format!("import {{test, beforeEach}} from '{provider}'; test(); beforeEach(); test('x', () => {{ function subject() {{}} }});");
            extracted(&source, extension, true);
        }
    }
}
