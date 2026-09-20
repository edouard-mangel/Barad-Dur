use crate::{metrics::complexity::analyse_file, snapshot::ResponsibilityDependency};
use std::path::Path;

fn callees(ext: &str, source: &str, name: &str) -> Vec<String> {
    analyse_file(Path::new(&format!("production.{ext}")), source)
        .functions
        .into_iter()
        .find(|f| f.name == name)
        .unwrap()
        .responsibility
        .unwrap()
        .dependencies
        .into_iter()
        .filter_map(|d| match d {
            ResponsibilityDependency::Callee { identity, .. } => Some(identity),
            _ => None,
        })
        .collect()
}

#[test]
fn empty_inner_type_shadows_outer_type_before_method_resolution() {
    for (ext, source) in [
        ("js", "class A { static helper() {} } function outer() { class A {} function is_a() { A.helper(); } }"),
        ("py", "class A:\n def helper(): pass\ndef outer():\n class A: pass\n def is_a(): A.helper()\n"),
    ] {
        assert!(callees(ext, source, "is_a").is_empty(), "{ext}");
    }
}

#[test]
fn type_qualification_requires_a_static_method() {
    for (ext, invalid, valid) in [
        (
            "js",
            "class A { helper() {} is_a() { A.helper(); } }",
            "class A { static helper() {} is_a() { A.helper(); } }",
        ),
        (
            "java",
            "class A { void helper() {} void is_a() { A.helper(); } }",
            "class A { static void helper() {} void is_a() { A.helper(); } }",
        ),
        (
            "cs",
            "class A { void helper() {} void is_a() { A.helper(); } }",
            "class A { static void helper() {} void is_a() { A.helper(); } }",
        ),
        (
            "php",
            "<?php class A { function helper() {} function is_a() { A::helper(); } }",
            "<?php class A { static function helper() {} function is_a() { A::helper(); } }",
        ),
    ] {
        assert!(callees(ext, invalid, "is_a").is_empty(), "{ext}");
        assert_eq!(callees(ext, valid, "is_a").len(), 1, "{ext}");
    }
}

#[test]
fn javascript_this_obeys_static_and_instance_method_lookup() {
    let source =
        "class A { static helper() {} is_a() { this.helper(); } static is_b() { this.helper(); } }";
    assert!(callees("js", source, "is_a").is_empty());
    assert_eq!(callees("js", source, "is_b").len(), 1);
}

#[test]
fn callees_outside_their_block_do_not_shadow_visible_functions() {
    for (ext, source) in [
        (
            "rs",
            "fn helper() {} fn is_a() { { fn helper() {} } helper(); }",
        ),
        (
            "js",
            "function helper() {} function is_a() { { function helper() {} } helper(); }",
        ),
    ] {
        assert_eq!(callees(ext, source, "is_a"), ["function:0"], "{ext}");
    }
    let source = "fn helper() {} fn is_a() { { fn helper() {} helper(); } }";
    assert_eq!(callees("rs", source, "is_a"), ["function:29"]);
}

#[test]
fn go_receiver_methods_can_call_the_other_receiver_mode_on_same_declaration() {
    for source in [
        "package p; type A struct {}; func (a *A) is_a() { a.helper() }; func (a A) helper() {}",
        "package p; type A struct {}; func (a A) is_a() { a.helper() }; func (a *A) helper() {}",
    ] {
        assert_eq!(callees("go", source, "is_a").len(), 1);
    }
}

#[test]
fn python_bare_calls_skip_class_members_and_resolve_module_functions() {
    let source =
        "def helper(): pass\nclass A:\n def helper(self): pass\n def is_a(self): helper()\n";
    assert_eq!(callees("py", source, "is_a"), ["function:0"]);
}

#[test]
fn anonymous_java_class_methods_are_visible_only_inside_the_class() {
    let source = "class A { void is_a() { Object x = new Object() { void helper() {} void is_b() { helper(); } }; helper(); } }";
    assert!(callees("java", source, "is_a").is_empty());
    assert_eq!(callees("java", source, "is_b").len(), 1);
}

#[test]
fn a_local_class_binding_shadows_an_outer_function() {
    assert!(callees(
        "py",
        "def helper(): pass\ndef is_a():\n class helper: pass\n helper()\n",
        "is_a"
    )
    .is_empty());
    assert!(callees(
        "js",
        "function helper() {} function is_a() { class helper {} helper(); }",
        "is_a"
    )
    .is_empty());
}

#[test]
fn type_resolution_does_not_cross_semicolon_namespaces() {
    let source = "<?php namespace First; class A { static function helper() {} } namespace Second; function is_a() { A::helper(); }";
    assert!(callees("php", source, "is_a").is_empty());
}

#[test]
fn javascript_static_and_instance_fields_have_distinct_identities() {
    let source = "class A { static is_a() { return this.ready; } is_b() { return this.ready; } }";
    let functions = analyse_file(Path::new("production.js"), source).functions;
    let first = functions[0].responsibility.as_ref().unwrap();
    let second = functions[1].responsibility.as_ref().unwrap();
    assert_eq!(first.owner_id, second.owner_id);
    assert_eq!(first.dependencies.len(), 1);
    assert_eq!(second.dependencies.len(), 1);
    assert_ne!(first.dependencies, second.dependencies);
}

#[test]
fn rust_lower_self_qualification_resolves_the_module_not_the_instance() {
    let absent = "struct A; impl A { fn helper(&self) {} fn is_a(&self) { self::helper(); } }";
    assert!(callees("rs", absent, "is_a").is_empty());
    let present = "fn helper() {} struct A; impl A { fn helper(&self) {} fn is_a(&self) { self::helper(); } }";
    assert_eq!(callees("rs", present, "is_a"), ["function:0"]);
}
