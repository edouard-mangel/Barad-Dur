use tree_sitter::StreamingIterator;

use crate::snapshot::FunctionMetrics;

use super::fallback::Language;
use super::lang_dispatch::{comment_query, complexity_queries, function_query, nesting_query};
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

// ── Per-function extraction ─────────────────────────────────────────

/// Nesting-kind node types used to measure nesting depth.
const NESTING_KINDS: &[&str] = &[
    "if_expression",
    "if_statement",
    "for_expression",
    "for_statement",
    "for_in_statement",
    "enhanced_for_statement",
    "foreach_statement",
    "while_expression",
    "while_statement",
    "loop_expression",
    "do_statement",
    "match_expression",
    "switch_statement",
    "switch_expression",
    "expression_switch_statement",
    "type_switch_statement",
    "with_statement",
];

/// Extract per-function metrics (name, LOC, CC, nesting) from the AST.
///
/// Compiles the function/complexity/nesting queries once, then reuses them
/// across all functions in the file. Compiling tree-sitter queries is expensive
/// (~ms per call), so doing it once per function would dominate runtime on
/// large files.
pub(super) fn extract_functions(
    tree: &tree_sitter::Tree,
    source: &[u8],
    content: &str,
    grammar: &tree_sitter::Language,
    lang: Language,
    ext: &str,
) -> Vec<FunctionMetrics> {
    let func_query_src = match function_query(lang, ext) {
        Some(q) => q,
        None => return Vec::new(),
    };
    let func_query = match tree_sitter::Query::new(grammar, func_query_src) {
        Ok(q) => q,
        Err(_) => return Vec::new(),
    };
    let func_idx = func_query.capture_index_for_name("func").unwrap_or(0);
    let name_idx = func_query.capture_index_for_name("name").unwrap_or(1);

    // Compile complexity and nesting queries once for the file.
    let (stmt_query_src, op_query_src) = complexity_queries(lang, ext);
    let stmt_query = tree_sitter::Query::new(grammar, stmt_query_src).ok();
    let op_query = op_query_src.and_then(|s| tree_sitter::Query::new(grammar, s).ok());
    let nest_query =
        nesting_query(lang, ext).and_then(|s| tree_sitter::Query::new(grammar, s).ok());

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut stream = cursor.matches(&func_query, tree.root_node(), source);

    let mut functions = Vec::new();
    while let Some(m) = stream.next() {
        let func_node = m
            .captures
            .iter()
            .find(|c| c.index == func_idx)
            .map(|c| c.node);
        let name_node = m
            .captures
            .iter()
            .find(|c| c.index == name_idx)
            .map(|c| c.node);

        let (func_node, name_node) = match (func_node, name_node) {
            (Some(f), Some(n)) => (f, n),
            _ => continue,
        };

        let name = std::str::from_utf8(&source[name_node.byte_range()])
            .unwrap_or("<unknown>")
            .to_string();

        let start_line = func_node.start_position().row;
        let end_line = func_node.end_position().row;
        let loc = content
            .lines()
            .enumerate()
            .filter(|(i, line)| *i >= start_line && *i <= end_line && !line.trim().is_empty())
            .count();

        let cc = count_cc_in_range(
            tree,
            source,
            stmt_query.as_ref(),
            op_query.as_ref(),
            func_node.byte_range(),
        );
        let max_nesting = compute_max_nesting(tree, source, nest_query.as_ref(), &func_node);

        functions.push(FunctionMetrics {
            name,
            loc,
            cyclomatic_complexity: cc,
            max_nesting_depth: max_nesting,
        });
    }
    functions
}

/// Count cyclomatic complexity decision nodes within a byte range (function subtree).
/// Takes pre-compiled queries to avoid recompiling per function.
fn count_cc_in_range(
    tree: &tree_sitter::Tree,
    source: &[u8],
    stmt_query: Option<&tree_sitter::Query>,
    op_query: Option<&tree_sitter::Query>,
    byte_range: std::ops::Range<usize>,
) -> u32 {
    let count_in_range = |query: &tree_sitter::Query| -> u32 {
        let mut cursor = tree_sitter::QueryCursor::new();
        cursor.set_byte_range(byte_range.clone());
        let mut stream = cursor.matches(query, tree.root_node(), source);
        let mut count = 0u32;
        while stream.next().is_some() {
            count += 1;
        }
        count
    };

    let stmts = stmt_query.map(count_in_range).unwrap_or(0);
    let ops = op_query.map(count_in_range).unwrap_or(0);
    stmts + ops
}

/// Compute the maximum nesting depth of nesting nodes within a function node.
/// Takes a pre-compiled nesting query to avoid recompiling per function.
fn compute_max_nesting(
    tree: &tree_sitter::Tree,
    source: &[u8],
    nest_query: Option<&tree_sitter::Query>,
    func_node: &tree_sitter::Node,
) -> u32 {
    let query = match nest_query {
        Some(q) => q,
        None => return 0,
    };
    let mut cursor = tree_sitter::QueryCursor::new();
    cursor.set_byte_range(func_node.byte_range());
    let mut stream = cursor.matches(query, tree.root_node(), source);

    let func_id = func_node.id();
    let mut max_depth = 0u32;

    while let Some(m) = stream.next() {
        for cap in m.captures.iter() {
            let depth = nesting_ancestors_until(cap.node, func_id);
            if depth > max_depth {
                max_depth = depth;
            }
        }
    }
    max_depth
}

// ── File-level nesting biomarkers ──────────────────────────────────

/// Compute file-level nesting biomarkers: (max_nesting_depth, nesting_variance).
///
/// `max_nesting_depth` is the deepest nesting level across all nesting nodes.
/// `nesting_variance` is the standard deviation of per-line nesting depths.
pub(super) fn compute_nesting_biomarkers(
    tree: &tree_sitter::Tree,
    source: &[u8],
    grammar: &tree_sitter::Language,
    lang: Language,
    ext: &str,
    total_lines: usize,
) -> (u32, f64) {
    let nesting_query_src = match nesting_query(lang, ext) {
        Some(q) => q,
        None => return (0, 0.0),
    };
    let query = match tree_sitter::Query::new(grammar, nesting_query_src) {
        Ok(q) => q,
        Err(_) => return (0, 0.0),
    };
    if total_lines == 0 {
        return (0, 0.0);
    }

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut stream = cursor.matches(&query, tree.root_node(), source);

    // Collect (depth, start_row, end_row) for each nesting node.
    // Depth = nesting-kind ancestors up to root + 1 for the node itself.
    let root_id = tree.root_node().id();
    let mut nesting_nodes: Vec<(u32, usize, usize)> = Vec::new();
    let mut max_depth = 0u32;

    while let Some(m) = stream.next() {
        for cap in m.captures.iter() {
            let depth = nesting_ancestors_until(cap.node, root_id);
            if depth > max_depth {
                max_depth = depth;
            }
            nesting_nodes.push((
                depth,
                cap.node.start_position().row,
                cap.node.end_position().row,
            ));
        }
    }

    if nesting_nodes.is_empty() {
        return (0, 0.0);
    }

    // Build per-line max nesting depth
    let mut per_line = vec![0u32; total_lines];
    for &(depth, start, end) in &nesting_nodes {
        for d in per_line
            .iter_mut()
            .take(end.min(total_lines - 1) + 1)
            .skip(start)
        {
            if depth > *d {
                *d = depth;
            }
        }
    }

    // Standard deviation of per-line depths
    let n = per_line.len() as f64;
    let mean = per_line.iter().map(|&d| d as f64).sum::<f64>() / n;
    let variance = per_line
        .iter()
        .map(|&d| (d as f64 - mean).powi(2))
        .sum::<f64>()
        / n;
    let std_dev = variance.sqrt();

    (max_depth, std_dev)
}

/// Count nesting-kind ancestors between `node` and the function node (exclusive).
fn nesting_ancestors_until(node: tree_sitter::Node, func_id: usize) -> u32 {
    let mut depth = 0u32;
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.id() == func_id {
            break;
        }
        if NESTING_KINDS.contains(&parent.kind()) {
            depth += 1;
        }
        current = parent.parent();
    }
    // Include the node itself if it's a nesting kind
    if NESTING_KINDS.contains(&node.kind()) {
        depth += 1;
    }
    depth
}
