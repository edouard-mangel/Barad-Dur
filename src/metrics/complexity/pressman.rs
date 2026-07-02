//! Pressman coupling detectors: content, common, and control coupling
//! findings extracted from a single file's AST. Pure — no I/O.

use std::path::Path;

use tree_sitter::Node;

use crate::snapshot::{CouplingFinding, CouplingKind};

use super::fallback::{detect_language, Language};
use super::lang_dispatch::grammar_for;
use super::treesitter::parse;

/// Extract Pressman coupling findings from one file's source.
/// Returns an empty Vec for unsupported languages or parse failures.
pub fn extract_coupling_findings(path: &Path, content: &str) -> Vec<CouplingFinding> {
    let lang = detect_language(&path.to_string_lossy());
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let Some(grammar) = grammar_for(lang, ext) else {
        return Vec::new();
    };
    let Some(tree) = parse(content, &grammar) else {
        return Vec::new();
    };
    match lang {
        Language::Rust => rust_findings(tree.root_node(), content, path),
        Language::JsTs => js_findings(tree.root_node(), content, path),
        _ => Vec::new(),
    }
}

/// All nodes of a subtree, preorder.
fn descendants(root: Node<'_>) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        out.push(n);
        for i in (0..n.child_count()).rev() {
            if let Some(c) = n.child(i as u32) {
                stack.push(c);
            }
        }
    }
    out
}

/// All nodes of a subtree, preorder, without descending into nested
/// scopes (closures and inner functions) whose parameters shadow the
/// enclosing function's.
fn same_scope_descendants(root: Node<'_>) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        out.push(n);
        if n != root && matches!(n.kind(), "closure_expression" | "function_item") {
            continue;
        }
        for i in (0..n.child_count()).rev() {
            if let Some(c) = n.child(i as u32) {
                stack.push(c);
            }
        }
    }
    out
}

fn text<'a>(node: Node<'_>, content: &'a str) -> &'a str {
    &content[node.byte_range()]
}

fn finding(path: &Path, node: Node<'_>, kind: CouplingKind, content: &str) -> CouplingFinding {
    let evidence: String = text(node, content)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .chars()
        .take(120)
        .collect();
    CouplingFinding {
        path: path.to_path_buf(),
        line: Some(node.start_position().row + 1),
        kind,
        evidence,
    }
}

/// True when `word` appears in `hay` with non-identifier characters (or
/// string boundaries) on both sides.
fn contains_word(hay: &str, word: &str) -> bool {
    hay.match_indices(word).any(|(i, _)| {
        let before_ok = !hay[..i]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_');
        let after_ok = !hay[i + word.len()..]
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '_');
        before_ok && after_ok
    })
}

// ── Rust detectors ─────────────────────────────────────────────────

fn rust_findings(root: Node<'_>, content: &str, path: &Path) -> Vec<CouplingFinding> {
    descendants(root)
        .into_iter()
        .filter_map(|n| match n.kind() {
            "static_item" => rust_common(n, content, path),
            "attribute_item" => rust_content(n, content, path),
            "function_item" => rust_control(n, content, path),
            _ => None,
        })
        .collect()
}

/// Interior-mutability type markers. Substring match over the whole item
/// text implements the look-through rule: `LazyLock<Mutex<…>>` matches
/// `Mutex<`, while `LazyLock<Regex>` matches nothing.
const INTERIOR_MUTABILITY: &[&str] = &[
    "Mutex<",
    "RwLock<",
    "RefCell<",
    "Cell<",
    "UnsafeCell<",
    "Atomic",
];

fn rust_common(node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let is_mut = (0..node.child_count())
        .filter_map(|i| node.child(i as u32))
        .any(|c| c.kind() == "mutable_specifier");
    let item_text = text(node, content);
    let interior = INTERIOR_MUTABILITY.iter().any(|p| item_text.contains(p));
    (is_mut || interior).then(|| finding(path, node, CouplingKind::Common, content))
}

fn rust_content(node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let normalized: String = text(node, content)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    normalized
        .starts_with("#[path=")
        .then(|| finding(path, node, CouplingKind::Content, content))
}

fn rust_control(node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let is_pub = (0..node.child_count())
        .filter_map(|i| node.child(i as u32))
        .any(|c| c.kind() == "visibility_modifier");
    if !is_pub {
        return None;
    }
    let params = node.child_by_field_name("parameters")?;
    let bool_params: Vec<&str> = (0..params.child_count())
        .filter_map(|i| params.child(i as u32))
        .filter(|p| p.kind() == "parameter")
        .filter(|p| {
            p.child_by_field_name("type")
                .is_some_and(|t| text(t, content) == "bool")
        })
        .filter_map(|p| {
            p.child_by_field_name("pattern")
                .map(|pat| text(pat, content))
        })
        .collect();
    if bool_params.is_empty() {
        return None;
    }
    let body = node.child_by_field_name("body")?;
    let branched = same_scope_descendants(body).into_iter().any(|n| {
        let cond = match n.kind() {
            "if_expression" | "while_expression" => n.child_by_field_name("condition"),
            "match_expression" => n.child_by_field_name("value"),
            _ => None,
        };
        cond.is_some_and(|c| {
            bool_params
                .iter()
                .any(|p| contains_word(text(c, content), p))
        })
    });
    branched.then(|| finding(path, node, CouplingKind::Control, content))
}

// ── TS/JS detectors ────────────────────────────────────────────────

fn js_findings(root: Node<'_>, content: &str, path: &Path) -> Vec<CouplingFinding> {
    descendants(root)
        .into_iter()
        .filter_map(|n| match n.kind() {
            "export_statement" => js_export(n, content, path),
            "assignment_expression" => js_global_write(n, content, path),
            "class_declaration" | "class" => js_singleton(n, content, path),
            _ => None,
        })
        .collect()
}

/// `export let` / `export var` → Common. `export function` → control check.
fn js_export(node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let decl = node.child_by_field_name("declaration")?;
    match decl.kind() {
        "lexical_declaration" if is_let_declaration(decl, content) => {
            Some(finding(path, node, CouplingKind::Common, content))
        }
        "variable_declaration" => Some(finding(path, node, CouplingKind::Common, content)),
        "function_declaration" => js_control(decl, content, path),
        _ => None,
    }
}

/// True when a `lexical_declaration`'s leading keyword token is `let`
/// (as opposed to `const`). Inspects the AST token, so destructuring
/// (`let[a, b]`) and wrapped declarations (`let\n  x`) are handled.
fn is_let_declaration(decl: Node<'_>, content: &str) -> bool {
    decl.child(0).is_some_and(|kw| text(kw, content) == "let")
}

/// Assignment to `globalThis.x` / `window.x` → Common.
fn js_global_write(node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let left = node.child_by_field_name("left")?;
    if left.kind() != "member_expression" {
        return None;
    }
    let obj = left.child_by_field_name("object")?;
    let is_global =
        obj.kind() == "identifier" && matches!(text(obj, content), "globalThis" | "window");
    is_global.then(|| finding(path, node, CouplingKind::Common, content))
}

/// Class with a static `instance` field or static `getInstance()` → Common.
fn js_singleton(class_node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let body = class_node.child_by_field_name("body")?;
    (0..body.child_count())
        .filter_map(|i| body.child(i as u32))
        .find_map(|member| {
            let is_static = (0..member.child_count())
                .filter_map(|i| member.child(i as u32))
                .any(|c| text(c, content) == "static");
            if !is_static {
                return None;
            }
            // Try to get name from field_name or property_identifier
            let name = member
                .child_by_field_name("name")
                .or_else(|| {
                    (0..member.child_count())
                        .filter_map(|i| member.child(i as u32))
                        .find(|c| matches!(c.kind(), "property_identifier" | "identifier"))
                })
                .map(|n| text(n, content))?;
            let hit = (member.kind() == "method_definition" && name == "getInstance")
                || (matches!(
                    member.kind(),
                    "field_definition" | "public_field_definition"
                ) && name == "instance");
            hit.then(|| finding(path, member, CouplingKind::Common, content))
        })
}

/// Exported function whose boolean parameter (TS annotation or JS
/// `= true/false` default) is branched on in the body → Control.
fn js_control(func: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let params = func.child_by_field_name("parameters")?;
    let flag_names: Vec<&str> = descendants(params)
        .into_iter()
        .filter_map(|p| match p.kind() {
            // TS: required_parameter / optional_parameter with `: boolean`
            "required_parameter" | "optional_parameter" => {
                let is_bool = (0..p.child_count())
                    .filter_map(|i| p.child(i as u32))
                    .any(|c| c.kind() == "type_annotation" && text(c, content).contains("boolean"));
                let pat = p.child_by_field_name("pattern")?;
                (is_bool && pat.kind() == "identifier").then(|| text(pat, content))
            }
            // JS: `param = true` / `param = false`
            "assignment_pattern" => {
                let right_is_bool = p
                    .child_by_field_name("right")
                    .is_some_and(|r| matches!(r.kind(), "true" | "false"));
                let left = p.child_by_field_name("left")?;
                (right_is_bool && left.kind() == "identifier").then(|| text(left, content))
            }
            _ => None,
        })
        .collect();
    if flag_names.is_empty() {
        return None;
    }
    let body = func.child_by_field_name("body")?;
    let branched = descendants(body).into_iter().any(|n| {
        let cond = match n.kind() {
            "if_statement" | "while_statement" | "ternary_expression" => {
                n.child_by_field_name("condition")
            }
            _ => None,
        };
        cond.is_some_and(|c| {
            flag_names
                .iter()
                .any(|f| contains_word(text(c, content), f))
        })
    });
    branched.then(|| finding(path, func, CouplingKind::Control, content))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::CouplingKind;
    use std::path::Path;

    fn findings_for(name: &str, content: &str) -> Vec<crate::snapshot::CouplingFinding> {
        extract_coupling_findings(Path::new(name), content)
    }

    // ── Rust common coupling ───────────────────────────────────────

    #[test]
    fn rust_static_mut_is_common_coupling() {
        let f = findings_for("src/a.rs", "static mut CACHE: usize = 0;\n");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, CouplingKind::Common);
        assert_eq!(f[0].line, Some(1));
        assert!(f[0].evidence.contains("static mut CACHE"));
    }

    #[test]
    fn rust_static_mutex_is_common_coupling() {
        let src =
            "use std::sync::Mutex;\nstatic REGISTRY: Mutex<Vec<u32>> = Mutex::new(Vec::new());\n";
        let f = findings_for("src/a.rs", src);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, CouplingKind::Common);
        assert_eq!(f[0].line, Some(2));
    }

    #[test]
    fn rust_atomic_static_is_common_coupling() {
        let src = "static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);\n";
        assert_eq!(findings_for("src/a.rs", src).len(), 1);
    }

    #[test]
    fn rust_lazylock_wrapping_mutex_is_flagged_lookthrough() {
        let src = "static STATE: std::sync::LazyLock<std::sync::Mutex<u32>> = std::sync::LazyLock::new(|| std::sync::Mutex::new(0));\n";
        let f = findings_for("src/a.rs", src);
        assert_eq!(
            f.len(),
            1,
            "write-once wrapper around Mutex is a mutable global"
        );
    }

    #[test]
    fn rust_pure_lazylock_is_not_flagged() {
        let src = "static KEYWORDS: std::sync::LazyLock<Vec<&'static str>> = std::sync::LazyLock::new(Vec::new);\n";
        assert!(
            findings_for("src/a.rs", src).is_empty(),
            "write-once pure static is not common coupling"
        );
    }

    #[test]
    fn rust_plain_immutable_static_is_not_flagged() {
        assert!(findings_for("src/a.rs", "static MAX: usize = 10;\n").is_empty());
    }

    // ── Rust content coupling ──────────────────────────────────────

    #[test]
    fn rust_path_attribute_is_content_coupling() {
        let src = "#[path = \"../other/impl.rs\"]\nmod stolen;\n";
        let f = findings_for("src/a.rs", src);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, CouplingKind::Content);
        assert!(f[0].evidence.contains("#[path"));
    }

    #[test]
    fn rust_other_attributes_are_not_flagged() {
        let src = "#[derive(Debug)]\n#[cfg(test)]\nstruct Foo;\n";
        assert!(findings_for("src/a.rs", src).is_empty());
    }

    #[test]
    fn unsupported_language_returns_empty() {
        assert!(findings_for("script.py", "x = 1\n").is_empty());
    }

    #[test]
    fn unparseable_extension_returns_empty() {
        assert!(findings_for("notes.txt", "hello\n").is_empty());
    }

    // ── Rust control coupling ──────────────────────────────────────

    #[test]
    fn pub_fn_with_branched_bool_is_control_coupling() {
        let src = "pub fn render(compact: bool) {\n    if compact {\n        short();\n    } else {\n        long();\n    }\n}\n";
        let f = findings_for("src/a.rs", src);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, CouplingKind::Control);
        assert!(f[0].evidence.contains("pub fn render"));
    }

    #[test]
    fn pub_fn_with_matched_bool_is_control_coupling() {
        let src = "pub fn go(fast: bool) {\n    match fast {\n        true => sprint(),\n        false => walk(),\n    }\n}\n";
        assert_eq!(findings_for("src/a.rs", src).len(), 1);
    }

    #[test]
    fn private_fn_with_branched_bool_is_not_flagged() {
        let src = "fn helper(flag: bool) {\n    if flag {\n        a();\n    }\n}\n";
        assert!(
            findings_for("src/a.rs", src).is_empty(),
            "coupling is inter-module; private fns exempt"
        );
    }

    #[test]
    fn pub_fn_with_stored_bool_is_not_flagged() {
        let src = "pub fn set_visible(visible: bool) {\n    STATE_VISIBLE.store(visible);\n}\n";
        assert!(
            findings_for("src/a.rs", src).is_empty(),
            "bool-as-data is not control coupling"
        );
    }

    #[test]
    fn pub_fn_without_bool_params_is_not_flagged() {
        let src = "pub fn add(a: u32, b: u32) -> u32 {\n    if a > b { a } else { b }\n}\n";
        assert!(findings_for("src/a.rs", src).is_empty());
    }

    #[test]
    fn closure_shadowing_bool_param_is_not_flagged() {
        let src = "pub fn outer(flag: bool) {\n    let f = |flag: bool| {\n        if flag {\n            do_it();\n        }\n    };\n    f(true);\n}\n";
        assert!(
            findings_for("src/a.rs", src).is_empty(),
            "closure's own bool param must not be attributed to the outer fn"
        );
    }

    #[test]
    fn bool_branched_outside_closure_is_still_flagged() {
        let src = "pub fn outer(flag: bool) {\n    let f = || do_it();\n    if flag {\n        f();\n    }\n}\n";
        assert_eq!(findings_for("src/a.rs", src).len(), 1);
    }

    #[test]
    fn similarly_named_variable_does_not_false_positive() {
        // param `flag` unused in branches; local `flagged` is branched on
        let src = "pub fn f(flag: bool) {\n    let flagged = compute();\n    if flagged {\n        a(flag);\n    }\n}\n";
        assert!(
            findings_for("src/a.rs", src).is_empty(),
            "word-boundary match must not catch 'flagged'"
        );
    }

    // ── TS/JS common coupling ──────────────────────────────────────

    #[test]
    fn ts_export_let_is_common_coupling() {
        let f = findings_for("src/state.ts", "export let counter = 0;\n");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, CouplingKind::Common);
    }

    #[test]
    fn js_export_var_is_common_coupling() {
        assert_eq!(
            findings_for("src/state.js", "export var mode = 'a';\n").len(),
            1
        );
    }

    #[test]
    fn ts_export_const_is_not_flagged() {
        assert!(findings_for("src/config.ts", "export const MAX = 10;\n").is_empty());
    }

    #[test]
    fn js_globalthis_write_is_common_coupling() {
        let f = findings_for("src/boot.js", "globalThis.appState = {};\n");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, CouplingKind::Common);
    }

    #[test]
    fn js_window_write_is_common_coupling() {
        assert_eq!(
            findings_for("src/boot.js", "window.cache = new Map();\n").len(),
            1
        );
    }

    #[test]
    fn js_reading_window_is_not_flagged() {
        assert!(findings_for("src/read.js", "const w = window.innerWidth;\n").is_empty());
    }

    #[test]
    fn ts_singleton_getinstance_is_common_coupling() {
        let src = "class Db {\n  private static instance: Db;\n  static getInstance(): Db {\n    return Db.instance;\n  }\n}\n";
        let f = findings_for("src/db.ts", src);
        assert!(!f.is_empty(), "getInstance singleton must be flagged");
        assert!(f.iter().all(|x| x.kind == CouplingKind::Common));
    }

    #[test]
    fn js_static_instance_field_is_common_coupling() {
        let src = "class Api {\n  static instance = null;\n}\n";
        assert_eq!(findings_for("src/api.js", src).len(), 1);
    }

    #[test]
    fn ts_plain_class_is_not_flagged() {
        let src = "class Point {\n  x = 0;\n  static origin() { return new Point(); }\n}\n";
        assert!(findings_for("src/p.ts", src).is_empty());
    }

    #[test]
    fn ts_export_let_destructuring_is_common_coupling() {
        let f = findings_for("src/state.ts", "export let[a, b] = pair();\n");
        assert_eq!(f.len(), 1, "destructuring let export must be flagged");
    }

    #[test]
    fn ts_export_let_with_newline_is_common_coupling() {
        let f = findings_for("src/state.ts", "export let\n  counter = 0;\n");
        assert_eq!(f.len(), 1, "wrapped let declaration must be flagged");
    }

    // ── TS/JS control coupling ─────────────────────────────────────

    #[test]
    fn ts_exported_fn_with_branched_boolean_is_control_coupling() {
        let src =
            "export function render(compact: boolean) {\n  if (compact) {\n    short();\n  }\n}\n";
        let f = findings_for("src/r.ts", src);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, CouplingKind::Control);
    }

    #[test]
    fn ts_exported_fn_with_ternary_boolean_is_control_coupling() {
        let src = "export function pick(fast: boolean): number {\n  return fast ? 1 : 2;\n}\n";
        assert_eq!(findings_for("src/p.ts", src).len(), 1);
    }

    #[test]
    fn js_exported_fn_with_default_bool_branched_is_control_coupling() {
        let src = "export function log(verbose = false) {\n  if (verbose) {\n    console.debug('x');\n  }\n}\n";
        assert_eq!(findings_for("src/l.js", src).len(), 1);
    }

    #[test]
    fn ts_non_exported_fn_is_not_flagged() {
        let src = "function helper(flag: boolean) {\n  if (flag) {\n    a();\n  }\n}\n";
        assert!(findings_for("src/h.ts", src).is_empty());
    }

    #[test]
    fn ts_exported_fn_with_stored_boolean_is_not_flagged() {
        let src = "export function setVisible(visible: boolean) {\n  state.visible = visible;\n}\n";
        assert!(findings_for("src/s.ts", src).is_empty());
    }
}
