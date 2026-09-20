use super::*;
use crate::metrics::complexity::{analyse_file, analyse_source};
use crate::snapshot::FunctionMetrics;
use std::path::Path;

fn functions(ext: &str, source: &str) -> Vec<FunctionMetrics> {
    let path = format!("src/production.{ext}");
    let standalone = analyse_file(Path::new(&path), source);
    assert_eq!(standalone, analyse_source(Path::new(&path), source).metrics);
    assert!(
        !standalone.functions.is_empty(),
        "empty extraction for {ext}"
    );
    standalone.functions
}

fn named<'a>(functions: &'a [FunctionMetrics], name: &str) -> &'a ResponsibilityProvenance {
    functions
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("missing {name}"))
        .responsibility
        .as_ref()
        .unwrap_or_else(|| panic!("unknown owner for {name}"))
}

fn field_names(provenance: &ResponsibilityProvenance) -> Vec<&str> {
    provenance
        .dependencies
        .iter()
        .filter_map(|d| match d {
            ResponsibilityDependency::Field { label, .. } => Some(label.as_str()),
            _ => None,
        })
        .collect()
}

fn callee_names(provenance: &ResponsibilityProvenance) -> Vec<&str> {
    provenance
        .dependencies
        .iter()
        .filter_map(|d| match d {
            ResponsibilityDependency::Callee { label, .. } => Some(label.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn every_extension_extracts_same_owner_direct_fields_and_callees() {
    for (ext, source) in [
        ("rs", "struct A { ready: bool } impl A { fn is_a(&self) { self.ready; self.helper(); } fn is_b(&self) { self.ready; self.helper(); } fn helper(&self) {} }"),
        ("js", "class A { is_a() { this.ready; this.helper(); } is_b() { this.ready; this.helper(); } helper() {} }"),
        ("jsx", "class A { is_a() { this.ready; this.helper(); } is_b() { this.ready; this.helper(); } helper() {} }"),
        ("mjs", "class A { is_a() { this.ready; this.helper(); } is_b() { this.ready; this.helper(); } helper() {} }"),
        ("cjs", "class A { is_a() { this.ready; this.helper(); } is_b() { this.ready; this.helper(); } helper() {} }"),
        ("ts", "class A { is_a() { this.ready; this.helper(); } is_b() { this.ready; this.helper(); } helper() {} }"),
        ("tsx", "class A { is_a() { this.ready; this.helper(); } is_b() { this.ready; this.helper(); } helper() {} }"),
        ("py", "class A:\n def is_a(self):\n  self.ready; self.helper()\n def is_b(self):\n  self.ready; self.helper()\n def helper(self): pass\n"),
        ("go", "package p; type A struct { ready bool }; func (a A) is_a() { a.ready; a.helper() }; func (a A) is_b() { a.ready; a.helper() }; func (a A) helper() {}"),
        ("java", "class A { boolean ready; void is_a() { var value = this.ready; this.helper(); } void is_b() { var value = this.ready; this.helper(); } void helper() {} }"),
        ("cs", "class A { bool ready; void is_a() { var value = this.ready; this.helper(); } void is_b() { var value = this.ready; this.helper(); } void helper() {} }"),
        ("kt", "class A {\n fun is_a() {\n this.ready\n this.helper()\n }\n fun is_b() {\n this.ready\n this.helper()\n }\n fun helper() {}\n}\n"),
        ("kts", "class A {\n fun is_a() {\n this.ready\n this.helper()\n }\n fun is_b() {\n this.ready\n this.helper()\n }\n fun helper() {}\n}\n"),
        ("php", "<?php class A { function is_a() { $this->ready; $this->helper(); } function is_b() { $this->ready; $this->helper(); } function helper() {} }"),
    ] {
        let functions = functions(ext, source);
        let a = named(&functions, "is_a");
        let b = named(&functions, "is_b");
        assert_eq!(a.owner_id, b.owner_id, "{ext}");
        assert_eq!(a.dependencies, b.dependencies, "{ext}");
        assert_eq!(field_names(a), ["ready"], "{ext}");
        assert_eq!(callee_names(a), ["helper"], "{ext}");
    }
}

#[test]
fn declarations_remain_separate_even_when_names_match() {
    for (ext, source) in [
        (
            "rs",
            "struct A; impl A { fn get_a() {} } impl A { fn get_b() {} }",
        ),
        ("js", "class A { get_a() {} } class A { get_b() {} }"),
        (
            "py",
            "class A:\n def get_a(self): pass\nclass A:\n def get_b(self): pass\n",
        ),
        (
            "java",
            "class A { void get_a() {} } class A { void get_b() {} }",
        ),
        (
            "cs",
            "partial class A { void get_a() {} } partial class A { void get_b() {} }",
        ),
        (
            "kt",
            "class A {\n fun get_a() {}\n}\nclass A {\n fun get_b() {}\n}\n",
        ),
        (
            "php",
            "<?php class A { function get_a() {} } class A { function get_b() {} }",
        ),
    ] {
        let functions = functions(ext, source);
        assert_ne!(
            named(&functions, "get_a").owner_id,
            named(&functions, "get_b").owner_id,
            "{ext}"
        );
    }
}

#[test]
fn free_functions_and_nested_scopes_have_lexical_owners() {
    let functions = functions("rs", "fn helper() {} fn is_a() { helper(); } fn is_b() { helper(); } mod inner { fn is_c() {} } fn outer() { fn is_d() {} fn is_e() {} }");
    assert_eq!(
        named(&functions, "is_a").owner_id,
        named(&functions, "is_b").owner_id
    );
    assert_eq!(callee_names(named(&functions, "is_a")), ["helper"]);
    assert_ne!(
        named(&functions, "is_a").owner_id,
        named(&functions, "is_c").owner_id
    );
    assert_ne!(
        named(&functions, "is_a").owner_id,
        named(&functions, "is_d").owner_id
    );
    assert_eq!(
        named(&functions, "is_d").owner_id,
        named(&functions, "is_e").owner_id
    );
}

#[test]
fn go_receivers_resolve_to_a_unique_local_declaration_and_keep_pointer_distinction() {
    let functions = functions("go", "package p; type A struct { ready bool }; func (a A) is_a() { a.ready }; func (a *A) is_b() { a.ready }; func (a Missing) is_c() { a.ready }");
    assert_ne!(
        named(&functions, "is_a").owner_id,
        named(&functions, "is_b").owner_id
    );
    // `Missing` is declared in another file: a named owner, never a local declaration's.
    assert_eq!(named(&functions, "is_c").owner_id, "go_type:Missing:value");
}

#[test]
fn nested_bodies_comments_strings_parameters_aliases_and_external_calls_supply_no_evidence() {
    for (ext, source) in [
        ("rs", "impl A { fn is_a(&self, other: A) { let alias = self; alias.ready; other.ready; external(); self.inherited(); let f = || self.ready; fn inner() { helper(); } let text = \"self.ready\"; /* self.ready */ } }"),
        ("js", "class A { is_a(other) { const alias = this; alias.ready; other.ready; external(); this.inherited(); const f = () => this.ready; const text = 'this.ready'; /* this.ready */ } }"),
        ("py", "class A:\n def is_a(self, other):\n  alias = self\n  alias.ready; other.ready; external(); self.inherited()\n  f = lambda: self.ready\n  text = 'self.ready' # self.ready\n"),
        ("java", "class A { void is_a(A other) { A alias = this; boolean value = alias.ready || other.ready; external(); this.inherited(); Runnable f = () -> this.ready; String text = \"this.ready\"; } }"),
        ("cs", "class A { void is_a(A other) { var alias = this; bool value = alias.ready || other.ready; external(); this.inherited(); Func<bool> f = () => this.ready; var text = \"this.ready\"; } }"),
        ("php", "<?php class A { function is_a($other) { $alias = $this; $alias->ready; $other->ready; external(); $this->inherited(); $f = fn() => $this->ready; $text = 'this.ready'; } }"),
    ] {
        let functions = functions(ext, source);
        assert!(named(&functions, "is_a").dependencies.is_empty(), "{ext}: {:?}", named(&functions,"is_a"));
    }
}

#[test]
fn local_bindings_and_reassignments_block_callees_and_receivers() {
    for (ext, source) in [
        ("rs", "fn helper() {} fn is_a(helper: fn()) { helper(); } fn is_b() { let helper = || {}; helper(); }"),
        ("js", "function helper() {} function is_a(helper) { helper(); } function is_b() { let helper = () => {}; helper(); }"),
        ("py", "def helper(): pass\ndef is_a(helper): helper()\ndef is_b():\n helper = lambda: True\n helper()\n"),
    ] {
        let functions = functions(ext, source);
        assert!(named(&functions, "is_a").dependencies.is_empty(), "{ext}");
        assert!(named(&functions, "is_b").dependencies.is_empty(), "{ext}");
    }
    let functions = functions(
        "py",
        "class A:\n def is_a(self, other):\n  self = other\n  return self.ready\n",
    );
    assert!(named(&functions, "is_a").dependencies.is_empty());
}

#[test]
fn implicit_receiver_fields_and_calls_require_local_declarations() {
    for (ext, source) in [
        ("java", "class A { boolean ready; boolean is_a() { helper(); inherited(); return ready; } boolean is_b(boolean ready) { return ready; } void helper() {} }"),
        ("cs", "class A { bool ready; bool is_a() { helper(); inherited(); return ready; } bool is_b(bool ready) { return ready; } void helper() {} }"),
        ("kt", "class A(val ready: Boolean) {\n fun is_a(): Boolean {\n helper()\n inherited()\n return ready\n }\n fun is_b(ready: Boolean) = ready\n fun helper() {}\n}\n"),
    ] {
        let functions = functions(ext, source);
        assert_eq!(field_names(named(&functions, "is_a")), ["ready"], "{ext}");
        assert_eq!(callee_names(named(&functions, "is_a")), ["helper"], "{ext}");
        assert!(named(&functions, "is_b").dependencies.is_empty(), "{ext}: {:?}", named(&functions, "is_b"));
    }
}

#[test]
fn overloads_and_test_callees_are_not_evidence() {
    let overloaded = functions("java", "class A { void is_a() { helper(); this.helper(); } void helper() {} void helper(int x) {} }");
    assert!(named(&overloaded, "is_a").dependencies.is_empty());
    let tests = functions("rs", "#[test] fn helper() {} fn is_a() { helper(); }");
    assert!(tests.iter().find(|f| f.name == "helper").unwrap().is_test);
    assert!(named(&tests, "is_a").dependencies.is_empty());
}

#[test]
fn fields_are_distinct_from_callees_and_evidence_is_sorted_and_deduplicated() {
    let functions = functions("js", "class A { helper() {} is_a() { this.z; this.a; this.z; this.helper; this.helper(); this.helper(); } }");
    let provenance = named(&functions, "is_a");
    assert_eq!(field_names(provenance), ["a", "z"]);
    assert_eq!(callee_names(provenance), ["helper"]);
    let mut sorted = provenance.dependencies.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(provenance.dependencies, sorted);
}

#[test]
fn dynamic_fields_and_unknown_receivers_are_ignored() {
    for (ext, source) in [
        (
            "js",
            "class A { is_a(key) { this[key]; this['ready']; const alias = this; alias.ready; } }",
        ),
        (
            "php",
            "<?php class A { function is_a($key) { $this->$key; $this->{$key}; } }",
        ),
    ] {
        let functions = functions(ext, source);
        assert!(named(&functions, "is_a").dependencies.is_empty(), "{ext}");
    }
}

#[test]
fn global_reassignments_enclosing_parameters_and_mutated_method_slots_block_resolution() {
    for source in [
        "function helper() {} helper = external; function is_a() { helper(); }",
        "function helper() {} function outer(helper) { function is_a() { helper(); } }",
        "class A { helper() {} is_a() { this.helper = external; this.helper(); } }",
        "class A { helper() {} mutate() { this.helper = external; } is_a() { this.helper(); } }",
        "class A { helper() {} is_a() { A.helper(); } } A.helper = external;",
    ] {
        let functions = functions("js", source);
        assert!(
            named(&functions, "is_a").dependencies.is_empty(),
            "{source}"
        );
    }
}

#[test]
fn local_static_receivers_cannot_resolve_out_of_scope_declarations() {
    let functions = functions("js", "function outer() { class Hidden { static helper() {} } } function is_a() { Hidden.helper(); }");
    assert!(named(&functions, "is_a").dependencies.is_empty());
    let visible = functions_for_static();
    assert_eq!(callee_names(named(&visible, "is_a")), ["helper"]);
}

fn functions_for_static() -> Vec<FunctionMetrics> {
    functions(
        "js",
        "class Visible { static helper() {} } function is_a() { Visible.helper(); }",
    )
}

#[test]
fn rust_modules_and_managed_nested_types_do_not_inherit_bare_callee_resolution() {
    for (ext, source) in [
        ("rs", "fn helper() {} mod child { fn is_a() { helper(); } }"),
        (
            "java",
            "class Outer { void helper() {} static class Inner { void is_a() { helper(); } } }",
        ),
        (
            "kt",
            "class Outer {\n fun helper() {}\n class Inner {\n fun is_a() { helper() }\n }\n}\n",
        ),
    ] {
        let functions = functions(ext, source);
        assert!(named(&functions, "is_a").dependencies.is_empty(), "{ext}");
    }
}

#[test]
fn namespaces_and_nested_types_are_distinct_owners() {
    for (ext, source) in [
        ("php", "<?php namespace One; function get_a() {} namespace Two; function get_b() {}"),
        ("ts", "namespace One { function get_a() {} } namespace Two { function get_b() {} }"),
        ("cs", "namespace One { class A { void get_a() {} } } namespace Two { class A { void get_b() {} } }"),
        ("rs", "trait A { fn get_a() {} } trait B { fn get_b() {} }"),
        ("kt", "object One {\n fun get_a() {}\n}\nobject Two {\n fun get_b() {}\n}\n"),
    ] {
        let functions = functions(ext, source);
        assert_ne!(named(&functions, "get_a").owner_id, named(&functions, "get_b").owner_id, "{ext}");
    }
}

#[test]
fn parameter_defaults_and_decorators_do_not_supply_function_body_evidence() {
    let functions = functions("js", "function helper() {} function is_a(value = helper()) {} function is_b() { return helper(); }");
    assert!(named(&functions, "is_a").dependencies.is_empty());
    assert_eq!(callee_names(named(&functions, "is_b")), ["helper"]);
}

#[test]
fn nested_functions_can_call_a_shared_sibling_without_leaking_calls_outward() {
    let functions = functions("js", "function outer() { function helper() {} function is_a() { helper(); } function is_b() { helper(); } }");
    assert!(named(&functions, "outer").dependencies.is_empty());
    assert_eq!(callee_names(named(&functions, "is_a")), ["helper"]);
    assert_eq!(
        named(&functions, "is_a").dependencies,
        named(&functions, "is_b").dependencies
    );
}

#[test]
fn malformed_function_retains_measurements_with_unknown_provenance() {
    let functions = functions("js", "class A { is_a() { return this.; } }");
    assert!(functions
        .iter()
        .find(|f| f.name == "is_a")
        .unwrap()
        .responsibility
        .is_none());
}

#[test]
fn go_callbacks_and_csharp_anonymous_delegates_do_not_leak_body_evidence() {
    for (ext, source) in [
        ("go", "package p; type A struct { ready bool }; func (a A) is_a() { f := func() { a.helper(); _ = a.ready }; _ = f }; func (a A) helper() {}"),
        ("cs", "class A { bool ready; void is_a() { System.Action f = delegate() { helper(); var x = this.ready; }; } void helper() {} }"),
    ] {
        let functions = functions(ext, source);
        assert!(named(&functions, "is_a").dependencies.is_empty(), "{ext}");
    }
}

#[test]
fn receiver_and_local_names_shadowed_by_loop_catch_or_with_bindings_are_not_evidence() {
    for (ext, source) in [
        ("go", "package p; type A struct { ready bool }; func (a A) is_a(xs []A) { for _, a := range xs { _ = a.ready; a.helper() } }; func (a A) helper() {}"),
        ("py", "class A:\n def is_a(self, other):\n  with other as self:\n   self.helper(); self.ready\n def helper(self): pass\n"),
        ("js", "function helper() {} function is_a() { try {} catch (helper) { helper(); } }"),
        ("java", "class A { boolean ready; void is_a(boolean[] values) { for (boolean ready : values) { consume(ready); } } }"),
    ] {
        let functions = functions(ext, source);
        assert!(named(&functions, "is_a").dependencies.is_empty(), "{ext}");
    }
}

#[test]
fn go_receivers_declared_in_another_file_share_a_named_owner_per_receiver_mode() {
    let functions = functions("go", "package p; func (s *Server) handleA() { s.ready = true; s.helper() }; func (s *Server) handleB() { _ = s.ready; s.helper() }; func (s *Server) helper() {}; func (s Server) handleC() {}");
    let a = named(&functions, "handleA");
    assert_eq!(a.owner_id, "go_type:Server:pointer");
    assert_eq!(a.owner_label, "*Server");
    assert_eq!(a.owner_id, named(&functions, "handleB").owner_id);
    assert_eq!(field_names(a), ["ready"]);
    assert_eq!(callee_names(a), ["helper"]);
    assert_eq!(a.dependencies, named(&functions, "handleB").dependencies);
    let c = named(&functions, "handleC");
    assert_eq!(c.owner_id, "go_type:Server:value");
    assert_eq!(c.owner_label, "Server");
}

#[test]
fn go_receivers_with_one_local_declaration_keep_the_declaration_owner() {
    let functions = functions(
        "go",
        "package p; type Server struct {}; func (s *Server) handleA() {}",
    );
    assert_eq!(named(&functions, "handleA").owner_id, "type:16:pointer");
}

#[test]
fn go_receivers_with_duplicate_local_declarations_stay_unknown() {
    let functions = functions(
        "go",
        "package p; type Server struct {}; type Server struct {}; func (s *Server) handleA() {}",
    );
    assert!(functions
        .iter()
        .find(|f| f.name == "handleA")
        .unwrap()
        .responsibility
        .is_none());
}

#[test]
fn go_receivers_with_a_malformed_local_declaration_stay_unknown() {
    let functions = functions(
        "go",
        "package p\ntype Server struct { a int\nfunc (s *Server) isA() bool { return s.a > 0 }\nfunc (s *Server) isB() bool { return s.a < 9 }\n",
    );
    let owners: Vec<_> = functions
        .iter()
        .map(|f| {
            (
                f.name.as_str(),
                f.responsibility.as_ref().map(|r| r.owner_id.as_str()),
            )
        })
        .collect();
    assert!(
        owners.iter().any(|(name, _)| *name == "isB"),
        "the method after the broken declaration must still be extracted: {owners:?}"
    );
    assert!(
        owners.iter().all(|(_, owner)| owner.is_none()),
        "malformed declaration produced owners: {owners:?}"
    );
}

#[test]
fn methods_under_a_parse_error_have_no_owner_in_any_language() {
    // The Go branch refuses ownership when any ancestor is an ERROR node. The
    // other languages stopped looking at the first scope, so an ERROR above
    // that scope went unseen and the file still produced advice derived from a
    // region the parser failed on — and a grammar that lags a language version
    // produces those nodes on valid source, not only on broken source.
    // The boundary is the ancestor chain, not the file: a recovery artefact
    // beside the owner (a PHP `MISSING ")"` in a sibling's parameters, a Java
    // ERROR earlier in the same interface body) leaves the owner itself
    // well-formed and keeps its advice, because refusing there would delete a
    // whole file's advice over one construct the grammar cannot yet parse.
    let owners: Vec<_> = functions(
        "js",
        "function broken( { class A { isA(){return this.r;} isB(){return this.r;} } }",
    )
    .into_iter()
    .map(|f| (f.name, f.responsibility.map(|r| r.owner_id)))
    .collect();
    assert!(
        owners.iter().all(|(_, owner)| owner.is_none()),
        "a parse error still produced owners: {owners:?}"
    );
}

fn go_owners(source: &str) -> Vec<(String, Option<String>)> {
    functions("go", source)
        .into_iter()
        .map(|f| (f.name, f.responsibility.map(|r| r.owner_id)))
        .collect()
}

#[test]
fn go_methods_swallowed_by_an_error_node_have_no_owner() {
    let owners = go_owners(
        "package p\ntype Server struct{a int}\nfunc (s *Server) isA() bool { return s.a > 0 }}\nfunc (s *Server) isB() bool { return s.a < 9 }\n",
    );
    assert_eq!(
        owners,
        vec![
            ("isA".to_owned(), None),
            ("isB".to_owned(), Some("type:15:pointer".to_owned())),
        ]
    );
}

#[test]
fn go_receivers_aliased_in_the_file_stay_unknown() {
    for source in [
        "package p\ntype Server = \nfunc (s *Server) isA() bool { return s.a > 0 }\nfunc (s *Server) isB() bool { return s.a < 9 }\n",
        "package p\ntype impl struct{a int}\ntype Server = impl\nfunc (s *Server) isA() bool { return s.a > 0 }\nfunc (s *Server) isB() bool { return s.a < 9 }\n",
    ] {
        let owners = go_owners(source);
        assert!(
            owners.iter().any(|(name, _)| name == "isB"),
            "isB not extracted: {owners:?}"
        );
        assert!(
            owners.iter().all(|(_, owner)| owner.is_none()),
            "aliased receiver got an owner: {owners:?}"
        );
    }
}

#[test]
fn go_type_declarations_inside_a_function_cannot_own_file_methods() {
    let functions = functions("go", "package p; func outer() { type A struct { ready bool }; _ = A{} }; func (a A) is_a() { _ = a.ready }");
    // The receiver names the package-level `A`, declared in another file.
    assert_eq!(named(&functions, "is_a").owner_id, "go_type:A:value");
}

#[test]
fn kotlin_companion_objects_are_distinct_and_extensions_have_unknown_ownership() {
    let functions = functions("kt", "class A {\n companion object {\n fun get_a() {}\n }\n fun get_b() {}\n fun String.is_a() = this.length\n}\n");
    assert_ne!(
        named(&functions, "get_a").owner_id,
        named(&functions, "get_b").owner_id
    );
    assert!(functions
        .iter()
        .find(|f| f.name == "is_a")
        .unwrap()
        .responsibility
        .is_none());
}

#[test]
fn python_annotated_receiver_is_resolved() {
    let functions = functions("py", "class A:\n def is_a(self: A):\n  return self.ready\n");
    assert_eq!(field_names(named(&functions, "is_a")), ["ready"]);
}

#[test]
fn anonymous_java_classes_have_distinct_owners_and_do_not_leak_fields() {
    let functions = functions("java", "class A { void outer() { Object a = new Object() { boolean ready; boolean is_a() { return this.ready; } }; Object b = new Object() { boolean ready; boolean is_b() { return this.ready; } }; } }");
    assert!(named(&functions, "outer").dependencies.is_empty());
    assert_ne!(
        named(&functions, "is_a").owner_id,
        named(&functions, "is_b").owner_id
    );
    assert_eq!(field_names(named(&functions, "is_a")), ["ready"]);
}

#[test]
fn rust_tuple_fields_are_direct_dependencies_and_distinct_from_indexing() {
    let functions = functions("rs", "struct A(bool, bool); impl A { fn is_a(&self) -> bool { self.0 } fn is_b(&self) -> bool { self.1 } fn is_c(&self, i: usize) -> bool { self[i] } }");
    assert_eq!(field_names(named(&functions, "is_a")), ["0"]);
    assert_eq!(field_names(named(&functions, "is_b")), ["1"]);
    assert!(named(&functions, "is_c").dependencies.is_empty());
}

#[test]
fn go_generic_receivers_resolve_the_same_local_type_and_keep_pointer_identity() {
    let functions = functions("go", "package p; type A[T any] struct { ready bool }; func (a *A[T]) is_a() bool { return a.ready }; func (a A[T]) is_b() bool { return a.ready }");
    assert_eq!(field_names(named(&functions, "is_a")), ["ready"]);
    assert_eq!(field_names(named(&functions, "is_b")), ["ready"]);
    assert_ne!(
        named(&functions, "is_a").owner_id,
        named(&functions, "is_b").owner_id
    );
}

#[test]
fn named_argument_labels_do_not_supply_implicit_field_evidence() {
    for (ext, source) in [
        (
            "cs",
            "class A { bool ready; void is_a() { Consume(ready: true); } }",
        ),
        (
            "kt",
            "class A(val ready: Boolean) {\n fun is_a() { consume(ready = true) }\n}\n",
        ),
    ] {
        let functions = functions(ext, source);
        assert!(named(&functions, "is_a").dependencies.is_empty(), "{ext}");
    }
}

#[test]
fn anonymous_php_classes_and_kotlin_objects_keep_separate_owner_declarations() {
    for (ext, source) in [
        ("php", "<?php function outer() { $a = new class { function get_a() { return $this->ready; } }; $b = new class { function get_b() { return $this->ready; } }; }"),
        ("kt", "fun outer() {\n val a = object {\n fun get_a() = this.ready\n }\n val b = object {\n fun get_b() = this.ready\n }\n}\n"),
    ] {
        let functions = functions(ext, source);
        assert!(named(&functions, "outer").dependencies.is_empty(), "{ext}");
        assert_ne!(named(&functions, "get_a").owner_id, named(&functions, "get_b").owner_id, "{ext}");
        assert_eq!(field_names(named(&functions, "get_a")), ["ready"], "{ext}");
    }
}

#[test]
fn generators_and_comprehension_receiver_shadowing_do_not_supply_evidence() {
    for (ext, source) in [
        (
            "js",
            "class A { is_a() { function* nested() { yield this.ready; } } }",
        ),
        (
            "py",
            "class A:\n def is_a(self, others):\n  return [self.ready for self in others]\n",
        ),
    ] {
        let functions = functions(ext, source);
        assert!(named(&functions, "is_a").dependencies.is_empty(), "{ext}");
    }
}

#[test]
fn bodyless_external_and_abstract_declarations_cannot_supply_callee_evidence() {
    for (ext, source) in [
        ("rs", "extern \"C\" { fn helper(); } fn is_a() { unsafe { helper(); } }"),
        ("java", "abstract class A { abstract boolean helper(); boolean is_a() { return helper(); } }"),
        ("cs", "abstract class A { abstract bool helper(); bool is_a() { return helper(); } }"),
        ("kt", "abstract class A {\n abstract fun helper(): Boolean\n fun is_a() = helper()\n}\n"),
        ("php", "<?php abstract class A { abstract function helper(); function is_a() { return $this->helper(); } }"),
    ] {
        let functions = functions(ext, source);
        assert!(named(&functions, "is_a").dependencies.is_empty(), "{ext}");
    }
}

#[test]
fn loop_pattern_catch_and_destructuring_bindings_cannot_supply_outer_dependencies() {
    for (ext, source) in [
        ("rs", "fn helper() {} fn is_a() { for helper in [|| ()] { helper(); } }"),
        ("rs", "fn helper() {} fn is_a(value: fn()) { match value { helper => helper() } }"),
        ("rs", "fn helper() {} fn is_a(value: Option<fn()>) { if let Some(helper) = value { helper(); } }"),
        ("java", "class A { boolean ready; void is_a() { try {} catch (Exception ready) { consume(ready); } } }"),
        ("kt", "class A {\n val ready = true\n fun is_a(values: List<Boolean>) {\n for (ready in values) { println(ready) }\n }\n}"),
        ("cs", "class A { bool ready; void is_a(bool[] values) { foreach (bool ready in values) { Consume(ready); } } }"),
        ("kt", "class A {\n val ready = true\n fun is_a(value: Pair<Boolean, Boolean>) {\n val (ready, other) = value\n println(ready)\n }\n}"),
        ("java", "class A { String ready; void is_a(Object value) { if (value instanceof String ready) { consume(ready); } } }"),
        ("cs", "class A { string ready; void is_a(object value) { if (value is string ready) { Consume(ready); } } }"),
    ] {
        let functions = functions(ext, source);
        assert!(named(&functions, "is_a").dependencies.is_empty(), "{ext}: {source}");
    }
}

#[test]
fn pattern_out_query_and_resource_bindings_shadow_implicit_fields() {
    let mut failures = Vec::new();
    for (form, ext, source) in [
        ("C# out var", "cs", "class A { string value; bool is_a(Dictionary<string,string> store, string k) { return store.TryGetValue(k, out var value) && value != null; } }"),
        ("C# catch declaration", "cs", "class A { string value; void is_a() { try {} catch (Exception value) { Consume(value); } } }"),
        ("C# deconstruction", "cs", "class A { object value; bool is_a(object o) { var (value, other) = Pair(o); return value != null; } }"),
        ("C# var pattern", "cs", "class A { object value; bool is_a(object o) { return o is Point { X: var value } && value != null; } }"),
        ("C# from clause", "cs", "class A { int value; object is_a(int[] xs) { return from value in xs select value; } }"),
        ("Java type pattern", "java", "class A { String value; boolean is_a(Object o) { return switch (o) { case String value -> consume(value); default -> false; }; } }"),
        ("Java record pattern component", "java", "class A { int value; boolean is_a(Object o) { return o instanceof Point(int value, int y) && consume(value); } }"),
        ("Java resource", "java", "class A { Reader value; void is_a() throws Exception { try (Reader value = open()) { consume(value); } } }"),
        ("Kotlin catch", "kt", "class A {\n val value = 1\n fun is_a() {\n try { run() } catch (value: Exception) { consume(value) }\n }\n}\n"),
        ("Kotlin when subject", "kt", "class A {\n val value = 1\n fun is_a() {\n when (val value = load()) { else -> consume(value) }\n }\n}\n"),
    ] {
        let functions = functions(ext, source);
        if !field_names(named(&functions, "is_a")).is_empty() {
            failures.push(form);
        }
    }
    assert!(
        failures.is_empty(),
        "fields still visible through: {failures:?}"
    );
}

#[test]
fn a_csharp_field_type_name_is_not_a_field() {
    let functions = functions("cs", "class A { Exception error; bool is_a() { try { return true; } catch (Exception e) { return false; } } }");
    assert!(
        field_names(named(&functions, "is_a")).is_empty(),
        "{:?}",
        named(&functions, "is_a")
    );
}

#[test]
fn callee_evidence_requires_the_call_to_match_the_declared_parameter_count() {
    let mut failures = Vec::new();
    for (ext, source) in [
        ("java", "class A { boolean helper() { return true; } boolean is_a() { return helper(2); } boolean is_b() { return this.helper(2); } }"),
        ("cs", "class A { bool helper() { return true; } bool is_a() { return helper(2); } bool is_b() { return this.helper(2); } }"),
        ("kt", "class A {\n fun helper() = true\n fun is_a() = helper(2)\n fun is_b() = this.helper(2)\n}\n"),
        ("go", "package p; type A struct{}; func (a A) helper() bool { return true }; func (a A) is_a() bool { return a.helper(2) }; func (a A) is_b() bool { return a.helper(2) }"),
        ("rs", "struct A; impl A { fn helper(&self) -> bool { true } fn is_a(&self) -> bool { self.helper(2) } fn is_b(&self) -> bool { Self::helper(self, 2) } }"),
        ("py", "class A:\n def helper(self): return True\n def is_a(self): return self.helper(2)\n def is_b(self): return A.helper(self, 2)\n"),
        ("js", "class A { helper() { return true; } is_a() { return this.helper(2); } is_b() { return this.helper(2); } }"),
        ("php", "<?php class A { function helper() { return true; } function is_a() { return $this->helper(2); } function is_b() { return $this->helper(2); } }"),
    ] {
        let functions = functions(ext, source);
        for caller in ["is_a", "is_b"] {
            if !callee_names(named(&functions, caller)).is_empty() {
                failures.push((ext, caller));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "mismatched calls resolved in: {failures:?}"
    );
}

#[test]
fn callee_evidence_keeps_matching_argument_counts_including_explicit_receivers() {
    for (ext, source) in [
        ("java", "class A { boolean helper(int x) { return true; } boolean is_a() { return helper(2); } }"),
        ("cs", "class A { bool helper(int x) { return true; } bool is_a() { return this.helper(2); } }"),
        ("kt", "class A {\n fun helper(x: Int, f: () -> Unit) = true\n fun is_a() = helper(2) { }\n}\n"),
        ("go", "package p; type A struct{}; func (a A) helper(x, y int) bool { return true }; func (a A) is_a() bool { return a.helper(1, 2) }"),
        ("rs", "struct A; impl A { fn helper(&self, x: i32) -> bool { true } fn is_a(&self) -> bool { self.helper(2) && Self::helper(self, 2) } }"),
        ("py", "class A:\n def helper(self, x): return True\n def is_a(self): return self.helper(2)\n"),
        ("js", "class A { helper(x) { return true; } is_a() { return this.helper(2); } }"),
        ("php", "<?php class A { function helper($x) { return true; } function is_a() { return $this->helper(2); } }"),
    ] {
        let functions = functions(ext, source);
        assert_eq!(callee_names(named(&functions, "is_a")), ["helper"], "{ext}: {:?}", named(&functions, "is_a"));
    }
}

#[test]
fn defaulted_variadic_and_spread_calls_are_undecidable_and_supply_no_callee() {
    let mut failures = Vec::new();
    for (ext, source) in [
        ("java", "class A { boolean helper(int... xs) { return true; } boolean is_a() { return helper(1); } }"),
        ("cs", "class A { bool helper(int x = 1) { return true; } bool is_a() { return helper(1); } }"),
        ("kt", "class A {\n fun helper(x: Int = 1) = true\n fun is_a() = helper(1)\n}\n"),
        ("go", "package p; type A struct{}; func (a A) helper(xs ...int) bool { return true }; func (a A) is_a(ys []int) bool { return a.helper(ys...) }"),
        ("py", "class A:\n def helper(self, x=1): return True\n def is_a(self): return self.helper(1)\n"),
        ("js", "class A { helper(...xs) { return true; } is_a() { return this.helper(1); } }"),
        ("ts", "class A { helper(x?: number) { return true; } is_a() { return this.helper(1); } }"),
        ("php", "<?php class A { function helper($x = 1) { return true; } function is_a() { return $this->helper(1); } }"),
        ("js", "class A { helper(x) { return true; } is_a(xs) { return this.helper(...xs); } }"),
    ] {
        let functions = functions(ext, source);
        if !callee_names(named(&functions, "is_a")).is_empty() {
            failures.push(source);
        }
    }
    assert!(
        failures.is_empty(),
        "undecidable calls resolved in: {failures:#?}"
    );
}

#[test]
fn tagged_template_calls_have_an_undecidable_argument_count() {
    for source in [
        "class A { tag(s) { return s; } is_a(x) { return this.tag`${x}`; } }",
        // Three template pieces against three parameters: equal only if the pieces were counted.
        "class A { tag(s, v, w) { return s; } is_a(x) { return this.tag`a${x}c`; } }",
        "function tag(s) { return s; } function is_a(x) { return tag`${x}`; }",
    ] {
        let functions = functions("js", source);
        assert!(
            callee_names(named(&functions, "is_a")).is_empty(),
            "tagged template resolved in: {source}"
        );
    }
}

#[test]
fn a_function_calling_itself_does_not_depend_on_itself() {
    let functions = functions(
        "rs",
        "fn is_sorted(v: &[i32]) -> bool { v.len() < 2 || is_sorted(&v[1..]) }",
    );
    assert!(named(&functions, "is_sorted").dependencies.is_empty());
}

#[test]
fn unindexed_local_callable_declarations_block_outer_callee_names() {
    for (ext, source) in [
        (
            "cs",
            "class A { void helper() {} void is_a() { void helper() {} helper(); } }",
        ),
        (
            "js",
            "function helper() {} function is_a() { function* helper() { yield 1; } helper(); }",
        ),
    ] {
        let functions = functions(ext, source);
        assert!(
            named(&functions, "is_a").dependencies.is_empty(),
            "{ext}: {source}"
        );
    }
}

#[test]
fn private_javascript_receiver_fields_and_local_callees_supply_direct_evidence() {
    for ext in ["js", "jsx", "mjs", "cjs", "ts", "tsx"] {
        let functions = functions(ext, "class A { #ready = true; #helper() { return true; } is_a() { this.#helper(); return this.#ready; } is_b() { this.#helper(); return this.#ready; } }");
        assert_eq!(field_names(named(&functions, "is_a")), ["#ready"], "{ext}");
        assert_eq!(
            callee_names(named(&functions, "is_a")),
            ["#helper"],
            "{ext}"
        );
        assert_eq!(
            named(&functions, "is_a").dependencies,
            named(&functions, "is_b").dependencies,
            "{ext}"
        );
        // Existing extraction deliberately excludes private method declarations.
        assert_eq!(functions.len(), 2, "{ext}");
    }
}

fn owners_and_labels(ext: &str, source: &str) -> Vec<(String, String, String)> {
    functions(ext, source)
        .into_iter()
        .filter_map(|f| {
            f.responsibility
                .map(|r| (f.name, r.owner_id, r.owner_label))
        })
        .collect()
}

#[test]
fn only_bodyless_namespaces_end_at_their_semicolon() {
    // A `use` declaration has no body either, but it is not a namespace: the
    // functions around it keep the file as their owner.
    assert_eq!(
        owners_and_labels("rs", "fn is_a() {} use std::fmt; fn is_b() {}"),
        [
            ("is_a".into(), "source_file:0".into(), "file".into()),
            ("is_b".into(), "source_file:0".into(), "file".into()),
        ]
    );
}

#[test]
fn unnamed_class_expressions_read_as_anonymous_classes() {
    assert_eq!(
        owners_and_labels("js", "const A = class { get_a() {} get_b() {} }"),
        [
            (
                "get_a".into(),
                "class:10".into(),
                "anonymous class (line 1)".into()
            ),
            (
                "get_b".into(),
                "class:10".into(),
                "anonymous class (line 1)".into()
            ),
        ]
    );
}

#[test]
fn unnamed_blocks_that_are_not_functions_read_as_blocks() {
    assert_eq!(
        owners_and_labels(
            "php",
            "<?php namespace { function get_a() {} function get_b() {} }"
        ),
        [
            (
                "get_a".into(),
                "namespace_definition:6".into(),
                "block (line 1)".into()
            ),
            (
                "get_b".into(),
                "namespace_definition:6".into(),
                "block (line 1)".into()
            ),
        ]
    );
}

fn callees_of_is_a(ext: &str, source: &str) -> Vec<String> {
    let functions = functions(ext, source);
    callee_names(named(&functions, "is_a"))
        .into_iter()
        .map(str::to_owned)
        .collect()
}

#[test]
fn comments_in_parameter_and_argument_lists_do_not_count() {
    for source in [
        "class A { helper(/* c */ x) { return true; } is_a() { return this.helper(2); } }",
        "class A { helper(x) { return true; } is_a() { return this.helper(2 /* c */); } }",
    ] {
        assert_eq!(callees_of_is_a("js", source), ["helper"], "{source}");
    }
}

#[test]
fn typescript_parameters_count_once_and_this_not_at_all() {
    for source in [
        "class A { helper(x: number) { return true; } is_a() { return this.helper(2); } }",
        "class A { helper(this: A, x: number) { return true; } is_a() { return this.helper(2); } }",
    ] {
        assert_eq!(callees_of_is_a("ts", source), ["helper"], "{source}");
    }
}

#[test]
fn typescript_rest_parameters_make_arity_undecidable() {
    assert!(callees_of_is_a(
        "ts",
        "class A { helper(...xs: number[]) { return true; } is_a() { return this.helper(1); } }"
    )
    .is_empty());
}

#[test]
fn a_python_generator_argument_counts_as_one() {
    assert_eq!(
        callees_of_is_a(
            "py",
            "class A:\n def helper(self, xs): return True\n def is_a(self, ys): return self.helper(y for y in ys)\n"
        ),
        ["helper"]
    );
}

#[test]
fn php_unpacked_arguments_make_arity_undecidable() {
    assert!(callees_of_is_a(
        "php",
        "<?php class A { function helper($x) { return true; } function is_a($xs) { return $this->helper(...$xs); } }"
    )
    .is_empty());
}

#[test]
fn a_kotlin_call_with_only_a_trailing_lambda_passes_one_argument() {
    assert_eq!(
        callees_of_is_a(
            "kt",
            "class A {\n fun helper(f: () -> Unit) = true\n fun is_a() = helper { }\n}\n"
        ),
        ["helper"]
    );
}

#[test]
fn a_kotlin_spread_argument_makes_arity_undecidable() {
    // The spread targets the inherited vararg overload, not the local one-parameter helper.
    let functions = functions(
        "kt",
        "open class B { fun helper(vararg xs: Int) = true }\nclass A : B() {\n fun helper(x: Int) = true\n fun is_a(xs: IntArray) = helper(*xs)\n}\n",
    );
    assert!(callee_names(named(&functions, "is_a")).is_empty());
}
