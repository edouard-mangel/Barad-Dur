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
#[expect(dead_code)]
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
    fn unsupported_language_returns_empty() {
        assert!(findings_for("script.py", "x = 1\n").is_empty());
    }

    #[test]
    fn unparseable_extension_returns_empty() {
        assert!(findings_for("notes.txt", "hello\n").is_empty());
    }
}
