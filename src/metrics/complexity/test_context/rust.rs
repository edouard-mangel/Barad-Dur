use std::ops::Range;
use tree_sitter::Node;

struct Binding {
    name: String,
    target: String,
    scope: usize,
}

fn text<'a>(node: Node<'_>, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}

fn children(node: Node<'_>) -> Vec<Node<'_>> {
    node.named_children(&mut node.walk()).collect()
}

fn walk(node: Node<'_>, visit: &mut impl FnMut(Node<'_>)) {
    super::named_preorder(node, |_| true).for_each(visit);
}

fn scope(node: Node<'_>) -> usize {
    std::iter::successors(node.parent(), |node| node.parent())
        .find(|node| {
            matches!(
                node.kind(),
                "source_file" | "declaration_list" | "block" | "function_item"
            )
        })
        .map_or(node.id(), |node| node.id())
}

fn imports(node: Node<'_>, source: &[u8], prefix: &str, scope: usize, out: &mut Vec<Binding>) {
    let join = |suffix: &str| {
        if prefix.is_empty() {
            suffix.trim_start_matches("::").to_owned()
        } else if suffix == "self" {
            prefix.to_owned()
        } else {
            format!("{prefix}::{suffix}")
        }
    };
    match node.kind() {
        "scoped_use_list" => {
            let path = node
                .child_by_field_name("path")
                .map(|node| join(text(node, source)))
                .unwrap_or_else(|| prefix.to_owned());
            if let Some(list) = node.child_by_field_name("list") {
                imports(list, source, &path, scope, out);
            }
        }
        "use_list" => {
            for child in children(node) {
                imports(child, source, prefix, scope, out);
            }
        }
        "use_as_clause" => {
            if let (Some(path), Some(alias)) = (
                node.child_by_field_name("path"),
                node.child_by_field_name("alias"),
            ) {
                out.push(Binding {
                    name: text(alias, source).to_owned(),
                    target: join(text(path, source)),
                    scope,
                });
            }
        }
        "identifier" | "scoped_identifier" | "self" => {
            let target = join(text(node, source));
            out.push(Binding {
                name: target.rsplit("::").next().unwrap_or("").to_owned(),
                target,
                scope,
            });
        }
        _ => {}
    }
}

fn bindings(root: Node<'_>, source: &[u8]) -> Vec<Binding> {
    let mut result = Vec::new();
    walk(root, &mut |node| match node.kind() {
        "use_declaration" if !node.has_error() => {
            if let Some(arg) = node.child_by_field_name("argument") {
                imports(arg, source, "", scope(node), &mut result);
            }
        }
        "extern_crate_declaration" if !node.has_error() => {
            if let Some(name) = node.child_by_field_name("name") {
                result.push(Binding {
                    name: text(node.child_by_field_name("alias").unwrap_or(name), source)
                        .to_owned(),
                    target: text(name, source).to_owned(),
                    scope: scope(node),
                });
            }
        }
        "mod_item" | "struct_item" | "enum_item" | "type_item" | "trait_item" | "union_item"
        | "macro_definition" => {
            if let Some(name) = node.child_by_field_name("name") {
                result.push(Binding {
                    name: text(name, source).to_owned(),
                    target: String::new(),
                    scope: scope(node),
                });
            }
        }
        "parameter" | "let_declaration" => {
            if let Some(pattern) = node.child_by_field_name("pattern") {
                walk(pattern, &mut |name| {
                    if name.kind() == "identifier" {
                        result.push(Binding {
                            name: text(name, source).to_owned(),
                            target: String::new(),
                            scope: scope(node),
                        });
                    }
                });
            }
        }
        _ => {}
    });
    result
}

fn resolve(name: &str, at: Node<'_>, bindings: &[Binding]) -> Option<String> {
    let name = name.trim_start_matches("::");
    let (head, tail) = name.split_once("::").unwrap_or((name, ""));
    for ancestor in std::iter::successors(Some(at), |node| node.parent()) {
        let mut candidates = bindings
            .iter()
            .filter(|binding| binding.scope == ancestor.id() && binding.name == head);
        if let Some(binding) = candidates.next() {
            if candidates.next().is_some() || binding.target.is_empty() {
                return None;
            }
            return Some(if tail.is_empty() {
                binding.target.clone()
            } else {
                format!("{}::{tail}", binding.target)
            });
        }
        // An inline module does not inherit the parent's use declarations.
        if ancestor.kind() == "declaration_list" {
            break;
        }
    }
    Some(name.to_owned())
}

fn attribute_evidence(
    attribute: Node<'_>,
    item: Node<'_>,
    source: &[u8],
    bindings: &[Binding],
    inner: bool,
) -> bool {
    if attribute.has_error() {
        return false;
    }
    let Some(value) = attribute
        .named_child(0)
        .filter(|node| node.kind() == "attribute")
    else {
        return false;
    };
    let compact = text(value, source)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    if compact == "cfg(test)" {
        return true;
    }
    if inner || item.kind() != "function_item" || value.child_by_field_name("arguments").is_some() {
        return false;
    }
    resolve(&compact, item, bindings)
        .is_some_and(|name| matches!(name.as_str(), "test" | "tokio::test" | "async_std::test"))
}

fn attached(node: Node<'_>, source: &[u8], bindings: &[Binding]) -> bool {
    node.kind().ends_with("_item")
        && std::iter::successors(node.prev_named_sibling(), |node| node.prev_named_sibling())
            .take_while(|node| {
                matches!(
                    node.kind(),
                    "attribute_item" | "line_comment" | "block_comment"
                )
            })
            .any(|attribute| {
                attribute.kind() == "attribute_item"
                    && attribute_evidence(attribute, node, source, bindings, false)
            })
}

pub(super) fn ranges(root: Node<'_>, source: &[u8]) -> Vec<Range<usize>> {
    let bindings = bindings(root, source);
    let mut result = Vec::new();
    walk(root, &mut |node| {
        let inner = matches!(node.kind(), "source_file" | "declaration_list" | "block")
            && children(node)
                .into_iter()
                .take_while(|child| {
                    matches!(
                        child.kind(),
                        "inner_attribute_item" | "line_comment" | "block_comment"
                    )
                })
                .any(|attribute| {
                    attribute.kind() == "inner_attribute_item"
                        && attribute_evidence(attribute, node, source, &bindings, true)
                });
        if attached(node, source, &bindings) || inner {
            result.push(node.byte_range());
        }
    });
    result
}

#[cfg(test)]
mod tests {
    use crate::metrics::complexity::{analyse_content, Language};
    fn flags(source: &str) -> Vec<(String, bool)> {
        let metrics = analyse_content(source, Language::Rust);
        assert!(!metrics.functions.is_empty());
        metrics
            .functions
            .into_iter()
            .map(|function| (function.name, function.is_test))
            .collect()
    }
    #[test]
    fn attributes_attach_across_comments_and_other_attributes_and_helpers_inherit() {
        assert_eq!(
            flags(
                r#"
            fn render_prod() {}
            #[cfg(test)] // attached to module
            #[allow(dead_code)] mod checks { fn render_helper() {} mod nested { fn build_fixture() {} } }
            #[test] fn validate_case() { fn validate_nested() {} }
            fn validate_separate() {}
        "#
            ),
            vec![
                ("render_prod".into(), false),
                ("render_helper".into(), true),
                ("build_fixture".into(), true),
                ("validate_case".into(), true),
                ("validate_nested".into(), true),
                ("validate_separate".into(), false)
            ]
        );
    }
    #[test]
    fn inner_cfg_applies_to_its_container_only() {
        assert_eq!(
            flags("mod checks { #![cfg(test)] fn helper() {} } fn prod() {}")
                .iter()
                .map(|(_, test)| *test)
                .collect::<Vec<_>>(),
            [true, false]
        );
        assert!(
            flags("#![cfg(test)] fn helper() {} mod inner { fn fixture() {} }")
                .iter()
                .all(|(_, test)| *test)
        );
    }
    #[test]
    fn imports_and_namespaces_resolve_without_classifying_siblings() {
        let source = "use tokio::{test as check, self as runtime}; use async_std as other; extern crate tokio as rt; #[check] fn a() {} #[runtime::test] fn b() {} #[other::test] fn c() {} #[rt::test] fn d() {} #[tokio::test] fn e() {} #[async_std::test] fn f() {} fn prod() {}";
        assert_eq!(
            flags(source)
                .iter()
                .map(|(_, test)| *test)
                .collect::<Vec<_>>(),
            [true, true, true, true, true, true, false]
        );
    }
    #[test]
    fn shadowed_conflicting_and_wildcard_bindings_are_unknown() {
        for source in [
            "use other::test; #[test] fn prod() {}",
            "mod tokio {} #[tokio::test] fn prod() {}",
            "use tokio::test as check; use other::test as check; #[check] fn prod() {}",
            "use tokio::*; #[check] fn prod() {}",
            "use tokio::test as check; mod child { #[check] fn prod() {} }",
            "fn outer(tokio: usize) { #[tokio::test] fn prod() {} }",
            "use tokio::test as check; fn outer() { let check = 2; #[check] fn prod() {} }",
        ] {
            assert!(flags(source).iter().all(|(_, test)| !test), "{source}");
        }
    }
    #[test]
    fn unsupported_and_lookalike_evidence_does_not_leak() {
        for source in [
            "#[cfg(any(test, unix))] fn prod() {}",
            "#[cfg(all(test, unix))] fn prod() {}",
            "#[cfg(not(test))] fn prod() {}",
            "#[cfg_attr(test, test)] fn prod() {}",
            "#[testing] fn prod() {}",
            "#[test()] fn prod() {}",
            "#[test] struct Foo; fn prod() {}",
            "#[test] mod checks { fn prod() {} }",
            "#[cfg(test)] use foo::bar; fn prod() {}",
            "mod tests { fn test_prod() {} }",
            "// #[test]\nfn prod() { let s = \"#[cfg(test)]\"; }",
            "#[cfg(test,)] fn prod() {}",
            "#![allow(dead_code)] fn prod() {}",
            "mod checks { #![cfg(not(test))] fn prod() {} }",
        ] {
            assert!(flags(source).iter().all(|(_, test)| !test), "{source}");
        }
    }
    #[test]
    fn function_value_names_do_not_shadow_attributes_but_local_types_do() {
        for source in [
            "#[test] fn test() {}",
            "use tokio::test; #[test] fn test() {}",
        ] {
            assert_eq!(flags(source), [("test".into(), true)], "{source}");
        }
        for source in [
            "trait tokio {} #[tokio::test] fn prod() {}",
            "union tokio { x: u32 } #[tokio::test] fn prod() {}",
        ] {
            assert_eq!(flags(source), [("prod".into(), false)], "{source}");
        }
    }

    #[test]
    fn malformed_imports_do_not_supply_runtime_attributes() {
        for source in [
            "use tokio::test as check = x; #[check] fn prod() {}",
            "extern crate tokio as rt = x; #[rt::test] fn prod() {}",
        ] {
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&tree_sitter_rust::LANGUAGE.into())
                .unwrap();
            let tree = parser.parse(source, None).unwrap();
            let declaration = tree.root_node().named_child(0).unwrap();
            assert!(declaration.has_error(), "{}", tree.root_node().to_sexp());
            assert_eq!(flags(source), [("prod".into(), false)], "{source}");
        }
    }
}
