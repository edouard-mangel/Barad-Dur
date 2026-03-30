use tree_sitter::StreamingIterator;

use super::fallback::Language;
use super::lang_dispatch::{comment_query, complexity_queries};
use super::queries;
use super::treesitter::{collect_matches, run_query};

// ── Cyclomatic complexity ────────────────────────────────────────────

pub(super) fn count_complexity(
    tree: &tree_sitter::Tree,
    source: &[u8],
    grammar: &tree_sitter::Language,
    lang: Language,
    ext: &str,
) -> u32 {
    let (stmt_query, op_query) = complexity_queries(lang, ext);
    let stmts = run_query(tree, source, stmt_query, grammar);
    let ops = op_query
        .map(|q| run_query(tree, source, q, grammar))
        .unwrap_or(0);
    stmts + ops
}

// ── Public methods ───────────────────────────────────────────────────

pub(super) fn count_public_methods(
    tree: &tree_sitter::Tree,
    source: &[u8],
    grammar: &tree_sitter::Language,
    lang: Language,
    ext: &str,
) -> u32 {
    match lang {
        Language::Rust => count_with_visibility_filter(
            tree,
            source,
            grammar,
            queries::RUST_PUBLIC_METHODS,
            "vis",
            |text| text.starts_with(b"pub"),
        ),
        Language::Python => count_with_name_filter(
            tree,
            source,
            grammar,
            queries::PYTHON_PUBLIC_METHODS,
            "name",
            |text| !text.starts_with(b"_"),
        ),
        Language::Go => count_with_name_filter(
            tree,
            source,
            grammar,
            queries::GO_PUBLIC_METHODS,
            "name",
            |first_byte| {
                first_byte
                    .first()
                    .map(|b| b.is_ascii_uppercase())
                    .unwrap_or(false)
            },
        ),
        Language::Java => count_with_visibility_filter(
            tree,
            source,
            grammar,
            queries::JAVA_PUBLIC_METHODS,
            "mods",
            |text| text.windows(6).any(|w| w == b"public"),
        ),
        Language::CSharp => count_with_visibility_filter(
            tree,
            source,
            grammar,
            queries::CSHARP_PUBLIC_METHODS,
            "mod",
            |text| text == b"public",
        ),
        Language::JsTs => {
            let q = match ext {
                "ts" | "tsx" => queries::TS_PUBLIC_METHODS,
                _ => queries::JS_PUBLIC_METHODS,
            };
            run_query(tree, source, q, grammar)
        }
        Language::Kotlin | Language::Generic => 0,
    }
}

// ── Properties ───────────────────────────────────────────────────────

pub(super) fn count_properties(
    tree: &tree_sitter::Tree,
    source: &[u8],
    grammar: &tree_sitter::Language,
    lang: Language,
    ext: &str,
) -> u32 {
    match lang {
        Language::Rust => count_with_visibility_filter(
            tree,
            source,
            grammar,
            queries::RUST_PROPERTIES,
            "vis",
            |text| text.starts_with(b"pub"),
        ),
        Language::Go => count_with_name_filter(
            tree,
            source,
            grammar,
            queries::GO_PROPERTIES,
            "name",
            |text| {
                text.first()
                    .map(|b| b.is_ascii_uppercase())
                    .unwrap_or(false)
            },
        ),
        Language::Java => run_query(tree, source, queries::JAVA_PROPERTIES, grammar),
        Language::CSharp => run_query(tree, source, queries::CSHARP_PROPERTIES, grammar),
        Language::JsTs => {
            let q = match ext {
                "ts" | "tsx" => queries::TS_PROPERTIES,
                _ => queries::JS_PROPERTIES,
            };
            run_query(tree, source, q, grammar)
        }
        Language::Python | Language::Kotlin | Language::Generic => 0,
    }
}

// ── LOC (non-blank, non-comment lines) ──────────────────────────────

pub(super) fn count_loc(
    content: &str,
    tree: &tree_sitter::Tree,
    grammar: &tree_sitter::Language,
    lang: Language,
    ext: &str,
) -> usize {
    let comment_query_src = comment_query(lang, ext);
    let query = match tree_sitter::Query::new(grammar, comment_query_src) {
        Ok(q) => q,
        Err(_) => return content.lines().filter(|l| !l.trim().is_empty()).count(),
    };

    let mut cursor = tree_sitter::QueryCursor::new();
    let source = content.as_bytes();
    let mut stream = cursor.matches(&query, tree.root_node(), source);

    let mut comment_lines = std::collections::HashSet::new();
    while let Some(m) = stream.next() {
        for c in m.captures.iter() {
            let start = c.node.start_position().row;
            let end = c.node.end_position().row;
            for line in start..=end {
                comment_lines.insert(line);
            }
        }
    }

    content
        .lines()
        .enumerate()
        .filter(|(i, line)| !line.trim().is_empty() && !comment_lines.contains(i))
        .count()
}

// ── Shared capture-filtering helpers ────────────────────────────────

pub(super) fn count_with_visibility_filter(
    tree: &tree_sitter::Tree,
    source: &[u8],
    grammar: &tree_sitter::Language,
    query_src: &str,
    capture_name: &str,
    predicate: fn(&[u8]) -> bool,
) -> u32 {
    let (query, matches) = collect_matches(tree, source, query_src, grammar);
    let query = match query {
        Some(q) => q,
        None => return 0,
    };
    let cap_idx = query.capture_index_for_name(capture_name).unwrap_or(0);
    matches
        .iter()
        .filter(|caps| {
            caps.iter()
                .filter(|(idx, _)| *idx == cap_idx)
                .any(|(_, range)| predicate(&source[range.clone()]))
        })
        .count() as u32
}

pub(super) fn count_with_name_filter(
    tree: &tree_sitter::Tree,
    source: &[u8],
    grammar: &tree_sitter::Language,
    query_src: &str,
    capture_name: &str,
    predicate: fn(&[u8]) -> bool,
) -> u32 {
    count_with_visibility_filter(tree, source, grammar, query_src, capture_name, predicate)
}
