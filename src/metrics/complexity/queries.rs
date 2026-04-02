// ── Rust ─────────────────────────────────────────────────────────────

pub const RUST_COMPLEXITY: &str = r#"[
  (if_expression)
  (for_expression)
  (while_expression)
  (loop_expression)
  (match_expression)
] @cc"#;

pub const RUST_COMPLEXITY_OPERATORS: &str = r#"(binary_expression operator: ["&&" "||"]) @cc"#;

pub const RUST_PUBLIC_METHODS: &str =
    r#"(function_item (visibility_modifier) @vis name: (identifier) @name)"#;

pub const RUST_PROPERTIES: &str = r#"(field_declaration (visibility_modifier) @vis)"#;

pub const RUST_COMMENTS: &str = r#"[(line_comment) (block_comment)] @comment"#;

pub const RUST_FUNCTIONS: &str = r#"(function_item name: (identifier) @name) @func"#;

pub const RUST_NESTING: &str = r#"[
  (if_expression)
  (for_expression)
  (while_expression)
  (loop_expression)
  (match_expression)
] @nest"#;

// ── JavaScript ──────────────────────────────────────────────────────

pub const JS_COMPLEXITY: &str = r#"[
  (if_statement)
  (for_statement)
  (for_in_statement)
  (while_statement)
  (do_statement)
  (switch_statement)
  (catch_clause)
  (ternary_expression)
] @cc"#;

pub const JS_COMPLEXITY_OPERATORS: &str = r#"(binary_expression operator: ["&&" "||" "??"]) @cc"#;

pub const JS_PUBLIC_METHODS: &str = r#"(export_statement
  declaration: [
    (function_declaration)
    (lexical_declaration)
  ]
) @pub_method"#;

pub const JS_PROPERTIES: &str = r#"(field_definition) @prop"#;

pub const JS_COMMENTS: &str = r#"(comment) @comment"#;

pub const JS_FUNCTIONS: &str = r#"[
  (function_declaration name: (identifier) @name)
  (method_definition name: (property_identifier) @name)
] @func"#;

pub const JS_NESTING: &str = r#"[
  (if_statement)
  (for_statement)
  (for_in_statement)
  (while_statement)
  (do_statement)
  (switch_statement)
] @nest"#;

// ── TypeScript ──────────────────────────────────────────────────────
// TypeScript shares the same complexity and comment queries as JavaScript.
// Public methods and properties differ due to TS-specific node types.

pub const TS_PUBLIC_METHODS: &str = r#"(export_statement
  declaration: [
    (function_declaration)
    (lexical_declaration)
  ]
) @pub_method"#;

pub const TS_PROPERTIES: &str = r#"[
  (public_field_definition)
  (property_signature)
] @prop"#;

// ── Python ──────────────────────────────────────────────────────────

pub const PYTHON_COMPLEXITY: &str = r#"[
  (if_statement)
  (elif_clause)
  (for_statement)
  (while_statement)
  (except_clause)
  (boolean_operator)
  (conditional_expression)
] @cc"#;

pub const PYTHON_PUBLIC_METHODS: &str = r#"(function_definition name: (identifier) @name)"#;

pub const PYTHON_COMMENTS: &str = r#"(comment) @comment"#;

pub const PYTHON_FUNCTIONS: &str = r#"(function_definition name: (identifier) @name) @func"#;

pub const PYTHON_NESTING: &str = r#"[
  (if_statement)
  (for_statement)
  (while_statement)
  (with_statement)
] @nest"#;

// ── Go ──────────────────────────────────────────────────────────────

pub const GO_COMPLEXITY: &str = r#"[
  (if_statement)
  (for_statement)
  (expression_switch_statement)
  (type_switch_statement)
] @cc"#;

pub const GO_COMPLEXITY_OPERATORS: &str = r#"(binary_expression operator: ["&&" "||"]) @cc"#;

pub const GO_PUBLIC_METHODS: &str = r#"[
  (function_declaration name: (identifier) @name)
  (method_declaration name: (field_identifier) @name)
]"#;

pub const GO_PROPERTIES: &str = r#"(field_declaration name: (field_identifier) @name)"#;

pub const GO_COMMENTS: &str = r#"(comment) @comment"#;

pub const GO_FUNCTIONS: &str = r#"[
  (function_declaration name: (identifier) @name)
  (method_declaration name: (field_identifier) @name)
] @func"#;

pub const GO_NESTING: &str = r#"[
  (if_statement)
  (for_statement)
  (expression_switch_statement)
  (type_switch_statement)
] @nest"#;

// ── Java ────────────────────────────────────────────────────────────

pub const JAVA_COMPLEXITY: &str = r#"[
  (if_statement)
  (for_statement)
  (enhanced_for_statement)
  (while_statement)
  (do_statement)
  (switch_expression)
  (catch_clause)
  (ternary_expression)
] @cc"#;

pub const JAVA_COMPLEXITY_OPERATORS: &str = r#"(binary_expression operator: ["&&" "||"]) @cc"#;

pub const JAVA_PUBLIC_METHODS: &str =
    r#"(method_declaration (modifiers) @mods name: (identifier) @name)"#;

pub const JAVA_PROPERTIES: &str = r#"(field_declaration (modifiers) @mods)"#;

pub const JAVA_COMMENTS: &str = r#"[(line_comment) (block_comment)] @comment"#;

pub const JAVA_FUNCTIONS: &str = r#"(method_declaration name: (identifier) @name) @func"#;

pub const JAVA_NESTING: &str = r#"[
  (if_statement)
  (for_statement)
  (enhanced_for_statement)
  (while_statement)
  (do_statement)
  (switch_expression)
] @nest"#;

// ── C# ──────────────────────────────────────────────────────────────

pub const CSHARP_COMPLEXITY: &str = r#"[
  (if_statement)
  (for_statement)
  (foreach_statement)
  (while_statement)
  (do_statement)
  (switch_expression)
  (catch_clause)
  (conditional_expression)
] @cc"#;

pub const CSHARP_COMPLEXITY_OPERATORS: &str = r#"(binary_expression operator: ["&&" "||"]) @cc"#;

pub const CSHARP_PUBLIC_METHODS: &str =
    r#"(method_declaration (modifier) @mod name: (identifier) @name)"#;

pub const CSHARP_PROPERTIES: &str = r#"(field_declaration (modifier) @mod)"#;

pub const CSHARP_COMMENTS: &str = r#"(comment) @comment"#;

pub const CSHARP_FUNCTIONS: &str = r#"(method_declaration name: (identifier) @name) @func"#;

pub const CSHARP_NESTING: &str = r#"[
  (if_statement)
  (for_statement)
  (foreach_statement)
  (while_statement)
  (do_statement)
  (switch_statement)
] @nest"#;

// ── Import queries ─────────────────────────────────────────────────

pub const RUST_IMPORTS: &str = r#"(use_declaration argument: (_) @path)"#;

pub const JS_IMPORTS: &str = r#"[
  (import_statement source: (string) @path)
  (call_expression
    function: (identifier) @fn
    arguments: (arguments (string) @path)
    (#eq? @fn "require"))
] "#;

pub const PYTHON_IMPORTS: &str = r#"[
  (import_statement name: (dotted_name) @path)
  (import_from_statement module_name: (dotted_name) @path)
]"#;

pub const GO_IMPORTS: &str = r#"(import_spec path: (interpreted_string_literal) @path)"#;

pub const JAVA_IMPORTS: &str = r#"(import_declaration (scoped_identifier) @path)"#;

pub const CSHARP_IMPORTS: &str = r#"(using_directive (_) @path)"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_valid_query(grammar: tree_sitter::Language, query_src: &str, label: &str) {
        tree_sitter::Query::new(&grammar, query_src)
            .unwrap_or_else(|e| panic!("{label} query failed: {e}"));
    }

    fn rust() -> tree_sitter::Language {
        tree_sitter_rust::LANGUAGE.into()
    }
    fn js() -> tree_sitter::Language {
        tree_sitter_javascript::LANGUAGE.into()
    }
    fn ts() -> tree_sitter::Language {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    }
    fn python() -> tree_sitter::Language {
        tree_sitter_python::LANGUAGE.into()
    }
    fn go() -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
    }
    fn java() -> tree_sitter::Language {
        tree_sitter_java::LANGUAGE.into()
    }
    fn csharp() -> tree_sitter::Language {
        tree_sitter_c_sharp::LANGUAGE.into()
    }

    #[test]
    fn rust_queries_are_valid() {
        assert_valid_query(rust(), RUST_COMPLEXITY, "rust complexity");
        assert_valid_query(rust(), RUST_COMPLEXITY_OPERATORS, "rust operators");
        assert_valid_query(rust(), RUST_PUBLIC_METHODS, "rust public_methods");
        assert_valid_query(rust(), RUST_PROPERTIES, "rust properties");
        assert_valid_query(rust(), RUST_COMMENTS, "rust comments");
        assert_valid_query(rust(), RUST_FUNCTIONS, "rust functions");
        assert_valid_query(rust(), RUST_NESTING, "rust nesting");
    }

    #[test]
    fn js_queries_are_valid() {
        assert_valid_query(js(), JS_COMPLEXITY, "js complexity");
        assert_valid_query(js(), JS_COMPLEXITY_OPERATORS, "js operators");
        assert_valid_query(js(), JS_PUBLIC_METHODS, "js public_methods");
        assert_valid_query(js(), JS_PROPERTIES, "js properties");
        assert_valid_query(js(), JS_COMMENTS, "js comments");
        assert_valid_query(js(), JS_FUNCTIONS, "js functions");
        assert_valid_query(js(), JS_NESTING, "js nesting");
    }

    #[test]
    fn ts_queries_are_valid() {
        // TS shares JS complexity + operators + comments queries
        assert_valid_query(ts(), JS_COMPLEXITY, "ts complexity");
        assert_valid_query(ts(), JS_COMPLEXITY_OPERATORS, "ts operators");
        assert_valid_query(ts(), TS_PUBLIC_METHODS, "ts public_methods");
        assert_valid_query(ts(), TS_PROPERTIES, "ts properties");
        assert_valid_query(ts(), JS_COMMENTS, "ts comments");
    }

    #[test]
    fn python_queries_are_valid() {
        assert_valid_query(python(), PYTHON_COMPLEXITY, "python complexity");
        assert_valid_query(python(), PYTHON_PUBLIC_METHODS, "python public_methods");
        assert_valid_query(python(), PYTHON_FUNCTIONS, "python functions");
        assert_valid_query(python(), PYTHON_NESTING, "python nesting");
        assert_valid_query(python(), PYTHON_COMMENTS, "python comments");
    }

    #[test]
    fn go_queries_are_valid() {
        assert_valid_query(go(), GO_COMPLEXITY, "go complexity");
        assert_valid_query(go(), GO_COMPLEXITY_OPERATORS, "go operators");
        assert_valid_query(go(), GO_PUBLIC_METHODS, "go public_methods");
        assert_valid_query(go(), GO_PROPERTIES, "go properties");
        assert_valid_query(go(), GO_COMMENTS, "go comments");
        assert_valid_query(go(), GO_FUNCTIONS, "go functions");
        assert_valid_query(go(), GO_NESTING, "go nesting");
    }

    #[test]
    fn java_queries_are_valid() {
        assert_valid_query(java(), JAVA_COMPLEXITY, "java complexity");
        assert_valid_query(java(), JAVA_COMPLEXITY_OPERATORS, "java operators");
        assert_valid_query(java(), JAVA_PUBLIC_METHODS, "java public_methods");
        assert_valid_query(java(), JAVA_PROPERTIES, "java properties");
        assert_valid_query(java(), JAVA_COMMENTS, "java comments");
        assert_valid_query(java(), JAVA_FUNCTIONS, "java functions");
        assert_valid_query(java(), JAVA_NESTING, "java nesting");
    }

    #[test]
    fn csharp_queries_are_valid() {
        assert_valid_query(csharp(), CSHARP_COMPLEXITY, "csharp complexity");
        assert_valid_query(csharp(), CSHARP_COMPLEXITY_OPERATORS, "csharp operators");
        assert_valid_query(csharp(), CSHARP_PUBLIC_METHODS, "csharp public_methods");
        assert_valid_query(csharp(), CSHARP_PROPERTIES, "csharp properties");
        assert_valid_query(csharp(), CSHARP_COMMENTS, "csharp comments");
        assert_valid_query(csharp(), CSHARP_FUNCTIONS, "csharp functions");
        assert_valid_query(csharp(), CSHARP_NESTING, "csharp nesting");
    }

    #[test]
    fn rust_import_query_is_valid() {
        assert_valid_query(rust(), RUST_IMPORTS, "rust imports");
    }

    #[test]
    fn js_import_query_is_valid() {
        assert_valid_query(js(), JS_IMPORTS, "js imports");
    }

    #[test]
    fn ts_import_query_is_valid() {
        assert_valid_query(ts(), JS_IMPORTS, "ts imports");
    }

    #[test]
    fn python_import_query_is_valid() {
        assert_valid_query(python(), PYTHON_IMPORTS, "python imports");
    }

    #[test]
    fn go_import_query_is_valid() {
        assert_valid_query(go(), GO_IMPORTS, "go imports");
    }

    #[test]
    fn java_import_query_is_valid() {
        assert_valid_query(java(), JAVA_IMPORTS, "java imports");
    }

    #[test]
    fn csharp_import_query_is_valid() {
        assert_valid_query(csharp(), CSHARP_IMPORTS, "csharp imports");
    }
}
