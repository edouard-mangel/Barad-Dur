mod fallback;
mod lang_dispatch;
mod queries;
mod treesitter;

use std::path::Path;

use crate::snapshot::FileComplexity;

pub use fallback::{detect_language, Language};

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
