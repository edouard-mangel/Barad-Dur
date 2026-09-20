Source:
use tokio::{test as async_test, self as runtime}; use async_std as other; extern crate tokio as rt;
#[test] fn validate_test() { fn helper() {} }
#[cfg(test)] // attach
#[allow(dead_code)] mod checks { fn helper() {} }
mod local { #![cfg(test)] fn helper() {} }
mod shadow { use other::test as async_test; #[async_test] fn production() {} }
#[runtime::test] async fn check() {}
#[cfg(any(test, unix))] fn production() {}
fn outer(tokio: usize) { #[tokio::test] fn nested() {} }

Parse (errors=false):
(source_file (use_declaration argument: (scoped_use_list path: (identifier) list: (use_list (use_as_clause path: (identifier) alias: (identifier)) (use_as_clause path: (self) alias: (identifier))))) (use_declaration argument: (use_as_clause path: (identifier) alias: (identifier))) (extern_crate_declaration (crate) name: (identifier) alias: (identifier)) (attribute_item (attribute (identifier))) (function_item name: (identifier) parameters: (parameters) body: (block (function_item name: (identifier) parameters: (parameters) body: (block)))) (attribute_item (attribute (identifier) arguments: (token_tree (identifier)))) (line_comment) (attribute_item (attribute (identifier) arguments: (token_tree (identifier)))) (mod_item name: (identifier) body: (declaration_list (function_item name: (identifier) parameters: (parameters) body: (block)))) (mod_item name: (identifier) body: (declaration_list (inner_attribute_item (attribute (identifier) arguments: (token_tree (identifier)))) (function_item name: (identifier) parameters: (parameters) body: (block)))) (mod_item name: (identifier) body: (declaration_list (use_declaration argument: (use_as_clause path: (scoped_identifier path: (identifier) name: (identifier)) alias: (identifier))) (attribute_item (attribute (identifier))) (function_item name: (identifier) parameters: (parameters) body: (block)))) (attribute_item (attribute (scoped_identifier path: (identifier) name: (identifier)))) (function_item (function_modifiers) name: (identifier) parameters: (parameters) body: (block)) (attribute_item (attribute (identifier) arguments: (token_tree (identifier) (token_tree (identifier) (identifier))))) (function_item name: (identifier) parameters: (parameters) body: (block)) (function_item name: (identifier) parameters: (parameters (parameter pattern: (identifier) type: (primitive_type))) body: (block (attribute_item (attribute (scoped_identifier path: (identifier) name: (identifier)))) (function_item name: (identifier) parameters: (parameters) body: (block)))))
