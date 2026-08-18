mod calls;
mod counters;
mod fallback;
mod inheritance;
mod lang_dispatch;
mod pressman;
mod queries;
mod treesitter;

use std::path::Path;

use crate::snapshot::{CouplingFinding, FileComplexity};

pub use calls::{extract_call_edges, RawCallEdge, RawCalleeRef};
pub use fallback::{detect_language, Language};
pub use inheritance::{
    extract_class_records, extract_reexports, RawBaseRef, RawClassRecord, RawReExport,
    RawReExportKind,
};
pub use pressman::extract_coupling_findings;

/// Everything the collector extracts from one source file, produced from a
/// single tree-sitter parse (the individual `extract_*`/`analyse_file`
/// functions each re-parse and exist for callers that need only one facet).
#[derive(Debug)]
pub struct SourceAnalysis {
    pub metrics: FileComplexity,
    pub imports: Vec<String>,
    pub coupling_findings: Vec<CouplingFinding>,
    pub class_records: Vec<RawClassRecord>,
    pub reexports: Vec<RawReExport>,
    pub call_edges: Vec<RawCallEdge>,
}

/// Analyse one file for complexity metrics, imports, coupling findings, and
/// class records, parsing its AST exactly once.
pub fn analyse_source(path: &Path, content: &str) -> SourceAnalysis {
    let lang = detect_language(&path.to_string_lossy());
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let parsed = lang_dispatch::grammar_for(lang, ext)
        .and_then(|grammar| treesitter::parse(content, &grammar).map(|tree| (tree, grammar)));
    match parsed {
        Some((tree, grammar)) => SourceAnalysis {
            metrics: treesitter::analyse_tree(&tree, content, lang, ext, &grammar),
            imports: treesitter::imports_from_tree(&tree, content, lang, ext, &grammar),
            coupling_findings: pressman::findings_from_tree(tree.root_node(), content, path, lang),
            class_records: match lang {
                Language::JsTs => inheritance::class_records_from_tree(tree.root_node(), content),
                _ => Vec::new(),
            },
            reexports: match lang {
                Language::JsTs => inheritance::reexports_from_tree(tree.root_node(), content),
                _ => Vec::new(),
            },
            call_edges: match lang {
                Language::JsTs => calls::call_edges_from_tree(tree.root_node(), content),
                _ => Vec::new(),
            },
        },
        None => SourceAnalysis {
            metrics: fallback::analyse_content(content, lang),
            imports: Vec::new(),
            coupling_findings: Vec::new(),
            class_records: Vec::new(),
            reexports: Vec::new(),
            call_edges: Vec::new(),
        },
    }
}

/// Extract raw import paths from a source file using tree-sitter.
pub fn extract_file_imports(path: &Path, content: &str) -> Vec<String> {
    let lang = detect_language(&path.to_string_lossy());
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    treesitter::extract_imports(content, lang, ext)
}

pub fn analyse_file(path: &Path, content: &str) -> FileComplexity {
    let lang = detect_language(&path.to_string_lossy());
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    treesitter::analyse(content, lang, ext)
        .unwrap_or_else(|| fallback::analyse_content(content, lang))
}

pub fn analyse_content(content: &str, lang: Language) -> FileComplexity {
    let default_ext = match lang {
        Language::Java => "java",
        Language::Kotlin => "kt",
        Language::CSharp => "cs",
        Language::JsTs => "js",
        _ => "",
    };
    treesitter::analyse(content, lang, default_ext)
        .unwrap_or_else(|| fallback::analyse_content(content, lang))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyse_source_matches_the_five_individual_extractions() {
        let content = "import { Base } from './base';\n\
                       export class Derived extends Base {}\n\
                       export function run(flag: boolean) {\n\
                         if (flag) {\n\
                           return check(flag);\n\
                         }\n\
                         return 2;\n\
                       }\n";
        let path = Path::new("src/derived.ts");

        let combined = analyse_source(path, content);

        assert_eq!(combined.metrics, analyse_file(path, content));
        assert_eq!(combined.imports, extract_file_imports(path, content));
        assert_eq!(
            combined.coupling_findings,
            extract_coupling_findings(path, content)
        );
        assert_eq!(combined.class_records, extract_class_records(path, content));
        assert_eq!(combined.call_edges, extract_call_edges(path, content));
        // The sample must actually exercise every channel.
        assert!(!combined.imports.is_empty());
        assert!(!combined.coupling_findings.is_empty());
        assert!(!combined.class_records.is_empty());
        assert!(!combined.call_edges.is_empty());
    }

    #[test]
    fn analyse_source_unsupported_language_falls_back() {
        let combined = analyse_source(Path::new("data.xyz"), "some content\nmore\n");
        assert_eq!(combined.metrics.total_lines, 2);
        assert!(combined.imports.is_empty());
        assert!(combined.coupling_findings.is_empty());
        assert!(combined.class_records.is_empty());
    }

    #[test]
    fn analyse_file_uses_treesitter_for_rust() {
        let content = "pub fn foo() {}\nfn bar() {}\n";
        let result = analyse_file(Path::new("test.rs"), content);
        assert_eq!(result.public_methods, 1);
    }

    #[test]
    fn analyse_content_generic_uses_fallback() {
        let content = "if x { } for y { }";
        let result = analyse_content(content, Language::Generic);
        assert!(result.total_lines > 0);
    }

    #[test]
    fn analyse_file_unknown_extension_uses_fallback() {
        let content = "some content\nmore content\n";
        let result = analyse_file(Path::new("data.xyz"), content);
        assert_eq!(result.total_lines, 2);
    }

    #[test]
    fn kotlin_falls_back_to_line_based() {
        let content = "fun main() {\n    if (true) {}\n}\n";
        let result = analyse_file(Path::new("app.kt"), content);
        assert!(result.total_lines > 0);
    }

    // Kill mutants: analyse_content match arms for each Language variant
    #[test]
    fn analyse_content_java_uses_treesitter() {
        let content = "class Foo {\n  public void bar() {}\n  private void baz() {}\n}\n";
        let result = analyse_content(content, Language::Java);
        assert_eq!(result.public_methods, 1);
    }

    #[test]
    fn analyse_content_csharp_uses_treesitter() {
        let content = "class Foo {\n  public void Bar() {}\n  private void Baz() {}\n}\n";
        let result = analyse_content(content, Language::CSharp);
        assert_eq!(result.public_methods, 1);
    }

    #[test]
    fn analyse_content_jsts_uses_treesitter() {
        let content = "export function foo() {}\nfunction bar() {}\n";
        let result = analyse_content(content, Language::JsTs);
        assert_eq!(result.public_methods, 1);
    }

    #[test]
    fn analyse_content_kotlin_falls_back() {
        let content = "fun foo() {}\nprivate fun bar() {}\n";
        let result = analyse_content(content, Language::Kotlin);
        assert_eq!(result.total_lines, 2);
    }
}
