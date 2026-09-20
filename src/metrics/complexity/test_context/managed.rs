//! Lexically resolved JVM annotations and CLR attributes, without framework-name heuristics.
use std::ops::Range;
use tree_sitter::Node;

use super::super::Language;

struct Binding {
    name: Option<String>,
    target: String,
    scope: Range<usize>,
}

fn children(node: Node<'_>) -> Vec<Node<'_>> {
    node.named_children(&mut node.walk()).collect()
}

fn nodes(node: Node<'_>) -> Vec<Node<'_>> {
    super::named_preorder(node, |_| true).collect()
}

fn text(node: Node<'_>, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or_default().to_owned()
}

fn path(node: Node<'_>, source: &[u8]) -> String {
    text(node, source).split_whitespace().collect()
}

fn scope(node: Node<'_>) -> Range<usize> {
    let mut parent = node.parent();
    while let Some(p) = parent {
        if matches!(
            p.kind(),
            "class_body"
                | "declaration_list"
                | "block"
                | "source_file"
                | "program"
                | "compilation_unit"
        ) {
            return p.byte_range();
        }
        parent = p.parent();
    }
    node.byte_range()
}

fn import(node: Node<'_>, source: &[u8]) -> Option<Binding> {
    if node.has_error() {
        return None;
    }
    let parts = children(node);
    match node.kind() {
        "import_declaration" | "import" => {
            // A star import never establishes a particular test-provider binding.
            if text(node, source).contains('*') {
                return None;
            }
            let target_node = parts.first()?;
            let target = path(*target_node, source);
            let name = if node.kind() == "import" && parts.len() > 1 {
                path(*parts.last()?, source)
            } else {
                target.rsplit('.').next()?.to_owned()
            };
            Some(Binding {
                name: Some(name),
                target,
                scope: scope(node),
            })
        }
        "using_directive" => {
            if text(node, source).split_whitespace().any(|s| s == "static") {
                return None;
            }
            let target = path(*parts.last()?, source)
                .trim_start_matches("global::")
                .to_owned();
            Some(Binding {
                name: node.child_by_field_name("name").map(|n| path(n, source)),
                target,
                scope: scope(node),
            })
        }
        _ => None,
    }
}

fn shadow(node: Node<'_>, source: &[u8]) -> Option<Binding> {
    if !matches!(
        node.kind(),
        "class_declaration"
            | "interface_declaration"
            | "annotation_type_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "struct_declaration"
            | "object_declaration"
            | "type_alias"
            | "type_parameter"
            | "namespace_declaration"
    ) {
        return None;
    }
    let name = node.child_by_field_name("name").or_else(|| {
        children(node)
            .into_iter()
            .find(|n| matches!(n.kind(), "identifier" | "type_identifier"))
    })?;
    let scope = if node.kind() == "type_parameter" {
        let mut p = node.parent()?;
        while matches!(p.kind(), "type_parameters" | "type_parameter_list") {
            p = p.parent()?;
        }
        p.byte_range()
    } else {
        scope(node)
    };
    Some(Binding {
        name: Some(path(name, source).split('.').next()?.to_owned()),
        target: String::new(),
        scope,
    })
}

fn resolved(reference: &str, at: usize, bindings: &[Binding], csharp: bool) -> Option<String> {
    let absolute = csharp && reference.starts_with("global::");
    let reference = reference.trim_start_matches("global::").replace("::", ".");
    let (head, tail) = reference.split_once('.').unwrap_or((&reference, ""));
    let visible: Vec<_> = bindings.iter().filter(|b| b.scope.contains(&at)).collect();
    let matching: Vec<_> = visible
        .iter()
        .filter(|b| {
            b.name.as_deref().is_some_and(|name| {
                name == head || (csharp && name.strip_suffix("Attribute") == Some(head))
            })
        })
        .collect();
    if !absolute && !matching.is_empty() {
        if matching.len() != 1 || matching[0].target.is_empty() {
            return None;
        }
        return Some(if tail.is_empty() {
            matching[0].target.clone()
        } else {
            format!("{}.{tail}", matching[0].target)
        });
    }
    if !tail.is_empty() {
        return Some(reference);
    }
    let candidates: Vec<_> = visible
        .iter()
        .filter(|b| b.name.is_none())
        .map(|b| format!("{}.{reference}", b.target))
        .filter(|name| class_marker(name, csharp) || method_marker(name, csharp, false))
        .collect();
    if candidates.len() == 1 {
        candidates.into_iter().next()
    } else {
        None
    }
}

fn class_marker(name: &str, csharp: bool) -> bool {
    let name = if csharp {
        name.strip_suffix("Attribute").unwrap_or(name)
    } else {
        name
    };
    if csharp {
        matches!(
            name,
            "NUnit.Framework.TestFixture"
                | "NUnit.Framework.TestFixtureSource"
                | "Microsoft.VisualStudio.TestTools.UnitTesting.TestClass"
        )
    } else {
        matches!(
            name,
            "org.junit.jupiter.api.Nested" | "org.testng.annotations.Test"
        )
    }
}

fn method_marker(name: &str, csharp: bool, kotlin: bool) -> bool {
    let name = if csharp {
        name.strip_suffix("Attribute").unwrap_or(name)
    } else {
        name
    };
    let Some((provider, marker)) = name.rsplit_once('.') else {
        return false;
    };
    if csharp {
        match provider {
            "Xunit" => matches!(marker, "Fact" | "Theory"),
            "NUnit.Framework" => matches!(
                marker,
                "Test"
                    | "TestCase"
                    | "TestCaseSource"
                    | "SetUp"
                    | "TearDown"
                    | "OneTimeSetUp"
                    | "OneTimeTearDown"
            ),
            "Microsoft.VisualStudio.TestTools.UnitTesting" => matches!(
                marker,
                "TestMethod"
                    | "DataTestMethod"
                    | "TestInitialize"
                    | "TestCleanup"
                    | "ClassInitialize"
                    | "ClassCleanup"
                    | "AssemblyInitialize"
                    | "AssemblyCleanup"
            ),
            _ => false,
        }
    } else {
        match provider {
            "org.junit" => matches!(
                marker,
                "Test" | "Before" | "After" | "BeforeClass" | "AfterClass"
            ),
            "org.junit.jupiter.api" => matches!(
                marker,
                "Test"
                    | "RepeatedTest"
                    | "TestFactory"
                    | "TestTemplate"
                    | "BeforeEach"
                    | "AfterEach"
                    | "BeforeAll"
                    | "AfterAll"
            ),
            "org.junit.jupiter.params" => marker == "ParameterizedTest",
            "org.testng.annotations" => matches!(
                marker,
                "Test"
                    | "BeforeSuite"
                    | "AfterSuite"
                    | "BeforeTest"
                    | "AfterTest"
                    | "BeforeGroups"
                    | "AfterGroups"
                    | "BeforeClass"
                    | "AfterClass"
                    | "BeforeMethod"
                    | "AfterMethod"
            ),
            "kotlin.test" if kotlin => matches!(marker, "Test" | "BeforeTest" | "AfterTest"),
            _ => false,
        }
    }
}

fn marker_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.has_error()
        || children(node).iter().any(|n| n.kind() == "use_site_target")
        || node.parent().is_some_and(|p| {
            p.kind() == "attribute_list"
                && children(p)
                    .iter()
                    .any(|n| n.kind() == "attribute_target_specifier")
        })
    {
        return None;
    }
    let name = node
        .child_by_field_name("name")
        .or_else(|| nodes(node).into_iter().find(|n| n.kind() == "user_type"))?;
    Some(path(name, source))
}

pub(super) fn ranges(root: Node<'_>, source: &[u8], lang: Language) -> Vec<Range<usize>> {
    let csharp = lang == Language::CSharp;
    let kotlin = lang == Language::Kotlin;
    let all = nodes(root);
    let bindings: Vec<_> = all
        .iter()
        .filter_map(|n| import(*n, source).or_else(|| shadow(*n, source)))
        .collect();
    all.into_iter()
        .filter(|n| {
            matches!(
                n.kind(),
                "class_declaration"
                    | "object_declaration"
                    | "method_declaration"
                    | "function_declaration"
                    | "local_function_statement"
            )
        })
        // Only declarations pay for the ancestor walk: `parent()` re-descends
        // from the root, so walking every node's ancestors was cubic in depth.
        .filter(|n| {
            !n.has_error()
                && !std::iter::successors(n.parent(), |parent| parent.parent())
                    .any(|parent| parent.is_error())
        })
        .filter(|n| {
            let class = matches!(n.kind(), "class_declaration" | "object_declaration");
            let marked = children(*n)
                .into_iter()
                .filter(|child| matches!(child.kind(), "modifiers" | "attribute_list"))
                .flat_map(nodes)
                .filter(|a| matches!(a.kind(), "annotation" | "marker_annotation" | "attribute"))
                .filter_map(|a| {
                    marker_name(a, source)
                        .and_then(|name| resolved(&name, a.start_byte(), &bindings, csharp))
                })
                .any(|name| {
                    if class {
                        class_marker(&name, csharp)
                    } else {
                        method_marker(&name, csharp, kotlin)
                    }
                });
            let test_base = class
                && !csharp
                && children(*n)
                    .into_iter()
                    .filter(|child| matches!(child.kind(), "superclass" | "delegation_specifiers"))
                    .any(|base| {
                        let types: Vec<_> = if kotlin {
                            children(base)
                                .into_iter()
                                .filter_map(|specifier| {
                                    children(specifier).into_iter().find(|n| {
                                        matches!(n.kind(), "constructor_invocation" | "user_type")
                                    })
                                })
                                .filter_map(|n| {
                                    if n.kind() == "constructor_invocation" {
                                        children(n).into_iter().find(|n| n.kind() == "user_type")
                                    } else {
                                        Some(n)
                                    }
                                })
                                .collect()
                        } else {
                            children(base)
                        };
                        types.into_iter().any(|ty| {
                            resolved(&path(ty, source), ty.start_byte(), &bindings, csharp)
                                .as_deref()
                                == Some("junit.framework.TestCase")
                        })
                    });
            marked || test_base
        })
        .map(|n| n.byte_range())
        .collect()
}

#[cfg(test)]
#[path = "managed_tests.rs"]
mod tests;
