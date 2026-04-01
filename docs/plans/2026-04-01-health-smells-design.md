# Health Metrics: Long Methods + Code Biomarkers

**Date:** 2026-04-01
**Status:** Approved
**Scope:** Add two new Health metrics (Long Methods, Code Biomarkers) via per-function tree-sitter analysis

## Motivation

The Health category currently has three metrics (Bus Factor, God Objects, Complex Hotspots). All are file-level. Adding per-function analysis enables detection of two important code smells:

- **Long Methods** (Fowler) — functions that are too large or too complex to understand easily
- **Code Biomarkers** (Tornhill) — files with deep nesting or erratic structure, indicators of accumulated complexity

Both require tree-sitter AST analysis at the function level, which the parser does not currently perform.

## Design

### 1. Data Model

Extend `FileComplexity` in `src/snapshot.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionMetrics {
    pub name: String,
    pub loc: usize,
    pub cyclomatic_complexity: u32,
    pub max_nesting_depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileComplexity {
    // existing fields unchanged
    pub total_lines: usize,
    pub loc: usize,
    pub cyclomatic_complexity: u32,
    pub public_methods: u32,
    pub properties: u32,
    // new fields
    pub functions: Vec<FunctionMetrics>,
    pub max_nesting_depth: u32,
    pub nesting_variance: f64,
}
```

**Cache compatibility:** This is a breaking change to the serialized `snapshot.bin`. Per project convention (no serde compat shims before v1.0), old caches are discarded on first run.

### 2. Parser Enhancement

Enhance the existing tree-sitter pass in `src/metrics/complexity/` to extract per-function data and nesting biomarkers in a single AST walk per file.

#### Per-function extraction

For each function/method node in the AST:
- **name** — from the identifier child node
- **loc** — non-blank, non-comment lines within the function's line range
- **cyclomatic_complexity** — count decision nodes within the function's subtree
- **max_nesting_depth** — count nested block ancestors within the function

#### Function node types by language

| Language | Node types |
|----------|-----------|
| Rust | `function_item`, `impl_item > function_item` |
| JS/TS | `function_declaration`, `method_definition`, named `arrow_function` |
| Python | `function_definition` |
| Go | `function_declaration`, `method_declaration` |
| JVM (Java/Kotlin/C#) | `method_declaration` |

#### Nesting depth nodes

Common across languages: `if_statement`, `for_statement`, `while_statement`, `match_expression`/`switch_statement`, `loop_expression`, nested `block` nodes. Language-specific variants (e.g., Rust `match_expression`, Python `with_statement`) included per grammar.

#### File-level biomarkers

Computed during the same walk:
- **`max_nesting_depth`** — deepest block nesting encountered anywhere in the file
- **`nesting_variance`** — standard deviation of nesting levels across all AST-significant lines

### 3. New Health Metrics

Both follow the established pattern: pure function `(snapshot) -> MetricValue`.

#### Long Methods (`src/metrics/health/long_methods.rs`)

**Detection:** Flatten all `file_metrics[*].functions`, flag functions where:
- `loc > 40` OR `cyclomatic_complexity > 10`

**Scoring** (percentage of flagged functions out of total):
| Flagged % | Score |
|-----------|-------|
| 0% | 100 |
| <= 5% | 75 |
| <= 15% | 50 |
| > 15% | 25 |

**Raw value:** `RawValue::List` of top offenders formatted as `"function_name (file.rs) — 85 LOC, CC=12"`.

#### Code Biomarkers (`src/metrics/health/biomarkers.rs`)

**Detection:** Flag source files where:
- `max_nesting_depth > 4` (deeply nested code)
- OR `nesting_variance > 2.0` (erratic structure — a std deviation of 2.0 means lines regularly jump between e.g., depth 0 and depth 4+)

**Scoring** (percentage of flagged source files):
| Flagged % | Score |
|-----------|-------|
| 0% | 100 |
| <= 3% | 75 |
| <= 10% | 50 |
| > 10% | 25 |

**Raw value:** `RawValue::List` of flagged file paths with their worst biomarker value.

### 4. Integration Points

#### `compute_health` (`src/metrics/health/mod.rs`)
Add calls to `long_methods()` and `biomarkers()`. Health score becomes the plain average of all five metrics.

#### Actions (`src/scorer/actions.rs`)
Add entries in:
- `suggest_action("Long methods")` — e.g., "Extract smaller functions from the longest methods to improve readability"
- `suggest_action("Code biomarkers")` — e.g., "Reduce nesting depth by applying early returns and guard clauses"
- `target_tab_for_metric("Long methods")` — `("hotspots", "complexity")`
- `target_tab_for_metric("Code biomarkers")` — `("hotspots", "complexity")`

#### Thresholds (`src/config.rs`)
Add optional fields to `HealthThresholds` with defaults:
- `long_method_loc: u32 = 40`
- `long_method_cc: u32 = 10`
- `biomarker_max_depth: u32 = 4`
- `biomarker_max_variance: f64 = 2.0`

### 5. HTML Report — Methodology Section

Add a collapsible "Methodology" section to the Health category card in the HTML report. For each of the five Health metrics, document:

- **What it measures** — plain English description
- **How it's calculated** — detection logic and thresholds
- **How it's scored** — the threshold bands (score table)
- **Why it matters** — one sentence linking to the code smell concept and its source (Fowler/Tornhill)

Static content embedded in the JS template (`src/renderer/html/js_shared.rs` or a new `js_methodology.rs`).

### 6. Testing Strategy

- **Parser tests:** For each language, provide a small source file with known functions of varying LOC, CC, and nesting depth. Assert correct `FunctionMetrics` extraction and biomarker values.
- **Metric unit tests:** Construct `RepoSnapshot` with synthetic `file_metrics` containing `FunctionMetrics` data. Assert correct detection and scoring for edge cases (zero functions, all flagged, none flagged, boundary values).
- **Integration test:** Run barad-dur on its own repo, verify Health category now reports five metrics.
- **Mutation testing:** New code must achieve >= 80% kill rate per project convention.

## Out of Scope

- Change coupling smells (cross-boundary temporal coupling) — deferred to a future iteration
- Per-function data in the Hotspots tab visualization — may be added later but not part of this work
- Refactoring existing metrics (god objects, complex hotspots, bus factor) — kept as-is

## Files to Create or Modify

| File | Action |
|------|--------|
| `src/snapshot.rs` | Add `FunctionMetrics`, extend `FileComplexity` |
| `src/metrics/complexity/counters.rs` | Enhance tree-sitter pass for per-function extraction |
| `src/metrics/complexity/treesitter.rs` | Add/extend tree-sitter queries for function nodes and nesting |
| `src/metrics/health/long_methods.rs` | New metric |
| `src/metrics/health/biomarkers.rs` | New metric |
| `src/metrics/health/mod.rs` | Wire in two new metrics |
| `src/config.rs` | Add threshold fields to `HealthThresholds` |
| `src/scorer/actions.rs` | Add suggest_action + target_tab entries |
| `src/renderer/html/js_shared.rs` | Add methodology section |
| Tests for all of the above | New test files/modules |
