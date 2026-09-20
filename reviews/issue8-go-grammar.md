# Go grammar evidence

Path classification supplies all Go test evidence; names and imports below alone do not.

```text
Source:
package sample
import "testing"
func TestRender(t *testing.T) {}
func BenchmarkRender(b *testing.B) {}
func FuzzRender(f *testing.F) {}
func ExampleRender() {}
func renderHelper() {}

errors=false
(source_file (package_clause (package_identifier)) (import_declaration (import_spec path: (interpreted_string_literal (interpreted_string_literal_content)))) (function_declaration name: (identifier) parameters: (parameter_list (parameter_declaration name: (identifier) type: (pointer_type (qualified_type package: (package_identifier) name: (type_identifier))))) body: (block)) (function_declaration name: (identifier) parameters: (parameter_list (parameter_declaration name: (identifier) type: (pointer_type (qualified_type package: (package_identifier) name: (type_identifier))))) body: (block)) (function_declaration name: (identifier) parameters: (parameter_list (parameter_declaration name: (identifier) type: (pointer_type (qualified_type package: (package_identifier) name: (type_identifier))))) body: (block)) (function_declaration name: (identifier) parameters: (parameter_list) body: (block)) (function_declaration name: (identifier) parameters: (parameter_list) body: (block)))
```
