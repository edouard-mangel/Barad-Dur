use tree_sitter::StreamingIterator;

use crate::snapshot::FileComplexity;

use super::fallback::Language;
use super::queries;

type Capture = (u32, std::ops::Range<usize>);
type MatchResult = (Option<tree_sitter::Query>, Vec<Vec<Capture>>);

/// Analyse source content using tree-sitter AST parsing.
/// Returns `None` for unsupported languages or complete parse failures.
pub fn analyse(content: &str, lang: Language, ext: &str) -> Option<FileComplexity> {
    let grammar = grammar_for(lang, ext)?;
    let tree = parse(content, &grammar)?;

    let total_lines = content.lines().count();
    let loc = count_loc(content, &tree, &grammar, lang, ext);
    let cyclomatic_complexity = count_complexity(&tree, content.as_bytes(), &grammar, lang, ext);
    let public_methods = count_public_methods(&tree, content.as_bytes(), &grammar, lang, ext);
    let properties = count_properties(&tree, content.as_bytes(), &grammar, lang, ext);

    Some(FileComplexity {
        total_lines,
        loc,
        cyclomatic_complexity,
        public_methods,
        properties,
    })
}

fn parse(content: &str, grammar: &tree_sitter::Language) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(grammar).ok()?;
    parser.parse(content, None)
}

fn grammar_for(lang: Language, ext: &str) -> Option<tree_sitter::Language> {
    match lang {
        Language::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
        Language::JsTs => match ext {
            "ts" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
            _ => Some(tree_sitter_javascript::LANGUAGE.into()),
        },
        Language::Python => Some(tree_sitter_python::LANGUAGE.into()),
        Language::Go => Some(tree_sitter_go::LANGUAGE.into()),
        Language::Java => Some(tree_sitter_java::LANGUAGE.into()),
        Language::CSharp => Some(tree_sitter_c_sharp::LANGUAGE.into()),
        Language::Kotlin | Language::Generic => None,
    }
}

/// Count matches for a simple query (no capture filtering needed).
fn run_query(
    tree: &tree_sitter::Tree,
    source: &[u8],
    query_src: &str,
    grammar: &tree_sitter::Language,
) -> u32 {
    let query = match tree_sitter::Query::new(grammar, query_src) {
        Ok(q) => q,
        Err(_) => return 0,
    };
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);
    let mut count = 0u32;
    while matches.next().is_some() {
        count += 1;
    }
    count
}

/// Collect all matches for a query, returning owned capture data.
/// Each match is a Vec of (capture_index, byte_range) pairs.
fn collect_matches(
    tree: &tree_sitter::Tree,
    source: &[u8],
    query_src: &str,
    grammar: &tree_sitter::Language,
) -> MatchResult {
    let query = match tree_sitter::Query::new(grammar, query_src) {
        Ok(q) => q,
        Err(_) => return (None, Vec::new()),
    };
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut stream = cursor.matches(&query, tree.root_node(), source);
    let mut results = Vec::new();
    while let Some(m) = stream.next() {
        let captures: Vec<Capture> = m
            .captures
            .iter()
            .map(|c| (c.index, c.node.byte_range()))
            .collect();
        results.push(captures);
    }
    (Some(query), results)
}

// ── Cyclomatic complexity ───────────────────────────────────────────

fn count_complexity(
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

fn complexity_queries(lang: Language, ext: &str) -> (&'static str, Option<&'static str>) {
    match lang {
        Language::Rust => (
            queries::RUST_COMPLEXITY,
            Some(queries::RUST_COMPLEXITY_OPERATORS),
        ),
        Language::JsTs => match ext {
            "ts" | "tsx" => (
                queries::JS_COMPLEXITY,
                Some(queries::JS_COMPLEXITY_OPERATORS),
            ),
            _ => (
                queries::JS_COMPLEXITY,
                Some(queries::JS_COMPLEXITY_OPERATORS),
            ),
        },
        Language::Python => (queries::PYTHON_COMPLEXITY, None),
        Language::Go => (
            queries::GO_COMPLEXITY,
            Some(queries::GO_COMPLEXITY_OPERATORS),
        ),
        Language::Java => (
            queries::JAVA_COMPLEXITY,
            Some(queries::JAVA_COMPLEXITY_OPERATORS),
        ),
        Language::CSharp => (
            queries::CSHARP_COMPLEXITY,
            Some(queries::CSHARP_COMPLEXITY_OPERATORS),
        ),
        Language::Kotlin | Language::Generic => (queries::RUST_COMPLEXITY, None), // unreachable
    }
}

// ── Public methods ──────────────────────────────────────────────────

fn count_public_methods(
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

// ── Properties ──────────────────────────────────────────────────────

fn count_properties(
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

// ── Shared capture-filtering helpers ────────────────────────────────

fn count_with_visibility_filter(
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

fn count_with_name_filter(
    tree: &tree_sitter::Tree,
    source: &[u8],
    grammar: &tree_sitter::Language,
    query_src: &str,
    capture_name: &str,
    predicate: fn(&[u8]) -> bool,
) -> u32 {
    count_with_visibility_filter(tree, source, grammar, query_src, capture_name, predicate)
}

// ── LOC (non-blank, non-comment lines) ──────────────────────────────

fn count_loc(
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

fn comment_query(lang: Language, ext: &str) -> &'static str {
    match lang {
        Language::Rust => queries::RUST_COMMENTS,
        Language::JsTs => match ext {
            "ts" | "tsx" => queries::JS_COMMENTS,
            _ => queries::JS_COMMENTS,
        },
        Language::Python => queries::PYTHON_COMMENTS,
        Language::Go => queries::GO_COMMENTS,
        Language::Java => queries::JAVA_COMMENTS,
        Language::CSharp => queries::CSHARP_COMMENTS,
        Language::Kotlin | Language::Generic => queries::RUST_COMMENTS, // unreachable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Rust ────────────────────────────────────────────────────────

    #[test]
    fn rust_public_methods() {
        let content = "pub fn foo() {}\nfn bar() {}\npub fn baz() {}\n";
        let result = analyse(content, Language::Rust, "rs").unwrap();
        assert_eq!(result.public_methods, 2);
    }

    #[test]
    fn rust_cyclomatic_complexity() {
        let content =
            "fn f() {\n  if x {}\n  for i in v {}\n  while z {}\n  match a { _ => {} }\n}\n";
        let result = analyse(content, Language::Rust, "rs").unwrap();
        assert!(
            result.cyclomatic_complexity >= 4,
            "expected >= 4, got {}",
            result.cyclomatic_complexity
        );
    }

    #[test]
    fn rust_complexity_with_logical_operators() {
        let content = "fn f() { if a && b || c {} }\n";
        let result = analyse(content, Language::Rust, "rs").unwrap();
        assert!(
            result.cyclomatic_complexity >= 3,
            "expected >= 3, got {}",
            result.cyclomatic_complexity
        );
    }

    #[test]
    fn rust_properties() {
        let content = "pub struct Foo {\n    pub x: i32,\n    pub y: String,\n    z: bool,\n}\n";
        let result = analyse(content, Language::Rust, "rs").unwrap();
        assert_eq!(result.properties, 2);
    }

    #[test]
    fn rust_loc_excludes_comments() {
        let content = "// comment\n\nfn main() {}\n    // indented\nlet x = 1;\n";
        let result = analyse(content, Language::Rust, "rs").unwrap();
        assert_eq!(result.loc, 2);
    }

    #[test]
    fn rust_loc_excludes_block_comments() {
        let content = "/* multi\n   line\n   comment */\nfn main() {}\n";
        let result = analyse(content, Language::Rust, "rs").unwrap();
        assert_eq!(result.loc, 1);
    }

    // ── JavaScript ──────────────────────────────────────────────────

    #[test]
    fn js_public_methods() {
        let content = "export function foo() {}\nfunction bar() {}\nexport const baz = () => {}\n";
        let result = analyse(content, Language::JsTs, "js").unwrap();
        assert_eq!(result.public_methods, 2);
    }

    #[test]
    fn js_complexity() {
        let content = "if (x) {} for (;;) {} while (y) {} switch (z) {}\n";
        let result = analyse(content, Language::JsTs, "js").unwrap();
        assert!(
            result.cyclomatic_complexity >= 4,
            "expected >= 4, got {}",
            result.cyclomatic_complexity
        );
    }

    // ── TypeScript ──────────────────────────────────────────────────

    #[test]
    fn ts_parses_type_annotations() {
        let content = "export function greet(name: string): void {}\n";
        let result = analyse(content, Language::JsTs, "ts").unwrap();
        assert_eq!(result.public_methods, 1);
    }

    #[test]
    fn ts_interface_properties() {
        // property_signature nodes only exist in the TS grammar, not JS
        let content = "interface Foo {\n  name: string;\n  age: number;\n}\n";
        let result = analyse(content, Language::JsTs, "ts").unwrap();
        assert_eq!(result.properties, 2);
    }

    #[test]
    fn tsx_parses_jsx_with_types() {
        let content = "export function App(props: { name: string }) { return <div/>; }\n";
        let result = analyse(content, Language::JsTs, "tsx").unwrap();
        assert_eq!(result.public_methods, 1);
    }

    #[test]
    fn ts_loc_excludes_comments() {
        let content = "// comment\nconst x: number = 1;\n";
        let result = analyse(content, Language::JsTs, "ts").unwrap();
        assert_eq!(result.loc, 1);
    }

    #[test]
    fn ts_complexity() {
        let content = "function f(x: number): void { if (x > 0) {} while (x) {} }\n";
        let result = analyse(content, Language::JsTs, "ts").unwrap();
        assert!(result.cyclomatic_complexity >= 2);
    }

    // ── Python ──────────────────────────────────────────────────────

    #[test]
    fn python_public_methods() {
        let content = "def foo():\n    pass\ndef _bar():\n    pass\ndef baz():\n    pass\n";
        let result = analyse(content, Language::Python, "py").unwrap();
        assert_eq!(result.public_methods, 2);
    }

    #[test]
    fn python_complexity() {
        let content =
            "if x:\n    pass\nelif y:\n    pass\nfor i in v:\n    pass\nwhile z:\n    pass\n";
        let result = analyse(content, Language::Python, "py").unwrap();
        assert!(
            result.cyclomatic_complexity >= 4,
            "expected >= 4, got {}",
            result.cyclomatic_complexity
        );
    }

    // ── Go ──────────────────────────────────────────────────────────

    #[test]
    fn go_public_methods() {
        let content = "package main\nfunc Foo() {}\nfunc bar() {}\n";
        let result = analyse(content, Language::Go, "go").unwrap();
        assert_eq!(result.public_methods, 1);
    }

    #[test]
    fn go_properties() {
        let content = "package main\ntype Foo struct {\n\tName string\n\tage int\n}\n";
        let result = analyse(content, Language::Go, "go").unwrap();
        assert_eq!(result.properties, 1);
    }

    // ── Java ────────────────────────────────────────────────────────

    #[test]
    fn java_public_methods() {
        let content = "class Foo {\n  public void bar() {}\n  private void baz() {}\n  public void qux() {}\n}\n";
        let result = analyse(content, Language::Java, "java").unwrap();
        assert_eq!(result.public_methods, 2);
    }

    #[test]
    fn java_complexity() {
        let content = "class Foo {\n  void f() {\n    if (x) {}\n    for (int i=0;;) {}\n    while (y) {}\n  }\n}\n";
        let result = analyse(content, Language::Java, "java").unwrap();
        assert!(
            result.cyclomatic_complexity >= 3,
            "expected >= 3, got {}",
            result.cyclomatic_complexity
        );
    }

    // ── C# ──────────────────────────────────────────────────────────

    #[test]
    fn csharp_public_methods() {
        let content = "class Foo {\n  public void Bar() {}\n  private void Baz() {}\n  public void Qux() {}\n}\n";
        let result = analyse(content, Language::CSharp, "cs").unwrap();
        assert_eq!(result.public_methods, 2);
    }

    // ── Edge cases ──────────────────────────────────────────────────

    #[test]
    fn generic_returns_none() {
        assert!(analyse("hello world", Language::Generic, "txt").is_none());
    }

    #[test]
    fn kotlin_returns_none() {
        assert!(analyse("fun main() {}", Language::Kotlin, "kt").is_none());
    }

    #[test]
    fn syntax_error_returns_best_effort() {
        let content = "pub fn foo() { if x {} }\npub fn bar() {{{{{";
        let result = analyse(content, Language::Rust, "rs");
        assert!(result.is_some(), "should parse despite syntax errors");
        assert!(result.unwrap().public_methods >= 1);
    }

    #[test]
    fn loc_counts_only_nonblank_nonccomment_lines() {
        // Tests the fallback branch inside count_loc (blank line filtering)
        let content = "\n\n// comment\nfn foo() {}\n\n";
        let result = analyse(content, Language::Rust, "rs").unwrap();
        assert_eq!(result.loc, 1);
        assert_eq!(result.total_lines, 5);
    }

    #[test]
    fn empty_content() {
        let result = analyse("", Language::Rust, "rs").unwrap();
        assert_eq!(result.total_lines, 0);
        assert_eq!(result.loc, 0);
        assert_eq!(result.cyclomatic_complexity, 0);
        assert_eq!(result.public_methods, 0);
        assert_eq!(result.properties, 0);
    }
}
