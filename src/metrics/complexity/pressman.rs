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
    findings_from_tree(tree.root_node(), content, path, lang)
}

/// Tree-level core of [`extract_coupling_findings`], for callers that
/// already hold a parsed tree (shared-parse path).
pub(super) fn findings_from_tree(
    root: Node<'_>,
    content: &str,
    path: &Path,
    lang: Language,
) -> Vec<CouplingFinding> {
    match lang {
        Language::Rust => rust_findings(root, content, path),
        Language::JsTs => js_findings(root, content, path),
        _ => Vec::new(),
    }
}

/// All nodes of a subtree, preorder.
pub(super) fn descendants(root: Node<'_>) -> Vec<Node<'_>> {
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

/// Node kinds that open a nested scope whose parameters shadow the
/// enclosing function's, per language.
/// Walls are unconditional: a nested scope that captures the outer flag
/// without shadowing it is also skipped — a known, accepted false negative
/// (conservative by design, mirrors the Rust closure rule from M1).
const RUST_SCOPE_BOUNDARIES: &[&str] = &["closure_expression", "function_item"];
const JS_SCOPE_BOUNDARIES: &[&str] = &[
    "arrow_function",
    "function_expression",
    "function_declaration",
    "method_definition",
    "generator_function",
    "generator_function_declaration",
];

/// All nodes of a subtree, preorder, without descending into nested
/// scopes (nodes whose kind is in `boundaries`, e.g. closures and inner
/// functions) whose parameters shadow the enclosing function's.
fn same_scope_descendants<'a>(root: Node<'a>, boundaries: &[&str]) -> Vec<Node<'a>> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        out.push(n);
        if n != root && boundaries.contains(&n.kind()) {
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
/// string boundaries) on both sides — and not preceded by `.`, which would
/// make it a field/property access (`settings.verbose`), never the parameter.
fn contains_word(hay: &str, word: &str) -> bool {
    hay.match_indices(word).any(|(i, _)| {
        let before_ok = !hay[..i]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.');
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
            "macro_invocation" => rust_lazy_static(n, content, path),
            _ => None,
        })
        .collect()
}

/// Interior-mutability type markers. Matched against the static item's
/// `type` field only (not the whole item text, which would also catch the
/// initializer expression) so the look-through rule stays intentional:
/// `LazyLock<Mutex<…>>` matches `Mutex<` in its type, while `LazyLock<Regex>`
/// matches nothing. Matching requires a left word boundary (see
/// `contains_marker_with_left_boundary`) so `OnceCell<Config>` does not
/// false-positive on the `Cell<` marker, while `RefCell<` and `UnsafeCell<`
/// still match at their own start.
const INTERIOR_MUTABILITY: &[&str] = &[
    "Mutex<",
    "RwLock<",
    "RefCell<",
    "Cell<",
    "UnsafeCell<",
    "Atomic",
];

/// True when `marker` occurs in `hay` with a non-identifier character (or
/// string start) immediately before it. The right side is intentionally
/// left open (no boundary check after the marker) — the look-through rule
/// wants `Mutex<` to match regardless of what follows the `<`.
fn contains_marker_with_left_boundary(hay: &str, marker: &str) -> bool {
    hay.match_indices(marker).any(|(i, _)| {
        !hay[..i]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

fn rust_common(node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let is_mut = (0..node.child_count())
        .filter_map(|i| node.child(i as u32))
        .any(|c| c.kind() == "mutable_specifier");
    // Only `static mut` applies when the type field is missing (malformed
    // parse), since there is no type text to scan for interior mutability.
    let interior = node
        .child_by_field_name("type")
        .map(|t| text(t, content))
        .is_some_and(|type_text| {
            INTERIOR_MUTABILITY
                .iter()
                .any(|marker| contains_marker_with_left_boundary(type_text, marker))
        });
    (is_mut || interior).then(|| finding(path, node, CouplingKind::Common, content))
}

/// `lazy_static! { static ref X: Mutex<…> = …; }` hides its statics from the
/// `static_item` detector (the macro body is an opaque token tree). Apply the
/// look-through rule to each `static ref` entry's type text by anchoring at
/// each `static ref` occurrence, scanning from its declaration colon to the
/// following `=`, so initializer expressions can't false-positive and array
/// types won't fracture the entry.
/// One finding per invocation: the macro block is the reportable unit.
fn rust_lazy_static(node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let is_lazy_static = node
        .child_by_field_name("macro")
        .is_some_and(|m| text(m, content).ends_with("lazy_static"));
    if !is_lazy_static {
        return None;
    }
    let body = text(node, content);
    let hit = body.match_indices("static ref").any(|(i, _)| {
        body[i..]
            .split_once(':')
            .and_then(|(_, rest)| rest.split_once('=').map(|(ty, _)| ty))
            .is_some_and(|ty| {
                INTERIOR_MUTABILITY
                    .iter()
                    .any(|m| contains_marker_with_left_boundary(ty, m))
            })
    });
    hit.then(|| finding(path, node, CouplingKind::Common, content))
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
    let branched = same_scope_descendants(body, RUST_SCOPE_BOUNDARIES)
        .into_iter()
        .any(|n| {
            let cond = match n.kind() {
                "if_expression" | "while_expression" => n.child_by_field_name("condition"),
                "match_expression" => n.child_by_field_name("value"),
                // `_ if flag => …` — the guard is the match_pattern's condition field
                "match_pattern" => n.child_by_field_name("condition"),
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
            "assignment_expression" | "augmented_assignment_expression" => {
                js_global_write(n, content, path)
            }
            "class_declaration" | "class" => js_singleton(n, content, path),
            _ => None,
        })
        .collect()
}

/// `export let` / `export var` → Common. `export function` → control check.
/// `export const f = (…) => {…}` → control check on the arrow/function-expression.
fn js_export(node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let decl = node.child_by_field_name("declaration")?;
    match decl.kind() {
        "lexical_declaration" if is_let_declaration(decl, content) => {
            Some(finding(path, node, CouplingKind::Common, content))
        }
        // `export const f = (…) => {…}` — a function export in const
        // clothing; run the control check on each declarator's function value.
        "lexical_declaration" => (0..decl.named_child_count())
            .filter_map(|i| decl.named_child(i as u32))
            .filter(|d| d.kind() == "variable_declarator")
            .filter_map(|d| d.child_by_field_name("value"))
            .filter(|v| matches!(v.kind(), "arrow_function" | "function_expression"))
            .find_map(|v| js_control(v, content, path)),
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

/// Walk `a.b.c` / `a["b"]` chains down to the leftmost object node.
/// Non-chain targets return None — a plain identifier write is not a
/// member write (shadowing `window` itself is a different sin).
fn member_chain_root(node: Node<'_>) -> Option<Node<'_>> {
    match node.kind() {
        "member_expression" | "subscript_expression" => {
            let obj = node.child_by_field_name("object")?;
            match obj.kind() {
                "member_expression" | "subscript_expression" => member_chain_root(obj),
                _ => Some(obj),
            }
        }
        _ => None,
    }
}

/// Assignment (plain or augmented) whose target chain is rooted at
/// `globalThis` / `window` → Common. Covers `window.x = …`,
/// `window["x"] = …`, `globalThis.a.b = …`, `window.count += 1`.
fn js_global_write(node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let left = node.child_by_field_name("left")?;
    let root = member_chain_root(left)?;
    let is_global =
        root.kind() == "identifier" && matches!(text(root, content), "globalThis" | "window");
    // Assigning browser navigation is an external side effect, not shared
    // mutable application state. Treating logout redirects as Common
    // coupling made ordinary web applications hit the severity cap.
    // Exempt window.location itself and member paths under it — not
    // unrelated globals sharing the prefix (window.locationService).
    let target = text(left, content);
    let rooted_at = |root: &str| {
        target == root
            || target
                .strip_prefix(root)
                .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('['))
    };
    let is_navigation = rooted_at("window.location") || rooted_at("globalThis.location");
    (is_global && !is_navigation).then(|| finding(path, node, CouplingKind::Common, content))
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

/// True when a `type_annotation`'s type is exactly the predefined `boolean` —
/// not `boolean[]`, not a union, not a look-alike named type. Unions and
/// arrays are data shapes, not control flags (maintainer decision, this MR).
fn annotation_is_exact_boolean(annotation: Node<'_>, content: &str) -> bool {
    (0..annotation.named_child_count())
        .filter_map(|i| annotation.named_child(i as u32))
        .any(|t| t.kind() == "predefined_type" && text(t, content) == "boolean")
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
                    .filter(|c| c.kind() == "type_annotation")
                    .any(|c| annotation_is_exact_boolean(c, content));
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
    let branched = same_scope_descendants(body, JS_SCOPE_BOUNDARIES)
        .into_iter()
        .any(|n| {
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

    #[test]
    fn rust_pure_oncecell_is_not_flagged() {
        let src = "static CONFIG: once_cell::sync::OnceCell<Config> = once_cell::sync::OnceCell::new();\n";
        assert!(
            findings_for("src/a.rs", src).is_empty(),
            "write-once OnceCell must not be flagged (Cell< substring trap)"
        );
    }

    #[test]
    fn rust_oncelock_wrapping_mutex_is_flagged_lookthrough() {
        let src = "static STATE: std::sync::OnceLock<std::sync::Mutex<u32>> = std::sync::OnceLock::new();\n";
        assert_eq!(
            findings_for("src/a.rs", src).len(),
            1,
            "OnceLock wrapping Mutex is a mutable global"
        );
    }

    #[test]
    fn lazy_static_mutex_is_common_coupling() {
        let src = "lazy_static::lazy_static! {\n    static ref REGISTRY: Mutex<Vec<u32>> = Mutex::new(Vec::new());\n}\n";
        let f = findings_for("src/a.rs", src);
        assert_eq!(f.len(), 1, "lazy_static wrapping Mutex is a mutable global");
        assert_eq!(f[0].kind, CouplingKind::Common);
    }

    #[test]
    fn lazy_static_pure_value_is_not_flagged() {
        let src = "lazy_static::lazy_static! {\n    static ref KEYWORDS: Vec<&'static str> = build_keywords();\n}\n";
        assert!(
            findings_for("src/a.rs", src).is_empty(),
            "write-once pure lazy_static is not common coupling"
        );
    }

    #[test]
    fn lazy_static_marker_in_initializer_only_is_not_flagged() {
        let src = "lazy_static::lazy_static! {\n    static ref N: usize = Cell::new(0).get();\n}\n";
        assert!(
            findings_for("src/a.rs", src).is_empty(),
            "only the type segment (between ':' and '=') is scanned, not the initializer"
        );
    }

    #[test]
    fn other_macros_are_not_flagged() {
        let src = "thread_local! {\n    static FOO: RefCell<u32> = RefCell::new(0);\n}\n";
        assert!(
            findings_for("src/a.rs", src).is_empty(),
            "thread-locals are per-thread, not shared globals — and only lazy_static is matched"
        );
    }

    #[test]
    fn lazy_static_atomic_prefixed_name_with_plain_type_is_not_flagged() {
        let src = "lazy_static::lazy_static! {\n    static ref AtomicFlagName: bool = false;\n}\n";
        assert!(
            findings_for("src/a.rs", src).is_empty(),
            "the identifier must not leak into the scanned type segment"
        );
    }

    #[test]
    fn lazy_static_array_of_mutexes_is_common_coupling() {
        let src =
            "lazy_static! {\n    static ref LOCKS: [Mutex<i32>; 4] = [Mutex::new(0); 4];\n}\n";
        assert_eq!(
            findings_for("src/a.rs", src).len(),
            1,
            "a semicolon inside the array type must not fracture the entry"
        );
    }

    #[test]
    fn lazy_static_multi_entry_block_yields_one_finding() {
        let src = "lazy_static! {\n    static ref NAMES: Vec<String> = Vec::new();\n    static ref STATE: Mutex<u32> = Mutex::new(0);\n}\n";
        assert_eq!(
            findings_for("src/a.rs", src).len(),
            1,
            "one finding per invocation, matched via the second entry's type"
        );
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
    fn rust_compact_path_attribute_is_content_coupling() {
        let src = "#[path=\"../other/impl.rs\"]\nmod stolen;\n";
        assert_eq!(
            findings_for("src/a.rs", src).len(),
            1,
            "no-space #[path=…] variant must match (whitespace is normalized)"
        );
    }

    #[test]
    fn rust_path_prefixed_attribute_is_not_flagged() {
        let src = "#[path2 = \"x\"]\nmod m;\n";
        assert!(
            findings_for("src/a.rs", src).is_empty(),
            "#[path2=…] must not match the #[path= prefix"
        );
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
    fn rust_nested_fn_shadowing_bool_param_is_not_flagged() {
        let src = "pub fn outer(flag: bool) {\n    fn inner(flag: bool) {\n        if flag {\n            do_it();\n        }\n    }\n    inner(true);\n}\n";
        assert!(
            findings_for("src/a.rs", src).is_empty(),
            "nested fn's own bool param must not be attributed to the outer fn"
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

    #[test]
    fn pub_fn_with_mut_bool_param_branched_is_control_coupling() {
        let src = "pub fn go(mut fast: bool) {\n    if fast {\n        sprint();\n    }\n}\n";
        assert_eq!(
            findings_for("src/a.rs", src).len(),
            1,
            "mut_pattern params must look through the mut"
        );
    }

    #[test]
    fn pub_fn_with_match_guard_on_bool_is_control_coupling() {
        let src = "pub fn pick(fast: bool, n: u32) {\n    match n {\n        _ if fast => sprint(),\n        _ => walk(),\n    }\n}\n";
        assert_eq!(
            findings_for("src/a.rs", src).len(),
            1,
            "guard arms branch on the flag just like if-expressions"
        );
    }

    #[test]
    fn match_guard_on_local_not_param_is_not_flagged() {
        let src = "pub fn pick(fast: bool, n: u32) {\n    let faster = n > 1;\n    match n {\n        _ if faster => sprint(),\n        _ => walk(),\n    }\n    store(fast);\n}\n";
        assert!(
            findings_for("src/a.rs", src).is_empty(),
            "guard on a local must not be attributed to the param"
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
    fn js_navigation_assignment_is_not_common_coupling() {
        assert!(findings_for("src/logout.ts", "window.location.href = logoutUrl;\n").is_empty());
        assert!(findings_for("src/nav.ts", "globalThis.location.href = url;\n").is_empty());
        assert!(findings_for("src/nav.ts", "window.location = url;\n").is_empty());
    }

    #[test]
    fn js_location_prefixed_globals_are_still_common_coupling() {
        // The navigation exemption is for window.location itself, not for
        // unrelated globals that merely share the prefix.
        assert_eq!(
            findings_for(
                "src/boot.ts",
                "window.locationService = new LocationService();\n"
            )
            .len(),
            1
        );
        assert_eq!(
            findings_for("src/boot.ts", "window.locations = [];\n").len(),
            1
        );
    }

    #[test]
    fn js_reading_window_is_not_flagged() {
        assert!(findings_for("src/read.js", "const w = window.innerWidth;\n").is_empty());
    }

    #[test]
    fn js_subscript_global_write_is_common_coupling() {
        let f = findings_for("src/boot.js", "window[\"cache\"] = new Map();\n");
        assert_eq!(
            f.len(),
            1,
            "computed-key global writes are still global writes"
        );
        assert_eq!(f[0].kind, CouplingKind::Common);
    }

    #[test]
    fn js_nested_global_write_is_common_coupling() {
        assert_eq!(
            findings_for("src/boot.js", "globalThis.app.state = {};\n").len(),
            1,
            "writing through a chain rooted at globalThis mutates global state"
        );
    }

    #[test]
    fn js_augmented_global_write_is_common_coupling() {
        assert_eq!(
            findings_for("src/boot.js", "window.count += 1;\n").len(),
            1,
            "+= is a write"
        );
    }

    #[test]
    fn js_local_subscript_write_is_not_flagged() {
        assert!(findings_for("src/a.js", "arr[0] = 1;\n").is_empty());
    }

    #[test]
    fn js_reading_global_subscript_is_not_flagged() {
        assert!(findings_for("src/a.js", "const v = window[\"x\"];\n").is_empty());
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
    fn js_class_expression_singleton_is_flagged() {
        let src = "const Db = class {\n  static instance = null;\n};\n";
        assert_eq!(
            findings_for("src/db.js", src).len(),
            1,
            "the bare `class` node kind arm must work, not just class_declaration"
        );
    }

    #[test]
    fn ts_plain_class_is_not_flagged() {
        let src = "class Point {\n  x = 0;\n  static origin() { return new Point(); }\n}\n";
        assert!(findings_for("src/p.ts", src).is_empty());
    }

    #[test]
    fn ts_non_static_instance_field_is_not_flagged() {
        let src = "class Foo {\n  instance = null;\n}\n";
        assert!(
            findings_for("src/f.ts", src).is_empty(),
            "instance-level field named 'instance' is not the singleton pattern"
        );
    }

    #[test]
    fn ts_non_static_getinstance_method_is_not_flagged() {
        let src = "class Foo {\n  getInstance() {\n    return this;\n  }\n}\n";
        assert!(
            findings_for("src/f.ts", src).is_empty(),
            "non-static getInstance is not the singleton pattern"
        );
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

    #[test]
    fn ts_nested_arrow_shadowing_bool_param_is_not_flagged() {
        let src = "export function outer(flag: boolean) {\n  const f = (flag: boolean) => {\n    if (flag) {\n      doIt();\n    }\n  };\n  f(true);\n}\n";
        assert!(
            findings_for("src/o.ts", src).is_empty(),
            "nested arrow's own shadowed bool param must not be attributed to outer fn"
        );
    }

    #[test]
    fn ts_bool_branched_outside_nested_fn_is_still_flagged() {
        let src = "export function outer(flag: boolean) {\n  const f = () => doIt();\n  if (flag) {\n    f();\n  }\n}\n";
        assert_eq!(findings_for("src/o.ts", src).len(), 1);
    }

    #[test]
    fn property_access_does_not_count_as_flag_use_js() {
        let src = "export function log(verbose: boolean) {\n  if (settings.verbose) {\n    console.debug('x');\n  }\n}\n";
        assert!(
            findings_for("src/l.ts", src).is_empty(),
            "settings.verbose is a property access, not the parameter"
        );
    }

    #[test]
    fn field_access_does_not_count_as_flag_use_rust() {
        let src = "pub fn render(compact: bool, s: &Settings) {\n    if s.compact {\n        short();\n    }\n    store(compact);\n}\n";
        assert!(
            findings_for("src/a.rs", src).is_empty(),
            "s.compact is a field access, not the parameter"
        );
    }

    #[test]
    fn ts_boolean_array_param_is_not_a_flag() {
        let src = "export function f(flags: boolean[]) {\n  if (flags) {\n    a();\n  }\n}\n";
        assert!(
            findings_for("src/f.ts", src).is_empty(),
            "boolean[] is data, not a control flag"
        );
    }

    #[test]
    fn ts_boolean_union_param_is_not_a_flag() {
        let src =
            "export function f(flag: boolean | undefined) {\n  if (flag) {\n    a();\n  }\n}\n";
        assert!(
            findings_for("src/f.ts", src).is_empty(),
            "only an exact boolean annotation qualifies (documented decision)"
        );
    }

    #[test]
    fn ts_exported_arrow_with_branched_boolean_is_control_coupling() {
        let src = "export const render = (compact: boolean) => {\n  if (compact) {\n    short();\n  }\n};\n";
        let f = findings_for("src/r.ts", src);
        assert_eq!(
            f.len(),
            1,
            "exported arrow functions are exported functions"
        );
        assert_eq!(f[0].kind, CouplingKind::Control);
    }

    #[test]
    fn ts_exported_function_expression_is_control_coupling() {
        let src = "export const render = function (compact: boolean) {\n  if (compact) {\n    short();\n  }\n};\n";
        assert_eq!(findings_for("src/r.ts", src).len(), 1);
    }

    #[test]
    fn ts_exported_const_arrow_without_flags_is_not_flagged() {
        let src = "export const add = (a: number, b: number) => a + b;\n";
        assert!(findings_for("src/a.ts", src).is_empty());
    }

    #[test]
    fn generator_shadowing_bool_param_is_not_flagged() {
        let src = "export function outer(flag: boolean) {\n  function* gen(flag: boolean) {\n    if (flag) {\n      yield 1;\n    }\n  }\n  gen(true);\n}\n";
        assert!(
            findings_for("src/g.ts", src).is_empty(),
            "generator's own shadowed param must not be attributed to outer fn"
        );
    }

    #[test]
    fn ts_exported_const_arrow_with_optional_boolean_is_control_coupling() {
        let src = "export const f = (flag?: boolean) => {\n  if (flag) {\n    a();\n  }\n};\n";
        let f = findings_for("src/r.ts", src);
        assert_eq!(
            f.len(),
            1,
            "exported arrow with optional boolean param is control coupling"
        );
        assert_eq!(f[0].kind, CouplingKind::Control);
    }
}
