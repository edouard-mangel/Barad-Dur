# Pressman Coupling Detection — M1 Walking Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect Pressman's dangerous coupling rungs (content, common, control) as evidenced findings in Rust and TS/JS, and score them as three new severity-banded metrics in the Coupling category.

**Architecture:** AST detectors run in the collector's existing per-file tree-sitter pass and store `CouplingFinding`s in `RepoSnapshot`; the TS/JS barrel-bypass rule is a pure function over the already-resolved import graph, computed at metric time (it depends on config `component_depth`, and cached snapshots must stay config-independent). Three pure metrics read the findings; a severity cap post-processes the category score. Spec: `docs/superpowers/specs/2026-07-02-pressman-coupling-design.md`.

**Tech Stack:** Rust, tree-sitter 0.26 (grammars already vendored), serde/bincode (snapshot cache), existing test conventions.

## Global Constraints

- Functional style: pure functions, iterator chains, `?` for errors, immutable bindings (project CLAUDE.md).
- TDD strictly: every task writes the failing test first (user memory: never skip, not even for "obvious" changes).
- Metrics are pure `(snapshot, thresholds) → MetricValue` — no I/O in `src/metrics/`.
- CI treats warnings as errors: run `RUSTFLAGS=-D warnings cargo test` and `cargo clippy --all-targets -- -D warnings` before each commit; `cargo fmt` before each commit (pre-push hook enforces).
- Commit messages: conventional commits, never mention Claude/AI.
- The snapshot cache self-heals: adding a field makes old `snapshot.bin` fail bincode deserialization → `cache::storage::load` deletes it and returns None → re-collection. No version constant exists or is needed.
- Rust content coupling = `#[path]` only. Rust common coupling = look-through rule (write-once wrappers with pure contents NOT flagged). Control = pub/exported + branched-on only. All per spec §Detection rules.
- Tree-sitter node-kind names in this plan are believed correct for tree-sitter-rust 0.24 / -javascript 0.25 / -typescript 0.23. If a detector test fails unexpectedly, print `tree.root_node().to_sexp()` in the test to see actual node kinds, and adjust kind strings — not the rule logic.

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/snapshot/mod.rs` | Modify | `CouplingKind`, `CouplingFinding` types; `coupling_findings` field on `RepoSnapshot` |
| `src/metrics/complexity/pressman.rs` | Create | All AST detectors; public entry `extract_coupling_findings(path, content)` |
| `src/metrics/complexity/mod.rs` | Modify | Declare `pressman` module, re-export entry point |
| `src/collector/snapshot_builder.rs` | Modify | Run detectors in the parallel file pass; populate snapshot field |
| `src/collector/mod.rs` | Modify | Adapt `collect_file_metrics` wrapper if its return type is affected |
| `src/config/thresholds.rs` | Modify | `content_barrel_rule: bool` on `CouplingThresholds` (default true) |
| `src/metrics/coupling/mod.rs` | Modify | Barrel-bypass pure function; three Pressman metrics; `score_pressman` bands; severity cap |
| `src/metrics/coupling/tests.rs` | Modify | Unit tests for new metric functions |
| `tests/pressman_coupling_walking_skeleton.rs` | Create | End-to-end: analyze a real repo, assert the three metrics exist |

---

### Task 1: Snapshot types and field

**Files:**
- Modify: `src/snapshot/mod.rs` (types near `FileComplexity` ~line 130; field on `RepoSnapshot` ~line 205; `new()` ~line 210)
- Modify: `src/collector/snapshot_builder.rs:238` and `:300` (struct literals gain the field)
- Test: `src/cache/storage.rs` (roundtrip test extension)

**Interfaces:**
- Produces: `snapshot::CouplingKind { Content, Common, Control }` (derives `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize`); `snapshot::CouplingFinding { path: PathBuf, line: Option<usize>, kind: CouplingKind, evidence: String }` (derives `Debug, Clone, PartialEq, Serialize, Deserialize`); `RepoSnapshot.coupling_findings: Vec<CouplingFinding>`. All later tasks consume these.

- [ ] **Step 1: Write the failing test** — in `src/cache/storage.rs` tests module:

```rust
#[test]
fn snapshot_roundtrips_coupling_findings() {
    use crate::snapshot::{CouplingFinding, CouplingKind};
    let dir = TempDir::new().unwrap();
    let mut snapshot = make_test_snapshot();
    snapshot.coupling_findings.push(CouplingFinding {
        path: PathBuf::from("src/lib.rs"),
        line: Some(42),
        kind: CouplingKind::Common,
        evidence: "static mut CACHE: usize = 0;".into(),
    });
    save(&snapshot, dir.path()).unwrap();
    let loaded = load(dir.path()).unwrap().unwrap();
    assert_eq!(loaded.coupling_findings.len(), 1);
    assert_eq!(loaded.coupling_findings[0].kind, CouplingKind::Common);
    assert_eq!(loaded.coupling_findings[0].line, Some(42));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test snapshot_roundtrips_coupling_findings`
Expected: COMPILE ERROR — `CouplingFinding` not found.

- [ ] **Step 3: Implement** — in `src/snapshot/mod.rs`, after `FileComplexity` (~line 142):

```rust
/// Pressman coupling taxonomy rung, ordered worst → least severe.
/// Only the statically detectable rungs are represented (data/stamp omitted).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CouplingKind {
    /// Reaching into another module's internals (e.g. `#[path]` imports).
    Content,
    /// Shared mutable global state (e.g. `static mut`, exported `let`).
    Common,
    /// Flag parameter steering a public function's internal control flow.
    Control,
}

/// A single evidenced coupling finding produced by the collector's AST pass
/// (or, for barrel bypass, derived from the import graph at metric time).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CouplingFinding {
    pub path: PathBuf,
    /// 1-based line. `None` for graph-derived findings (no line information).
    pub line: Option<usize>,
    pub kind: CouplingKind,
    /// Short human-readable snippet, e.g. `static mut CACHE: usize = 0;`.
    pub evidence: String,
}
```

Add to `RepoSnapshot` (after `import_graph`):

```rust
    pub coupling_findings: Vec<CouplingFinding>,
```

Add `coupling_findings: Vec::new(),` to the three struct literals: `RepoSnapshot::new()` in `src/snapshot/mod.rs`, and both literals in `src/collector/snapshot_builder.rs` (`collect_snapshot_inner` ~line 238 — a later task fills this with real data — and `collect_snapshot_at` ~line 300, which stays empty by design, consistent with ADR-005's empty `file_metrics`).

- [ ] **Step 4: Verify the compiler found every construction site**

Run: `cargo check 2>&1 | grep -c "missing field"`
Expected: `0` after the three edits. If more literals exist, the compiler lists them — add the field there too.

- [ ] **Step 5: Run tests**

Run: `cargo test --lib cache::storage`
Expected: PASS, including the new roundtrip test.

- [ ] **Step 6: Commit**

```bash
git add src/snapshot/mod.rs src/collector/snapshot_builder.rs src/cache/storage.rs
git commit -m "feat(snapshot): add CouplingFinding type and coupling_findings field"
```

---

### Task 2: Detector module + Rust common coupling

**Files:**
- Create: `src/metrics/complexity/pressman.rs`
- Modify: `src/metrics/complexity/mod.rs` (module decl + re-export)

**Interfaces:**
- Consumes: `super::treesitter::parse` (`pub(super) fn parse(content: &str, grammar: &tree_sitter::Language) -> Option<tree_sitter::Tree>`), `super::lang_dispatch::grammar_for`, `super::fallback::{detect_language, Language}`, Task 1's types.
- Produces: `complexity::extract_coupling_findings(path: &Path, content: &str) -> Vec<CouplingFinding>` — the single public entry point all later detector tasks extend and Task 7 wires into the collector. Internal helpers `descendants`, `text`, `finding`, `contains_word` reused by Tasks 3–6.

- [ ] **Step 1: Write the failing tests** — bottom of the new `src/metrics/complexity/pressman.rs` (create the file with just the test module first, or write tests + skeleton together; the cycle below assumes tests-first with a stub that returns `Vec::new()`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::CouplingKind;
    use std::path::Path;

    fn findings_for(name: &str, content: &str) -> Vec<crate::snapshot::CouplingFinding> {
        extract_coupling_findings(Path::new(name), content)
    }

    // ── Rust common coupling ───────────────────────────────────────

    #[test]
    fn rust_static_mut_is_common_coupling() {
        let f = findings_for("src/a.rs", "static mut CACHE: usize = 0;\n");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, CouplingKind::Common);
        assert_eq!(f[0].line, Some(1));
        assert!(f[0].evidence.contains("static mut CACHE"));
    }

    #[test]
    fn rust_static_mutex_is_common_coupling() {
        let src = "use std::sync::Mutex;\nstatic REGISTRY: Mutex<Vec<u32>> = Mutex::new(Vec::new());\n";
        let f = findings_for("src/a.rs", src);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, CouplingKind::Common);
        assert_eq!(f[0].line, Some(2));
    }

    #[test]
    fn rust_atomic_static_is_common_coupling() {
        let src = "static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);\n";
        assert_eq!(findings_for("src/a.rs", src).len(), 1);
    }

    #[test]
    fn rust_lazylock_wrapping_mutex_is_flagged_lookthrough() {
        let src = "static STATE: std::sync::LazyLock<std::sync::Mutex<u32>> = std::sync::LazyLock::new(|| std::sync::Mutex::new(0));\n";
        let f = findings_for("src/a.rs", src);
        assert_eq!(f.len(), 1, "write-once wrapper around Mutex is a mutable global");
    }

    #[test]
    fn rust_pure_lazylock_is_not_flagged() {
        let src = "static KEYWORDS: std::sync::LazyLock<Vec<&'static str>> = std::sync::LazyLock::new(Vec::new);\n";
        assert!(findings_for("src/a.rs", src).is_empty(), "write-once pure static is not common coupling");
    }

    #[test]
    fn rust_plain_immutable_static_is_not_flagged() {
        assert!(findings_for("src/a.rs", "static MAX: usize = 10;\n").is_empty());
    }

    #[test]
    fn unsupported_language_returns_empty() {
        assert!(findings_for("script.py", "x = 1\n").is_empty());
    }

    #[test]
    fn unparseable_extension_returns_empty() {
        assert!(findings_for("notes.txt", "hello\n").is_empty());
    }
}
```

- [ ] **Step 2: Write the module skeleton so tests compile, with `rust_common` unimplemented** — top of `src/metrics/complexity/pressman.rs`:

```rust
//! Pressman coupling detectors: content, common, and control coupling
//! findings extracted from a single file's AST. Pure — no I/O.

use std::path::Path;

use tree_sitter::Node;

use crate::snapshot::{CouplingFinding, CouplingKind};

use super::fallback::{detect_language, Language};
use super::lang_dispatch::grammar_for;
use super::treesitter::parse;

/// Extract Pressman coupling findings from one file's source.
/// Returns an empty Vec for unsupported languages or parse failures.
pub fn extract_coupling_findings(path: &Path, content: &str) -> Vec<CouplingFinding> {
    let lang = detect_language(&path.to_string_lossy());
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let Some(grammar) = grammar_for(lang, ext) else {
        return Vec::new();
    };
    let Some(tree) = parse(content, &grammar) else {
        return Vec::new();
    };
    match lang {
        Language::Rust => rust_findings(tree.root_node(), content, path),
        _ => Vec::new(),
    }
}

/// All nodes of a subtree, preorder.
fn descendants(root: Node<'_>) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        out.push(n);
        for i in (0..n.child_count()).rev() {
            if let Some(c) = n.child(i) {
                stack.push(c);
            }
        }
    }
    out
}

fn text<'a>(node: Node<'_>, content: &'a str) -> &'a str {
    &content[node.byte_range()]
}

fn finding(path: &Path, node: Node<'_>, kind: CouplingKind, content: &str) -> CouplingFinding {
    let evidence: String = text(node, content)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .chars()
        .take(120)
        .collect();
    CouplingFinding {
        path: path.to_path_buf(),
        line: Some(node.start_position().row + 1),
        kind,
        evidence,
    }
}

/// True when `word` appears in `hay` with non-identifier characters (or
/// string boundaries) on both sides.
fn contains_word(hay: &str, word: &str) -> bool {
    hay.match_indices(word).any(|(i, _)| {
        let before_ok = !hay[..i]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_');
        let after_ok = !hay[i + word.len()..]
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '_');
        before_ok && after_ok
    })
}

// ── Rust detectors ─────────────────────────────────────────────────

fn rust_findings(root: Node<'_>, content: &str, path: &Path) -> Vec<CouplingFinding> {
    descendants(root)
        .into_iter()
        .filter_map(|n| match n.kind() {
            "static_item" => rust_common(n, content, path),
            _ => None,
        })
        .collect()
}

fn rust_common(_node: Node<'_>, _content: &str, _path: &Path) -> Option<CouplingFinding> {
    None // implemented in Step 4
}
```

Register in `src/metrics/complexity/mod.rs` (after `mod queries;`):

```rust
mod pressman;
```
and after the `pub use fallback::…` line:
```rust
pub use pressman::extract_coupling_findings;
```

- [ ] **Step 3: Run tests to verify the right ones fail**

Run: `cargo test --lib complexity::pressman`
Expected: `rust_static_mut…`, `rust_static_mutex…`, `rust_atomic…`, `rust_lazylock_wrapping…` FAIL (empty findings); the three negative tests and two unsupported-language tests PASS.

- [ ] **Step 4: Implement `rust_common`**

```rust
/// Interior-mutability type markers. Substring match over the whole item
/// text implements the look-through rule: `LazyLock<Mutex<…>>` matches
/// `Mutex<`, while `LazyLock<Regex>` matches nothing.
const INTERIOR_MUTABILITY: &[&str] = &[
    "Mutex<",
    "RwLock<",
    "RefCell<",
    "Cell<",
    "UnsafeCell<",
    "Atomic",
];

fn rust_common(node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let is_mut = (0..node.child_count())
        .filter_map(|i| node.child(i))
        .any(|c| c.kind() == "mutable_specifier");
    let item_text = text(node, content);
    let interior = INTERIOR_MUTABILITY.iter().any(|p| item_text.contains(p));
    (is_mut || interior).then(|| finding(path, node, CouplingKind::Common, content))
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib complexity::pressman`
Expected: all PASS. (Note `Cell<` also matches `RefCell<` occurrences — harmless, both are flagged kinds.)

- [ ] **Step 6: Commit**

```bash
cargo fmt && git add src/metrics/complexity/pressman.rs src/metrics/complexity/mod.rs
git commit -m "feat(metrics): Rust common-coupling detector with look-through rule"
```

---

### Task 3: Rust content coupling (`#[path]`)

**Files:**
- Modify: `src/metrics/complexity/pressman.rs`

**Interfaces:**
- Consumes: Task 2's helpers and `rust_findings` dispatch.
- Produces: `attribute_item` arm in `rust_findings`.

- [ ] **Step 1: Write the failing tests** (in the existing tests module):

```rust
    // ── Rust content coupling ──────────────────────────────────────

    #[test]
    fn rust_path_attribute_is_content_coupling() {
        let src = "#[path = \"../other/impl.rs\"]\nmod stolen;\n";
        let f = findings_for("src/a.rs", src);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, CouplingKind::Content);
        assert!(f[0].evidence.contains("#[path"));
    }

    #[test]
    fn rust_other_attributes_are_not_flagged() {
        let src = "#[derive(Debug)]\n#[cfg(test)]\nstruct Foo;\n";
        assert!(findings_for("src/a.rs", src).is_empty());
    }
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test --lib complexity::pressman::tests::rust_path_attribute`
Expected: FAIL — 0 findings.

- [ ] **Step 3: Implement** — add the arm to `rust_findings`:

```rust
            "attribute_item" => rust_content(n, content, path),
```

and the detector:

```rust
fn rust_content(node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let normalized: String = text(node, content)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    normalized
        .starts_with("#[path=")
        .then(|| finding(path, node, CouplingKind::Content, content))
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib complexity::pressman`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt && git add src/metrics/complexity/pressman.rs
git commit -m "feat(metrics): Rust content-coupling detector for #[path] imports"
```

---

### Task 4: Rust control coupling

**Files:**
- Modify: `src/metrics/complexity/pressman.rs`

**Interfaces:**
- Consumes: Task 2's helpers (`descendants`, `contains_word`).
- Produces: `function_item` arm in `rust_findings`.

- [ ] **Step 1: Write the failing tests:**

```rust
    // ── Rust control coupling ──────────────────────────────────────

    #[test]
    fn pub_fn_with_branched_bool_is_control_coupling() {
        let src = "pub fn render(compact: bool) {\n    if compact {\n        short();\n    } else {\n        long();\n    }\n}\n";
        let f = findings_for("src/a.rs", src);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, CouplingKind::Control);
        assert!(f[0].evidence.contains("pub fn render"));
    }

    #[test]
    fn pub_fn_with_matched_bool_is_control_coupling() {
        let src = "pub fn go(fast: bool) {\n    match fast {\n        true => sprint(),\n        false => walk(),\n    }\n}\n";
        assert_eq!(findings_for("src/a.rs", src).len(), 1);
    }

    #[test]
    fn private_fn_with_branched_bool_is_not_flagged() {
        let src = "fn helper(flag: bool) {\n    if flag {\n        a();\n    }\n}\n";
        assert!(findings_for("src/a.rs", src).is_empty(), "coupling is inter-module; private fns exempt");
    }

    #[test]
    fn pub_fn_with_stored_bool_is_not_flagged() {
        let src = "pub fn set_visible(visible: bool) {\n    STATE_VISIBLE.store(visible);\n}\n";
        assert!(findings_for("src/a.rs", src).is_empty(), "bool-as-data is not control coupling");
    }

    #[test]
    fn pub_fn_without_bool_params_is_not_flagged() {
        let src = "pub fn add(a: u32, b: u32) -> u32 {\n    if a > b { a } else { b }\n}\n";
        assert!(findings_for("src/a.rs", src).is_empty());
    }

    #[test]
    fn similarly_named_variable_does_not_false_positive() {
        // param `flag` unused in branches; local `flagged` is branched on
        let src = "pub fn f(flag: bool) {\n    let flagged = compute();\n    if flagged {\n        a(flag);\n    }\n}\n";
        assert!(findings_for("src/a.rs", src).is_empty(), "word-boundary match must not catch 'flagged'");
    }
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test --lib complexity::pressman`
Expected: the two positive tests FAIL; negatives PASS.

- [ ] **Step 3: Implement** — add the arm:

```rust
            "function_item" => rust_control(n, content, path),
```

and the detector:

```rust
fn rust_control(node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let is_pub = (0..node.child_count())
        .filter_map(|i| node.child(i))
        .any(|c| c.kind() == "visibility_modifier");
    if !is_pub {
        return None;
    }
    let params = node.child_by_field_name("parameters")?;
    let bool_params: Vec<&str> = (0..params.child_count())
        .filter_map(|i| params.child(i))
        .filter(|p| p.kind() == "parameter")
        .filter(|p| {
            p.child_by_field_name("type")
                .is_some_and(|t| text(t, content) == "bool")
        })
        .filter_map(|p| p.child_by_field_name("pattern").map(|pat| text(pat, content)))
        .collect();
    if bool_params.is_empty() {
        return None;
    }
    let body = node.child_by_field_name("body")?;
    let branched = descendants(body).into_iter().any(|n| {
        let cond = match n.kind() {
            "if_expression" | "while_expression" => n.child_by_field_name("condition"),
            "match_expression" => n.child_by_field_name("value"),
            _ => None,
        };
        cond.is_some_and(|c| {
            bool_params
                .iter()
                .any(|p| contains_word(text(c, content), p))
        })
    });
    branched.then(|| finding(path, node, CouplingKind::Control, content))
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib complexity::pressman`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt && git add src/metrics/complexity/pressman.rs
git commit -m "feat(metrics): Rust control-coupling detector (branched-on pub bool params)"
```

---

### Task 5: TS/JS common coupling

**Files:**
- Modify: `src/metrics/complexity/pressman.rs`

**Interfaces:**
- Consumes: Task 2's helpers; `Language::JsTs` grammar dispatch (ext-sensitive: ts/tsx/js).
- Produces: `js_findings(root, content, path) -> Vec<CouplingFinding>` dispatched from `extract_coupling_findings`; `js_control` slot left as a stub returning `None` (Task 6 fills it).

- [ ] **Step 1: Write the failing tests:**

```rust
    // ── TS/JS common coupling ──────────────────────────────────────

    #[test]
    fn ts_export_let_is_common_coupling() {
        let f = findings_for("src/state.ts", "export let counter = 0;\n");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, CouplingKind::Common);
    }

    #[test]
    fn js_export_var_is_common_coupling() {
        assert_eq!(findings_for("src/state.js", "export var mode = 'a';\n").len(), 1);
    }

    #[test]
    fn ts_export_const_is_not_flagged() {
        assert!(findings_for("src/config.ts", "export const MAX = 10;\n").is_empty());
    }

    #[test]
    fn js_globalthis_write_is_common_coupling() {
        let f = findings_for("src/boot.js", "globalThis.appState = {};\n");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, CouplingKind::Common);
    }

    #[test]
    fn js_window_write_is_common_coupling() {
        assert_eq!(findings_for("src/boot.js", "window.cache = new Map();\n").len(), 1);
    }

    #[test]
    fn js_reading_window_is_not_flagged() {
        assert!(findings_for("src/read.js", "const w = window.innerWidth;\n").is_empty());
    }

    #[test]
    fn ts_singleton_getinstance_is_common_coupling() {
        let src = "class Db {\n  private static instance: Db;\n  static getInstance(): Db {\n    return Db.instance;\n  }\n}\n";
        let f = findings_for("src/db.ts", src);
        assert!(!f.is_empty(), "getInstance singleton must be flagged");
        assert!(f.iter().all(|x| x.kind == CouplingKind::Common));
    }

    #[test]
    fn js_static_instance_field_is_common_coupling() {
        let src = "class Api {\n  static instance = null;\n}\n";
        assert_eq!(findings_for("src/api.js", src).len(), 1);
    }

    #[test]
    fn ts_plain_class_is_not_flagged() {
        let src = "class Point {\n  x = 0;\n  static origin() { return new Point(); }\n}\n";
        assert!(findings_for("src/p.ts", src).is_empty());
    }
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test --lib complexity::pressman`
Expected: new positive tests FAIL (JsTs currently returns `Vec::new()`); negatives PASS vacuously.

- [ ] **Step 3: Implement** — change the dispatch in `extract_coupling_findings`:

```rust
    match lang {
        Language::Rust => rust_findings(tree.root_node(), content, path),
        Language::JsTs => js_findings(tree.root_node(), content, path),
        _ => Vec::new(),
    }
```

and add:

```rust
// ── TS/JS detectors ────────────────────────────────────────────────

fn js_findings(root: Node<'_>, content: &str, path: &Path) -> Vec<CouplingFinding> {
    descendants(root)
        .into_iter()
        .filter_map(|n| match n.kind() {
            "export_statement" => js_export(n, content, path),
            "assignment_expression" => js_global_write(n, content, path),
            "class_declaration" | "class" => js_singleton(n, content, path),
            _ => None,
        })
        .collect()
}

/// `export let` / `export var` → Common. `export function` → control check.
fn js_export(node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let decl = node.child_by_field_name("declaration")?;
    match decl.kind() {
        "lexical_declaration" if text(decl, content).starts_with("let ") => {
            Some(finding(path, node, CouplingKind::Common, content))
        }
        "variable_declaration" => Some(finding(path, node, CouplingKind::Common, content)),
        "function_declaration" => js_control(decl, content, path),
        _ => None,
    }
}

/// Assignment to `globalThis.x` / `window.x` → Common.
fn js_global_write(node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let left = node.child_by_field_name("left")?;
    if left.kind() != "member_expression" {
        return None;
    }
    let obj = left.child_by_field_name("object")?;
    let is_global =
        obj.kind() == "identifier" && matches!(text(obj, content), "globalThis" | "window");
    is_global.then(|| finding(path, node, CouplingKind::Common, content))
}

/// Class with a static `instance` field or static `getInstance()` → Common.
fn js_singleton(class_node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let body = class_node.child_by_field_name("body")?;
    (0..body.child_count())
        .filter_map(|i| body.child(i))
        .find_map(|member| {
            let is_static = (0..member.child_count())
                .filter_map(|i| member.child(i))
                .any(|c| text(c, content) == "static");
            if !is_static {
                return None;
            }
            let name = member
                .child_by_field_name("name")
                .map(|n| text(n, content))?;
            let hit = (member.kind() == "method_definition" && name == "getInstance")
                || (matches!(member.kind(), "field_definition" | "public_field_definition")
                    && name == "instance");
            hit.then(|| finding(path, member, CouplingKind::Common, content))
        })
}

fn js_control(_func: Node<'_>, _content: &str, _path: &Path) -> Option<CouplingFinding> {
    None // implemented in Task 6
}
```

Note: TS's grammar wraps static fields as `public_field_definition`; JS uses `field_definition` — both handled. If the singleton tests fail on a kind mismatch, print the sexp per Global Constraints and adjust the two kind strings.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib complexity::pressman`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt && git add src/metrics/complexity/pressman.rs
git commit -m "feat(metrics): TS/JS common-coupling detectors (export let, global writes, singletons)"
```

---

### Task 6: TS/JS control coupling

**Files:**
- Modify: `src/metrics/complexity/pressman.rs`

**Interfaces:**
- Consumes: `js_control` stub from Task 5 (already dispatched for exported function declarations only — the exported-only rule is enforced by construction).
- Produces: full `js_control` implementation.

- [ ] **Step 1: Write the failing tests:**

```rust
    // ── TS/JS control coupling ─────────────────────────────────────

    #[test]
    fn ts_exported_fn_with_branched_boolean_is_control_coupling() {
        let src = "export function render(compact: boolean) {\n  if (compact) {\n    short();\n  }\n}\n";
        let f = findings_for("src/r.ts", src);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, CouplingKind::Control);
    }

    #[test]
    fn ts_exported_fn_with_ternary_boolean_is_control_coupling() {
        let src = "export function pick(fast: boolean): number {\n  return fast ? 1 : 2;\n}\n";
        assert_eq!(findings_for("src/p.ts", src).len(), 1);
    }

    #[test]
    fn js_exported_fn_with_default_bool_branched_is_control_coupling() {
        let src = "export function log(verbose = false) {\n  if (verbose) {\n    console.debug('x');\n  }\n}\n";
        assert_eq!(findings_for("src/l.js", src).len(), 1);
    }

    #[test]
    fn ts_non_exported_fn_is_not_flagged() {
        let src = "function helper(flag: boolean) {\n  if (flag) {\n    a();\n  }\n}\n";
        assert!(findings_for("src/h.ts", src).is_empty());
    }

    #[test]
    fn ts_exported_fn_with_stored_boolean_is_not_flagged() {
        let src = "export function setVisible(visible: boolean) {\n  state.visible = visible;\n}\n";
        assert!(findings_for("src/s.ts", src).is_empty());
    }
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test --lib complexity::pressman`
Expected: three positive tests FAIL; negatives PASS.

- [ ] **Step 3: Implement `js_control`** (replace the stub):

```rust
/// Exported function whose boolean parameter (TS annotation or JS
/// `= true/false` default) is branched on in the body → Control.
fn js_control(func: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let params = func.child_by_field_name("parameters")?;
    let flag_names: Vec<&str> = descendants(params)
        .into_iter()
        .filter_map(|p| match p.kind() {
            // TS: required_parameter / optional_parameter with `: boolean`
            "required_parameter" | "optional_parameter" => {
                let is_bool = (0..p.child_count())
                    .filter_map(|i| p.child(i))
                    .any(|c| c.kind() == "type_annotation" && text(c, content).contains("boolean"));
                let pat = p.child_by_field_name("pattern")?;
                (is_bool && pat.kind() == "identifier").then(|| text(pat, content))
            }
            // JS: `param = true` / `param = false`
            "assignment_pattern" => {
                let right_is_bool = p
                    .child_by_field_name("right")
                    .is_some_and(|r| matches!(r.kind(), "true" | "false"));
                let left = p.child_by_field_name("left")?;
                (right_is_bool && left.kind() == "identifier").then(|| text(left, content))
            }
            _ => None,
        })
        .collect();
    if flag_names.is_empty() {
        return None;
    }
    let body = func.child_by_field_name("body")?;
    let branched = descendants(body).into_iter().any(|n| {
        let cond = match n.kind() {
            "if_statement" | "while_statement" | "ternary_expression" => {
                n.child_by_field_name("condition")
            }
            _ => None,
        };
        cond.is_some_and(|c| flag_names.iter().any(|f| contains_word(text(c, content), f)))
    });
    branched.then(|| finding(path, func, CouplingKind::Control, content))
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib complexity::pressman`
Expected: all PASS. (If `ternary_expression`'s field is not `condition` in this grammar version, check the sexp; tree-sitter-javascript names it `condition`.)

- [ ] **Step 5: Commit**

```bash
cargo fmt && git add src/metrics/complexity/pressman.rs
git commit -m "feat(metrics): TS/JS control-coupling detector for exported flag params"
```

---

### Task 7: Collector wiring

**Files:**
- Modify: `src/collector/snapshot_builder.rs:18-45` (`collect_file_metrics_with_progress`), `:225-255` (`collect_snapshot_inner`)
- Modify: `src/collector/mod.rs:123` (`collect_file_metrics` wrapper — keep its existing signature; it can discard findings)

**Interfaces:**
- Consumes: `complexity::extract_coupling_findings` (Task 2), `RepoSnapshot.coupling_findings` (Task 1).
- Produces: `collect_file_metrics_with_progress` now returns `(HashMap<PathBuf, FileComplexity>, RawImports, Vec<CouplingFinding>)`; snapshots built by `collect_snapshot_inner` carry real findings, **sorted by (path, line)** for determinism under rayon.

- [ ] **Step 1: Write the failing test** — in `snapshot_builder.rs` tests module (self-analysis pattern, skips gracefully outside a repo):

```rust
    #[test]
    fn collect_snapshot_populates_coupling_findings_deterministically() {
        let Ok(collector) = Collector::open(&test_repo_path(), TimeWindow::default()) else {
            return;
        };
        let files = collector.collect_files().expect("should collect files");
        // NoProgress is already imported at the top of snapshot_builder.rs
        // (`use super::progress::{NoProgress, Progress};`) and reaches the
        // tests module via `use super::*`.
        let (_, _, findings) = collector.collect_file_metrics_with_progress(&files, &NoProgress);
        // barad-dur's own code should produce a deterministic, sorted list
        let mut sorted = findings.clone();
        sorted.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
        assert_eq!(findings, sorted, "findings must be sorted by (path, line)");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib collect_snapshot_populates_coupling_findings`
Expected: COMPILE ERROR — the function returns a 2-tuple.

- [ ] **Step 3: Implement** — in `collect_file_metrics_with_progress`, extend the pipeline:

```rust
    pub(super) fn collect_file_metrics_with_progress(
        &self,
        files: &[FileEntry],
        progress: &dyn Progress,
    ) -> (
        HashMap<PathBuf, FileComplexity>,
        RawImports,
        Vec<CouplingFinding>,
    ) {
        let root = self.repo_path();
        let results: Vec<(PathBuf, FileComplexity, Vec<String>, Vec<CouplingFinding>)> = files
            .par_iter()
            .filter(|entry| !entry.is_binary)
            .filter_map(|entry| {
                let abs_path = root.join(&entry.path);
                let content = std::fs::read_to_string(&abs_path).ok()?;
                let metrics = complexity::analyse_file(&entry.path, &content);
                let imports = complexity::extract_file_imports(&entry.path, &content);
                let findings = complexity::extract_coupling_findings(&entry.path, &content);
                progress.inc(1);
                Some((entry.path.clone(), metrics, imports, findings))
            })
            .collect();
        let mut file_metrics = HashMap::new();
        let mut raw_imports = HashMap::new();
        let mut coupling_findings = Vec::new();
        for (path, metrics, imports, findings) in results {
            file_metrics.insert(path.clone(), metrics);
            if !imports.is_empty() {
                raw_imports.insert(path, imports);
            }
            coupling_findings.extend(findings);
        }
        coupling_findings.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
        (file_metrics, raw_imports, coupling_findings)
    }
```

Add `use crate::snapshot::CouplingFinding;` to the imports. In `collect_snapshot_inner`, destructure the 3-tuple and set the field:

```rust
        let (file_metrics, raw_imports, coupling_findings) =
            self.collect_file_metrics_with_progress(&files, complexity_progress);
```
and in the `RepoSnapshot` literal replace `coupling_findings: Vec::new(),` with `coupling_findings,`.

In `src/collector/mod.rs`, adapt the `collect_file_metrics` wrapper (~line 123) to destructure and discard the extra elements, keeping its public signature unchanged.

- [ ] **Step 4: Run the full lib suite**

Run: `RUSTFLAGS=-D warnings cargo test --lib`
Expected: PASS, including the new test.

- [ ] **Step 5: Commit**

```bash
cargo fmt && git add src/collector/snapshot_builder.rs src/collector/mod.rs
git commit -m "feat(collector): populate coupling findings during file metrics pass"
```

---

### Task 8: Config toggle + barrel-bypass rule

**Files:**
- Modify: `src/config/thresholds.rs:135-156` (`CouplingThresholds`)
- Modify: `src/metrics/coupling/mod.rs` (new pure function)
- Test: `src/metrics/coupling/tests.rs`

**Interfaces:**
- Consumes: `snapshot.import_graph`, `snapshot.files`, `extract_component` (exists in `src/metrics/coupling/mod.rs:28`), Task 1's types.
- Produces: `CouplingThresholds.content_barrel_rule: bool` (serde default `true`; update `impl Default` too); `pub(crate) fn barrel_bypass_findings(snapshot: &RepoSnapshot, component_depth: usize) -> Vec<CouplingFinding>` with `line: None` on every finding.

- [ ] **Step 1: Write the failing tests** — in `src/metrics/coupling/tests.rs`:

```rust
#[test]
fn barrel_bypass_cross_component_is_detected() {
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![
        crate::metrics::testutil::make_file("app/main.ts"),
        crate::metrics::testutil::make_file("lib/index.ts"),
        crate::metrics::testutil::make_file("lib/impl.ts"),
    ];
    // app/main.ts deep-imports lib/impl.ts although lib/index.ts exists
    snapshot.import_graph.insert(
        PathBuf::from("app/main.ts"),
        vec![PathBuf::from("lib/impl.ts")],
    );
    let findings = barrel_bypass_findings(&snapshot, 1);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, crate::snapshot::CouplingKind::Content);
    assert_eq!(findings[0].path, PathBuf::from("app/main.ts"));
    assert_eq!(findings[0].line, None);
    assert!(findings[0].evidence.contains("lib/impl.ts"));
}

#[test]
fn barrel_bypass_same_component_is_not_flagged() {
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![
        crate::metrics::testutil::make_file("lib/a.ts"),
        crate::metrics::testutil::make_file("lib/sub/index.ts"),
        crate::metrics::testutil::make_file("lib/sub/impl.ts"),
    ];
    snapshot.import_graph.insert(
        PathBuf::from("lib/a.ts"),
        vec![PathBuf::from("lib/sub/impl.ts")],
    );
    // component_depth 1: both sides are component "lib" → internal structure
    assert!(barrel_bypass_findings(&snapshot, 1).is_empty());
}

#[test]
fn barrel_bypass_without_barrel_is_not_flagged() {
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![
        crate::metrics::testutil::make_file("app/main.ts"),
        crate::metrics::testutil::make_file("lib/impl.ts"), // no index.ts in lib/
    ];
    snapshot.import_graph.insert(
        PathBuf::from("app/main.ts"),
        vec![PathBuf::from("lib/impl.ts")],
    );
    assert!(barrel_bypass_findings(&snapshot, 1).is_empty());
}

#[test]
fn barrel_import_itself_is_not_flagged() {
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![
        crate::metrics::testutil::make_file("app/main.ts"),
        crate::metrics::testutil::make_file("lib/index.ts"),
    ];
    snapshot.import_graph.insert(
        PathBuf::from("app/main.ts"),
        vec![PathBuf::from("lib/index.ts")], // the sanctioned route
    );
    assert!(barrel_bypass_findings(&snapshot, 1).is_empty());
}

#[test]
fn barrel_bypass_ignores_rust_files() {
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![
        crate::metrics::testutil::make_file("app/main.rs"),
        crate::metrics::testutil::make_file("lib/index.ts"),
        crate::metrics::testutil::make_file("lib/util.rs"),
    ];
    snapshot.import_graph.insert(
        PathBuf::from("app/main.rs"),
        vec![PathBuf::from("lib/util.rs")],
    );
    assert!(barrel_bypass_findings(&snapshot, 1).is_empty());
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test --lib metrics::coupling`
Expected: COMPILE ERROR — `barrel_bypass_findings` not found.

- [ ] **Step 3: Implement** — in `src/metrics/coupling/mod.rs`:

```rust
const BARREL_NAMES: &[&str] = &["index.ts", "index.tsx", "index.js", "index.jsx"];
const JS_EXTS: &[&str] = &["ts", "tsx", "js", "jsx", "mjs", "cjs"];

/// Content coupling via barrel bypass: a cross-component relative import
/// that resolves to a non-index file in a directory that has a barrel.
/// Line info is unavailable (graph-derived), so `line: None`.
pub(crate) fn barrel_bypass_findings(
    snapshot: &RepoSnapshot,
    component_depth: usize,
) -> Vec<CouplingFinding> {
    let barrel_dirs: HashSet<&Path> = snapshot
        .files
        .iter()
        .filter(|f| {
            f.path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| BARREL_NAMES.contains(&n))
        })
        .filter_map(|f| f.path.parent())
        .collect();

    snapshot
        .import_graph
        .iter()
        .flat_map(|(source, targets)| {
            targets.iter().filter_map(move |target| {
                let is_js = target
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| JS_EXTS.contains(&e));
                let is_barrel_file = target
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| BARREL_NAMES.contains(&n));
                let target_dir = target.parent()?;
                let bypass = is_js
                    && !is_barrel_file
                    && barrel_dirs.contains(target_dir)
                    && source.parent() != Some(target_dir)
                    && extract_component(source, component_depth)
                        != extract_component(target, component_depth);
                bypass.then(|| CouplingFinding {
                    path: source.clone(),
                    line: None,
                    kind: CouplingKind::Content,
                    evidence: format!(
                        "imports {} directly — barrel {}/index.* exists",
                        target.display(),
                        target_dir.display()
                    ),
                })
            })
        })
        .collect()
}
```

Imports: `std::collections::HashSet` and `std::path::Path` are already imported at the top of `src/metrics/coupling/mod.rs`; add `use crate::snapshot::{CouplingFinding, CouplingKind};` (extend the existing `use crate::snapshot::RepoSnapshot;` line).

In `src/config/thresholds.rs`:

```rust
pub struct CouplingThresholds {
    #[serde(default = "default_component_depth")]
    pub component_depth: usize,
    #[serde(default = "default_change_coupling_min_ratio")]
    pub change_coupling_min_ratio: f64,
    /// Enable the TS/JS barrel-bypass content-coupling rule. Teams whose
    /// culture prefers deep imports can turn it off.
    #[serde(default = "default_content_barrel_rule")]
    pub content_barrel_rule: bool,
}

fn default_content_barrel_rule() -> bool {
    true
}
```

and add `content_barrel_rule: default_content_barrel_rule(),` to `impl Default for CouplingThresholds`. If any test constructs `CouplingThresholds` literally, the compiler will list the sites — add the field there.

- [ ] **Step 4: Run tests**

Run: `RUSTFLAGS=-D warnings cargo test --lib`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt && git add src/metrics/coupling/mod.rs src/metrics/coupling/tests.rs src/config/thresholds.rs
git commit -m "feat(metrics): barrel-bypass content-coupling rule with config toggle"
```

---

### Task 9: Three Pressman metrics + severity bands

> **USER DECISION POINT:** `score_pressman` encodes how harshly each rung is judged — how bad is one `static mut`, how many flag args are tolerable. The signature, contract, and a suggested default are below so the task is executable, but the maintainer should author or adjust the bands (constraint: `Content ≤ 50` at count ≥ 1 and `Common ≤ 25` at high counts must remain reachable, because Task 10's severity cap keys on those values).

**Files:**
- Modify: `src/metrics/coupling/mod.rs` (`compute_coupling` + new functions)
- Test: `src/metrics/coupling/tests.rs`

**Interfaces:**
- Consumes: Task 1 types, Task 8's `barrel_bypass_findings` and `content_barrel_rule`.
- Produces: metrics named exactly `"Content coupling"`, `"Common coupling"`, `"Control coupling"` appended to `compute_coupling`'s metric vec (after `change_coupling_smells`); `pub(crate) fn score_pressman(kind: CouplingKind, count: usize) -> u32`. Task 10 and the integration test match on these exact metric names.

- [ ] **Step 1: Write the failing tests:**

```rust
fn snapshot_with_findings(findings: Vec<CouplingFinding>) -> RepoSnapshot {
    let mut s = crate::metrics::testutil::make_snapshot();
    s.files = vec![crate::metrics::testutil::make_file("src/a.rs")];
    s.coupling_findings = findings;
    s
}

fn make_finding(kind: CouplingKind) -> CouplingFinding {
    CouplingFinding {
        path: PathBuf::from("src/a.rs"),
        line: Some(1),
        kind,
        evidence: "e".into(),
    }
}

#[test]
fn pressman_metrics_appear_in_category() {
    let snapshot = snapshot_with_findings(vec![]);
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    for name in ["Content coupling", "Common coupling", "Control coupling"] {
        assert!(
            result.metrics.iter().any(|m| m.name == name),
            "missing metric {name}"
        );
    }
}

#[test]
fn clean_snapshot_scores_100_on_all_pressman_metrics() {
    let snapshot = snapshot_with_findings(vec![]);
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    let m = result.metrics.iter().find(|m| m.name == "Content coupling").unwrap();
    assert_eq!(m.score, Some(100));
}

#[test]
fn one_content_finding_scores_at_most_50() {
    let snapshot = snapshot_with_findings(vec![make_finding(CouplingKind::Content)]);
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    let m = result.metrics.iter().find(|m| m.name == "Content coupling").unwrap();
    assert!(m.score.unwrap() <= 50, "one content finding must hit the cap trigger");
}

#[test]
fn pressman_metrics_unscored_without_detectable_files() {
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![crate::metrics::testutil::make_file("main.py")];
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    let m = result.metrics.iter().find(|m| m.name == "Common coupling").unwrap();
    assert_eq!(m.score, None, "no Rust/TS/JS files → unscored dash");
}

#[test]
fn content_metric_includes_barrel_findings_when_enabled() {
    let mut snapshot = snapshot_with_findings(vec![]);
    snapshot.files = vec![
        crate::metrics::testutil::make_file("app/main.ts"),
        crate::metrics::testutil::make_file("lib/index.ts"),
        crate::metrics::testutil::make_file("lib/impl.ts"),
    ];
    snapshot.import_graph.insert(
        PathBuf::from("app/main.ts"),
        vec![PathBuf::from("lib/impl.ts")],
    );
    let thresholds = CouplingThresholds { component_depth: 1, ..Default::default() };
    let result = compute_coupling(&snapshot, &thresholds);
    let m = result.metrics.iter().find(|m| m.name == "Content coupling").unwrap();
    assert!(m.score.unwrap() <= 50);

    let off = CouplingThresholds {
        component_depth: 1,
        content_barrel_rule: false,
        ..Default::default()
    };
    let result_off = compute_coupling(&snapshot, &off);
    let m_off = result_off.metrics.iter().find(|m| m.name == "Content coupling").unwrap();
    assert_eq!(m_off.score, Some(100), "toggle off → barrel findings excluded");
}

#[test]
fn control_findings_are_scored_leniently() {
    let findings = (0..3).map(|_| make_finding(CouplingKind::Control)).collect();
    let snapshot = snapshot_with_findings(findings);
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    let m = result.metrics.iter().find(|m| m.name == "Control coupling").unwrap();
    assert!(m.score.unwrap() > 70, "a few flag args must not tank the metric");
}
```

Add needed imports to the tests file: `use crate::snapshot::{CouplingFinding, CouplingKind, RepoSnapshot};`, `use crate::config::CouplingThresholds;`.

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test --lib metrics::coupling`
Expected: FAIL — metrics missing.

- [ ] **Step 3: Implement** — in `src/metrics/coupling/mod.rs`:

```rust
const DETECTABLE_EXTS: &[&str] = &["rs", "ts", "tsx", "js", "jsx", "mjs", "cjs"];

fn has_detectable_files(snapshot: &RepoSnapshot) -> bool {
    snapshot.files.iter().any(|f| {
        f.path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| DETECTABLE_EXTS.contains(&e))
    })
}

/// Severity-banded score for a Pressman finding count.
///
/// MAINTAINER-AUTHORED BANDS. Invariants the rest of the system relies on:
/// - count 0 → 100 for every kind
/// - Content: any count ≥ 1 must score ≤ 50 (triggers the category cap)
/// - Common: large counts must reach ≤ 25 (triggers the category cap)
/// - Control is the mildest rung: keep bands lenient
pub(crate) fn score_pressman(kind: CouplingKind, count: usize) -> u32 {
    match kind {
        CouplingKind::Content => match count {
            0 => 100,
            1 => 50,
            2..=3 => 35,
            _ => 25,
        },
        CouplingKind::Common => match count {
            // Maintainer decision (2026-07-02): harsher than the draft —
            // one mutable global already stings, four trigger the category cap.
            0 => 100,
            1 => 60,
            2..=3 => 40,
            _ => 25,
        },
        CouplingKind::Control => match count {
            0 => 100,
            1..=5 => 85,
            6..=15 => 70,
            _ => 50,
        },
    }
}

fn pressman_metric(
    snapshot: &RepoSnapshot,
    kind: CouplingKind,
    extra: Vec<CouplingFinding>,
) -> MetricValue {
    let (name, rung) = match kind {
        CouplingKind::Content => ("Content coupling", "worst rung: another module's internals reached"),
        CouplingKind::Common => ("Common coupling", "shared mutable global state"),
        CouplingKind::Control => ("Control coupling", "flag parameters steering callee logic"),
    };
    if !has_detectable_files(snapshot) {
        return MetricValue {
            name: name.to_string(),
            description: "No files in detectable languages (Rust, TS/JS)".to_string(),
            raw_value: RawValue::Count(0),
            score: None,
        };
    }
    let findings: Vec<CouplingFinding> = snapshot
        .coupling_findings
        .iter()
        .filter(|f| f.kind == kind)
        .cloned()
        .chain(extra)
        .collect();
    let count = findings.len();
    let list: Vec<String> = findings
        .iter()
        .take(10)
        .map(|f| match f.line {
            Some(l) => format!("{}:{} — {}", f.path.display(), l, f.evidence),
            None => format!("{} — {}", f.path.display(), f.evidence),
        })
        .collect();
    MetricValue {
        name: name.to_string(),
        description: format!("{} finding(s) — {}", count, rung),
        raw_value: RawValue::List(list),
        score: Some(score_pressman(kind, count)),
    }
}
```

Extend `compute_coupling`:

```rust
pub fn compute_coupling(
    snapshot: &RepoSnapshot,
    thresholds: &CouplingThresholds,
) -> CategoryResult {
    let barrel = if thresholds.content_barrel_rule {
        barrel_bypass_findings(snapshot, thresholds.component_depth)
    } else {
        Vec::new()
    };
    let metrics = vec![
        afferent_coupling(snapshot),
        efferent_coupling(snapshot),
        circular_dependencies(snapshot),
        change_coupling_smells(snapshot, thresholds),
        pressman_metric(snapshot, CouplingKind::Content, barrel),
        pressman_metric(snapshot, CouplingKind::Common, Vec::new()),
        pressman_metric(snapshot, CouplingKind::Control, Vec::new()),
    ];
    CategoryResult {
        name: "Coupling".to_string(),
        score: 0,
        metrics,
    }
    .compute_score()
}
```

- [ ] **Step 4: Run tests**

Run: `RUSTFLAGS=-D warnings cargo test --lib`
Expected: PASS. (Existing coupling tests still pass — the four legacy metrics are untouched; category averages shift only when findings exist, and legacy tests use snapshots without findings. If a legacy test asserted an exact 4-metric count, update it to 7.)

- [ ] **Step 5: Commit**

```bash
cargo fmt && git add src/metrics/coupling/mod.rs src/metrics/coupling/tests.rs
git commit -m "feat(metrics): content/common/control coupling metrics with severity bands"
```

---

### Task 10: Severity cap

**Files:**
- Modify: `src/metrics/coupling/mod.rs` (`compute_coupling` + `apply_severity_cap`)
- Test: `src/metrics/coupling/tests.rs`

**Interfaces:**
- Consumes: Task 9's exact metric names and band invariants.
- Produces: category score capped at 70 while `"Content coupling"` scores ≤ 50 or `"Common coupling"` scores ≤ 25; triggering metric's description gains `" — category score capped at 70 (severity cap)"`.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn severity_cap_limits_category_when_content_coupling_found() {
    // One content finding among otherwise-perfect metrics: flat average
    // would be ~93 (6×100+50)/7 — the cap must pull it to 70.
    let snapshot = snapshot_with_findings(vec![make_finding(CouplingKind::Content)]);
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    assert!(
        result.score <= 70,
        "category must not be green with content coupling present, got {}",
        result.score
    );
    let m = result.metrics.iter().find(|m| m.name == "Content coupling").unwrap();
    assert!(
        m.description.contains("capped"),
        "cap must be visible in the triggering metric's description"
    );
}

#[test]
fn severity_cap_not_applied_when_clean() {
    let snapshot = snapshot_with_findings(vec![]);
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    let m = result.metrics.iter().find(|m| m.name == "Content coupling").unwrap();
    assert!(!m.description.contains("capped"));
}

#[test]
fn severity_cap_triggers_on_many_common_findings() {
    let findings = (0..6).map(|_| make_finding(CouplingKind::Common)).collect();
    let snapshot = snapshot_with_findings(findings);
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    assert!(result.score <= 70, "got {}", result.score);
}

#[test]
fn severity_cap_does_not_raise_already_low_scores() {
    // If the average is already below 70 the cap must not touch it.
    let findings = vec![
        make_finding(CouplingKind::Content),
        make_finding(CouplingKind::Content),
        make_finding(CouplingKind::Content),
        make_finding(CouplingKind::Content),
    ];
    let snapshot = snapshot_with_findings(findings);
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    let flat_average_would_be = result
        .metrics
        .iter()
        .filter_map(|m| m.score)
        .sum::<u32>()
        / result.metrics.iter().filter(|m| m.score.is_some()).count() as u32;
    assert!(result.score <= flat_average_would_be.min(70));
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test --lib metrics::coupling`
Expected: cap tests FAIL (score above 70 / no "capped" note).

- [ ] **Step 3: Implement:**

```rust
/// Pressman's scale is ordinal by severity: the worst rung present bounds
/// how healthy the category can be called. A flat average would hide one
/// catastrophic finding behind six healthy metrics (see spec, resolved
/// question 5).
fn apply_severity_cap(mut cat: CategoryResult) -> CategoryResult {
    let triggers: Vec<String> = cat
        .metrics
        .iter()
        .filter(|m| {
            let limit = match m.name.as_str() {
                "Content coupling" => 50,
                "Common coupling" => 25,
                _ => return false,
            };
            m.score.is_some_and(|s| s <= limit)
        })
        .map(|m| m.name.clone())
        .collect();
    if cat.score > 70 && !triggers.is_empty() {
        cat.score = 70;
        for m in cat.metrics.iter_mut().filter(|m| triggers.contains(&m.name)) {
            m.description
                .push_str(" — category score capped at 70 (severity cap)");
        }
    }
    cat
}
```

Change `compute_coupling`'s tail to:

```rust
    apply_severity_cap(
        CategoryResult {
            name: "Coupling".to_string(),
            score: 0,
            metrics,
        }
        .compute_score(),
    )
```

- [ ] **Step 4: Run tests**

Run: `RUSTFLAGS=-D warnings cargo test --lib`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt && git add src/metrics/coupling/mod.rs src/metrics/coupling/tests.rs
git commit -m "feat(metrics): severity cap — coupling category bounded by worst Pressman rung"
```

---

### Task 11: Integration test + dogfood

**Files:**
- Create: `tests/pressman_coupling_walking_skeleton.rs`

**Interfaces:**
- Consumes: public lib API — `barad_dur::collector::Collector`, `barad_dur::metrics::coupling::compute_coupling`, `barad_dur::config::CouplingThresholds`, `barad_dur::snapshot::TimeWindow`.

- [ ] **Step 1: Write the test:**

```rust
//! Walking skeleton: end-to-end Pressman coupling detection on a real repo.
//! Uses BARAD_DUR_TEST_REPO (CI: CI_PROJECT_DIR) or `.` — same convention
//! as the other integration suites.

use barad_dur::collector::Collector;
use barad_dur::config::CouplingThresholds;
use barad_dur::metrics::coupling::compute_coupling;
use barad_dur::snapshot::TimeWindow;
use std::path::PathBuf;

fn test_repo_path() -> PathBuf {
    std::env::var("BARAD_DUR_TEST_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

#[test]
fn analysis_reports_three_pressman_metrics() {
    let Ok(collector) = Collector::open(&test_repo_path(), TimeWindow::default()) else {
        return; // not a git repo (e.g. cargo-mutants temp dir) — skip
    };
    let snapshot = collector
        .collect_snapshot()
        .expect("snapshot collection must succeed");

    let result = compute_coupling(&snapshot, &CouplingThresholds::default());

    for name in ["Content coupling", "Common coupling", "Control coupling"] {
        let metric = result
            .metrics
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("metric '{name}' missing from Coupling category"));
        assert!(
            metric.score.is_some(),
            "barad-dur has Rust files, so '{name}' must be scored"
        );
    }
    // Dogfood expectation: this codebase avoids mutable globals entirely.
    let common = result
        .metrics
        .iter()
        .find(|m| m.name == "Common coupling")
        .unwrap();
    assert_eq!(
        common.score,
        Some(100),
        "unexpected common-coupling findings in barad-dur itself: {:?}",
        common.raw_value
    );
}
```

- [ ] **Step 2: Run it**

Run: `cargo test --test pressman_coupling_walking_skeleton -- --nocapture`
Expected: PASS. If the dogfood assertion fails, inspect the listed findings: either barad-dur has a real mutable global (fix it in a separate commit) or a detector has a false positive (fix the detector — do not weaken the test).

- [ ] **Step 3: Full verification sweep**

Run:
```bash
RUSTFLAGS=-D warnings cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
cargo run -- analyze . --no-cache -v
```
Expected: all green; the analyze output's Coupling category shows the three new metrics (dashes only if the repo had no Rust/TS/JS files, which barad-dur does have).

- [ ] **Step 4: Commit**

```bash
git add tests/pressman_coupling_walking_skeleton.rs
git commit -m "test(coupling): Pressman detection walking-skeleton integration test"
```

---

## Post-plan notes

- **Not in M1** (spec milestones M2–M6): trend counts, gate ratchet, hotspot cross-referencing, corroboration, action suggestions. Do not add them opportunistically.
- **Renderers need no changes**: CLI/JSON/HTML render `CategoryResult.metrics` generically; `RawValue::List` is already displayed (circular dependencies use it today).
- **Parse cost note**: `extract_coupling_findings` performs its own parse, matching the existing `extract_file_imports` pattern (which also parses separately from `analyse`). Unifying the three parses into one is a possible future refactor, deliberately out of scope here.
