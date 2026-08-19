//! Rust call-edge extraction (call-graph M5): the Rust counterpart of
//! `calls.rs`, sharing its edge types and aggregation. Same honesty
//! contract: `SameFile` requires an in-file declaration, shadowed names
//! downgrade, method calls and macro bodies stay out of the resolved set.
//! Specifier strings are shaped like `use` paths so the collector's
//! existing `resolve_rust_import` maps them to files unchanged.

use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use super::calls::{aggregate, RawCallEdge, RawCalleeRef};
use super::pressman::descendants;

/// Tree-level Rust call-edge extraction, for callers that already hold a
/// parsed Rust tree (shared-parse path).
pub(super) fn rust_call_edges_from_tree(root: Node<'_>, content: &str) -> Vec<RawCallEdge> {
    let declared = declared_names(root, content);
    let uses = use_bindings(root, content);
    let sites = descendants(root)
        .into_iter()
        .filter(|n| n.kind() == "call_expression")
        .filter_map(|n| {
            let callee = classify_callee(n, content, &declared, &uses)?;
            Some((enclosing_caller(n, content), callee))
        });
    aggregate(sites)
}

/// local binding → full `use` path (`use crate::a::b as c` binds
/// c → "crate::a::b"). Groups and nesting flattened; globs skipped —
/// a glob binds names we cannot enumerate, so calls through it stay
/// `Unresolved` (under-count, never fabricate).
fn use_bindings(root: Node<'_>, content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for ud in descendants(root)
        .into_iter()
        .filter(|n| n.kind() == "use_declaration")
    {
        if let Some(arg) = ud.child_by_field_name("argument") {
            collect_use(arg, "", content, &mut map);
        }
    }
    map
}

fn join_path(prefix: &str, seg: &str) -> String {
    if prefix.is_empty() {
        seg.to_string()
    } else {
        format!("{prefix}::{seg}")
    }
}

fn collect_use(node: Node<'_>, prefix: &str, content: &str, out: &mut HashMap<String, String>) {
    match node.kind() {
        "identifier" => {
            let name = text(node, content).to_string();
            out.insert(name.clone(), join_path(prefix, &name));
        }
        "scoped_identifier" => {
            let full = join_path(prefix, text(node, content));
            let local = node
                .child_by_field_name("name")
                .map(|n| text(n, content).to_string())
                .unwrap_or_else(|| full.rsplit("::").next().unwrap_or(&full).to_string());
            out.insert(local, full);
        }
        "use_as_clause" => {
            let (Some(path), Some(alias)) = (
                node.child_by_field_name("path"),
                node.child_by_field_name("alias"),
            ) else {
                return;
            };
            let full = join_path(prefix, text(path, content));
            out.insert(text(alias, content).to_string(), full);
        }
        "scoped_use_list" => {
            let new_prefix = node
                .child_by_field_name("path")
                .map(|p| join_path(prefix, text(p, content)))
                .unwrap_or_else(|| prefix.to_string());
            if let Some(list) = node.child_by_field_name("list") {
                collect_use(list, &new_prefix, content, out);
            }
        }
        "use_list" => {
            (0..node.named_child_count())
                .filter_map(|i| node.named_child(i as u32))
                .for_each(|c| collect_use(c, prefix, content, out));
        }
        // use_wildcard and anything else: no enumerable bindings.
        _ => {}
    }
}

/// Item kinds whose `name` introduces a file-local binding a bare call
/// could target (fns anywhere incl. impl blocks, tuple-struct and enum
/// constructors).
const DECL_KINDS: &[&str] = &["function_item", "struct_item", "enum_item"];

/// Every name this file declares — the `SameFile` gate (review F1 parity).
fn declared_names(root: Node<'_>, content: &str) -> HashSet<String> {
    descendants(root)
        .into_iter()
        .filter(|n| DECL_KINDS.contains(&n.kind()))
        .filter_map(|n| n.child_by_field_name("name"))
        .map(|name| text(name, content).to_string())
        .collect()
}

/// Caller identity universe: `function_item` only, matching what the
/// `RUST_FUNCTIONS` query captures into `FunctionMetrics` (design D2).
const FN_KINDS: &[&str] = &["function_item"];

/// Innermost enclosing named function (closures pass through), or
/// `"<toplevel>"` for const/static initializers and module-level code.
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
    declared: &HashSet<String>,
    uses: &HashMap<String, String>,
) -> Option<RawCalleeRef> {
    let mut callee = call.child_by_field_name("function")?;
    // `helper::<T>()` — the turbofish wraps the real callee.
    if callee.kind() == "generic_function" {
        callee = callee.child_by_field_name("function")?;
    }
    match callee.kind() {
        "identifier" => {
            let ident = text(callee, content).to_string();
            // A shadowed name targets the local rebinding (review F2).
            if is_shadowed(call, &ident, content) {
                return Some(RawCalleeRef::Unresolved { name: ident });
            }
            Some(match uses.get(&ident) {
                Some(full) => specifier_for(full),
                None if declared.contains(&ident) => RawCalleeRef::SameFile(ident),
                // Prelude names (`Some`, `drop`, …) and out-of-scope
                // bindings — a real call whose target we cannot name.
                None => RawCalleeRef::Unresolved { name: ident },
            })
        }
        "scoped_identifier" => Some(classify_scoped(text(callee, content), declared, uses)),
        // `x.method()` / `self.method()` — unknowable without type info.
        "field_expression" => {
            let field = callee.child_by_field_name("field")?;
            Some(RawCalleeRef::Unresolved {
                name: text(field, content).to_string(),
            })
        }
        // Computed callee (`fns[i]()`, closure results, …).
        _ => Some(RawCalleeRef::Unresolved {
            name: "<dynamic>".to_string(),
        }),
    }
}

/// Function-scope kinds whose parameters can shadow a binding.
const SCOPE_FN_KINDS: &[&str] = &["function_item", "closure_expression"];

/// Whether `name` is rebound between the call site and the file scope —
/// a parameter or a `let` binding in an enclosing scope (review F2
/// parity). Over-approximates only toward `Unresolved` (under-count).
fn is_shadowed(call: Node<'_>, name: &str, content: &str) -> bool {
    std::iter::successors(call.parent(), |n| n.parent())
        .any(|anc| params_contain(anc, name, content) || block_lets(anc, name, content))
}

fn params_contain(node: Node<'_>, name: &str, content: &str) -> bool {
    SCOPE_FN_KINDS.contains(&node.kind())
        && node
            .child_by_field_name("parameters")
            .is_some_and(|params| {
                descendants(params)
                    .into_iter()
                    .filter(|n| n.kind() == "identifier")
                    .any(|n| text(n, content) == name)
            })
}

fn block_lets(node: Node<'_>, name: &str, content: &str) -> bool {
    if node.kind() != "block" {
        return false;
    }
    (0..node.named_child_count())
        .filter_map(|i| node.named_child(i as u32))
        .filter(|c| c.kind() == "let_declaration")
        .filter_map(|l| l.child_by_field_name("pattern"))
        .any(|pat| {
            pat.kind() == "identifier" && text(pat, content) == name
                || descendants(pat)
                    .into_iter()
                    .filter(|n| n.kind() == "identifier")
                    .any(|n| text(n, content) == name)
        })
}

/// Classify a `a::b::f()` path callee: expand the head through the `use`
/// table, then split off the callee name. A capitalized second-to-last
/// segment is a type by Rust convention (`Type::assoc_fn`), so the file
/// target is the *module* declaring the type, not a `Type.rs` that never
/// exists.
fn classify_scoped(
    path_text: &str,
    declared: &HashSet<String>,
    uses: &HashMap<String, String>,
) -> RawCalleeRef {
    let segments: Vec<&str> = path_text.split("::").collect();
    let (Some(head), Some(&name)) = (segments.first(), segments.last()) else {
        return RawCalleeRef::Unresolved {
            name: path_text.to_string(),
        };
    };
    // `Self::f()` is only legal inside an impl in this very file.
    if *head == "Self" {
        return RawCalleeRef::SameFile(name.to_string());
    }
    let expanded: Vec<String> = match uses.get(*head) {
        Some(full) => full
            .split("::")
            .map(str::to_string)
            .chain(segments[1..].iter().map(|s| s.to_string()))
            .collect(),
        None => segments.iter().map(|s| s.to_string()).collect(),
    };
    let name = expanded.last().cloned().unwrap_or_default();
    let type_segment = expanded
        .len()
        .checked_sub(2)
        .map(|i| expanded[i].as_str())
        .filter(|s| s.chars().next().is_some_and(char::is_uppercase));
    let module_end = if type_segment.is_some() {
        expanded.len() - 2 // drop `Type::name` — the module declares the type
    } else {
        expanded.len() - 1 // drop `name`
    };
    if module_end == 0 {
        // `Foo::new()` with no path left: same-file when the type is
        // declared here, otherwise unknowable.
        return match type_segment {
            Some(t) if declared.contains(t) => RawCalleeRef::SameFile(name),
            _ => RawCalleeRef::Unresolved { name },
        };
    }
    RawCalleeRef::Specifier {
        specifier: expanded[..module_end].join("::"),
        name,
    }
}

/// A full logical path (`crate::metrics::median`) as a callee: the module
/// part becomes the specifier the collector resolves via
/// `resolve_rust_import`; the last segment is the callee name.
fn specifier_for(full: &str) -> RawCalleeRef {
    match full.rsplit_once("::") {
        Some((module, name)) => RawCalleeRef::Specifier {
            specifier: module.to_string(),
            name: name.to_string(),
        },
        None => RawCalleeRef::Unresolved {
            name: full.to_string(),
        },
    }
}

fn text<'a>(node: Node<'_>, content: &'a str) -> &'a str {
    &content[node.byte_range()]
}

#[cfg(test)]
mod tests {
    use super::super::calls::{extract_call_edges, RawCallEdge, RawCalleeRef};
    use std::path::Path;

    fn edges(name: &str, src: &str) -> Vec<RawCallEdge> {
        extract_call_edges(Path::new(name), src)
    }

    #[test]
    fn use_bound_call_records_module_specifier() {
        let src = "use crate::metrics::median;\nfn f() { median(); }\n";
        let e = edges("src/lib.rs", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "f".into(),
                callee: RawCalleeRef::Specifier {
                    specifier: "crate::metrics".into(),
                    name: "median".into(),
                },
                count: 1,
            }]
        );
    }

    #[test]
    fn aliased_use_unwraps_to_original_name() {
        let src = "use crate::metrics::median as mid;\nfn f() { mid(); }\n";
        let e = edges("src/lib.rs", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "f".into(),
                callee: RawCalleeRef::Specifier {
                    specifier: "crate::metrics".into(),
                    name: "median".into(),
                },
                count: 1,
            }]
        );
    }

    #[test]
    fn use_list_and_nested_groups_bind_each_name() {
        let src = "use crate::a::{f, g as h, b::k};\nfn r() { f(); h(); k(); }\n";
        let e = edges("src/lib.rs", src);
        let spec = |s: &str, n: &str| RawCalleeRef::Specifier {
            specifier: s.into(),
            name: n.into(),
        };
        let callees: Vec<&RawCalleeRef> = e.iter().map(|r| &r.callee).collect();
        assert_eq!(
            callees,
            vec![
                &spec("crate::a", "f"),
                &spec("crate::a", "g"),
                &spec("crate::a::b", "k"),
            ]
        );
    }

    #[test]
    fn fully_qualified_crate_call_records_module_specifier() {
        let src = "fn f() { crate::metrics::median(); }\n";
        let e = edges("src/lib.rs", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "f".into(),
                callee: RawCalleeRef::Specifier {
                    specifier: "crate::metrics".into(),
                    name: "median".into(),
                },
                count: 1,
            }]
        );
    }

    #[test]
    fn use_bound_module_head_expands_to_full_path() {
        let src = "use crate::metrics;\nfn f() { metrics::median(); }\n";
        let e = edges("src/lib.rs", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "f".into(),
                callee: RawCalleeRef::Specifier {
                    specifier: "crate::metrics".into(),
                    name: "median".into(),
                },
                count: 1,
            }]
        );
    }

    #[test]
    fn assoc_fn_on_in_file_type_is_same_file() {
        let src = "struct Foo;\nimpl Foo { fn new() -> Self { Foo } }\nfn f() { Foo::new(); }\n";
        let e = edges("src/lib.rs", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "f".into(),
                callee: RawCalleeRef::SameFile("new".into()),
                count: 1,
            }]
        );
    }

    #[test]
    fn assoc_fn_on_use_bound_type_targets_declaring_module() {
        // `use crate::x::Foo; Foo::new()` — the file declaring Foo is the
        // module file crate::x (capitalization convention: the segment
        // before the method is a type, not a module).
        let src = "use crate::x::Foo;\nfn f() { Foo::new(); }\n";
        let e = edges("src/lib.rs", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "f".into(),
                callee: RawCalleeRef::Specifier {
                    specifier: "crate::x".into(),
                    name: "new".into(),
                },
                count: 1,
            }]
        );
    }

    #[test]
    fn self_type_assoc_call_is_same_file() {
        let src = "struct A;\nimpl A { fn go(&self) { Self::helper(); }\n fn helper() {} }\n";
        let e = edges("src/lib.rs", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "go".into(),
                callee: RawCalleeRef::SameFile("helper".into()),
                count: 1,
            }]
        );
    }

    #[test]
    fn unexpandable_head_stays_a_specifier_for_honest_failure() {
        // `super::x::f()` — resolve_rust_import can't map super paths, so
        // the collector will downgrade to Unresolved; extraction still
        // emits the fact rather than guessing.
        let src = "fn f() { super::progress::tick(); }\n";
        let e = edges("src/lib.rs", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "f".into(),
                callee: RawCalleeRef::Specifier {
                    specifier: "super::progress".into(),
                    name: "tick".into(),
                },
                count: 1,
            }]
        );
    }

    #[test]
    fn method_calls_are_unresolved_with_method_name() {
        let src = "fn f(x: Vec<u32>) { x.sort(); self.tick(); }\n";
        let e = edges("src/lib.rs", src);
        let callees: Vec<&RawCalleeRef> = e.iter().map(|r| &r.callee).collect();
        assert_eq!(
            callees,
            vec![
                &RawCalleeRef::Unresolved {
                    name: "sort".into()
                },
                &RawCalleeRef::Unresolved {
                    name: "tick".into()
                },
            ]
        );
    }

    #[test]
    fn macro_invocations_produce_no_edges() {
        // `format!` is not a call edge, and calls inside a macro's token
        // tree are not parsed as call_expressions (documented under-count).
        let src = "fn helper() -> u32 { 1 }\nfn f() { println!(\"{}\", helper()); }\n";
        assert!(edges("src/lib.rs", src).is_empty());
    }

    #[test]
    fn prelude_names_are_unresolved() {
        let e = edges("src/lib.rs", "fn f() { drop(1); }\n");
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "f".into(),
                callee: RawCalleeRef::Unresolved {
                    name: "drop".into()
                },
                count: 1,
            }]
        );
    }

    #[test]
    fn generic_turbofish_call_classifies_its_function() {
        let src = "fn helper<T>(x: T) {}\nfn f() { helper::<u32>(1); }\n";
        let e = edges("src/lib.rs", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "f".into(),
                callee: RawCalleeRef::SameFile("helper".into()),
                count: 1,
            }]
        );
    }

    #[test]
    fn closure_calls_attribute_to_enclosing_named_fn() {
        let src = "fn helper() {}\nfn f() { let go = || helper(); go(); }\n";
        let e = edges("src/lib.rs", src);
        // helper() inside the closure attributes to f; go() is a call to
        // a let-bound closure — declared as a local, hence Unresolved
        // (not a file-level target).
        assert_eq!(e.len(), 2, "{e:?}");
        assert_eq!(e[0].caller, "f");
        assert_eq!(e[0].callee, RawCalleeRef::SameFile("helper".into()));
        assert_eq!(e[1].caller, "f");
    }

    #[test]
    fn const_initializer_call_attributes_to_toplevel() {
        let src = "fn init() -> u32 { 1 }\nstatic X: u32 = init();\n";
        let e = edges("src/lib.rs", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "<toplevel>".into(),
                callee: RawCalleeRef::SameFile("init".into()),
                count: 1,
            }]
        );
    }

    #[test]
    fn repeated_calls_aggregate_counts() {
        let src = "fn helper() {}\nfn f() { helper(); helper(); helper(); }\n";
        let e = edges("src/lib.rs", src);
        assert_eq!(e[0].count, 3);
        assert_eq!(e.len(), 1);
    }

    #[test]
    fn parameter_shadowing_a_use_binding_downgrades_to_unresolved() {
        // Review F2 parity for Rust: the call targets the parameter.
        let src = "use crate::db::save;\nfn retry(save: fn()) { save(); }\n";
        let e = edges("src/lib.rs", src);
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
    fn let_binding_shadowing_downgrades_to_unresolved() {
        let src = "use crate::db::save;\nfn f() { let save = || 1; save(); }\n";
        let e = edges("src/lib.rs", src);
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
    fn unrelated_fn_params_do_not_shadow() {
        let src = "use crate::db::save;\nfn other(save: fn()) {}\nfn f() { save(); }\n";
        let e = edges("src/lib.rs", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "f".into(),
                callee: RawCalleeRef::Specifier {
                    specifier: "crate::db".into(),
                    name: "save".into(),
                },
                count: 1,
            }]
        );
    }

    #[test]
    fn caller_attribution_universe_matches_the_rust_functions_query() {
        // F5 parity: FN_KINDS mirrors what RUST_FUNCTIONS captures into
        // FunctionMetrics. One call inside every candidate shape — the
        // query's names must equal the attributed callers exactly.
        let src = "fn marker() {}\n\
                   fn free() { marker(); }\n\
                   struct S;\n\
                   impl S { fn method(&self) { marker(); } }\n\
                   static X: fn() = || { marker(); };\n";
        let grammar: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let tree = super::super::treesitter::parse(src, &grammar).expect("parse");
        let (query, matches) = super::super::treesitter::collect_matches(
            &tree,
            src.as_bytes(),
            super::super::queries::RUST_FUNCTIONS,
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
        let attributed: std::collections::HashSet<String> = edges("src/lib.rs", src)
            .into_iter()
            .map(|e| e.caller)
            .filter(|c| c != "<toplevel>")
            .collect();
        let expected: std::collections::HashSet<String> = query_names
            .iter()
            .filter(|n| n.as_str() != "marker")
            .cloned()
            .collect();
        assert_eq!(
            attributed, expected,
            "FN_KINDS (attribution) and RUST_FUNCTIONS (queries.rs) diverged"
        );
    }

    #[test]
    fn bare_declared_call_is_same_file_with_fn_attribution() {
        let src = "fn helper() {}\nfn run() { helper(); }\n";
        let e = edges("src/lib.rs", src);
        assert_eq!(
            e,
            vec![RawCallEdge {
                caller: "run".into(),
                callee: RawCalleeRef::SameFile("helper".into()),
                count: 1,
            }]
        );
    }
}
