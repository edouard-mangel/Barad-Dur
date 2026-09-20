use std::{collections::HashMap, ops::Range, rc::Rc};

use tree_sitter::Node;

type Bindings = HashMap<String, Option<String>>;

/// A node still to visit, with the bindings of its scope and, inside a class
/// body, the bindings of the function or module that encloses that class.
type Pending<'tree> = (Node<'tree>, Rc<Bindings>, Option<Rc<Bindings>>);

pub(super) fn ranges(root: Node<'_>, source: &[u8]) -> Vec<Range<usize>> {
    // An explicit stack: nesting depth in the source never becomes call depth.
    let mut pending: Vec<Pending<'_>> = vec![(root, Rc::new(Bindings::new()), None)];
    let mut ranges = Vec::new();
    while let Some((node, inherited, enclosing_function)) = pending.pop() {
        let (found, next) = visit(node, source, &inherited, enclosing_function.as_ref());
        ranges.extend(found);
        pending.extend(next.into_iter().rev());
    }
    ranges
}

fn visit<'tree>(
    node: Node<'tree>,
    source: &[u8],
    inherited: &Rc<Bindings>,
    enclosing_function: Option<&Rc<Bindings>>,
) -> (Option<Range<usize>>, Vec<Pending<'tree>>) {
    let scope = enclosing_function.unwrap_or(inherited);
    let children_with = |bindings: &Rc<Bindings>, enclosing: Option<&Rc<Bindings>>| {
        named_children(node)
            .map(|child| (child, Rc::clone(bindings), enclosing.cloned()))
            .collect()
    };
    let body_with = |definition: Node<'tree>, bindings: Bindings| {
        definition
            .child_by_field_name("body")
            .map(|body| (body, Rc::new(bindings), None))
            .into_iter()
            .collect()
    };
    match node.kind() {
        "class_definition" => {
            let direct_test_base = node
                .child_by_field_name("superclasses")
                .is_some_and(|bases| {
                    named_children(bases).any(|base| {
                        resolve(&expression_text(base, source), inherited).as_deref()
                            == Some("unittest.TestCase")
                    })
                });
            if direct_test_base {
                return (Some(node.byte_range()), Vec::new());
            }
            let local = Rc::new(bindings_for(node, source, scope));
            let body = node
                .child_by_field_name("body")
                .map(|body| (body, local, Some(Rc::clone(scope))))
                .into_iter()
                .collect();
            (None, body)
        }
        "decorated_definition" => {
            let definition = named_children(node)
                .find(|child| matches!(child.kind(), "function_definition" | "class_definition"));
            match definition {
                None => (None, Vec::new()),
                // pytest marks on classes do not make unmarked sibling methods evidence.
                Some(class) if class.kind() == "class_definition" => (
                    None,
                    vec![(class, Rc::clone(inherited), enclosing_function.cloned())],
                ),
                Some(function) if has_test_decorator(node, source, inherited) => {
                    (Some(function.byte_range()), Vec::new())
                }
                Some(function) => (
                    None,
                    body_with(function, bindings_for(function, source, scope)),
                ),
            }
        }
        "function_definition" => (None, body_with(node, bindings_for(node, source, scope))),
        "module" => (
            None,
            children_with(
                &Rc::new(bindings_for(node, source, inherited)),
                enclosing_function,
            ),
        ),
        _ => (None, children_with(inherited, enclosing_function)),
    }
}

fn has_test_decorator(node: Node<'_>, source: &[u8], bindings: &Bindings) -> bool {
    named_children(node)
        .filter(|child| child.kind() == "decorator")
        .filter_map(|decorator| decorator.named_child(0))
        .map(|expression| decorator_name(expression, source))
        .filter_map(|name| resolve(&name, bindings))
        .any(|resolved| {
            resolved == "pytest.fixture"
                || matches!(
                    resolved.as_str(),
                    "pytest.mark.parametrize"
                        | "pytest.mark.skip"
                        | "pytest.mark.skipif"
                        | "pytest.mark.xfail"
                        | "pytest.mark.usefixtures"
                )
        })
}

fn decorator_name(node: Node<'_>, source: &[u8]) -> String {
    let expression = if node.kind() == "call" {
        node.child_by_field_name("function").unwrap_or(node)
    } else {
        node
    };
    expression_text(expression, source)
}

fn bindings_for(node: Node<'_>, source: &[u8], inherited: &Bindings) -> Bindings {
    let mut bindings = inherited.clone();
    if node.kind() == "function_definition" {
        if let Some(parameters) = node.child_by_field_name("parameters") {
            invalidate_parameters(parameters, source, &mut bindings);
        }
    }
    let body = node.child_by_field_name("body").unwrap_or(node);
    for child in named_children(body) {
        collect_bindings(child, source, &mut bindings);
    }
    bindings
}

// Python control-flow blocks share their containing module, class, or function
// scope. Stop at nested definitions, whose bodies get their own binding index.
fn collect_bindings(node: Node<'_>, source: &[u8], bindings: &mut Bindings) {
    let opens_own_scope = |kind: &str| {
        matches!(
            kind,
            "lambda"
                | "import_statement"
                | "import_from_statement"
                | "function_definition"
                | "class_definition"
                | "decorated_definition"
        )
    };
    super::named_preorder(node, |n| !opens_own_scope(n.kind())).for_each(|current| {
        match current.kind() {
            "import_statement" => parse_import(text(current, source), bindings),
            "import_from_statement" => parse_from_import(text(current, source), bindings),
            "function_definition" | "class_definition" => {
                if let Some(name) = current.child_by_field_name("name") {
                    invalidate(text(name, source), bindings);
                }
            }
            "decorated_definition" => {
                if let Some(name) = named_children(current)
                    .find(|n| matches!(n.kind(), "function_definition" | "class_definition"))
                    .and_then(|definition| definition.child_by_field_name("name"))
                {
                    invalidate(text(name, source), bindings);
                }
            }
            "assignment" | "augmented_assignment" | "for_statement" => {
                if let Some(left) = current.child_by_field_name("left") {
                    invalidate_identifiers(left, source, bindings);
                }
            }
            "named_expression" => {
                if let Some(name) = current.child_by_field_name("name") {
                    invalidate_identifiers(name, source, bindings);
                }
            }
            "as_pattern" => {
                if let Some(alias) = current.child_by_field_name("alias") {
                    invalidate_identifiers(alias, source, bindings);
                }
            }
            _ => {}
        }
    });
}

fn parse_import(statement: &str, bindings: &mut Bindings) {
    let Some(imports) = statement.trim().strip_prefix("import ") else {
        return;
    };
    for item in imports.split(',').map(str::trim) {
        let mut words = item.split_whitespace();
        let Some(path) = words.next() else { continue };
        let alias = match (words.next(), words.next()) {
            (Some("as"), Some(alias)) => alias,
            (None, None) => path.split('.').next().unwrap_or(path),
            _ => continue,
        };
        let target = if alias == path {
            path
        } else if !item.contains(" as ") {
            path.split('.').next().unwrap_or(path)
        } else {
            path
        };
        bind(alias, target, bindings);
    }
}

fn parse_from_import(statement: &str, bindings: &mut Bindings) {
    let Some(rest) = statement.trim().strip_prefix("from ") else {
        return;
    };
    let Some((module, imports)) = rest.split_once(" import ") else {
        return;
    };
    for item in imports.trim_matches(['(', ')']).split(',').map(str::trim) {
        if item == "*" {
            continue;
        }
        let mut words = item.split_whitespace();
        let Some(name) = words.next() else { continue };
        let alias = match (words.next(), words.next()) {
            (Some("as"), Some(alias)) => alias,
            (None, None) => name,
            _ => continue,
        };
        bind(alias, &format!("{module}.{name}"), bindings);
    }
}

fn invalidate_parameters(node: Node<'_>, source: &[u8], bindings: &mut Bindings) {
    for parameter in named_children(node) {
        match parameter.kind() {
            "identifier" => invalidate(text(parameter, source), bindings),
            "default_parameter" | "typed_default_parameter" => {
                if let Some(name) = parameter.child_by_field_name("name") {
                    invalidate_identifiers(name, source, bindings);
                }
            }
            "typed_parameter" => {
                let type_range = parameter
                    .child_by_field_name("type")
                    .map(|node| node.byte_range());
                for child in
                    named_children(parameter).filter(|child| Some(child.byte_range()) != type_range)
                {
                    invalidate_identifiers(child, source, bindings);
                }
            }
            "list_splat_pattern" | "dictionary_splat_pattern" | "tuple_pattern" => {
                invalidate_identifiers(parameter, source, bindings);
            }
            _ => {}
        }
    }
}

fn bind(name: &str, target: &str, bindings: &mut Bindings) {
    match bindings.get(name) {
        None => {
            bindings.insert(name.to_owned(), Some(target.to_owned()));
        }
        Some(Some(existing)) if existing == target => {}
        Some(_) => {
            bindings.insert(name.to_owned(), None);
        }
    }
}

fn invalidate(name: &str, bindings: &mut Bindings) {
    bindings.insert(name.to_owned(), None);
}

fn invalidate_identifiers(node: Node<'_>, source: &[u8], bindings: &mut Bindings) {
    super::named_preorder(node, |n| n.kind() != "identifier")
        .filter(|n| n.kind() == "identifier")
        .for_each(|identifier| invalidate(text(identifier, source), bindings));
}

fn resolve(expression: &str, bindings: &Bindings) -> Option<String> {
    let mut parts = expression.split('.');
    let head = parts.next()?;
    let target = bindings.get(head)?.clone()?;
    Some(parts.fold(target, |mut resolved, part| {
        resolved.push('.');
        resolved.push_str(part);
        resolved
    }))
}

fn expression_text(node: Node<'_>, source: &[u8]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn detected(source: &str) -> Vec<String> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        ranges(tree.root_node(), source.as_bytes())
            .into_iter()
            .map(|range| source[range].to_owned())
            .collect()
    }

    #[test]
    fn a_decorated_redefinition_invalidates_an_imported_decorator() {
        let source = "from pytest import fixture\n@decorate\ndef fixture(): pass\n@fixture\ndef helper(): pass\n";
        assert!(detected(source).is_empty());
    }

    #[test]
    fn resolves_unittest_direct_module_and_alias_bases() {
        let source = "import unittest\nimport unittest as ut\nfrom unittest import TestCase as Case\nclass A(unittest.TestCase):\n def helper(self): pass\nclass B(ut.TestCase):\n def test_b(self): pass\nclass C(Case):\n def test_c(self): pass\ndef production(): pass\n";
        let found = detected(source);
        assert_eq!(found.len(), 3);
        assert!(found.iter().all(|scope| scope.starts_with("class ")));
    }

    #[test]
    fn recognizes_only_documented_pytest_decorators_and_keeps_method_scope() {
        let source = "import pytest as pt\nfrom pytest import fixture as fx\n@fx\ndef setup():\n def nested(): pass\n@pt.mark.parametrize('x', [1])\ndef case(x): pass\nclass Mixed:\n @pt.mark.skip(reason='x')\n def marked(self): pass\n def sibling(self): pass\ndef test_name_only(): pass\n";
        let found = detected(source);
        assert_eq!(found.len(), 3);
        assert!(found.iter().any(|scope| scope.contains("def nested")));
        assert!(found.iter().any(|scope| scope.contains("def marked")));
        assert!(!found.iter().any(|scope| scope.contains("sibling")));
    }

    #[test]
    fn rejects_shadowed_conflicting_wildcard_and_lookalike_evidence() {
        let source = "import pytest as p\np = object()\nfrom pytest import *\nfrom fake import fixture\n@p.fixture\ndef shadowed(): pass\n@fixture\ndef wildcard_or_fake(): pass\n# @pytest.fixture\ndef comment(): pass\nTEXT = '@pytest.fixture\\ndef stringed(): pass'\n";
        assert!(detected(source).is_empty());
    }

    #[test]
    fn a_marked_class_does_not_classify_its_sibling_methods() {
        let source = "import pytest\n@pytest.mark.usefixtures('db')\nclass Marked:\n def helper(self): pass\n";
        assert!(detected(source).is_empty());
    }

    #[test]
    fn malformed_decorator_does_not_trigger() {
        assert!(
            detected("import pytest\n@pytest.mark.parametrize(\ndef broken(): pass\n").is_empty()
        );
    }

    #[test]
    fn parameters_shadow_provider_bindings_without_treating_annotations_as_bindings() {
        let source = "import pytest\ndef outer(pytest, typed: pytest, *args, **kwargs):\n @pytest.fixture\n def shadowed(): pass\ndef annotated(value: pytest):\n @pytest.fixture\n def still_resolved(): pass\n";
        let found = detected(source);
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("still_resolved"));
    }

    #[test]
    fn foreign_imports_invalidate_decorator_and_base_aliases() {
        let source = "from pytest import fixture as mark\nfrom fake import other as mark\nfrom unittest import TestCase as Base\nfrom fake import Base\n@mark\ndef case(): pass\nclass C(Base):\n def helper(self): pass\n";
        assert!(detected(source).is_empty());
    }

    #[test]
    fn foreign_module_and_namespace_imports_invalidate_provider_aliases() {
        let source = "import pytest as provider\nimport fake as provider\nfrom pytest import mark as markers\nfrom fake import markers\n@provider.fixture\ndef first(): pass\n@markers.skip\ndef second(): pass\n";
        assert!(detected(source).is_empty());
    }
    #[test]
    fn control_flow_bindings_shadow_the_entire_enclosing_scope() {
        for statement in [
            "if flag:\n  pytest = fake",
            "for pytest in providers:\n  pass",
            "while flag:\n  import fake as pytest",
            "with provider() as pytest:\n  pass",
            "try:\n  pass\n except Error as pytest:\n  pass",
            "if (pytest := fake):\n  pass",
        ] {
            let source = format!("import pytest\ndef outer():\n {statement}\n @pytest.fixture\n def subject(): pass\n");
            assert!(detected(&source).is_empty(), "{source}");
        }
    }

    #[test]
    fn declarations_before_imports_remain_conflicting() {
        for declaration in ["pytest = fake", "def pytest(): pass", "class pytest: pass"] {
            let source =
                format!("{declaration}\nimport pytest\n@pytest.fixture\ndef subject(): pass\n");
            assert!(detected(&source).is_empty(), "{source}");
        }
    }

    #[test]
    fn class_bases_resolve_outside_the_class_body() {
        let source = "import unittest\nclass Fixture(unittest.TestCase):\n unittest = object()\n def helper(self): pass\n";
        let found = detected(source);
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("helper"));
    }
    #[test]
    fn class_bindings_apply_to_decorators_but_do_not_enclose_method_bodies() {
        let source = "import pytest\nclass Fixture:\n import pytest as class_only\n pytest = fake\n @class_only.fixture\n def marked(self): pass\n def method(self):\n  @class_only.fixture\n  def unavailable(): pass\n  @pytest.fixture\n  def module_provider(): pass\n";
        let found = detected(source);
        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|scope| scope.contains("def marked")));
        assert!(found
            .iter()
            .any(|scope| scope.contains("def module_provider")));
        assert!(!found.iter().any(|scope| scope.contains("unavailable")));
    }
    #[test]
    fn lambda_local_bindings_do_not_shadow_the_surrounding_scope() {
        let source =
            "import pytest\nfn = lambda: (pytest := fake)\n@pytest.fixture\ndef subject(): pass\n";
        let found = detected(source);
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("subject"));
    }
    #[test]
    fn nested_class_body_skips_outer_class_bindings_but_its_base_can_use_them() {
        let source = "class Outer:\n import pytest as local\n from unittest import TestCase as Base\n class Inner:\n  @local.fixture\n  def unavailable(self): pass\n class Fixture(Base):\n  def helper(self): pass\n";
        let found = detected(source);
        assert_eq!(found.len(), 1);
        assert!(found[0].starts_with("class Fixture"));
    }
    #[test]
    fn every_parameter_binding_form_shadows_provider_names() {
        for parameter in [
            "pytest=None",
            "pytest: object=None",
            "pytest: object",
            "*pytest",
            "**pytest",
            "*pytest: object",
            "**pytest: object",
        ] {
            let source = format!(
                "import pytest\ndef outer({parameter}):\n @pytest.fixture\n def subject(): pass\n"
            );
            assert!(detected(&source).is_empty(), "{source}");
        }
        for parameter in [
            "value: pytest",
            "value: pytest=None",
            "*values: pytest",
            "**values: pytest",
        ] {
            let source = format!(
                "import pytest\ndef outer({parameter}):\n @pytest.fixture\n def subject(): pass\n"
            );
            let found = detected(&source);
            assert_eq!(found.len(), 1, "{source}");
            assert!(found[0].contains("def subject"));
        }
    }
    #[test]
    fn repeated_and_dotted_imports_preserve_the_bound_root_namespace() {
        let source = "import pytest\nimport pytest\nimport unittest.case\nimport unittest.case\n@pytest.fixture\ndef subject(): pass\nclass Fixture(unittest.TestCase):\n def helper(self): pass\n";
        let found = detected(source);
        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|scope| scope.contains("def subject")));
        assert!(found.iter().any(|scope| scope.contains("def helper")));
    }
}
