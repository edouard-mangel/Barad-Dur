//! Call-edge extraction (call-graph M1): per-file aggregated caller→callee
//! edges from one tree-sitter parse. Pure — no I/O, no specifier
//! resolution (that happens in the collector's snapshot builder, like
//! class records). TS/JS only by design (design doc D1; the inheritance
//! rung set the precedent for this scoping).

use std::collections::HashMap;
use std::path::Path;

use tree_sitter::Node;

use super::fallback::{detect_language, Language};
use super::inheritance::import_bindings;
use super::lang_dispatch::grammar_for;
use super::pressman::descendants;
use super::treesitter::parse;

/// One aggregated caller→callee edge in a file, pre-resolution (D5:
/// call sites collapse to counted edges at extraction; no line numbers).
#[derive(Debug, Clone, PartialEq)]
pub struct RawCallEdge {
    /// Innermost enclosing *named* function (D2: ancestor walk), or
    /// `"<toplevel>"` for module-level calls and calls inside anonymous
    /// functions.
    pub caller: String,
    pub callee: RawCalleeRef,
    /// Number of call sites aggregated onto this edge.
    pub count: u32,
}

/// The callee as extraction sees it. `Specifier` is resolved to a repo
/// path (or `Unresolved`) by the collector's snapshot builder.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RawCalleeRef {
    /// Callee identifier not bound by any import — assumed same-file.
    SameFile(String),
    /// Callee bound by an import: module specifier + exported name
    /// (aliases unwrapped), or a direct namespace-import receiver (D4).
    Specifier { specifier: String, name: String },
    /// A real call whose target static analysis cannot name: method on a
    /// value, deep/aliased namespace access, or a computed callee
    /// (`"<dynamic>"`). Name kept for honest accounting.
    Unresolved { name: String },
}

/// Extract aggregated call edges from one TS/JS file. Other languages
/// yield none — call-graph extraction is TS/JS-only in M1 (design D1).
pub fn extract_call_edges(path: &Path, content: &str) -> Vec<RawCallEdge> {
    let lang = detect_language(&path.to_string_lossy());
    if !matches!(lang, Language::JsTs) {
        return Vec::new();
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let Some(grammar) = grammar_for(lang, ext) else {
        return Vec::new();
    };
    let Some(tree) = parse(content, &grammar) else {
        return Vec::new();
    };
    call_edges_from_tree(tree.root_node(), content)
}

/// Tree-level core of [`extract_call_edges`], for callers that already
/// hold a parsed TS/JS tree (shared-parse path).
pub(super) fn call_edges_from_tree(root: Node<'_>, content: &str) -> Vec<RawCallEdge> {
    let imports = import_bindings(root, content);
    let namespaces = namespace_bindings(root, content);
    let sites = descendants(root)
        .into_iter()
        .filter(|n| matches!(n.kind(), "call_expression" | "new_expression"))
        .filter_map(|n| {
            let callee = classify_callee(n, content, &imports, &namespaces)?;
            Some((enclosing_caller(n, content), callee))
        });
    aggregate(sites)
}

/// local namespace binding → module specifier (`import * as h from './x'`).
/// Complements `import_bindings`, which covers named and default imports.
fn namespace_bindings(root: Node<'_>, content: &str) -> HashMap<String, String> {
    descendants(root)
        .into_iter()
        .filter(|n| n.kind() == "import_statement")
        .filter_map(|stmt| {
            let source = stmt.child_by_field_name("source")?;
            let specifier = text(source, content)
                .trim_matches(|c| c == '"' || c == '\'')
                .to_string();
            let ns = descendants(stmt)
                .into_iter()
                .find(|n| n.kind() == "namespace_import")?;
            let ident = descendants(ns)
                .into_iter()
                .find(|n| n.kind() == "identifier")?;
            Some((text(ident, content).to_string(), specifier))
        })
        .collect()
}

/// Node kinds that carry a function identity in `FunctionMetrics` — the
/// walk stops at these and nowhere else (D2: arrows and anonymous
/// function expressions are passed through).
const FN_KINDS: &[&str] = &["function_declaration", "method_definition"];

/// Innermost enclosing *named* function of a call site (D2: ancestor
/// walk — the first `FN_KINDS` ancestor is the innermost), or
/// `"<toplevel>"` when none exists.
fn enclosing_caller(call: Node<'_>, content: &str) -> String {
    std::iter::successors(call.parent(), |n| n.parent())
        .find(|n| FN_KINDS.contains(&n.kind()))
        .and_then(|f| f.child_by_field_name("name"))
        .map(|name| text(name, content).to_string())
        .unwrap_or_else(|| "<toplevel>".to_string())
}

fn classify_callee(
    call: Node<'_>,
    content: &str,
    imports: &HashMap<String, (String, String)>,
    namespaces: &HashMap<String, String>,
) -> Option<RawCalleeRef> {
    // `new Foo()` (D3) classifies exactly like `foo()` — the constructor
    // expression goes through the same binding rules.
    let callee = call
        .child_by_field_name("function")
        .or_else(|| call.child_by_field_name("constructor"))?;
    match callee.kind() {
        "identifier" => {
            let ident = text(callee, content).to_string();
            Some(match imports.get(&ident) {
                Some((specifier, name)) => RawCalleeRef::Specifier {
                    specifier: specifier.clone(),
                    name: name.clone(),
                },
                None => RawCalleeRef::SameFile(ident),
            })
        }
        // `h.run()` with a *direct* namespace-import receiver resolves
        // (D4); any other `obj.method()` is unknowable without type info —
        // keep the method name for honest accounting.
        "member_expression" => {
            let prop = callee.child_by_field_name("property")?;
            let name = text(prop, content).to_string();
            let namespace_specifier = callee
                .child_by_field_name("object")
                .filter(|o| o.kind() == "identifier")
                .and_then(|o| namespaces.get(text(o, content)));
            Some(match namespace_specifier {
                Some(specifier) => RawCalleeRef::Specifier {
                    specifier: specifier.clone(),
                    name,
                },
                None => RawCalleeRef::Unresolved { name },
            })
        }
        // Computed or otherwise dynamic callee (`fns[k]()`, IIFEs, …).
        _ => Some(RawCalleeRef::Unresolved {
            name: "<dynamic>".to_string(),
        }),
    }
}

/// Collapse call sites onto counted edges (D5), sorted deterministically.
fn aggregate(sites: impl Iterator<Item = (String, RawCalleeRef)>) -> Vec<RawCallEdge> {
    let counts = sites.fold(
        HashMap::<(String, RawCalleeRef), u32>::new(),
        |mut m, key| {
            *m.entry(key).or_insert(0) += 1;
            m
        },
    );
    let mut edges: Vec<RawCallEdge> = counts
        .into_iter()
        .map(|((caller, callee), count)| RawCallEdge {
            caller,
            callee,
            count,
        })
        .collect();
    edges.sort_by(|a, b| {
        (&a.caller, callee_sort_key(&a.callee)).cmp(&(&b.caller, callee_sort_key(&b.callee)))
    });
    edges
}

/// Stable ordering for edges within a file: variant rank, then names.
fn callee_sort_key(c: &RawCalleeRef) -> (u8, &str, &str) {
    match c {
        RawCalleeRef::SameFile(name) => (0, name, ""),
        RawCalleeRef::Specifier { specifier, name } => (1, specifier, name),
        RawCalleeRef::Unresolved { name } => (2, name, ""),
    }
}

fn text<'a>(node: Node<'_>, content: &'a str) -> &'a str {
    &content[node.byte_range()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn edges(name: &str, src: &str) -> Vec<RawCallEdge> {
        extract_call_edges(Path::new(name), src)
    }

    #[test]
    fn imported_call_records_specifier_with_alias_unwrapped() {
        let src = "import { compute as c } from './tax';\nc(1);\n";
        let e = edges("src/a.ts", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "<toplevel>".into(),
                callee: RawCalleeRef::Specifier {
                    specifier: "./tax".into(),
                    name: "compute".into(),
                },
                count: 1,
            }]
        );
    }

    #[test]
    fn method_call_on_value_is_unresolved_with_method_name() {
        let e = edges("src/a.ts", "const x = mk();\nx.save();\n");
        assert_eq!(
            e,
            vec![
                RawCallEdge {
                    caller: "<toplevel>".into(),
                    callee: RawCalleeRef::SameFile("mk".into()),
                    count: 1,
                },
                RawCallEdge {
                    caller: "<toplevel>".into(),
                    callee: RawCalleeRef::Unresolved {
                        name: "save".into()
                    },
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn computed_callee_is_unresolved_dynamic() {
        let e = edges("src/a.ts", "fns[k]();\n");
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "<toplevel>".into(),
                callee: RawCalleeRef::Unresolved {
                    name: "<dynamic>".into()
                },
                count: 1,
            }]
        );
    }

    #[test]
    fn direct_namespace_receiver_resolves_to_specifier() {
        let src = "import * as h from './helpers';\nh.run();\n";
        let e = edges("src/a.ts", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "<toplevel>".into(),
                callee: RawCalleeRef::Specifier {
                    specifier: "./helpers".into(),
                    name: "run".into(),
                },
                count: 1,
            }]
        );
    }

    #[test]
    fn deep_namespace_receiver_stays_unresolved() {
        // D4: only a *direct* namespace receiver qualifies — `h.a.b()`
        // reaches through a nested object we can't statically follow.
        let src = "import * as h from './helpers';\nh.a.b();\n";
        let e = edges("src/a.ts", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "<toplevel>".into(),
                callee: RawCalleeRef::Unresolved { name: "b".into() },
                count: 1,
            }]
        );
    }

    #[test]
    fn constructor_call_classifies_like_identifier_call() {
        // D3: `new Foo()` follows the same binding rules as `foo()`.
        let src = "import { Foo } from './foo';\nclass Local {}\nnew Foo();\nnew Local();\n";
        let e = edges("src/a.ts", src);
        assert_eq!(
            e,
            vec![
                RawCallEdge {
                    caller: "<toplevel>".into(),
                    callee: RawCalleeRef::SameFile("Local".into()),
                    count: 1,
                },
                RawCallEdge {
                    caller: "<toplevel>".into(),
                    callee: RawCalleeRef::Specifier {
                        specifier: "./foo".into(),
                        name: "Foo".into(),
                    },
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn call_attributes_to_enclosing_function() {
        let e = edges("src/a.ts", "function outer() { helper(); }\n");
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "outer".into(),
                callee: RawCalleeRef::SameFile("helper".into()),
                count: 1,
            }]
        );
    }

    #[test]
    fn nested_function_call_attributes_to_innermost() {
        let src = "function outer() {\n  function inner() { helper(); }\n}\n";
        let e = edges("src/a.ts", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "inner".into(),
                callee: RawCalleeRef::SameFile("helper".into()),
                count: 1,
            }]
        );
    }

    #[test]
    fn method_call_site_attributes_to_method_name() {
        let src = "class A {\n  render() { helper(); }\n}\n";
        let e = edges("src/a.ts", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "render".into(),
                callee: RawCalleeRef::SameFile("helper".into()),
                count: 1,
            }]
        );
    }

    #[test]
    fn call_inside_anonymous_arrow_attributes_to_toplevel() {
        // D2: arrows have no FunctionMetrics identity — the walk passes
        // through them to the nearest *named* function or "<toplevel>".
        let e = edges("src/a.ts", "const go = () => { helper(); };\n");
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "<toplevel>".into(),
                callee: RawCalleeRef::SameFile("helper".into()),
                count: 1,
            }]
        );
    }

    #[test]
    fn repeated_calls_aggregate_onto_one_counted_edge() {
        // D5: sites collapse to edges — kills `+= 1` → `= 1` mutants.
        let e = edges("src/a.ts", "function f() {}\nf();\nf();\nf();\n");
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "<toplevel>".into(),
                callee: RawCalleeRef::SameFile("f".into()),
                count: 3,
            }]
        );
    }

    #[test]
    fn callback_reference_produces_no_edge() {
        // A function passed as a value is not a call — only the `.map`
        // call itself is recorded. Kills mutants that widen extraction
        // to argument identifiers.
        let e = edges("src/a.ts", "arr.map(transform);\n");
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "<toplevel>".into(),
                callee: RawCalleeRef::Unresolved { name: "map".into() },
                count: 1,
            }]
        );
    }

    #[test]
    fn default_import_call_records_specifier() {
        let e = edges("src/a.ts", "import f from './f';\nf();\n");
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "<toplevel>".into(),
                callee: RawCalleeRef::Specifier {
                    specifier: "./f".into(),
                    name: "f".into(),
                },
                count: 1,
            }]
        );
    }

    #[test]
    fn non_jsts_files_yield_no_edges() {
        assert!(edges("src/lib.rs", "fn f() {}\nfn g() { f(); }\n").is_empty());
    }

    #[test]
    fn edges_are_sorted_deterministically() {
        let src = "function z() { b(); a(); }\nfunction a() {}\nfunction b() {}\nz();\n";
        let e = edges("src/a.ts", src);
        let callees: Vec<&RawCalleeRef> = e.iter().map(|r| &r.callee).collect();
        assert_eq!(
            callees,
            vec![
                &RawCalleeRef::SameFile("z".into()),
                &RawCalleeRef::SameFile("a".into()),
                &RawCalleeRef::SameFile("b".into()),
            ],
            "sorted by caller (<toplevel> before z), then callee: {e:?}"
        );
    }

    #[test]
    fn bare_local_call_is_same_file_from_toplevel() {
        let e = edges("src/a.ts", "function f() {}\nf();\n");
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "<toplevel>".into(),
                callee: RawCalleeRef::SameFile("f".into()),
                count: 1,
            }]
        );
    }
}
