use std::{collections::HashMap, ops::Range, rc::Rc};

use tree_sitter::Node;

#[derive(Clone, Default)]
struct Bindings {
    names: HashMap<String, Option<String>>,
    namespaced: bool,
}

/// A node still to visit, with the bindings of its scope.
type Pending<'tree> = (Node<'tree>, Rc<Bindings>);

pub(super) fn ranges(root: Node<'_>, source: &[u8]) -> Vec<Range<usize>> {
    // An explicit stack: nesting depth in the source never becomes call depth.
    let mut pending: Vec<Pending<'_>> = vec![(root, Rc::new(Bindings::default()))];
    let mut ranges = Vec::new();
    while let Some((node, inherited)) = pending.pop() {
        let (found, next) = visit(node, source, &inherited);
        ranges.extend(found);
        pending.extend(next.into_iter().rev());
    }
    ranges
}

fn visit<'tree>(
    node: Node<'tree>,
    source: &[u8],
    inherited: &Rc<Bindings>,
) -> (Option<Range<usize>>, Vec<Pending<'tree>>) {
    let children_with = |bindings: &Rc<Bindings>| {
        named_children(node)
            .map(|child| (child, Rc::clone(bindings)))
            .collect()
    };
    match node.kind() {
        "program" => (None, program_children(node, source, inherited)),
        "class_declaration" => {
            let local = Rc::new(bindings_for(node, source, inherited));
            let is_test_case = named_children(node)
                .find(|child| child.kind() == "base_clause")
                .and_then(|base| base.named_child(0))
                .and_then(|base| resolve(&normalized_name(base, source), &local))
                .as_deref()
                == Some("PHPUnit\\Framework\\TestCase");
            if is_test_case && !node.has_error() {
                (Some(node.byte_range()), Vec::new())
            } else {
                (None, children_with(&local))
            }
        }
        "method_declaration" if node.has_error() => (None, Vec::new()),
        "method_declaration" if has_test_attribute(node, source, inherited) => {
            (Some(node.byte_range()), Vec::new())
        }
        "namespace_definition" | "declaration_list" => (
            None,
            children_with(&Rc::new(bindings_for(node, source, inherited))),
        ),
        _ => (None, children_with(inherited)),
    }
}

/// Top-level statements, each paired with the bindings of the unbraced
/// namespace segment it belongs to.
fn program_children<'tree>(
    program: Node<'tree>,
    source: &[u8],
    inherited: &Bindings,
) -> Vec<Pending<'tree>> {
    let children = named_children(program).collect::<Vec<_>>();
    let starts = std::iter::once(0)
        .chain((1..children.len()).filter(|&index| is_unbraced_namespace(children[index])));
    let ends = starts
        .clone()
        .skip(1)
        .chain(std::iter::once(children.len()));
    starts
        .zip(ends)
        .filter(|(start, end)| start < end)
        .flat_map(|(start, end)| {
            let segment = &children[start..end];
            let local = Rc::new(bindings_from_nodes(segment, source, inherited));
            segment
                .iter()
                .map(move |&child| (child, Rc::clone(&local)))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn is_unbraced_namespace(node: Node<'_>) -> bool {
    node.kind() == "namespace_definition" && node.child_by_field_name("body").is_none()
}

fn has_test_attribute(method: Node<'_>, source: &[u8], bindings: &Bindings) -> bool {
    method
        .child_by_field_name("attributes")
        .into_iter()
        .flat_map(descendants)
        .filter(|node| node.kind() == "attribute")
        .filter_map(|attribute| attribute.named_child(0))
        .filter_map(|name| resolve(&normalized_name(name, source), bindings))
        .any(|resolved| {
            matches!(
                resolved.as_str(),
                "PHPUnit\\Framework\\Attributes\\Test"
                    | "PHPUnit\\Framework\\Attributes\\Before"
                    | "PHPUnit\\Framework\\Attributes\\After"
                    | "PHPUnit\\Framework\\Attributes\\BeforeClass"
                    | "PHPUnit\\Framework\\Attributes\\AfterClass"
            )
        })
}

fn bindings_for(node: Node<'_>, source: &[u8], inherited: &Bindings) -> Bindings {
    let children: Vec<Node<'_>> = if node.kind() == "namespace_definition" {
        node.child_by_field_name("body")
            .map(named_children)
            .map(Iterator::collect::<Vec<_>>)
            .unwrap_or_default()
    } else {
        named_children(node).collect::<Vec<_>>()
    };
    let mut bindings = bindings_from_nodes(&children, source, inherited);
    if node.kind() == "namespace_definition" {
        bindings.namespaced = node.child_by_field_name("name").is_some();
    }
    bindings
}

fn bindings_from_nodes(nodes: &[Node<'_>], source: &[u8], inherited: &Bindings) -> Bindings {
    let mut bindings = inherited.clone();
    if let Some(namespace) = nodes.iter().find(|node| is_unbraced_namespace(**node)) {
        bindings.namespaced = namespace.child_by_field_name("name").is_some();
    }
    for &child in nodes {
        match child.kind() {
            "namespace_use_declaration" if !child.has_error() => {
                let prefix = named_children(child)
                    .find(|n| n.kind() == "namespace_name")
                    .map(|n| normalized_name(n, source));
                for clause in descendants(child).filter(|n| n.kind() == "namespace_use_clause") {
                    parse_use_clause(clause, prefix.as_deref(), source, &mut bindings);
                }
            }
            "class_declaration"
            | "interface_declaration"
            | "trait_declaration"
            | "enum_declaration" => {
                if let Some(name) = child.child_by_field_name("name") {
                    invalidate(text(name, source), &mut bindings);
                }
            }
            _ => {}
        }
    }
    bindings
}

fn parse_use_clause(
    clause: Node<'_>,
    prefix: Option<&str>,
    source: &[u8],
    bindings: &mut Bindings,
) {
    let Some(name_node) = named_children(clause)
        .find(|node| matches!(node.kind(), "qualified_name" | "name" | "namespace_name"))
    else {
        return;
    };
    let name = normalized_name(name_node, source)
        .trim_start_matches('\\')
        .to_owned();
    let target = prefix.map_or_else(|| name.clone(), |prefix| format!("{prefix}\\{name}"));
    let alias = clause
        .child_by_field_name("alias")
        .map(|node| text(node, source).to_owned())
        .or_else(|| target.rsplit('\\').next().map(str::to_owned));
    if let Some(alias) = alias {
        bind(&alias, &target, bindings);
    }
}

fn bind(name: &str, target: &str, bindings: &mut Bindings) {
    match bindings.names.get(name) {
        None => {
            bindings
                .names
                .insert(name.to_owned(), Some(target.to_owned()));
        }
        Some(Some(existing)) if existing == target => {}
        Some(_) => {
            bindings.names.insert(name.to_owned(), None);
        }
    }
}

fn invalidate(name: &str, bindings: &mut Bindings) {
    bindings.names.insert(name.to_owned(), None);
}

fn resolve(name: &str, bindings: &Bindings) -> Option<String> {
    if let Some(absolute) = name.strip_prefix('\\') {
        return Some(absolute.to_owned());
    }
    let mut parts = name.split('\\');
    let head = parts.next()?;
    let target = match bindings.names.get(head) {
        Some(target) => target.clone()?,
        None if !bindings.namespaced => head.to_owned(),
        None => return None,
    };
    Some(parts.fold(target, |mut resolved, part| {
        resolved.push('\\');
        resolved.push_str(part);
        resolved
    }))
}

fn normalized_name(node: Node<'_>, source: &[u8]) -> String {
    text(node, source)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

fn text<'a>(node: Node<'_>, source: &'a [u8]) -> &'a str {
    std::str::from_utf8(&source[node.byte_range()]).unwrap_or("")
}

fn named_children(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .collect::<Vec<_>>()
        .into_iter()
}

fn descendants(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    let mut pending = vec![node];
    std::iter::from_fn(move || {
        let current = pending.pop()?;
        pending.extend(named_children(current));
        Some(current)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detected(source: &str) -> Vec<String> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_php::LANGUAGE_PHP.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        ranges(tree.root_node(), source.as_bytes())
            .into_iter()
            .map(|range| source[range].to_owned())
            .collect()
    }

    #[test]
    fn resolves_direct_aliased_and_fully_qualified_testcase_bases() {
        let source = "<?php\nuse PHPUnit\\Framework\\TestCase as Base;\nclass A extends Base { public function helper(): void {} }\nclass B extends \\PHPUnit\\Framework\\TestCase { public function testB(): void {} }\nfunction production(): void {}\n";
        let found = detected(source);
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|scope| scope.starts_with("class ")));
    }

    #[test]
    fn resolves_attribute_alias_and_namespace_import_but_only_marks_methods() {
        let source = "<?php\nuse PHPUnit\\Framework\\Attributes as A;\nuse PHPUnit\\Framework\\Attributes\\Test as Check;\nclass Fixture {\n #[Check] public function first(): void { $nested = function () {}; }\n #[A\\Before] protected function setup(): void {}\n public function sibling(): void {}\n}\n";
        let found = detected(source);
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|scope| scope.contains("function")));
        assert!(!found.iter().any(|scope| scope.contains("sibling")));
    }

    #[test]
    fn recognizes_the_complete_bounded_attribute_catalog() {
        let source = "<?php\nuse PHPUnit\\Framework\\Attributes\\{Test, Before, After, BeforeClass, AfterClass};\nclass C { #[Test] function a() {} #[Before] function b() {} #[After] function c() {} #[BeforeClass] static function d() {} #[AfterClass] static function e() {} }\n";
        assert_eq!(detected(source).len(), 5);
    }

    #[test]
    fn rejects_conflicts_lookalikes_comments_strings_and_names() {
        let source = "<?php\nuse PHPUnit\\Framework\\Attributes\\Test as Mark;\nuse Vendor\\Test as Mark;\nclass TestCase { public function testNameOnly(): void {} }\nclass C { #[Mark] function conflict() {} #[Vendor\\Test] function fake() {} }\n// #[\\PHPUnit\\Framework\\Attributes\\Test] function commented() {}
$text = '#[\\PHPUnit\\Framework\\Attributes\\Test] function stringed() {}';\n";
        assert!(detected(source).is_empty());
    }

    #[test]
    fn a_test_method_with_a_parse_error_in_its_body_does_not_trigger() {
        for source in [
            "<?php use PHPUnit\\Framework\\Attributes\\Test; class C { #[Test] function broken() { return 1 } }",
            "<?php use PHPUnit\\Framework\\Attributes\\Test; class C { #[Test] function broken() { $x = ; } }",
        ] {
            assert!(detected(source).is_empty(), "{source}");
        }
    }

    #[test]
    fn malformed_attribute_does_not_trigger() {
        assert!(detected("<?php use PHPUnit\\Framework\\Attributes\\Test; class C { #[Test( function broken() {} }").is_empty());
    }

    #[test]
    fn resolves_testcase_and_attribute_imports_inside_braced_namespace() {
        let source = "<?php namespace App { use PHPUnit\\Framework\\TestCase as Base; use PHPUnit\\Framework\\Attributes\\Test as Check; class C extends Base { #[Check] function marked() {} function sibling() {} } }";
        let found = detected(source);
        assert_eq!(found.len(), 1);
        assert!(found[0].starts_with("class C"));
        assert!(found[0].contains("marked"));
        assert!(found[0].contains("sibling"));
    }

    #[test]
    fn braced_namespace_imports_do_not_leak_to_neighboring_namespaces() {
        let source = "<?php namespace A { use PHPUnit\\Framework\\Attributes\\Test as Check; class One { #[Check] function marked() {} } } namespace B { class Two { #[Check] function lookalike() {} } }";
        let found = detected(source);
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("marked"));
    }

    #[test]
    fn unbraced_namespace_imports_do_not_conflict_with_or_leak_to_neighbors() {
        let source = "<?php namespace A; use PHPUnit\\Framework\\Attributes\\Test as Check; class One { #[Check] function marked() {} } namespace B; use Vendor\\Test as Check; class Two { #[Check] function lookalike() {} }";
        let found = detected(source);
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("marked"));
    }

    #[test]
    fn recognized_nested_class_inside_unmarked_method_keeps_its_own_boundary() {
        let source = "<?php use PHPUnit\\Framework\\TestCase as Base; class Factory { function create() { class Nested extends Base { function helper() {} } } function sibling() {} }";
        let found = detected(source);
        assert_eq!(found.len(), 1);
        assert!(found[0].starts_with("class Nested"));
        assert!(found[0].contains("helper"));
        assert!(!found[0].contains("sibling"));
    }
    #[test]
    fn namespace_relative_framework_names_are_not_absolute_evidence() {
        for namespace in ["namespace App;", "namespace App {"] {
            let close = if namespace.ends_with('{') { "}" } else { "" };
            let source = format!("<?php {namespace} class C extends PHPUnit\\Framework\\TestCase {{ function helper() {{}} }} class D {{ #[PHPUnit\\Framework\\Attributes\\Test] function subject() {{}} }} {close}");
            assert!(detected(&source).is_empty(), "{source}");
        }
    }

    #[test]
    fn root_framework_alias_conflicts_do_not_override_absolute_references() {
        let source = "<?php use Vendor as PHPUnit; class C extends PHPUnit\\Framework\\TestCase { function helper() {} } class D extends \\PHPUnit\\Framework\\TestCase { function absolute() {} }";
        let found = detected(source);
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("absolute"));
    }

    #[test]
    fn local_declaration_before_import_is_still_a_conflict() {
        let source = "<?php class Base {} use PHPUnit\\Framework\\TestCase as Base; class C extends Base { function helper() {} }";
        assert!(detected(source).is_empty());
    }
    #[test]
    fn called_attributes_and_identical_imports_keep_resolved_evidence() {
        let source = "<?php use PHPUnit\\Framework\\Attributes\\Test; use PHPUnit\\Framework\\Attributes\\Test; class C { #[Test()] function subject() {} function production() {} }";
        let found = detected(source);
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("function subject"));
        assert!(!found[0].contains("production"));
    }

    #[test]
    fn malformed_import_cannot_supply_attribute_evidence() {
        let source = "<?php use PHPUnit\\Framework\\Attributes\\Test as; class C { #[Test()] function subject() {} }";
        assert!(detected(source).is_empty());
    }
    #[test]
    fn global_qualified_references_resolve_without_a_leading_separator() {
        let source = "<?php class Fixture extends PHPUnit\\Framework\\TestCase { function helper() {} } class Subject { #[PHPUnit\\Framework\\Attributes\\Test()] function subject() {} function production() {} }";
        let found = detected(source);
        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|scope| scope.starts_with("class Fixture")));
        assert!(found.iter().any(|scope| scope.contains("function subject")));
        assert!(!found.iter().any(|scope| scope.contains("production")));
    }
}
