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

// ── Demeter (method chains depth ≥ 3) ───────────────────────────────

/// Rust: detects `x.a().b().c()` — three nested call_expression/field_expression pairs.
pub const RUST_DEMETER: &str = r#"(call_expression
  function: (field_expression
    value: (call_expression
      function: (field_expression
        value: (call_expression)
      )
    )
  )
) @demeter"#;

/// JavaScript: detects `a.b().c().d()` chains.
pub const JS_DEMETER: &str = r#"(call_expression
  function: (member_expression
    object: (call_expression
      function: (member_expression
        object: (call_expression)
      )
    )
  )
) @demeter"#;

/// TypeScript shares the same query as JavaScript.
pub const TS_DEMETER: &str = JS_DEMETER;

/// Python: detects `a.b.c.d` attribute chains (depth ≥ 3).
pub const PYTHON_DEMETER: &str = r#"(attribute
  object: (attribute
    object: (attribute)
  )
) @demeter"#;

/// Go: detects `a.Foo().Bar().Baz()` selector chains.
pub const GO_DEMETER: &str = r#"(call_expression
  function: (selector_expression
    operand: (call_expression
      function: (selector_expression
        operand: (call_expression)
      )
    )
  )
) @demeter"#;

/// Java: detects `a.foo().bar().baz()` method invocation chains.
pub const JAVA_DEMETER: &str = r#"(method_invocation
  object: (method_invocation
    object: (method_invocation)
  )
) @demeter"#;

/// C#: detects `a.Foo().Bar().Baz()` invocation chains.
pub const CSHARP_DEMETER: &str = r#"(invocation_expression
  function: (member_access_expression
    expression: (invocation_expression
      function: (member_access_expression
        expression: (invocation_expression)
      )
    )
  )
) @demeter"#;

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
        assert_valid_query(rust(), RUST_DEMETER, "rust demeter");
    }

    #[test]
    fn js_queries_are_valid() {
        assert_valid_query(js(), JS_COMPLEXITY, "js complexity");
        assert_valid_query(js(), JS_COMPLEXITY_OPERATORS, "js operators");
        assert_valid_query(js(), JS_PUBLIC_METHODS, "js public_methods");
        assert_valid_query(js(), JS_PROPERTIES, "js properties");
        assert_valid_query(js(), JS_COMMENTS, "js comments");
        assert_valid_query(js(), JS_DEMETER, "js demeter");
    }

    #[test]
    fn ts_queries_are_valid() {
        // TS shares JS complexity + operators + comments queries
        assert_valid_query(ts(), JS_COMPLEXITY, "ts complexity");
        assert_valid_query(ts(), JS_COMPLEXITY_OPERATORS, "ts operators");
        assert_valid_query(ts(), TS_PUBLIC_METHODS, "ts public_methods");
        assert_valid_query(ts(), TS_PROPERTIES, "ts properties");
        assert_valid_query(ts(), JS_COMMENTS, "ts comments");
        assert_valid_query(ts(), TS_DEMETER, "ts demeter");
    }

    #[test]
    fn python_queries_are_valid() {
        assert_valid_query(python(), PYTHON_COMPLEXITY, "python complexity");
        assert_valid_query(python(), PYTHON_PUBLIC_METHODS, "python public_methods");
        assert_valid_query(python(), PYTHON_COMMENTS, "python comments");
        assert_valid_query(python(), PYTHON_DEMETER, "python demeter");
    }

    #[test]
    fn go_queries_are_valid() {
        assert_valid_query(go(), GO_COMPLEXITY, "go complexity");
        assert_valid_query(go(), GO_COMPLEXITY_OPERATORS, "go operators");
        assert_valid_query(go(), GO_PUBLIC_METHODS, "go public_methods");
        assert_valid_query(go(), GO_PROPERTIES, "go properties");
        assert_valid_query(go(), GO_COMMENTS, "go comments");
        assert_valid_query(go(), GO_DEMETER, "go demeter");
    }

    #[test]
    fn java_queries_are_valid() {
        assert_valid_query(java(), JAVA_COMPLEXITY, "java complexity");
        assert_valid_query(java(), JAVA_COMPLEXITY_OPERATORS, "java operators");
        assert_valid_query(java(), JAVA_PUBLIC_METHODS, "java public_methods");
        assert_valid_query(java(), JAVA_PROPERTIES, "java properties");
        assert_valid_query(java(), JAVA_COMMENTS, "java comments");
        assert_valid_query(java(), JAVA_DEMETER, "java demeter");
    }

    #[test]
    fn csharp_queries_are_valid() {
        assert_valid_query(csharp(), CSHARP_COMPLEXITY, "csharp complexity");
        assert_valid_query(csharp(), CSHARP_COMPLEXITY_OPERATORS, "csharp operators");
        assert_valid_query(csharp(), CSHARP_PUBLIC_METHODS, "csharp public_methods");
        assert_valid_query(csharp(), CSHARP_PROPERTIES, "csharp properties");
        assert_valid_query(csharp(), CSHARP_COMMENTS, "csharp comments");
        assert_valid_query(csharp(), CSHARP_DEMETER, "csharp demeter");
    }
}
