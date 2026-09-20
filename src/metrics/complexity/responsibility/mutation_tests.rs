use crate::metrics::complexity::analyse_file;
use crate::snapshot::{FunctionMetrics, ResponsibilityDependency, ResponsibilityProvenance};
use std::path::Path;

fn functions(ext: &str, source: &str) -> Vec<FunctionMetrics> {
    analyse_file(Path::new(&format!("src/production.{ext}")), source).functions
}

fn named<'a>(functions: &'a [FunctionMetrics], name: &str) -> &'a ResponsibilityProvenance {
    functions
        .iter()
        .find(|function| function.name == name)
        .unwrap_or_else(|| panic!("missing {name}"))
        .responsibility
        .as_ref()
        .unwrap_or_else(|| panic!("unknown owner for {name}"))
}

fn field_names(provenance: &ResponsibilityProvenance) -> Vec<&str> {
    provenance
        .dependencies
        .iter()
        .filter_map(|dependency| match dependency {
            ResponsibilityDependency::Field { label, .. } => Some(label.as_str()),
            _ => None,
        })
        .collect()
}

fn callee_names(provenance: &ResponsibilityProvenance) -> Vec<&str> {
    provenance
        .dependencies
        .iter()
        .filter_map(|dependency| match dependency {
            ResponsibilityDependency::Callee { label, .. } => Some(label.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn python_method_replacement_outside_the_class_blocks_static_callee_evidence() {
    let functions = functions(
        "py",
        "class A:\n def helper(self): pass\n def is_a(self): self.helper()\ndef reset():\n A.helper = external\n",
    );

    assert!(named(&functions, "is_a").dependencies.is_empty());
}

#[test]
fn php_functions_belong_to_the_preceding_semicolon_namespace() {
    let functions = functions(
        "php",
        "<?php namespace First; function get_a() {} namespace Second; function get_b() {}",
    );

    assert!(named(&functions, "get_a")
        .owner_label
        .starts_with("First (line "));
    assert!(named(&functions, "get_b")
        .owner_label
        .starts_with("Second (line "));
}

#[test]
fn a_free_function_reports_the_file_as_its_owner() {
    let functions = functions("rs", "fn is_a() {}");

    assert_eq!(named(&functions, "is_a").owner_label, "file");
}

#[test]
fn anonymous_class_arguments_belong_to_the_caller_but_its_body_does_not() {
    let functions = functions(
        "java",
        "class Container { boolean nested; Container(boolean ready) {} } class A { boolean ready; void is_a() { Container value = new Container(this.ready) { boolean copy = this.nested; }; } }",
    );

    assert_eq!(field_names(named(&functions, "is_a")), ["ready"]);
}

#[test]
fn a_later_python_import_replaces_a_same_name_local_callee() {
    let functions = functions(
        "py",
        "def helper(): pass\nfrom external import helper\ndef is_a(): helper()\n",
    );

    assert!(named(&functions, "is_a").dependencies.is_empty());
}

#[test]
fn a_kotlin_body_property_supplies_implicit_field_evidence() {
    let functions = functions(
        "kt",
        "class A {\n val ready = true\n fun is_a(): Boolean { return ready }\n}\n",
    );

    assert_eq!(field_names(named(&functions, "is_a")), ["ready"]);
}

#[test]
fn a_kotlin_companion_property_does_not_become_an_instance_field() {
    let functions = functions(
        "kt",
        "class A {\n companion object {\n val ready = true\n }\n fun is_a(): Boolean { return ready }\n}\n",
    );

    assert!(named(&functions, "is_a").dependencies.is_empty());
}

#[test]
fn a_go_type_method_expression_resolves_its_value_receiver_method() {
    let functions = functions(
        "go",
        "package p; type A struct {}; func (a A) helper() {}; func is_a(a A) { A.helper(a) }",
    );

    assert_eq!(callee_names(named(&functions, "is_a")), ["helper"]);
}

#[test]
fn a_nested_python_function_shadows_a_same_name_type() {
    let functions = functions(
        "py",
        "class A:\n @staticmethod\n def helper(): pass\ndef outer():\n def A(): pass\n def is_a(): A.helper()\n",
    );

    assert!(named(&functions, "is_a").dependencies.is_empty());
}

#[test]
fn an_anonymous_object_owner_reports_a_readable_kind_and_source_line() {
    let functions = functions("js", "const owner = { is_a() {} };\n");

    assert_eq!(
        named(&functions, "is_a").owner_label,
        "object literal (line 1)"
    );
}

#[test]
fn anonymous_owners_never_leak_grammar_kinds_into_their_labels() {
    for (ext, source, name, label) in [
        (
            "js",
            "(function () {\n  function get_a() {}\n})();\n",
            "get_a",
            "anonymous function (line 1)",
        ),
        (
            "js",
            "const run = () => {\n  function get_a() {}\n};\n",
            "get_a",
            "anonymous function (line 1)",
        ),
        (
            "kt",
            "class A {\n companion object {\n fun get_a() {}\n }\n}\n",
            "get_a",
            "companion object (line 2)",
        ),
    ] {
        assert_eq!(
            named(&functions(ext, source), name).owner_label,
            label,
            "{ext}: {source}"
        );
    }
}

#[test]
fn multi_line_owner_type_text_collapses_to_single_spaces() {
    let functions = functions("rs", "struct Cache<K, V>(K, V);\nimpl<K, V> Cache<\n    K,\n    V,\n> {\n    fn get_a(&self) {}\n}\n");

    assert_eq!(
        named(&functions, "get_a").owner_label,
        "Cache< K, V, > (line 2)"
    );
}

#[test]
fn same_named_go_methods_on_other_types_do_not_make_a_receiver_call_ambiguous() {
    let functions = functions(
        "go",
        "package p; type A struct {}; type B struct {}; func (a A) helper() {}; func (b B) helper() {}; func (a A) is_a() { a.helper() }",
    );

    assert_eq!(callee_names(named(&functions, "is_a")), ["helper"]);
}

#[test]
fn self_type_calls_resolve_only_methods_on_the_current_owner() {
    for (ext, source) in [
        ("rs", "struct A; struct B; impl A { fn helper() {} fn is_a() { Self::helper(); } } impl B { fn helper() {} }"),
        ("php", "<?php class A { static function helper() {} static function is_a() { self::helper(); } } class B { static function helper() {} }"),
    ] {
        let functions = functions(ext, source);
        let declaration = if ext == "rs" {
            "fn helper"
        } else {
            "static function helper"
        };
        assert_eq!(
            named(&functions, "is_a").dependencies,
            [ResponsibilityDependency::Callee {
                identity: format!("function:{}", source.find(declaration).unwrap()),
                label: "helper".to_owned(),
            }],
            "{ext}"
        );
    }
}

#[test]
fn explicit_other_type_calls_do_not_resolve_on_the_current_owner() {
    for (ext, source) in [
        ("rs", "struct A; struct B; impl A { fn is_a() { B::helper(); } } impl B { fn helper() {} }"),
        ("php", "<?php class A { static function is_a() { B::helper(); } } class B { static function helper() {} }"),
    ] {
        let functions = functions(ext, source);
        assert_eq!(callee_names(named(&functions, "is_a")), ["helper"], "{ext}");
    }
}

#[test]
fn a_php_nested_function_resolves_inside_a_semicolon_namespace() {
    let functions = functions(
        "php",
        "<?php namespace One; function is_a() { function helper() {} helper(); }",
    );

    assert_eq!(callee_names(named(&functions, "is_a")), ["helper"]);
}

#[test]
fn a_bare_php_call_resolves_the_current_semicolon_namespace() {
    let functions = functions(
        "php",
        "<?php namespace One; function helper() {} function is_a() { helper(); } namespace Two; function unrelated() {}",
    );

    assert_eq!(callee_names(named(&functions, "is_a")), ["helper"]);
}

#[test]
fn separate_partial_type_declarations_make_type_qualified_calls_unknown() {
    let functions = functions(
        "cs",
        "partial class A { public static void helper() {} } partial class A {} class B { void is_a() { A.helper(); } }",
    );

    assert!(named(&functions, "is_a").dependencies.is_empty());
}

#[test]
fn implicit_method_calls_do_not_mix_same_named_methods_from_other_classes() {
    let functions = functions(
        "java",
        "class A { void helper() {} void is_a() { helper(); } } class B { void helper() {} }",
    );

    assert_eq!(callee_names(named(&functions, "is_a")), ["helper"]);
}

#[test]
fn an_anonymous_class_constructor_argument_resolves_the_callers_implicit_method() {
    let functions = functions(
        "java",
        "class Container { Container(boolean value) {} } class A { boolean helper() { return true; } void is_a() { Container value = new Container(helper()) {}; } }",
    );

    assert_eq!(callee_names(named(&functions, "is_a")), ["helper"]);
}

#[test]
fn rust_branch_local_functions_do_not_shadow_calls_in_the_condition() {
    let functions = functions(
        "rs",
        "fn helper() -> bool { true } fn is_a() { if helper() { fn helper() -> bool { false } } }",
    );

    assert_eq!(
        named(&functions, "is_a").dependencies,
        [ResponsibilityDependency::Callee {
            identity: "function:0".to_owned(),
            label: "helper".to_owned(),
        }]
    );
}

#[test]
fn visibility_modifiers_do_not_make_an_instance_method_callable_through_a_type() {
    for (ext, source) in [
        (
            "java",
            "class A { public void helper() {} } class B { void is_a() { A.helper(); } }",
        ),
        (
            "cs",
            "class A { public void helper() {} } class B { void is_a() { A.helper(); } }",
        ),
    ] {
        let functions = functions(ext, source);
        assert!(named(&functions, "is_a").dependencies.is_empty(), "{ext}");
    }
}

#[test]
fn a_rust_module_keeps_its_free_functions_in_the_local_call_scope() {
    let functions = functions("rs", "mod inner { fn helper() {} fn is_a() { helper(); } }");

    assert_eq!(callee_names(named(&functions, "is_a")), ["helper"]);
}
