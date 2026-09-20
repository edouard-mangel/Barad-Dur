# Responsibility provenance P0 grammar probes

Executed before implementing detectors, using pinned tree-sitter dependencies via a standalone Rust parser. These ASTs establish declaration, receiver, nested scope, binding, overload and malformed-syntax shapes. Malformed functions will have unknown provenance.

### rust

Source: `struct A { x: bool } impl A { fn is_a(&self) { self.x; self.help(); let help = || true; help(); fn nested() { helper(); } } fn help(&self) {} } impl A { fn is_b(&self) { self.x; } } fn bad( {`

```text
(source_file (struct_item name: (type_identifier) body: (field_declaration_list (field_declaration name: (field_identifier) type: (primitive_type)))) (impl_item type: (type_identifier) body: (declaration_list (function_item name: (identifier) parameters: (parameters (self_parameter (self))) body: (block (expression_statement (field_expression value: (self) field: (field_identifier))) (expression_statement (call_expression function: (field_expression value: (self) field: (field_identifier)) arguments: (arguments))) (let_declaration pattern: (identifier) value: (closure_expression parameters: (closure_parameters) body: (boolean_literal))) (expression_statement (call_expression function: (identifier) arguments: (arguments))) (function_item name: (identifier) parameters: (parameters) body: (block (expression_statement (call_expression function: (identifier) arguments: (arguments))))))) (function_item name: (identifier) parameters: (parameters (self_parameter (self))) body: (block)))) (impl_item type: (type_identifier) body: (declaration_list (function_item name: (identifier) parameters: (parameters (self_parameter (self))) body: (block (expression_statement (field_expression value: (self) field: (field_identifier))))))) (ERROR (identifier)))
```

### js

Source: `class A { x; isA(other) { this.x; this.help(); other.x; let help = () => this.x; help(); } help() {} } function outer() { function inner() {} }`

```text
(program (class_declaration name: (identifier) body: (class_body member: (field_definition property: (property_identifier)) member: (method_definition name: (property_identifier) parameters: (formal_parameters (identifier)) body: (statement_block (expression_statement (member_expression object: (this) property: (property_identifier))) (expression_statement (call_expression function: (member_expression object: (this) property: (property_identifier)) arguments: (arguments))) (expression_statement (member_expression object: (identifier) property: (property_identifier))) (lexical_declaration (variable_declarator name: (identifier) value: (arrow_function parameters: (formal_parameters) body: (member_expression object: (this) property: (property_identifier))))) (expression_statement (call_expression function: (identifier) arguments: (arguments))))) member: (method_definition name: (property_identifier) parameters: (formal_parameters) body: (statement_block)))) (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block)))))
```

### ts

Source: `namespace N { interface A { help(): boolean; } class B { x: boolean; isA() { return this.x; } } }`

```text
(program (expression_statement (internal_module name: (identifier) body: (statement_block (interface_declaration name: (type_identifier) body: (interface_body (method_signature name: (property_identifier) parameters: (formal_parameters) return_type: (type_annotation (predefined_type))))) (class_declaration name: (type_identifier) body: (class_body (public_field_definition name: (property_identifier) type: (type_annotation (predefined_type))) (method_definition name: (property_identifier) parameters: (formal_parameters) body: (statement_block (return_statement (member_expression object: (this) property: (property_identifier)))))))))))
```

### python

Source: `class A:
 def is_a(self, other):
  self.x; self.help(); other.x
  self = other
  def nested(): return self.x
 def help(self): pass
`

```text
(module (class_definition name: (identifier) body: (block (function_definition name: (identifier) parameters: (parameters (identifier) (identifier)) body: (block (expression_statement (attribute object: (identifier) attribute: (identifier))) (expression_statement (call function: (attribute object: (identifier) attribute: (identifier)) arguments: (argument_list))) (expression_statement (attribute object: (identifier) attribute: (identifier))) (expression_statement (assignment left: (identifier) right: (identifier))) (function_definition name: (identifier) parameters: (parameters) body: (block (return_statement (attribute object: (identifier) attribute: (identifier))))))) (function_definition name: (identifier) parameters: (parameters (identifier)) body: (block (pass_statement))))))
```

### go

Source: `package p; type A struct { x bool }; func (a *A) isA() bool { a.help(); return a.x }; func (a A) help() {}`

```text
(source_file (package_clause (package_identifier)) (type_declaration (type_spec name: (type_identifier) type: (struct_type (field_declaration_list (field_declaration name: (field_identifier) type: (type_identifier)))))) (method_declaration receiver: (parameter_list (parameter_declaration name: (identifier) type: (pointer_type (type_identifier)))) name: (field_identifier) parameters: (parameter_list) result: (type_identifier) body: (block (statement_list (expression_statement (call_expression function: (selector_expression operand: (identifier) field: (field_identifier)) arguments: (argument_list))) (return_statement (expression_list (selector_expression operand: (identifier) field: (field_identifier))))))) (method_declaration receiver: (parameter_list (parameter_declaration name: (identifier) type: (type_identifier))) name: (field_identifier) parameters: (parameter_list) body: (block)))
```

### java

Source: `class A { boolean x; boolean isA(boolean x) { help(); this.help(); return this.x; } void help() {} void help(int x) {} }`

```text
(program (class_declaration name: (identifier) body: (class_body (field_declaration type: (boolean_type) declarator: (variable_declarator name: (identifier))) (method_declaration type: (boolean_type) name: (identifier) parameters: (formal_parameters (formal_parameter type: (boolean_type) name: (identifier))) body: (block (expression_statement (method_invocation name: (identifier) arguments: (argument_list))) (expression_statement (method_invocation object: (this) name: (identifier) arguments: (argument_list))) (return_statement (field_access object: (this) field: (identifier))))) (method_declaration type: (void_type) name: (identifier) parameters: (formal_parameters) body: (block)) (method_declaration type: (void_type) name: (identifier) parameters: (formal_parameters (formal_parameter type: (integral_type) name: (identifier))) body: (block)))))
```

### cs

Source: `namespace N { class A { bool x; bool IsA(bool x) { Help(); this.Help(); return this.x; } void Help() {} } }`

```text
(compilation_unit (namespace_declaration name: (identifier) body: (declaration_list (class_declaration name: (identifier) body: (declaration_list (field_declaration (variable_declaration type: (predefined_type) (variable_declarator name: (identifier)))) (method_declaration returns: (predefined_type) name: (identifier) parameters: (parameter_list (parameter type: (predefined_type) name: (identifier))) body: (block (expression_statement (invocation_expression function: (identifier) arguments: (argument_list))) (expression_statement (invocation_expression function: (member_access_expression name: (identifier)) arguments: (argument_list))) (return_statement (member_access_expression name: (identifier))))) (method_declaration returns: (predefined_type) name: (identifier) parameters: (parameter_list) body: (block)))))))
```

### kt

Source: `class A(val x: Boolean) { fun isA(): Boolean { help(); this.help(); return x }; fun help() {} } object B { fun isB() = this.x }`

```text
(source_file (ERROR (identifier) (primary_constructor (class_parameters (class_parameter (identifier) (user_type (identifier))))) (function_declaration name: (identifier) (function_value_parameters) (user_type (identifier)) (function_body (block (call_expression (identifier) (value_arguments)) (call_expression (navigation_expression (this_expression) (identifier)) (value_arguments)) (return_expression (identifier))))) (function_declaration name: (identifier) (function_value_parameters) (function_body (block)))))
```

### php

Source: `<?php namespace N; class A { private $x; function isA($other) { $this->help(); $other->x; return $this->x; } function help() {} }`

```text
(program (php_tag) (namespace_definition name: (namespace_name (name))) (class_declaration name: (name) body: (declaration_list (property_declaration (visibility_modifier) (property_element name: (variable_name (name)))) (method_declaration name: (name) parameters: (formal_parameters (simple_parameter name: (variable_name (name)))) body: (compound_statement (expression_statement (member_call_expression object: (variable_name (name)) name: (name) arguments: (arguments))) (expression_statement (member_access_expression object: (variable_name (name)) name: (name))) (return_statement (member_access_expression object: (variable_name (name)) name: (name))))) (method_declaration name: (name) parameters: (formal_parameters) body: (compound_statement)))))
```


## Corrected Kotlin newline probe

### kt

Source: `class A(val x: Boolean) {
 fun isA(): Boolean {
 help()
 this.help()
 return x
 }
 fun help() {}
}
object B {
 fun isB() = this.x
}`

```text
(source_file (class_declaration name: (identifier) (primary_constructor (class_parameters (class_parameter (identifier) (user_type (identifier))))) (class_body (function_declaration name: (identifier) (function_value_parameters) (user_type (identifier)) (function_body (block (call_expression (identifier) (value_arguments)) (call_expression (navigation_expression (this_expression) (identifier)) (value_arguments)) (return_expression (identifier))))) (function_declaration name: (identifier) (function_value_parameters) (function_body (block))))) (object_declaration name: (identifier) (class_body (function_declaration name: (identifier) (function_value_parameters) (function_body (navigation_expression (this_expression) (identifier)))))))
```


## Additional executable scope probes before extending detector cases

```text
## go_callback
AST (source_file (package_clause (package_identifier)) (type_declaration (type_spec name: (type_identifier) type: (struct_type (field_declaration_list (field_declaration name: (field_identifier) type: (type_identifier)))))) (method_declaration receiver: (parameter_list (parameter_declaration name: (identifier) type: (type_identifier))) name: (field_identifier) parameters: (parameter_list) body: (block (statement_list (short_var_declaration left: (expression_list (identifier)) right: (expression_list (func_literal parameters: (parameter_list) body: (block (statement_list (expression_statement (call_expression function: (selector_expression operand: (identifier) field: (field_identifier)) arguments: (argument_list))) (assignment_statement left: (expression_list (identifier)) right: (expression_list (selector_expression operand: (identifier) field: (field_identifier))))))))) (assignment_statement left: (expression_list (identifier)) right: (expression_list (identifier)))))) (method_declaration receiver: (parameter_list (parameter_declaration name: (identifier) type: (type_identifier))) name: (field_identifier) parameters: (parameter_list) body: (block)))
helper ResponsibilityProvenance { owner_id: "type:16:value", owner_label: "A", dependencies: [] }
is_a ResponsibilityProvenance { owner_id: "type:16:value", owner_label: "A", dependencies: [Field { identity: "type:16:value:field:ready", label: "ready" }, Callee { identity: "function:111", label: "helper" }] }

## cs_callback
AST (compilation_unit (class_declaration name: (identifier) body: (declaration_list (field_declaration (variable_declaration type: (predefined_type) (variable_declarator name: (identifier)))) (method_declaration returns: (predefined_type) name: (identifier) parameters: (parameter_list) body: (block (local_declaration_statement (variable_declaration type: (qualified_name qualifier: (identifier) name: (identifier)) (variable_declarator name: (identifier) (anonymous_method_expression parameters: (parameter_list) (block (expression_statement (invocation_expression function: (identifier) arguments: (argument_list))) (local_declaration_statement (variable_declaration type: (implicit_type) (variable_declarator name: (identifier) (member_access_expression name: (identifier)))))))))))) (method_declaration returns: (predefined_type) name: (identifier) parameters: (parameter_list) body: (block)))))
helper ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [] }
is_a ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [Field { identity: "class_declaration:0:field:ready", label: "ready" }, Callee { identity: "function:102", label: "helper" }] }

## go_nested_type
AST (source_file (package_clause (package_identifier)) (function_declaration name: (identifier) parameters: (parameter_list) body: (block (statement_list (type_declaration (type_spec name: (type_identifier) type: (struct_type (field_declaration_list (field_declaration name: (field_identifier) type: (type_identifier)))))) (assignment_statement left: (expression_list (identifier)) right: (expression_list (composite_literal type: (type_identifier) body: (literal_value))))))) (method_declaration receiver: (parameter_list (parameter_declaration name: (identifier) type: (type_identifier))) name: (field_identifier) parameters: (parameter_list) body: (block (statement_list (assignment_statement left: (expression_list (identifier)) right: (expression_list (selector_expression operand: (identifier) field: (field_identifier))))))))
is_a ResponsibilityProvenance { owner_id: "type:31:value", owner_label: "A", dependencies: [Field { identity: "type:31:value:field:ready", label: "ready" }] }
outer ResponsibilityProvenance { owner_id: "source_file:0", owner_label: "file", dependencies: [] }

## js_shadow_type
AST (program (class_declaration name: (identifier) body: (class_body member: (method_definition name: (property_identifier) parameters: (formal_parameters) body: (statement_block)))) (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block (class_declaration name: (identifier) body: (class_body)) (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block (expression_statement (call_expression function: (member_expression object: (identifier) property: (property_identifier)) arguments: (arguments))))))))
outer ResponsibilityProvenance { owner_id: "program:0", owner_label: "file", dependencies: [] }
is_a ResponsibilityProvenance { owner_id: "function_declaration:31", owner_label: "outer (line 1)", dependencies: [Callee { identity: "function:10", label: "helper" }] }
helper ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [] }

## py_shadow_type
AST (module (class_definition name: (identifier) body: (block (function_definition name: (identifier) parameters: (parameters) body: (block (pass_statement))))) (function_definition name: (identifier) parameters: (parameters) body: (block (class_definition name: (identifier) body: (block (pass_statement))) (function_definition name: (identifier) parameters: (parameters) body: (block (expression_statement (call function: (attribute object: (identifier) attribute: (identifier)) arguments: (argument_list))))))))
outer ResponsibilityProvenance { owner_id: "module:0", owner_label: "file", dependencies: [] }
is_a ResponsibilityProvenance { owner_id: "function_definition:29", owner_label: "outer (line 3)", dependencies: [Callee { identity: "function:10", label: "helper" }] }
helper ResponsibilityProvenance { owner_id: "class_definition:0", owner_label: "A (line 1)", dependencies: [] }

## js_block_scope
AST (program (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block)) (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block (statement_block (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block))) (expression_statement (call_expression function: (identifier) arguments: (arguments))))))
is_a ResponsibilityProvenance { owner_id: "program:0", owner_label: "file", dependencies: [Callee { identity: "function:41", label: "helper" }] }
helper ResponsibilityProvenance { owner_id: "function_declaration:21", owner_label: "is_a (line 1)", dependencies: [] }
helper ResponsibilityProvenance { owner_id: "program:0", owner_label: "file", dependencies: [] }

## go_for_receiver
AST (source_file (package_clause (package_identifier)) (type_declaration (type_spec name: (type_identifier) type: (struct_type (field_declaration_list (field_declaration name: (field_identifier) type: (type_identifier)))))) (method_declaration receiver: (parameter_list (parameter_declaration name: (identifier) type: (type_identifier))) name: (field_identifier) parameters: (parameter_list (parameter_declaration name: (identifier) type: (slice_type element: (type_identifier)))) body: (block (statement_list (for_statement (range_clause left: (expression_list (identifier) (identifier)) right: (identifier)) body: (block (statement_list (assignment_statement left: (expression_list (identifier)) right: (expression_list (selector_expression operand: (identifier) field: (field_identifier)))) (expression_statement (call_expression function: (selector_expression operand: (identifier) field: (field_identifier)) arguments: (argument_list))))))))) (method_declaration receiver: (parameter_list (parameter_declaration name: (identifier) type: (type_identifier))) name: (field_identifier) parameters: (parameter_list) body: (block)))
helper ResponsibilityProvenance { owner_id: "type:16:value", owner_label: "A", dependencies: [] }
is_a ResponsibilityProvenance { owner_id: "type:16:value", owner_label: "A", dependencies: [Field { identity: "type:16:value:field:ready", label: "ready" }, Callee { identity: "function:119", label: "helper" }] }

## py_with_receiver
AST (module (class_definition name: (identifier) body: (block (function_definition name: (identifier) parameters: (parameters (identifier) (identifier)) body: (block (with_statement (with_clause (with_item value: (as_pattern (identifier) alias: (as_pattern_target (identifier))))) body: (block (expression_statement (call function: (attribute object: (identifier) attribute: (identifier)) arguments: (argument_list))) (expression_statement (attribute object: (identifier) attribute: (identifier))))))) (function_definition name: (identifier) parameters: (parameters (identifier)) body: (block (pass_statement))))))
helper ResponsibilityProvenance { owner_id: "class_definition:0", owner_label: "A (line 1)", dependencies: [] }
is_a ResponsibilityProvenance { owner_id: "class_definition:0", owner_label: "A (line 1)", dependencies: [Field { identity: "class_definition:0:field:ready", label: "ready" }, Callee { identity: "function:89", label: "helper" }] }

## js_catch_shadow
AST (program (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block)) (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block (try_statement body: (statement_block) handler: (catch_clause parameter: (identifier) body: (statement_block (expression_statement (call_expression function: (identifier) arguments: (arguments)))))))))
is_a ResponsibilityProvenance { owner_id: "program:0", owner_label: "file", dependencies: [Callee { identity: "function:0", label: "helper" }] }
helper ResponsibilityProvenance { owner_id: "program:0", owner_label: "file", dependencies: [] }

## java_enhanced_shadow
AST (program (class_declaration name: (identifier) body: (class_body (field_declaration type: (boolean_type) declarator: (variable_declarator name: (identifier))) (method_declaration type: (void_type) name: (identifier) parameters: (formal_parameters (formal_parameter type: (array_type element: (boolean_type) dimensions: (dimensions)) name: (identifier))) body: (block (enhanced_for_statement type: (boolean_type) name: (identifier) value: (identifier) body: (block (expression_statement (method_invocation name: (identifier) arguments: (argument_list (identifier)))))))))))
is_a ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [Field { identity: "class_declaration:0:field:ready", label: "ready" }] }

## kt_property_initializer
AST (source_file (class_declaration name: (identifier) (class_body (property_declaration (variable_declaration (identifier)) (call_expression (identifier) (value_arguments))) (function_declaration name: (identifier) (function_value_parameters) (function_body (identifier))) (function_declaration name: (identifier) (function_value_parameters) (function_body (identifier))))))
factory ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [] }
is_a ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [Field { identity: "class_declaration:0:field:ready", label: "ready" }] }


```

## Owner boundary and annotated receiver probes

```text
## kt_companion
AST (source_file (class_declaration name: (identifier) (class_body (companion_object (class_body (function_declaration name: (identifier) (function_value_parameters) (function_body (call_expression (identifier) (value_arguments)))) (function_declaration name: (identifier) (function_value_parameters) (function_body (identifier))))) (function_declaration name: (identifier) (function_value_parameters) (function_body (call_expression (identifier) (value_arguments)))) (function_declaration name: (identifier) (function_value_parameters) (function_body (identifier))))))
helper ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [] }
is_b ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [] }
helper ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [] }
is_a ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [] }

## kt_extension
AST (source_file (class_declaration name: (identifier) (class_body (function_declaration (user_type (identifier)) name: (identifier) (function_value_parameters) (function_body (navigation_expression (this_expression) (identifier)))) (function_declaration name: (identifier) (function_value_parameters) (function_body (navigation_expression (this_expression) (identifier)))))))
is_b ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [Field { identity: "class_declaration:0:field:length", label: "length" }] }
is_a ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [Field { identity: "class_declaration:0:field:length", label: "length" }] }

## java_anon
AST (program (class_declaration name: (identifier) body: (class_body (field_declaration type: (boolean_type) declarator: (variable_declarator name: (identifier))) (method_declaration type: (void_type) name: (identifier) parameters: (formal_parameters) body: (block (local_variable_declaration type: (type_identifier) declarator: (variable_declarator name: (identifier) value: (object_creation_expression type: (type_identifier) arguments: (argument_list) (class_body (field_declaration type: (boolean_type) declarator: (variable_declarator name: (identifier))) (method_declaration type: (boolean_type) name: (identifier) parameters: (formal_parameters) body: (block (return_statement (field_access object: (this) field: (identifier))))))))))))))
is_a ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [] }
get ResponsibilityProvenance { owner_id: "method_declaration:25", owner_label: "is_a (line 1)", dependencies: [] }

## js_object
AST (program (lexical_declaration (variable_declarator name: (identifier) value: (object (method_definition name: (property_identifier) parameters: (formal_parameters) body: (statement_block (return_statement (member_expression object: (this) property: (property_identifier))))) (method_definition name: (property_identifier) parameters: (formal_parameters) body: (statement_block (return_statement (member_expression object: (this) property: (property_identifier)))))))))
is_b ResponsibilityProvenance { owner_id: "object:10", owner_label: "object (line 1)", dependencies: [Field { identity: "object:10:field:ready", label: "ready" }] }
is_a ResponsibilityProvenance { owner_id: "object:10", owner_label: "object (line 1)", dependencies: [Field { identity: "object:10:field:ready", label: "ready" }] }

## kt_expression_call
AST (source_file (class_declaration name: (identifier) (class_body (function_declaration name: (identifier) (function_value_parameters) (function_body (call_expression (identifier) (value_arguments)))) (function_declaration name: (identifier) (function_value_parameters) (function_body (identifier))))))
helper ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [] }
is_a ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [Callee { identity: "function:34", label: "helper" }] }

## py_ann_receiver
AST (module (class_definition name: (identifier) body: (block (function_definition name: (identifier) parameters: (parameters (typed_parameter (identifier) type: (type (identifier)))) body: (block (return_statement (attribute object: (identifier) attribute: (identifier))))))))
is_a ResponsibilityProvenance { owner_id: "class_definition:0", owner_label: "A (line 1)", dependencies: [] }

## rust_scoped_block
AST (source_file (function_item name: (identifier) parameters: (parameters) body: (block)) (function_item name: (identifier) parameters: (parameters) body: (block (expression_statement (block (function_item name: (identifier) parameters: (parameters) body: (block)))) (expression_statement (call_expression function: (identifier) arguments: (arguments))))))
is_a ResponsibilityProvenance { owner_id: "source_file:0", owner_label: "file", dependencies: [Callee { identity: "function:29", label: "helper" }] }
helper ResponsibilityProvenance { owner_id: "function_item:15", owner_label: "is_a (line 1)", dependencies: [] }
helper ResponsibilityProvenance { owner_id: "source_file:0", owner_label: "file", dependencies: [] }

## rust_shadow_self
AST (source_file (struct_item name: (type_identifier)) (impl_item type: (type_identifier) body: (declaration_list (function_item name: (identifier) parameters: (parameters (self_parameter (self))) body: (block (let_declaration pattern: (identifier) value: (identifier)) (expression_statement (call_expression function: (scoped_identifier path: (identifier) name: (identifier)) arguments: (arguments))))) (function_item name: (identifier) parameters: (parameters) body: (block)))))
helper ResponsibilityProvenance { owner_id: "impl_item:10", owner_label: "A (line 1)", dependencies: [] }
is_a ResponsibilityProvenance { owner_id: "impl_item:10", owner_label: "A (line 1)", dependencies: [] }

## js_import_namespace
AST (program (import_statement (import_clause (namespace_import (identifier))) source: (string (string_fragment))) (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block)) (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block (expression_statement (call_expression function: (identifier) arguments: (arguments))))))
is_a ResponsibilityProvenance { owner_id: "program:0", owner_label: "file", dependencies: [] }
helper ResponsibilityProvenance { owner_id: "program:0", owner_label: "file", dependencies: [] }

## go_receiver_alias
AST (source_file (package_clause (package_identifier)) (type_declaration (type_spec name: (type_identifier) type: (struct_type (field_declaration_list (field_declaration name: (field_identifier) type: (type_identifier)))))) (type_declaration (type_alias name: (type_identifier) type: (type_identifier))) (method_declaration receiver: (parameter_list (parameter_declaration name: (identifier) type: (type_identifier))) name: (field_identifier) parameters: (parameter_list) body: (block (statement_list (assignment_statement left: (expression_list (identifier)) right: (expression_list (selector_expression operand: (identifier) field: (field_identifier))))))))

## php_named_arg
AST (program (php_tag) (class_declaration name: (name) body: (declaration_list (method_declaration name: (name) parameters: (formal_parameters) body: (compound_statement (expression_statement (member_call_expression object: (variable_name (name)) name: (name) arguments: (arguments))))) (method_declaration name: (name) parameters: (formal_parameters) body: (compound_statement)))))
helper ResponsibilityProvenance { owner_id: "class_declaration:6", owner_label: "A (line 1)", dependencies: [] }
is_a ResponsibilityProvenance { owner_id: "class_declaration:6", owner_label: "A (line 1)", dependencies: [Callee { identity: "function:53", label: "helper" }] }


```

## Direct tuple fields and generic receiver probes

### rust tuple

`struct A(bool); impl A { fn is_a(&self) -> bool { self.0 } }`

```text
(source_file (struct_item name: (type_identifier) body: (ordered_field_declaration_list type: (primitive_type))) (impl_item type: (type_identifier) body: (declaration_list (function_item name: (identifier) parameters: (parameters (self_parameter (self))) return_type: (primitive_type) body: (block (field_expression value: (self) field: (integer_literal)))))))
```
### go generic

`package p; type A[T any] struct { ready bool }; func (a *A[T]) is_a() bool { return a.ready }`

```text
(source_file (package_clause (package_identifier)) (type_declaration (type_spec name: (type_identifier) type_parameters: (type_parameter_list (type_parameter_declaration name: (identifier) type: (type_constraint (type_identifier)))) type: (struct_type (field_declaration_list (field_declaration name: (field_identifier) type: (type_identifier)))))) (method_declaration receiver: (parameter_list (parameter_declaration name: (identifier) type: (pointer_type (generic_type type: (type_identifier) type_arguments: (type_arguments (type_elem (type_identifier))))))) name: (field_identifier) parameters: (parameter_list) result: (type_identifier) body: (block (statement_list (return_statement (expression_list (selector_expression operand: (identifier) field: (field_identifier))))))))
```

## Named argument identifiers are not field reads

### kotlin named argument

`class A(val ready: Boolean) {
 fun is_a() { external(ready = true) }
}
`

```text
(source_file (ERROR (identifier) (primary_constructor (class_parameters (class_parameter (identifier) (user_type (identifier))))) (identifier) (function_value_parameters) (function_modifier)))
```
### csharp named argument

`class A { bool ready; void is_a() { External(ready: true); } }`

```text
(compilation_unit (class_declaration name: (identifier) body: (declaration_list (field_declaration (variable_declaration type: (predefined_type) (variable_declarator name: (identifier)))) (method_declaration returns: (predefined_type) name: (identifier) parameters: (parameter_list) body: (block (expression_statement (invocation_expression function: (identifier) arguments: (argument_list (argument name: (identifier) (boolean_literal))))))))))
```

Corrected Kotlin probe (`external` is a keyword):

### kotlin named argument

`class A(val ready: Boolean) {
 fun is_a() { consume(ready = true) }
}
`

```text
(source_file (class_declaration name: (identifier) (primary_constructor (class_parameters (class_parameter (identifier) (user_type (identifier))))) (class_body (function_declaration name: (identifier) (function_value_parameters) (function_body (block (call_expression (identifier) (value_arguments (value_argument (identifier) (identifier))))))))))
```

## Anonymous object ownership probes

### kotlin anonymous object

`fun outer() {
 val a = object {
 fun is_a() = this.ready
 }
}
`

```text
(source_file (function_declaration name: (identifier) (function_value_parameters) (function_body (block (property_declaration (variable_declaration (identifier)) (object_literal (class_body (function_declaration name: (identifier) (function_value_parameters) (function_body (navigation_expression (this_expression) (identifier)))))))))))
```
### php anonymous class

`<?php function outer() { $a = new class { function is_a() { return $this->ready; } }; }`

```text
(program (php_tag) (function_definition name: (name) parameters: (formal_parameters) body: (compound_statement (expression_statement (assignment_expression left: (variable_name (name)) right: (object_creation_expression (anonymous_class body: (declaration_list (method_declaration name: (name) parameters: (formal_parameters) body: (compound_statement (return_statement (member_access_expression object: (variable_name (name)) name: (name)))))))))))))
```

## Generator and comprehension scope probes

### javascript generator

`class A { is_a() { function* nested() { yield this.ready; } } }`

```text
(program (class_declaration name: (identifier) body: (class_body member: (method_definition name: (property_identifier) parameters: (formal_parameters) body: (statement_block (generator_function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block (expression_statement (yield_expression (member_expression object: (this) property: (property_identifier)))))))))))
```
### python comprehensions

`class A:
 def is_a(self, others):
  return [self.ready for self in others]
`

```text
(module (class_definition name: (identifier) body: (block (function_definition name: (identifier) parameters: (parameters (identifier) (identifier)) body: (block (return_statement (list_comprehension body: (attribute object: (identifier) attribute: (identifier)) (for_in_clause left: (identifier) right: (identifier)))))))))
```

## External function declarations lack a local body

### rust external declaration

`extern "C" { fn helper(); } fn is_a() { unsafe { helper(); } }`

```text
(source_file (foreign_mod_item (extern_modifier (string_literal (string_content))) body: (declaration_list (function_signature_item name: (identifier) parameters: (parameters)))) (function_item name: (identifier) parameters: (parameters) body: (block (expression_statement (unsafe_block (block (expression_statement (call_expression function: (identifier) arguments: (arguments)))))))))
```

## Final review binding-scope probes before detector extension

```text
## rust_for
AST (source_file (function_item name: (identifier) parameters: (parameters) body: (block)) (function_item name: (identifier) parameters: (parameters) body: (block (expression_statement (for_expression pattern: (identifier) value: (array_expression (closure_expression parameters: (closure_parameters) body: (unit_expression))) body: (block (expression_statement (call_expression function: (identifier) arguments: (arguments)))))))))
is_a ResponsibilityProvenance { owner_id: "source_file:0", owner_label: "file", dependencies: [Callee { identity: "function:0", label: "helper" }] }
helper ResponsibilityProvenance { owner_id: "source_file:0", owner_label: "file", dependencies: [] }
## rust_match
AST (source_file (function_item name: (identifier) parameters: (parameters) body: (block)) (function_item name: (identifier) parameters: (parameters (parameter pattern: (identifier) type: (function_type parameters: (parameters)))) body: (block (expression_statement (match_expression value: (identifier) body: (match_block (match_arm pattern: (match_pattern (identifier)) value: (call_expression function: (identifier) arguments: (arguments)))))))))
is_a ResponsibilityProvenance { owner_id: "source_file:0", owner_label: "file", dependencies: [Callee { identity: "function:0", label: "helper" }] }
helper ResponsibilityProvenance { owner_id: "source_file:0", owner_label: "file", dependencies: [] }
## rust_iflet
AST (source_file (function_item name: (identifier) parameters: (parameters) body: (block)) (function_item name: (identifier) parameters: (parameters (parameter pattern: (identifier) type: (generic_type type: (type_identifier) type_arguments: (type_arguments (function_type parameters: (parameters)))))) body: (block (expression_statement (if_expression condition: (let_condition pattern: (tuple_struct_pattern type: (identifier) (identifier)) value: (identifier)) consequence: (block (expression_statement (call_expression function: (identifier) arguments: (arguments)))))))))
is_a ResponsibilityProvenance { owner_id: "source_file:0", owner_label: "file", dependencies: [Callee { identity: "function:0", label: "helper" }] }
helper ResponsibilityProvenance { owner_id: "source_file:0", owner_label: "file", dependencies: [] }
## java_catch
AST (program (class_declaration name: (identifier) body: (class_body (field_declaration type: (boolean_type) declarator: (variable_declarator name: (identifier))) (method_declaration type: (void_type) name: (identifier) parameters: (formal_parameters) body: (block (try_statement body: (block) (catch_clause (catch_formal_parameter (catch_type (type_identifier)) name: (identifier)) body: (block (expression_statement (method_invocation name: (identifier) arguments: (argument_list (identifier))))))))))))
is_a ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [Field { identity: "class_declaration:0:field:ready", label: "ready" }] }
## kt_for
AST (source_file (class_declaration name: (identifier) (class_body (property_declaration (variable_declaration (identifier)) (identifier)) (function_declaration name: (identifier) (function_value_parameters (parameter (identifier) (user_type (identifier) (type_arguments (type_projection (user_type (identifier))))))) (function_body (block (for_statement (variable_declaration (identifier)) (identifier) (block (call_expression (identifier) (value_arguments (value_argument (identifier))))))))))))
is_a ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [Field { identity: "class_declaration:0:field:ready", label: "ready" }] }
## python_except
AST (module (function_definition name: (identifier) parameters: (parameters) body: (block (pass_statement))) (function_definition name: (identifier) parameters: (parameters) body: (block (try_statement body: (block (pass_statement)) (except_clause value: (as_pattern (identifier) alias: (as_pattern_target (identifier))) (block (expression_statement (call function: (identifier) arguments: (argument_list)))))))))
is_a ResponsibilityProvenance { owner_id: "module:0", owner_label: "file", dependencies: [] }
helper ResponsibilityProvenance { owner_id: "module:0", owner_label: "file", dependencies: [] }
## csharp_foreach
AST (compilation_unit (class_declaration name: (identifier) body: (declaration_list (field_declaration (variable_declaration type: (predefined_type) (variable_declarator name: (identifier)))) (method_declaration returns: (predefined_type) name: (identifier) parameters: (parameter_list (parameter type: (array_type type: (predefined_type) rank: (array_rank_specifier)) name: (identifier))) body: (block (foreach_statement type: (predefined_type) left: (identifier) right: (identifier) body: (block (expression_statement (invocation_expression function: (identifier) arguments: (argument_list (argument (identifier))))))))))))
is_a ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [Field { identity: "class_declaration:0:field:ready", label: "ready" }] }
## cs_local_function
AST (compilation_unit (class_declaration name: (identifier) body: (declaration_list (method_declaration returns: (predefined_type) name: (identifier) parameters: (parameter_list) body: (block)) (method_declaration returns: (predefined_type) name: (identifier) parameters: (parameter_list) body: (block (local_function_statement type: (predefined_type) name: (identifier) parameters: (parameter_list) body: (block)) (expression_statement (invocation_expression function: (identifier) arguments: (argument_list))))))))
is_a ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [Callee { identity: "function:10", label: "helper" }] }
helper ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [] }
## js_generator
AST (program (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block)) (function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block (generator_function_declaration name: (identifier) parameters: (formal_parameters) body: (statement_block (expression_statement (yield_expression (number))))) (expression_statement (call_expression function: (identifier) arguments: (arguments))))))
is_a ResponsibilityProvenance { owner_id: "program:0", owner_label: "file", dependencies: [Callee { identity: "function:0", label: "helper" }] }
helper ResponsibilityProvenance { owner_id: "program:0", owner_label: "file", dependencies: [] }
## kt_destructure
AST (source_file (class_declaration name: (identifier) (class_body (property_declaration (variable_declaration (identifier)) (identifier)) (function_declaration name: (identifier) (function_value_parameters (parameter (identifier) (user_type (identifier) (type_arguments (type_projection (user_type (identifier))) (type_projection (user_type (identifier))))))) (function_body (block (property_declaration (multi_variable_declaration (variable_declaration (identifier)) (variable_declaration (identifier))) (identifier)) (call_expression (identifier) (value_arguments (value_argument (identifier))))))))))
is_a ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [Field { identity: "class_declaration:0:field:ready", label: "ready" }] }
## java_instanceof
AST (program (class_declaration name: (identifier) body: (class_body (field_declaration type: (type_identifier) declarator: (variable_declarator name: (identifier))) (method_declaration type: (void_type) name: (identifier) parameters: (formal_parameters (formal_parameter type: (type_identifier) name: (identifier))) body: (block (if_statement condition: (parenthesized_expression (instanceof_expression left: (identifier) right: (type_identifier) name: (identifier))) consequence: (block (expression_statement (method_invocation name: (identifier) arguments: (argument_list (identifier)))))))))))
is_a ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [Field { identity: "class_declaration:0:field:ready", label: "ready" }] }
## cs_pattern
AST (compilation_unit (class_declaration name: (identifier) body: (declaration_list (field_declaration (variable_declaration type: (predefined_type) (variable_declarator name: (identifier)))) (method_declaration returns: (predefined_type) name: (identifier) parameters: (parameter_list (parameter type: (predefined_type) name: (identifier))) body: (block (if_statement condition: (is_pattern_expression expression: (identifier) pattern: (declaration_pattern type: (predefined_type) name: (identifier))) consequence: (block (expression_statement (invocation_expression function: (identifier) arguments: (argument_list (argument (identifier))))))))))))
is_a ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [Field { identity: "class_declaration:0:field:ready", label: "ready" }] }
## js_private
AST (program (class_declaration name: (identifier) body: (class_body member: (field_definition property: (private_property_identifier) value: (true)) member: (method_definition name: (property_identifier) parameters: (formal_parameters) body: (statement_block (return_statement (member_expression object: (this) property: (private_property_identifier))))) member: (method_definition name: (property_identifier) parameters: (formal_parameters) body: (statement_block (return_statement (member_expression object: (this) property: (private_property_identifier))))))))
is_b ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [] }
is_a ResponsibilityProvenance { owner_id: "class_declaration:0", owner_label: "A (line 1)", dependencies: [] }

```
