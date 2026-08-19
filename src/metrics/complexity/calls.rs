//! Call-edge extraction (call-graph M1): per-file aggregated caller→callee
//! edges from one tree-sitter parse. Pure — no I/O, no specifier
//! resolution (that happens in the collector's snapshot builder, like
//! class records). TS/JS only by design (design doc D1; the inheritance
//! rung set the precedent for this scoping).

use std::collections::HashMap;
use std::path::Path;

use tree_sitter::Node;

use super::fallback::{detect_language, Language};
use super::inheritance::{import_bindings, namespace_bindings};
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
///
/// Variant order is load-bearing: the derived `Ord` is the edge sort
/// order within a file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    let declared = declared_names(root, content);
    let sites = descendants(root)
        .into_iter()
        .filter(|n| matches!(n.kind(), "call_expression" | "new_expression"))
        .filter_map(|n| {
            let callee = classify_callee(n, content, &imports, &namespaces, &declared);
            let callee = callee?;
            Some((enclosing_caller(n, content), callee))
        });
    aggregate(sites)
}

/// Node kinds whose `name` field (or declarator pattern) introduces a
/// file-local binding a bare call could target.
const DECL_KINDS: &[&str] = &[
    "function_declaration",
    "generator_function_declaration",
    "class_declaration",
    "abstract_class_declaration",
    "variable_declarator",
];

/// Every name the file itself declares (functions, generators, classes,
/// `const`/`let`/`var` declarators at any depth). A bare identifier call
/// is `SameFile` only when its name is in this set — anything else
/// (`fetch`, `setTimeout`, other globals) is a real call whose target we
/// cannot name, and claiming it resolved would inflate the resolution
/// rate the trust floor gates on (review F1).
fn declared_names(root: Node<'_>, content: &str) -> std::collections::HashSet<String> {
    descendants(root)
        .into_iter()
        .filter(|n| DECL_KINDS.contains(&n.kind()))
        .filter_map(|n| n.child_by_field_name("name"))
        // TS class names parse as `type_identifier`; everything else
        // introduces a plain `identifier`.
        .filter(|name| matches!(name.kind(), "identifier" | "type_identifier"))
        .map(|name| text(name, content).to_string())
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

/// Function-scope node kinds whose parameters can shadow a binding.
const SCOPE_FN_KINDS: &[&str] = &[
    "function_declaration",
    "generator_function_declaration",
    "function_expression",
    "generator_function",
    "arrow_function",
    "method_definition",
];

/// Whether `name` is rebound between the call site and the file scope —
/// a parameter or a block-local declaration in any enclosing scope
/// (review F2). Deliberately over-approximates in edge cases: treating a
/// call as shadowed only downgrades it to `Unresolved`, which under-counts
/// and never fabricates (design §1.2).
fn is_shadowed(call: Node<'_>, name: &str, content: &str) -> bool {
    std::iter::successors(call.parent(), |n| n.parent())
        .any(|anc| scope_params_contain(anc, name, content) || block_declares(anc, name, content))
}

fn scope_params_contain(node: Node<'_>, name: &str, content: &str) -> bool {
    SCOPE_FN_KINDS.contains(&node.kind())
        && ["parameters", "parameter"]
            .iter()
            .filter_map(|f| node.child_by_field_name(f))
            .any(|params| {
                let mut idents = Vec::new();
                pattern_identifiers(params, &mut idents);
                idents.iter().any(|n| text(*n, content) == name)
            })
}

fn block_declares(node: Node<'_>, name: &str, content: &str) -> bool {
    if node.kind() != "statement_block" {
        return false;
    }
    (0..node.named_child_count())
        .filter_map(|i| node.named_child(i as u32))
        .filter(|c| matches!(c.kind(), "lexical_declaration" | "variable_declaration"))
        .any(|decl| {
            let mut idents = Vec::new();
            pattern_identifiers(decl, &mut idents);
            idents.iter().any(|n| text(*n, content) == name)
        })
}

/// Identifiers *bound* by a parameter list, declaration, or destructuring
/// pattern — default values and declarator initializers are skipped so an
/// expression on the right-hand side never counts as a binding.
fn pattern_identifiers<'a>(node: Node<'a>, out: &mut Vec<Node<'a>>) {
    match node.kind() {
        "identifier" => out.push(node),
        "required_parameter" | "optional_parameter" => {
            if let Some(p) = node.child_by_field_name("pattern") {
                pattern_identifiers(p, out);
            }
        }
        "assignment_pattern" => {
            if let Some(l) = node.child_by_field_name("left") {
                pattern_identifiers(l, out);
            }
        }
        "variable_declarator" => {
            if let Some(n) = node.child_by_field_name("name") {
                pattern_identifiers(n, out);
            }
        }
        _ => {
            (0..node.named_child_count())
                .filter_map(|i| node.named_child(i as u32))
                .for_each(|c| pattern_identifiers(c, out));
        }
    }
}

fn classify_callee(
    call: Node<'_>,
    content: &str,
    imports: &HashMap<String, (String, String)>,
    namespaces: &HashMap<String, String>,
    declared: &std::collections::HashSet<String>,
) -> Option<RawCalleeRef> {
    // `new Foo()` (D3) classifies exactly like `foo()` — the constructor
    // expression goes through the same binding rules.
    let callee = call
        .child_by_field_name("function")
        .or_else(|| call.child_by_field_name("constructor"))?;
    match callee.kind() {
        "identifier" => {
            let ident = text(callee, content).to_string();
            // A shadowed name targets the local rebinding, not the import
            // or file-level declaration (review F2).
            if is_shadowed(call, &ident, content) {
                return Some(RawCalleeRef::Unresolved { name: ident });
            }
            Some(match imports.get(&ident) {
                Some((specifier, name)) => RawCalleeRef::Specifier {
                    specifier: specifier.clone(),
                    name: name.clone(),
                },
                None if declared.contains(&ident) => RawCalleeRef::SameFile(ident),
                // Unbound identifier — a global or host builtin, not a
                // file-local target (review F1).
                None => RawCalleeRef::Unresolved { name: ident },
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
                .filter(|o| !is_shadowed(call, text(*o, content), content))
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
    edges.sort_by(|a, b| (&a.caller, &a.callee).cmp(&(&b.caller, &b.callee)));
    edges
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
        let e = edges("src/a.ts", "function mk() {}\nconst x = mk();\nx.save();\n");
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
        let e = edges(
            "src/a.ts",
            "function helper() {}\nfunction outer() { helper(); }\n",
        );
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
        let src = "function helper() {}\nfunction outer() {\n  function inner() { helper(); }\n}\n";
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
        let src = "function helper() {}\nclass A {\n  render() { helper(); }\n}\n";
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
        let e = edges(
            "src/a.ts",
            "function helper() {}\nconst go = () => { helper(); };\n",
        );
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
    fn named_import_receiver_is_not_a_namespace() {
        // Only `import * as h` receivers resolve (D4) — a *named* import
        // binding used as a receiver is a value whose members we can't
        // follow (kills the namespace_import kind-check inversion mutant).
        let src = "import { h } from './x';\nh.run();\n";
        let e = edges("src/a.ts", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "<toplevel>".into(),
                callee: RawCalleeRef::Unresolved { name: "run".into() },
                count: 1,
            }]
        );
    }

    #[test]
    fn edge_sort_is_not_input_order() {
        // Call sites appear in REVERSE of the sorted output — a degenerate
        // sort key (any constant) would keep AST order via stable sort.
        let e = edges(
            "src/a.ts",
            "function b() {}\nfunction a() {}\no.save();\nb();\na();\n",
        );
        let callees: Vec<&RawCalleeRef> = e.iter().map(|r| &r.callee).collect();
        assert_eq!(
            callees,
            vec![
                &RawCalleeRef::SameFile("a".into()),
                &RawCalleeRef::SameFile("b".into()),
                &RawCalleeRef::Unresolved {
                    name: "save".into()
                },
            ]
        );
    }

    #[test]
    fn unbound_global_calls_are_unresolved_not_same_file() {
        // Review F1: `fetch`, `setTimeout`, … are neither import-bound nor
        // declared in the file — claiming SameFile would inflate the
        // resolution rate and surface phantom hubs.
        let e = edges("src/a.ts", "fetch(u);\nsetTimeout(cb, 5);\n");
        assert_eq!(
            e,
            vec![
                RawCallEdge {
                    caller: "<toplevel>".into(),
                    callee: RawCalleeRef::Unresolved {
                        name: "fetch".into()
                    },
                    count: 1,
                },
                RawCallEdge {
                    caller: "<toplevel>".into(),
                    callee: RawCalleeRef::Unresolved {
                        name: "setTimeout".into()
                    },
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn const_arrow_declaration_still_counts_as_same_file() {
        // A file-local `const f = () => …` is a real in-file target.
        let e = edges("src/a.ts", "const f = () => 1;\nf();\n");
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "<toplevel>".into(),
                callee: RawCalleeRef::SameFile("f".into()),
                count: 1,
            }]
        );
    }

    #[test]
    fn parameter_shadowing_an_import_downgrades_to_unresolved() {
        // Review F2: the call targets the parameter, not './db' — a
        // resolved edge here would be fabricated.
        let src = "import { save } from './db';\n\
                   export function retry(save: () => void) { save(); }\n";
        let e = edges("src/a.ts", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "retry".into(),
                callee: RawCalleeRef::Unresolved {
                    name: "save".into()
                },
                count: 1,
            }]
        );
    }

    #[test]
    fn local_declaration_shadowing_an_import_downgrades_to_unresolved() {
        let src = "import { save } from './db';\n\
                   export function f() { const save = () => 1; save(); }\n";
        let e = edges("src/a.ts", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "f".into(),
                callee: RawCalleeRef::Unresolved {
                    name: "save".into()
                },
                count: 1,
            }]
        );
    }

    #[test]
    fn parameter_shadowing_a_namespace_import_downgrades_to_unresolved() {
        let src = "import * as h from './x';\nfunction g(h: any) { h.run(); }\n";
        let e = edges("src/a.ts", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "g".into(),
                callee: RawCalleeRef::Unresolved { name: "run".into() },
                count: 1,
            }]
        );
    }

    #[test]
    fn unrelated_parameters_do_not_shadow_an_import() {
        // The shadow check must be scoped to the call's own ancestors —
        // a different function's parameter must not over-trigger it.
        let src = "import { save } from './db';\n\
                   function other(save: any) {}\n\
                   function g(x: any) { save(); }\n";
        let e = edges("src/a.ts", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "g".into(),
                callee: RawCalleeRef::Specifier {
                    specifier: "./db".into(),
                    name: "save".into(),
                },
                count: 1,
            }]
        );
    }

    #[test]
    fn caller_attribution_universe_matches_the_js_functions_query() {
        // Review F5: FN_KINDS hand-mirrors the JS_FUNCTIONS capture set.
        // This fixture holds one call inside every candidate function
        // shape; whatever the query captures must be exactly the set of
        // caller names attribution produces. Extending either list alone
        // fails here.
        let src = "function marker() {}\n\
                   function decl() { marker(); }\n\
                   class C { meth() { marker(); } }\n\
                   function* gen() { marker(); }\n\
                   const arrow = () => { marker(); };\n";
        let grammar: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        let tree = parse(src, &grammar).expect("parse");
        let (query, matches) = super::super::treesitter::collect_matches(
            &tree,
            src.as_bytes(),
            super::super::queries::JS_FUNCTIONS,
            &grammar,
        );
        let query = query.expect("valid query");
        let name_idx = query
            .capture_names()
            .iter()
            .position(|n| *n == "name")
            .expect("@name capture") as u32;
        let query_names: std::collections::HashSet<String> = matches
            .iter()
            .flatten()
            .filter(|(idx, _)| *idx == name_idx)
            .map(|(_, range)| src[range.clone()].to_string())
            .collect();

        let attributed_callers: std::collections::HashSet<String> = edges("src/a.ts", src)
            .into_iter()
            .map(|e| e.caller)
            .filter(|c| c != "<toplevel>")
            .collect();

        // `marker` is a declared function with no calls inside it — it
        // appears in the query universe but never as a caller.
        let expected: std::collections::HashSet<String> = query_names
            .iter()
            .filter(|n| n.as_str() != "marker")
            .cloned()
            .collect();
        assert_eq!(
            attributed_callers, expected,
            "FN_KINDS (attribution) and JS_FUNCTIONS (queries.rs) diverged"
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
