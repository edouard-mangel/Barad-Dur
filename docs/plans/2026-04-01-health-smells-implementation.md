# Health Smells Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Long Methods and Code Biomarkers metrics to the Health category via per-function tree-sitter analysis.

**Architecture:** Extend the existing tree-sitter complexity pipeline with per-function extraction and nesting depth analysis. Two new metric modules in `src/metrics/health/` consume the enriched `FileComplexity` data. The `(snapshot) → value` functional pattern is preserved throughout.

**Tech Stack:** Rust, tree-sitter (existing grammars: rust, javascript, typescript, python, go, java, c_sharp)

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src/snapshot.rs` | Modify | Add `FunctionMetrics` struct, extend `FileComplexity` |
| `src/metrics/complexity/queries.rs` | Modify | Add function-node and nesting-node query constants per language |
| `src/metrics/complexity/lang_dispatch.rs` | Modify | Add `function_query()` and `nesting_query()` dispatch functions |
| `src/metrics/complexity/counters.rs` | Modify | Add `extract_functions()` and `compute_nesting_biomarkers()` |
| `src/metrics/complexity/treesitter.rs` | Modify | Wire new counters into `analyse()` |
| `src/metrics/health/long_methods.rs` | Create | Long Methods metric |
| `src/metrics/health/biomarkers.rs` | Create | Code Biomarkers metric |
| `src/metrics/health/mod.rs` | Modify | Wire in two new metrics |
| `src/config.rs` | Modify | Add threshold fields to `HealthThresholds` |
| `src/scorer/actions.rs` | Modify | Add action text and tab routing for new metrics |
| `src/renderer/html/js_shared.rs` | Modify | Add Health methodology section |

---

### Task 1: Extend Data Model

**Files:**
- Modify: `src/snapshot.rs:84-120` (add `FunctionMetrics`, extend `FileComplexity`)

- [ ] **Step 1: Add `FunctionMetrics` struct**

In `src/snapshot.rs`, add before `FileComplexity`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionMetrics {
    pub name: String,
    pub loc: usize,
    pub cyclomatic_complexity: u32,
    pub max_nesting_depth: u32,
}
```

- [ ] **Step 2: Extend `FileComplexity` with new fields**

Add three new fields to the `FileComplexity` struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileComplexity {
    pub total_lines: usize,
    pub loc: usize,
    pub cyclomatic_complexity: u32,
    pub public_methods: u32,
    pub properties: u32,
    pub functions: Vec<FunctionMetrics>,
    pub max_nesting_depth: u32,
    pub nesting_variance: f64,
}
```

- [ ] **Step 3: Fix all existing `FileComplexity` literal constructions**

Search for all places that construct `FileComplexity { ... }` and add the new fields with defaults. These are in:
- `src/metrics/complexity/treesitter.rs:61` — the main analysis path
- `src/metrics/complexity/fallback.rs` — the fallback path
- All test files that construct `FileComplexity` (grep for `FileComplexity {`)

Add to each: `functions: Vec::new(), max_nesting_depth: 0, nesting_variance: 0.0`

- [ ] **Step 4: Verify compilation**

Run: `cargo check`
Expected: compiles with no errors

- [ ] **Step 5: Run existing tests**

Run: `cargo test`
Expected: all existing tests pass (new fields default to zero, no behavior change)

- [ ] **Step 6: Commit**

```bash
git add src/snapshot.rs src/metrics/complexity/treesitter.rs src/metrics/complexity/fallback.rs
# also add any test files that needed updating
git commit -m "feat(snapshot): add FunctionMetrics and nesting biomarker fields to FileComplexity"
```

---

### Task 2: Add Tree-Sitter Queries for Function Nodes

**Files:**
- Modify: `src/metrics/complexity/queries.rs`
- Modify: `src/metrics/complexity/lang_dispatch.rs`

- [ ] **Step 1: Add function-node queries to `queries.rs`**

Append to each language section:

```rust
// ── Rust ──
pub const RUST_FUNCTIONS: &str = r#"(function_item name: (identifier) @name) @func"#;

pub const RUST_NESTING: &str = r#"[
  (if_expression)
  (for_expression)
  (while_expression)
  (loop_expression)
  (match_expression)
] @nest"#;

// ── JavaScript ──
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

// ── Python ──
pub const PYTHON_FUNCTIONS: &str = r#"(function_definition name: (identifier) @name) @func"#;

pub const PYTHON_NESTING: &str = r#"[
  (if_statement)
  (for_statement)
  (while_statement)
  (with_statement)
] @nest"#;

// ── Go ──
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

// ── Java ──
pub const JAVA_FUNCTIONS: &str = r#"(method_declaration name: (identifier) @name) @func"#;

pub const JAVA_NESTING: &str = r#"[
  (if_statement)
  (for_statement)
  (enhanced_for_statement)
  (while_statement)
  (do_statement)
  (switch_expression)
] @nest"#;

// ── C# ──
pub const CSHARP_FUNCTIONS: &str = r#"(method_declaration name: (identifier) @name) @func"#;

pub const CSHARP_NESTING: &str = r#"[
  (if_statement)
  (for_statement)
  (for_each_statement)
  (while_statement)
  (do_statement)
  (switch_statement)
] @nest"#;
```

- [ ] **Step 2: Add dispatch functions in `lang_dispatch.rs`**

Add two new dispatch functions:

```rust
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
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: compiles (new queries/dispatchers not yet called)

- [ ] **Step 4: Commit**

```bash
git add src/metrics/complexity/queries.rs src/metrics/complexity/lang_dispatch.rs
git commit -m "feat(complexity): add tree-sitter queries for function nodes and nesting"
```

---

### Task 3: Implement Per-Function Extraction

**Files:**
- Modify: `src/metrics/complexity/counters.rs`
- Modify: `src/metrics/complexity/treesitter.rs`

- [ ] **Step 1: Write failing test for Rust function extraction**

Add to `src/metrics/complexity/treesitter.rs` tests:

```rust
#[test]
fn rust_extracts_function_metrics() {
    let content = "fn short() { 1 }\nfn long() {\n    if x {\n        if y {\n            for z in v {\n                match a {\n                    _ => {}\n                }\n            }\n        }\n    }\n}\n";
    let result = analyse(content, Language::Rust, "rs").unwrap();
    assert_eq!(result.functions.len(), 2);
    let short = result.functions.iter().find(|f| f.name == "short").unwrap();
    assert_eq!(short.loc, 1);
    let long = result.functions.iter().find(|f| f.name == "long").unwrap();
    assert!(long.loc > 5);
    assert!(long.cyclomatic_complexity >= 3);
    assert!(long.max_nesting_depth >= 3);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib -- treesitter::tests::rust_extracts_function_metrics -v`
Expected: FAIL (functions is empty)

- [ ] **Step 3: Implement `extract_functions` in `counters.rs`**

Add to `src/metrics/complexity/counters.rs`:

```rust
use crate::snapshot::FunctionMetrics;
use super::lang_dispatch::{function_query, nesting_query};

pub(super) fn extract_functions(
    tree: &tree_sitter::Tree,
    content: &str,
    source: &[u8],
    grammar: &tree_sitter::Language,
    lang: Language,
    ext: &str,
) -> Vec<FunctionMetrics> {
    let query_src = match function_query(lang, ext) {
        Some(q) => q,
        None => return Vec::new(),
    };
    let query = match tree_sitter::Query::new(grammar, query_src) {
        Ok(q) => q,
        Err(_) => return Vec::new(),
    };

    let func_idx = query.capture_index_for_name("func").unwrap_or(0);
    let name_idx = query.capture_index_for_name("name").unwrap_or(1);

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut stream = cursor.matches(&query, tree.root_node(), source);
    let mut functions = Vec::new();

    while let Some(m) = stream.next() {
        let func_node = m.captures.iter().find(|c| c.index == func_idx);
        let name_node = m.captures.iter().find(|c| c.index == name_idx);

        let (func_node, name_node) = match (func_node, name_node) {
            (Some(f), Some(n)) => (f.node, n.node),
            _ => continue,
        };

        let name = std::str::from_utf8(&source[name_node.byte_range()])
            .unwrap_or("?")
            .to_string();

        let start_line = func_node.start_position().row;
        let end_line = func_node.end_position().row;
        let func_lines: Vec<&str> = content.lines().skip(start_line).take(end_line - start_line + 1).collect();
        let loc = func_lines.iter().filter(|l| !l.trim().is_empty()).count();

        let cc = count_cc_in_subtree(func_node, grammar, source, lang, ext);
        let depth = max_nesting_in_subtree(func_node, grammar, source, lang, ext);

        functions.push(FunctionMetrics {
            name,
            loc,
            cyclomatic_complexity: cc,
            max_nesting_depth: depth,
        });
    }

    functions
}

fn count_cc_in_subtree(
    node: tree_sitter::Node,
    grammar: &tree_sitter::Language,
    source: &[u8],
    lang: Language,
    ext: &str,
) -> u32 {
    let (stmt_query, op_query) = super::lang_dispatch::complexity_queries(lang, ext);
    let count_in_range = |q_src: &str| -> u32 {
        let query = match tree_sitter::Query::new(grammar, q_src) {
            Ok(q) => q,
            Err(_) => return 0,
        };
        let mut cursor = tree_sitter::QueryCursor::new();
        cursor.set_byte_range(node.byte_range());
        let mut stream = cursor.matches(&query, node, source);
        let mut count = 0u32;
        while stream.next().is_some() {
            count += 1;
        }
        count
    };
    let stmts = count_in_range(stmt_query);
    let ops = op_query.map(count_in_range).unwrap_or(0);
    stmts + ops
}

fn max_nesting_in_subtree(
    node: tree_sitter::Node,
    grammar: &tree_sitter::Language,
    source: &[u8],
    lang: Language,
    ext: &str,
) -> u32 {
    let query_src = match nesting_query(lang, ext) {
        Some(q) => q,
        None => return 0,
    };
    let query = match tree_sitter::Query::new(grammar, query_src) {
        Ok(q) => q,
        Err(_) => return 0,
    };
    let mut cursor = tree_sitter::QueryCursor::new();
    cursor.set_byte_range(node.byte_range());
    let mut stream = cursor.matches(&query, node, source);
    let mut max_depth = 0u32;

    while let Some(m) = stream.next() {
        for cap in m.captures.iter() {
            let depth = nesting_depth_of(cap.node, node);
            if depth > max_depth {
                max_depth = depth;
            }
        }
    }
    max_depth
}

/// Count how many nesting-relevant ancestors `child` has up to (but not including) `root`.
fn nesting_depth_of(child: tree_sitter::Node, root: tree_sitter::Node) -> u32 {
    let mut depth = 0u32;
    let mut current = child.parent();
    while let Some(node) = current {
        if node.id() == root.id() {
            break;
        }
        let kind = node.kind();
        if is_nesting_kind(kind) {
            depth += 1;
        }
        current = node.parent();
    }
    depth
}

fn is_nesting_kind(kind: &str) -> bool {
    matches!(
        kind,
        "if_expression" | "if_statement"
            | "for_expression" | "for_statement" | "for_in_statement"
            | "enhanced_for_statement" | "for_each_statement"
            | "while_expression" | "while_statement"
            | "loop_expression" | "do_statement"
            | "match_expression" | "switch_statement" | "switch_expression"
            | "expression_switch_statement" | "type_switch_statement"
            | "with_statement"
    )
}
```

- [ ] **Step 4: Wire `extract_functions` into `treesitter::analyse()`**

In `src/metrics/complexity/treesitter.rs`, update the `analyse` function to call the new extractor:

```rust
pub fn analyse(content: &str, lang: Language, ext: &str) -> Option<FileComplexity> {
    let grammar = grammar_for(lang, ext)?;
    let tree = parse(content, &grammar)?;

    let total_lines = content.lines().count();
    let loc = count_loc(content, &tree, &grammar, lang, ext);
    let cyclomatic_complexity = count_complexity(&tree, content.as_bytes(), &grammar, lang, ext);
    let public_methods = count_public_methods(&tree, content.as_bytes(), &grammar, lang, ext);
    let properties = count_properties(&tree, content.as_bytes(), &grammar, lang, ext);
    let functions = extract_functions(&tree, content, content.as_bytes(), &grammar, lang, ext);

    Some(FileComplexity {
        total_lines,
        loc,
        cyclomatic_complexity,
        public_methods,
        properties,
        functions,
        max_nesting_depth: 0,   // filled in Task 4
        nesting_variance: 0.0,  // filled in Task 4
    })
}
```

Add to the imports at the top of `treesitter.rs`:

```rust
use super::counters::{extract_functions, compute_nesting_biomarkers};
```

Note: both `extract_functions` and `compute_nesting_biomarkers` must be `pub(super)` in `counters.rs` (they already are in the code above).

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib -- treesitter::tests::rust_extracts_function_metrics -v`
Expected: PASS

- [ ] **Step 6: Add tests for other languages**

Add to `src/metrics/complexity/treesitter.rs` tests:

```rust
#[test]
fn js_extracts_function_metrics() {
    let content = "function short() { return 1; }\nfunction long() {\n    if (x) {\n        for (let i = 0; i < 10; i++) {\n            console.log(i);\n        }\n    }\n}\n";
    let result = analyse(content, Language::JsTs, "js").unwrap();
    assert!(result.functions.len() >= 2, "got {} functions", result.functions.len());
    let long = result.functions.iter().find(|f| f.name == "long").unwrap();
    assert!(long.cyclomatic_complexity >= 2);
}

#[test]
fn python_extracts_function_metrics() {
    let content = "def short():\n    return 1\ndef long():\n    if x:\n        for i in v:\n            pass\n";
    let result = analyse(content, Language::Python, "py").unwrap();
    assert!(result.functions.len() >= 2, "got {} functions", result.functions.len());
}

#[test]
fn go_extracts_function_metrics() {
    let content = "package main\nfunc Short() int { return 1 }\nfunc Long() {\n    if x {\n        for i := range v {\n            _ = i\n        }\n    }\n}\n";
    let result = analyse(content, Language::Go, "go").unwrap();
    assert!(result.functions.len() >= 2, "got {} functions", result.functions.len());
}

#[test]
fn java_extracts_function_metrics() {
    let content = "class Foo {\n    void shortMethod() { return; }\n    void longMethod() {\n        if (x) {\n            for (int i = 0; i < 10; i++) {\n                System.out.println(i);\n            }\n        }\n    }\n}\n";
    let result = analyse(content, Language::Java, "java").unwrap();
    assert!(result.functions.len() >= 2, "got {} functions", result.functions.len());
}

#[test]
fn csharp_extracts_function_metrics() {
    let content = "class Foo {\n    void ShortMethod() { return; }\n    void LongMethod() {\n        if (x) {\n            for (int i = 0; i < 10; i++) {\n                Console.WriteLine(i);\n            }\n        }\n    }\n}\n";
    let result = analyse(content, Language::CSharp, "cs").unwrap();
    assert!(result.functions.len() >= 2, "got {} functions", result.functions.len());
}

#[test]
fn generic_has_no_functions() {
    let result = analyse("hello world", Language::Rust, "rs").unwrap();
    assert!(result.functions.is_empty());
}
```

- [ ] **Step 7: Run all tests**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 8: Commit**

```bash
git add src/metrics/complexity/counters.rs src/metrics/complexity/treesitter.rs
git commit -m "feat(complexity): extract per-function metrics via tree-sitter"
```

---

### Task 4: Implement File-Level Nesting Biomarkers

**Files:**
- Modify: `src/metrics/complexity/counters.rs`
- Modify: `src/metrics/complexity/treesitter.rs`

- [ ] **Step 1: Write failing test**

Add to `src/metrics/complexity/treesitter.rs` tests:

```rust
#[test]
fn rust_nesting_biomarkers() {
    let content = "fn deep() {\n    if x {\n        for i in v {\n            match a {\n                _ => {\n                    if y {\n                        loop {\n                        }\n                    }\n                }\n            }\n        }\n    }\n}\nfn shallow() { let x = 1; }\n";
    let result = analyse(content, Language::Rust, "rs").unwrap();
    assert!(result.max_nesting_depth >= 5, "expected depth >= 5, got {}", result.max_nesting_depth);
    assert!(result.nesting_variance > 0.0, "expected non-zero variance");
}

#[test]
fn flat_file_has_zero_nesting() {
    let content = "fn a() {}\nfn b() {}\nfn c() {}\n";
    let result = analyse(content, Language::Rust, "rs").unwrap();
    assert_eq!(result.max_nesting_depth, 0);
    assert!((result.nesting_variance - 0.0).abs() < f64::EPSILON);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib -- treesitter::tests::rust_nesting_biomarkers -v`
Expected: FAIL (max_nesting_depth is 0)

- [ ] **Step 3: Implement `compute_nesting_biomarkers` in `counters.rs`**

Add to `src/metrics/complexity/counters.rs`:

```rust
pub(super) fn compute_nesting_biomarkers(
    tree: &tree_sitter::Tree,
    source: &[u8],
    grammar: &tree_sitter::Language,
    lang: Language,
    ext: &str,
    total_lines: usize,
) -> (u32, f64) {
    let query_src = match nesting_query(lang, ext) {
        Some(q) => q,
        None => return (0, 0.0),
    };
    let query = match tree_sitter::Query::new(grammar, query_src) {
        Ok(q) => q,
        Err(_) => return (0, 0.0),
    };

    // For each nesting node, compute its depth by walking up to the root
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut stream = cursor.matches(&query, tree.root_node(), source);
    let mut max_depth = 0u32;
    let mut line_depths: Vec<u32> = vec![0; total_lines];

    while let Some(m) = stream.next() {
        for cap in m.captures.iter() {
            let node = cap.node;
            let depth = nesting_depth_of(node, tree.root_node()) + 1; // +1 because the node itself is a nesting level
            if depth > max_depth {
                max_depth = depth;
            }
            // Mark all lines within this node with their max depth
            let start = node.start_position().row;
            let end = node.end_position().row;
            for line in start..=end.min(total_lines.saturating_sub(1)) {
                if depth > line_depths[line] {
                    line_depths[line] = depth;
                }
            }
        }
    }

    let variance = if total_lines == 0 {
        0.0
    } else {
        let mean = line_depths.iter().sum::<u32>() as f64 / total_lines as f64;
        let sq_diff_sum: f64 = line_depths.iter().map(|&d| {
            let diff = d as f64 - mean;
            diff * diff
        }).sum();
        (sq_diff_sum / total_lines as f64).sqrt()
    };

    (max_depth, variance)
}
```

- [ ] **Step 4: Wire into `treesitter::analyse()`**

Update `analyse()` in `treesitter.rs`:

```rust
    let functions = extract_functions(&tree, content, content.as_bytes(), &grammar, lang, ext);
    let (max_nesting_depth, nesting_variance) = compute_nesting_biomarkers(&tree, content.as_bytes(), &grammar, lang, ext, total_lines);

    Some(FileComplexity {
        total_lines,
        loc,
        cyclomatic_complexity,
        public_methods,
        properties,
        functions,
        max_nesting_depth,
        nesting_variance,
    })
```

Add to imports: `use super::counters::compute_nesting_biomarkers;`

- [ ] **Step 5: Run tests**

Run: `cargo test --lib -- treesitter::tests::rust_nesting -v`
Expected: PASS

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 7: Commit**

```bash
git add src/metrics/complexity/counters.rs src/metrics/complexity/treesitter.rs
git commit -m "feat(complexity): compute file-level nesting biomarkers (max depth + variance)"
```

---

### Task 5: Long Methods Health Metric

**Files:**
- Create: `src/metrics/health/long_methods.rs`
- Modify: `src/metrics/health/mod.rs`

- [ ] **Step 1: Write the metric tests first**

Create `src/metrics/health/long_methods.rs`:

```rust
use crate::metrics::{MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

use super::god_objects::is_source_file;

pub(super) fn long_methods(snapshot: &RepoSnapshot) -> MetricValue {
    todo!()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::snapshot::*;

    fn make_snapshot_with_functions(files: Vec<(&str, Vec<FunctionMetrics>)>) -> RepoSnapshot {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        for (path, functions) in files {
            snapshot.file_metrics.insert(
                PathBuf::from(path),
                FileComplexity {
                    total_lines: 100,
                    loc: 80,
                    cyclomatic_complexity: 5,
                    public_methods: 2,
                    properties: 0,
                    functions,
                    max_nesting_depth: 0,
                    nesting_variance: 0.0,
                },
            );
        }
        snapshot
    }

    fn func(name: &str, loc: usize, cc: u32) -> FunctionMetrics {
        FunctionMetrics {
            name: name.to_string(),
            loc,
            cyclomatic_complexity: cc,
            max_nesting_depth: 0,
        }
    }

    #[test]
    fn scores_100_when_no_long_methods() {
        let snapshot = make_snapshot_with_functions(vec![
            ("a.rs", vec![func("short", 10, 3), func("tiny", 5, 1)]),
        ]);
        let result = long_methods(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn detects_long_method_by_loc() {
        // 1 long out of 20 = 5% → score 75
        let mut funcs: Vec<FunctionMetrics> = (0..19).map(|i| func(&format!("ok{}", i), 10, 3)).collect();
        funcs.push(func("monster", 50, 3));
        let snapshot = make_snapshot_with_functions(vec![("a.rs", funcs)]);
        let result = long_methods(&snapshot);
        assert_eq!(result.score, 75);
    }

    #[test]
    fn detects_long_method_by_cc() {
        // 1 complex out of 20 = 5% → score 75
        let mut funcs: Vec<FunctionMetrics> = (0..19).map(|i| func(&format!("ok{}", i), 10, 3)).collect();
        funcs.push(func("complex", 20, 12));
        let snapshot = make_snapshot_with_functions(vec![("a.rs", funcs)]);
        let result = long_methods(&snapshot);
        assert_eq!(result.score, 75);
    }

    #[test]
    fn scores_50_at_medium_pct() {
        // 3 long out of 20 = 15% → score 50
        let mut funcs: Vec<FunctionMetrics> = (0..17).map(|i| func(&format!("ok{}", i), 10, 3)).collect();
        for i in 0..3 {
            funcs.push(func(&format!("long{}", i), 50, 3));
        }
        let snapshot = make_snapshot_with_functions(vec![("a.rs", funcs)]);
        let result = long_methods(&snapshot);
        assert_eq!(result.score, 50);
    }

    #[test]
    fn scores_25_at_high_pct() {
        // 4 long out of 20 = 20% > 15% → score 25
        let mut funcs: Vec<FunctionMetrics> = (0..16).map(|i| func(&format!("ok{}", i), 10, 3)).collect();
        for i in 0..4 {
            funcs.push(func(&format!("long{}", i), 50, 3));
        }
        let snapshot = make_snapshot_with_functions(vec![("a.rs", funcs)]);
        let result = long_methods(&snapshot);
        assert_eq!(result.score, 25);
    }

    #[test]
    fn empty_repo_scores_100() {
        let snapshot = make_snapshot_with_functions(vec![]);
        let result = long_methods(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn no_functions_scores_100() {
        let snapshot = make_snapshot_with_functions(vec![("a.rs", vec![])]);
        let result = long_methods(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn boundary_loc_40_not_flagged() {
        let snapshot = make_snapshot_with_functions(vec![
            ("a.rs", vec![func("boundary", 40, 3)]),
        ]);
        let result = long_methods(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn boundary_cc_10_not_flagged() {
        let snapshot = make_snapshot_with_functions(vec![
            ("a.rs", vec![func("boundary", 20, 10)]),
        ]);
        let result = long_methods(&snapshot);
        assert_eq!(result.score, 100);
    }
}
```

- [ ] **Step 2: Register the module in `mod.rs`**

Add `mod long_methods;` to `src/metrics/health/mod.rs` (don't wire into `compute_health` yet).

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --lib -- health::long_methods -v`
Expected: FAIL with `todo!()`

- [ ] **Step 4: Implement `long_methods`**

Replace the `todo!()` body:

```rust
pub(super) fn long_methods(snapshot: &RepoSnapshot) -> MetricValue {
    let all_functions: Vec<(&str, &str, &crate::snapshot::FunctionMetrics)> = snapshot
        .file_metrics
        .iter()
        .filter(|(p, _)| is_source_file(p))
        .flat_map(|(p, m)| {
            m.functions.iter().map(move |f| {
                (p.to_str().unwrap_or("?"), f.name.as_str(), f)
            })
        })
        .collect();

    let total = all_functions.len();
    if total == 0 {
        return MetricValue {
            name: "Long methods".to_string(),
            description: "No functions found".to_string(),
            raw_value: RawValue::List(Vec::new()),
            score: 100,
        };
    }

    let flagged: Vec<String> = all_functions
        .iter()
        .filter(|(_, _, f)| f.loc > 40 || f.cyclomatic_complexity > 10)
        .map(|(path, _, f)| {
            format!("{} ({}) \u{2014} {} LOC, CC={}", f.name, path, f.loc, f.cyclomatic_complexity)
        })
        .collect();

    let count = flagged.len();
    let pct = count as f64 / total as f64 * 100.0;

    let score = if count == 0 {
        100
    } else if pct <= 5.0 {
        75
    } else if pct <= 15.0 {
        50
    } else {
        25
    };

    MetricValue {
        name: "Long methods".to_string(),
        description: format!(
            "{}/{} functions oversized ({:.1}%)",
            count, total, pct
        ),
        raw_value: RawValue::List(flagged),
        score,
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib -- health::long_methods -v`
Expected: all PASS

- [ ] **Step 6: Commit**

```bash
git add src/metrics/health/long_methods.rs src/metrics/health/mod.rs
git commit -m "feat(health): add Long Methods metric"
```

---

### Task 6: Code Biomarkers Health Metric

**Files:**
- Create: `src/metrics/health/biomarkers.rs`
- Modify: `src/metrics/health/mod.rs`

- [ ] **Step 1: Write the metric tests first**

Create `src/metrics/health/biomarkers.rs`:

```rust
use crate::metrics::{MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

use super::god_objects::is_source_file;

pub(super) fn biomarkers(snapshot: &RepoSnapshot) -> MetricValue {
    todo!()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::snapshot::*;

    fn make_snapshot_with_nesting(files: Vec<(&str, u32, f64)>) -> RepoSnapshot {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        for (path, depth, variance) in files {
            snapshot.file_metrics.insert(
                PathBuf::from(path),
                FileComplexity {
                    total_lines: 100,
                    loc: 80,
                    cyclomatic_complexity: 5,
                    public_methods: 2,
                    properties: 0,
                    functions: Vec::new(),
                    max_nesting_depth: depth,
                    nesting_variance: variance,
                },
            );
        }
        snapshot
    }

    #[test]
    fn scores_100_when_no_deep_nesting() {
        let snapshot = make_snapshot_with_nesting(vec![
            ("a.rs", 2, 0.5),
            ("b.rs", 3, 1.0),
        ]);
        let result = biomarkers(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn flags_deep_nesting() {
        // 1/100 = 1% ≤ 3% → score 75
        let mut files: Vec<(&str, u32, f64)> = (0..99)
            .map(|i| {
                // Leak a string so we get &str with 'static-like lifetime for the test
                let name = Box::leak(format!("ok{}.rs", i).into_boxed_str());
                (name as &str, 2, 0.5)
            })
            .collect();
        files.push(("deep.rs", 6, 0.5));
        let snapshot = make_snapshot_with_nesting(files);
        let result = biomarkers(&snapshot);
        assert_eq!(result.score, 75);
    }

    #[test]
    fn flags_high_variance() {
        // 1/100 = 1% ≤ 3% → score 75
        let mut files: Vec<(&str, u32, f64)> = (0..99)
            .map(|i| {
                let name = Box::leak(format!("ok{}.rs", i).into_boxed_str());
                (name as &str, 2, 0.5)
            })
            .collect();
        files.push(("erratic.rs", 3, 2.5));
        let snapshot = make_snapshot_with_nesting(files);
        let result = biomarkers(&snapshot);
        assert_eq!(result.score, 75);
    }

    #[test]
    fn scores_50_at_medium_pct() {
        // 5/100 = 5% → score 50
        let mut files: Vec<(&str, u32, f64)> = (0..95)
            .map(|i| {
                let name = Box::leak(format!("ok{}.rs", i).into_boxed_str());
                (name as &str, 2, 0.5)
            })
            .collect();
        for i in 0..5 {
            let name = Box::leak(format!("deep{}.rs", i).into_boxed_str());
            files.push((name, 6, 0.5));
        }
        let snapshot = make_snapshot_with_nesting(files);
        let result = biomarkers(&snapshot);
        assert_eq!(result.score, 50);
    }

    #[test]
    fn scores_25_at_high_pct() {
        // 12/100 = 12% > 10% → score 25
        let mut files: Vec<(&str, u32, f64)> = (0..88)
            .map(|i| {
                let name = Box::leak(format!("ok{}.rs", i).into_boxed_str());
                (name as &str, 2, 0.5)
            })
            .collect();
        for i in 0..12 {
            let name = Box::leak(format!("deep{}.rs", i).into_boxed_str());
            files.push((name, 6, 0.5));
        }
        let snapshot = make_snapshot_with_nesting(files);
        let result = biomarkers(&snapshot);
        assert_eq!(result.score, 25);
    }

    #[test]
    fn empty_repo_scores_100() {
        let snapshot = make_snapshot_with_nesting(vec![]);
        let result = biomarkers(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn boundary_depth_4_not_flagged() {
        let snapshot = make_snapshot_with_nesting(vec![("a.rs", 4, 1.0)]);
        let result = biomarkers(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn boundary_variance_2_0_not_flagged() {
        let snapshot = make_snapshot_with_nesting(vec![("a.rs", 3, 2.0)]);
        let result = biomarkers(&snapshot);
        assert_eq!(result.score, 100);
    }
}
```

- [ ] **Step 2: Register the module in `mod.rs`**

Add `mod biomarkers;` to `src/metrics/health/mod.rs`.

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --lib -- health::biomarkers -v`
Expected: FAIL with `todo!()`

- [ ] **Step 4: Implement `biomarkers`**

Replace the `todo!()` body:

```rust
pub(super) fn biomarkers(snapshot: &RepoSnapshot) -> MetricValue {
    let source_files: Vec<(&std::path::PathBuf, &crate::snapshot::FileComplexity)> = snapshot
        .file_metrics
        .iter()
        .filter(|(p, _)| is_source_file(p))
        .collect();

    let total = source_files.len();
    if total == 0 {
        return MetricValue {
            name: "Code biomarkers".to_string(),
            description: "No source files found".to_string(),
            raw_value: RawValue::List(Vec::new()),
            score: 100,
        };
    }

    let flagged: Vec<String> = source_files
        .iter()
        .filter(|(_, m)| m.max_nesting_depth > 4 || m.nesting_variance > 2.0)
        .map(|(p, m)| {
            if m.max_nesting_depth > 4 {
                format!("{} \u{2014} nesting depth {}", p.display(), m.max_nesting_depth)
            } else {
                format!("{} \u{2014} nesting variance {:.1}", p.display(), m.nesting_variance)
            }
        })
        .collect();

    let count = flagged.len();
    let pct = count as f64 / total as f64 * 100.0;

    let score = if count == 0 {
        100
    } else if pct <= 3.0 {
        75
    } else if pct <= 10.0 {
        50
    } else {
        25
    };

    MetricValue {
        name: "Code biomarkers".to_string(),
        description: format!(
            "{}/{} source files with deep nesting or high variance ({:.1}%)",
            count, total, pct
        ),
        raw_value: RawValue::List(flagged),
        score,
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib -- health::biomarkers -v`
Expected: all PASS

- [ ] **Step 6: Commit**

```bash
git add src/metrics/health/biomarkers.rs src/metrics/health/mod.rs
git commit -m "feat(health): add Code Biomarkers metric"
```

---

### Task 7: Wire New Metrics Into Health Category

**Files:**
- Modify: `src/metrics/health/mod.rs`
- Modify: `src/scorer/actions.rs`
- Modify: `src/config.rs`

- [ ] **Step 1: Update `compute_health` to include new metrics**

Replace `src/metrics/health/mod.rs`:

```rust
mod biomarkers;
mod bus_factor;
mod complex_hotspots;
mod god_objects;
mod long_methods;

use crate::config::HealthThresholds;
use crate::metrics::CategoryResult;
use crate::snapshot::RepoSnapshot;

pub fn compute_health(snapshot: &RepoSnapshot, thresholds: &HealthThresholds) -> CategoryResult {
    let metrics = vec![
        bus_factor::bus_factor(snapshot, thresholds),
        god_objects::god_objects(snapshot),
        complex_hotspots::complex_hotspots(snapshot),
        long_methods::long_methods(snapshot),
        biomarkers::biomarkers(snapshot),
    ];

    CategoryResult {
        name: "Health".to_string(),
        score: 0,
        metrics,
    }
    .compute_score()
}
```

- [ ] **Step 2: Add action text and tab routing**

In `src/scorer/actions.rs`, add to `target_tab_for_metric`:

```rust
"Long methods" => (Some("hotspots"), Some("complexity")),
"Code biomarkers" => (Some("hotspots"), Some("complexity")),
```

Add to `suggest_action`:

```rust
"Long methods" => "Extract smaller functions from the longest methods to improve readability",
"Code biomarkers" => "Reduce nesting depth by applying early returns and guard clauses",
```

- [ ] **Step 3: Add threshold fields to `HealthThresholds`**

In `src/config.rs`, extend `HealthThresholds`:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct HealthThresholds {
    #[serde(default = "default_max_complexity")]
    pub max_complexity: u32,
    #[serde(default = "default_hotspot_top_n")]
    pub hotspot_top_n: usize,
    #[serde(default = "default_coupling_min_commits")]
    pub coupling_min_commits: usize,
    #[serde(default = "default_long_method_loc")]
    pub long_method_loc: usize,
    #[serde(default = "default_long_method_cc")]
    pub long_method_cc: u32,
    #[serde(default = "default_biomarker_max_depth")]
    pub biomarker_max_depth: u32,
    #[serde(default = "default_biomarker_max_variance")]
    pub biomarker_max_variance: f64,
}

fn default_long_method_loc() -> usize { 40 }
fn default_long_method_cc() -> u32 { 10 }
fn default_biomarker_max_depth() -> u32 { 4 }
fn default_biomarker_max_variance() -> f64 { 2.0 }
```

Note: The new metric functions currently use hardcoded thresholds (matching these defaults). Wiring the thresholds into the metric functions can be done as a follow-up if configurability is needed.

- [ ] **Step 4: Run full test suite**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add src/metrics/health/mod.rs src/scorer/actions.rs src/config.rs
git commit -m "feat(health): wire Long Methods and Code Biomarkers into Health category"
```

---

### Task 8: Health Methodology in HTML Report

**Files:**
- Modify: `src/renderer/html/js_shared.rs`

- [ ] **Step 1: Add methodology HTML content**

In `src/renderer/html/js_shared.rs`, locate the `buildCatCard` function. After the metrics list, add a collapsible methodology section for the Health category. Add a helper function:

```javascript
function buildHealthMethodology() {
  return `
    <details class="methodology" style="margin-top:1rem;border-top:1px solid #333;padding-top:0.5rem;">
      <summary style="cursor:pointer;font-weight:600;color:#888;">Methodology</summary>
      <div style="font-size:0.85rem;color:#aaa;margin-top:0.5rem;line-height:1.6;">
        <h4>Bus Factor</h4>
        <p><b>What:</b> Percentage of files where a single author owns &gt;50% of lines.</p>
        <p><b>Scoring:</b> &lt;10% → 100 | &lt;25% → 75 | &lt;50% → 50 | &gt;50% → 25</p>
        <p><b>Why:</b> Low bus factor means critical knowledge is concentrated in too few people.</p>

        <h4>God Objects</h4>
        <p><b>What:</b> Files with LOC &gt; 500, or LOC &gt; 300 with &gt;15 public methods.</p>
        <p><b>Scoring:</b> 0% → 100 | &le;2% → 75 | &le;8% → 50 | &gt;8% → 25</p>
        <p><b>Why:</b> Large classes with many responsibilities are hard to understand and change (Fowler: Large Class).</p>

        <h4>Complex Hotspots</h4>
        <p><b>What:</b> Files above the 75th percentile in both cyclomatic complexity and churn.</p>
        <p><b>Scoring:</b> 0 → 100 | 1-2 → 75 | 3-5 → 50 | &gt;5 → 25</p>
        <p><b>Why:</b> Code that is both complex and frequently changed is the highest-risk area for bugs (Tornhill).</p>

        <h4>Long Methods</h4>
        <p><b>What:</b> Functions with LOC &gt; 40 or cyclomatic complexity &gt; 10.</p>
        <p><b>Scoring:</b> 0% → 100 | &le;5% → 75 | &le;15% → 50 | &gt;15% → 25</p>
        <p><b>Why:</b> Long or complex functions are harder to test, understand, and maintain (Fowler: Long Method).</p>

        <h4>Code Biomarkers</h4>
        <p><b>What:</b> Files with nesting depth &gt; 4 or nesting variance &gt; 2.0.</p>
        <p><b>Scoring:</b> 0% → 100 | &le;3% → 75 | &le;10% → 50 | &gt;10% → 25</p>
        <p><b>Why:</b> Deeply nested code signals accumulated complexity. High variance indicates erratic structure (Tornhill: Code Biomarkers).</p>
      </div>
    </details>`;
}
```

Then in `buildCatCard`, after the metrics loop, add:

```javascript
if (cat.name === 'Health') {
  html += buildHealthMethodology();
}
```

- [ ] **Step 2: Run integration test to verify report generates**

Run: `cargo test --test integration_tests -- analyze_json_is_valid -v`
Expected: PASS

- [ ] **Step 3: Visually verify (optional)**

Run: `cargo run -- analyze . --html -o /tmp/test-report.html`
Open in browser and check the Health card has the Methodology section.

- [ ] **Step 4: Commit**

```bash
git add src/renderer/html/js_shared.rs
git commit -m "feat(html): add Health methodology section to report"
```

---

### Task 9: End-to-End Verification

**Files:** None (verification only)

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings

- [ ] **Step 3: Run fmt**

Run: `cargo fmt -- --check`
Expected: no formatting issues

- [ ] **Step 4: Self-analysis**

Run: `cargo run -- analyze . -v`
Expected: Health category now shows 5 metrics. Output includes "Long methods" and "Code biomarkers" lines.

- [ ] **Step 5: Verify JSON output**

Run: `cargo run -- analyze . --json --pretty | python3 -c "import json,sys; r=json.load(sys.stdin); health=[c for c in r['categories'] if c['name']=='Health'][0]; print(f'Health: {health[\"score\"]}, metrics: {len(health[\"metrics\"])}'); [print(f'  {m[\"name\"]}: {m[\"score\"]}') for m in health['metrics']]"`
Expected: 5 metrics listed under Health.

- [ ] **Step 6: Generate HTML report**

Run: `cargo run -- analyze . --html -o /tmp/health-report.html`
Expected: file created, Health card shows 5 metrics with methodology section.

- [ ] **Step 7: Final commit if any fmt changes**

```bash
cargo fmt
git add -A
git commit -m "style: rustfmt after health smells implementation"
```
