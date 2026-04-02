use super::fallback::Language;
use super::queries;

/// Map a language + file extension to the corresponding tree-sitter grammar.
pub fn grammar_for(lang: Language, ext: &str) -> Option<tree_sitter::Language> {
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

/// Map a language to its import query string, if supported.
pub fn import_query(lang: Language, _ext: &str) -> Option<&'static str> {
    match lang {
        Language::Rust => Some(queries::RUST_IMPORTS),
        Language::JsTs => Some(queries::JS_IMPORTS),
        Language::Python => Some(queries::PYTHON_IMPORTS),
        Language::Go => Some(queries::GO_IMPORTS),
        Language::Java => Some(queries::JAVA_IMPORTS),
        Language::CSharp => Some(queries::CSHARP_IMPORTS),
        Language::Kotlin | Language::Generic => None,
    }
}

/// Map a language to its cyclomatic complexity query strings.
pub fn complexity_queries(lang: Language, ext: &str) -> (&'static str, Option<&'static str>) {
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

/// Map a language to its function-node query string, if supported.
pub fn function_query(lang: Language, _ext: &str) -> Option<&'static str> {
    match lang {
        Language::Rust => Some(queries::RUST_FUNCTIONS),
        Language::JsTs => Some(queries::JS_FUNCTIONS),
        Language::Python => Some(queries::PYTHON_FUNCTIONS),
        Language::Go => Some(queries::GO_FUNCTIONS),
        Language::Java => Some(queries::JAVA_FUNCTIONS),
        Language::CSharp => Some(queries::CSHARP_FUNCTIONS),
        Language::Kotlin | Language::Generic => None,
    }
}

/// Map a language to its nesting query string, if supported.
pub fn nesting_query(lang: Language, _ext: &str) -> Option<&'static str> {
    match lang {
        Language::Rust => Some(queries::RUST_NESTING),
        Language::JsTs => Some(queries::JS_NESTING),
        Language::Python => Some(queries::PYTHON_NESTING),
        Language::Go => Some(queries::GO_NESTING),
        Language::Java => Some(queries::JAVA_NESTING),
        Language::CSharp => Some(queries::CSHARP_NESTING),
        Language::Kotlin | Language::Generic => None,
    }
}

/// Map a language to its comment query string (used for LOC calculation).
pub fn comment_query(lang: Language, ext: &str) -> &'static str {
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
