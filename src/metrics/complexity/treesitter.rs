use tree_sitter::StreamingIterator;

use crate::snapshot::FileComplexity;

use super::counters::{count_complexity, count_loc, count_properties, count_public_methods};
use super::fallback::Language;
use super::lang_dispatch::{grammar_for, import_query};

pub(super) type Capture = (u32, std::ops::Range<usize>);
pub(super) type MatchResult = (Option<tree_sitter::Query>, Vec<Vec<Capture>>);

/// Extract raw import paths from source content using tree-sitter.
/// Returns an empty Vec for unsupported languages or parse failures.
pub fn extract_imports(content: &str, lang: Language, ext: &str) -> Vec<String> {
    let grammar = match grammar_for(lang, ext) {
        Some(g) => g,
        None => return Vec::new(),
    };
    let tree = match parse(content, &grammar) {
        Some(t) => t,
        None => return Vec::new(),
    };
    let query_src = import_query(lang, ext);
    let query_src = match query_src {
        Some(q) => q,
        None => return Vec::new(),
    };
    let (query, matches) = collect_matches(&tree, content.as_bytes(), query_src, &grammar);
    let query = match query {
        Some(q) => q,
        None => return Vec::new(),
    };
    let path_idx = query.capture_index_for_name("path").unwrap_or(0);
    matches
        .iter()
        .flat_map(|caps| {
            caps.iter()
                .filter(|(idx, _)| *idx == path_idx)
                .filter_map(|(_, range)| {
                    let text = std::str::from_utf8(&content.as_bytes()[range.clone()]).ok()?;
                    // Strip surrounding quotes from string literals
                    let cleaned = text.trim_matches('"').trim_matches('\'');
                    Some(cleaned.to_string())
                })
        })
        .collect()
}

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

pub(super) fn parse(content: &str, grammar: &tree_sitter::Language) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(grammar).ok()?;
    parser.parse(content, None)
}

/// Count matches for a simple query (no capture filtering needed).
pub(super) fn run_query(
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
pub(super) fn collect_matches(
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

    // ── Import extraction ──────────────────────────────────────────

    #[test]
    fn rust_extract_imports() {
        let content =
            "use std::collections::HashMap;\nuse crate::snapshot::RepoSnapshot;\nfn main() {}\n";
        let imports = extract_imports(content, Language::Rust, "rs");
        assert_eq!(imports.len(), 2);
        assert!(imports
            .iter()
            .any(|i| i.contains("std::collections::HashMap")));
        assert!(imports
            .iter()
            .any(|i| i.contains("crate::snapshot::RepoSnapshot")));
    }

    #[test]
    fn js_extract_imports() {
        let content = "import { foo } from './utils';\nconst bar = require('lodash');\n";
        let imports = extract_imports(content, Language::JsTs, "js");
        assert!(
            imports.iter().any(|i| i == "./utils"),
            "imports: {:?}",
            imports
        );
        assert!(
            imports.iter().any(|i| i == "lodash"),
            "imports: {:?}",
            imports
        );
    }

    #[test]
    fn ts_extract_imports() {
        let content =
            "import { Component } from '@angular/core';\nimport type { Foo } from './foo';\n";
        let imports = extract_imports(content, Language::JsTs, "ts");
        assert!(
            imports.iter().any(|i| i == "@angular/core"),
            "imports: {:?}",
            imports
        );
        assert!(
            imports.iter().any(|i| i == "./foo"),
            "imports: {:?}",
            imports
        );
    }

    #[test]
    fn python_extract_imports() {
        let content = "import os\nfrom collections import defaultdict\nimport sys\n";
        let imports = extract_imports(content, Language::Python, "py");
        assert!(imports.iter().any(|i| i == "os"), "imports: {:?}", imports);
        assert!(
            imports.iter().any(|i| i == "collections"),
            "imports: {:?}",
            imports
        );
        assert!(imports.iter().any(|i| i == "sys"), "imports: {:?}", imports);
    }

    #[test]
    fn go_extract_imports() {
        let content = "package main\nimport (\n\t\"fmt\"\n\t\"os\"\n)\n";
        let imports = extract_imports(content, Language::Go, "go");
        assert!(imports.iter().any(|i| i == "fmt"), "imports: {:?}", imports);
        assert!(imports.iter().any(|i| i == "os"), "imports: {:?}", imports);
    }

    #[test]
    fn java_extract_imports() {
        let content = "import java.util.HashMap;\nimport java.io.File;\nclass Foo {}\n";
        let imports = extract_imports(content, Language::Java, "java");
        assert!(
            imports.iter().any(|i| i.contains("java.util.HashMap")),
            "imports: {:?}",
            imports
        );
        assert!(
            imports.iter().any(|i| i.contains("java.io.File")),
            "imports: {:?}",
            imports
        );
    }

    #[test]
    fn csharp_extract_imports() {
        let content = "using System;\nusing System.Collections.Generic;\nclass Foo {}\n";
        let imports = extract_imports(content, Language::CSharp, "cs");
        assert!(
            imports.iter().any(|i| i.contains("System")),
            "imports: {:?}",
            imports
        );
    }

    #[test]
    fn generic_extract_imports_returns_empty() {
        let imports = extract_imports("hello world", Language::Generic, "txt");
        assert!(imports.is_empty());
    }

    #[test]
    fn kotlin_extract_imports_returns_empty() {
        let imports = extract_imports("import foo.bar", Language::Kotlin, "kt");
        assert!(imports.is_empty());
    }
}
