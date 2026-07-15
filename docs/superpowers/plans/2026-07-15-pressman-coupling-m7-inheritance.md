# Pressman Coupling M7 — Inheritance Coupling — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the fourth Pressman rung — `CouplingKind::Inheritance` — detecting TS/JS class chains with project-local DIT ≥ 2, wired through scoring, config, gate, trend, actions, hotspots, and renderers.

**Architecture:** Collector's existing tree-sitter pass emits flat per-class facts (`ClassRecord`, cache-versioned into `RepoSnapshot`); depth is a pure memoized DFS at metric time so the threshold knob is live without `--no-cache`. Findings join `all_coupling_findings` (the M6 seam), so actions/gate/hotspots/counts pick them up with no new wiring.

**Tech Stack:** Rust (tree-sitter, serde/bincode, clap), React 19 + Vite + vitest dashboard.

**Spec:** `docs/superpowers/specs/2026-07-10-pressman-coupling-m7-inheritance-design.md` (approved 2026-07-15).

## Global Constraints

- Branch: `feat/pressman-coupling-m7`, cut from a **freshly fetched** `origin/main` (`git fetch origin && git checkout -b feat/pressman-coupling-m7 origin/main`).
- Tests as CI runs them: `RUSTFLAGS=-D warnings cargo test`; also `cargo clippy --all-targets -- -D warnings` and `cargo fmt -- --check` before each commit (pre-push hook enforces them).
- Commit messages: conventional commits, **never mention Claude/AI**. A PreToolUse hook injects trailers into `git commit -m` — always write the message to a file and use `git commit -F <file>`, then verify with `git cat-file commit HEAD | tail -3` (no `Co-Authored-By`/nWave trailers).
- Functional style: pure functions, iterator chains, `?` propagation, immutable bindings.
- Locked design values (do not renegotiate): bands 0→100, 1–2→70, 3–6→55, >6→40; `inheritance_min_depth` default 2, `0` disables, `1` rejected by validation; severity ladder Content ≻ Common ≻ Inheritance ≻ Control; Rust emits zero findings; hotspot red-highlight and score boost stay `content + common > 0`.
- No `innerHTML` in HTML renderer templates (security hook).
- `CARGO_BIN_EXE_barad-dur` integration tests build the real binary — first run is slow; that's normal.

---

### Task 1: Class-record extraction (TS/JS AST → `RawClassRecord`)

**Files:**
- Create: `src/metrics/complexity/inheritance.rs`
- Modify: `src/metrics/complexity/mod.rs` (module decl + re-exports, next to `pub use pressman::extract_coupling_findings;` at line 13)
- Modify: `src/metrics/complexity/pressman.rs:33` (`fn descendants` → `pub(super) fn descendants` for reuse)

**Interfaces:**
- Consumes: `detect_language`, `grammar_for`, `parse` (same imports as `pressman.rs:10-12`); `pressman::descendants`.
- Produces (used by Task 2):
  ```rust
  pub struct RawClassRecord { pub line: usize, pub class_name: String, pub base: RawBaseRef }
  pub enum RawBaseRef { SameFile(String), Specifier { specifier: String, name: String }, Unresolvable }
  pub fn extract_class_records(path: &Path, content: &str) -> Vec<RawClassRecord>
  ```

- [ ] **Step 1: Write the failing tests**

Create `src/metrics/complexity/inheritance.rs` with only the types, a `todo!()`-free stub returning `Vec::new()`, and the test module:

```rust
//! Class-record extraction for the inheritance-coupling rung (M7):
//! per-file `class … extends …` facts. Pure — no I/O, no hierarchy
//! resolution (that happens at metric time in `metrics/coupling`).

use std::collections::HashMap;
use std::path::Path;

use tree_sitter::Node;

use super::fallback::{detect_language, Language};
use super::lang_dispatch::grammar_for;
use super::pressman::descendants;
use super::treesitter::parse;

/// A `class … extends …` site, before import-specifier resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct RawClassRecord {
    /// 1-based declaration line.
    pub line: usize,
    pub class_name: String,
    pub base: RawBaseRef,
}

/// The extends-target as extraction sees it. `Specifier` is resolved to a
/// repo path (or `Unresolvable`) by the collector's snapshot builder.
#[derive(Debug, Clone, PartialEq)]
pub enum RawBaseRef {
    /// Base identifier not bound by any import — assumed same-file.
    SameFile(String),
    /// Base bound by an import: module specifier + exported name
    /// (aliases unwrapped: `import { A as B }` records name "A").
    Specifier { specifier: String, name: String },
    /// Non-identifier extends expression (`extends mixin(Base)`);
    /// terminates depth counting.
    Unresolvable,
}

/// Extract class records from one TS/JS file. Other languages (including
/// Rust) yield no records — the rung is TS/JS-only by design.
pub fn extract_class_records(path: &Path, content: &str) -> Vec<RawClassRecord> {
    let _ = (path, content);
    Vec::new() // implemented in Step 3
}
```

Append the test module to the same file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn records(name: &str, src: &str) -> Vec<RawClassRecord> {
        extract_class_records(Path::new(name), src)
    }

    #[test]
    fn same_file_extends() {
        let r = records("src/a.ts", "class A {}\nclass B extends A {}\n");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].class_name, "B");
        assert_eq!(r[0].line, 2);
        assert_eq!(r[0].base, RawBaseRef::SameFile("A".into()));
    }

    #[test]
    fn imported_named_base() {
        let src = "import { A } from './a';\nexport class B extends A {}\n";
        let r = records("src/b.ts", src);
        assert_eq!(
            r[0].base,
            RawBaseRef::Specifier { specifier: "./a".into(), name: "A".into() }
        );
    }

    #[test]
    fn aliased_import_records_exported_name() {
        let src = "import { A as Base } from './a';\nclass B extends Base {}\n";
        let r = records("src/b.ts", src);
        assert_eq!(
            r[0].base,
            RawBaseRef::Specifier { specifier: "./a".into(), name: "A".into() }
        );
    }

    #[test]
    fn default_import_base_maps_to_local_name() {
        let src = "import A from './a';\nclass B extends A {}\n";
        let r = records("src/b.js", src);
        assert_eq!(
            r[0].base,
            RawBaseRef::Specifier { specifier: "./a".into(), name: "A".into() }
        );
    }

    #[test]
    fn mixin_call_is_unresolvable() {
        let r = records("src/b.ts", "class B extends mixin(Base) {}\n");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].base, RawBaseRef::Unresolvable);
    }

    #[test]
    fn class_without_extends_yields_no_record() {
        assert!(records("src/a.ts", "class A { m() {} }\n").is_empty());
    }

    #[test]
    fn implements_only_yields_no_record() {
        let src = "interface I {}\nclass A implements I {}\n";
        assert!(records("src/a.ts", src).is_empty());
    }

    #[test]
    fn export_default_anonymous_class_is_captured() {
        let src = "class A {}\nexport default class extends A {}\n";
        let r = records("src/b.ts", src);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].class_name, "default");
        assert_eq!(r[0].base, RawBaseRef::SameFile("A".into()));
    }

    #[test]
    fn rust_files_yield_no_records() {
        let src = "pub trait T {}\npub struct S;\nimpl T for S {}\n";
        assert!(records("src/lib.rs", src).is_empty());
    }

    #[test]
    fn js_grammar_heritage_shape_works_too() {
        // The JS grammar has no extends_clause wrapper — the heritage
        // node's child is the expression itself.
        let r = records("src/b.js", "class A {}\nclass B extends A {}\n");
        assert_eq!(r[0].base, RawBaseRef::SameFile("A".into()));
    }

    #[test]
    fn ts_type_arguments_on_base_still_resolve_the_identifier() {
        let src = "class A<T> {}\nclass B extends A<number> {}\n";
        let r = records("src/b.ts", src);
        assert_eq!(r[0].base, RawBaseRef::SameFile("A".into()));
    }
}
```

Register the module in `src/metrics/complexity/mod.rs` (next to the existing `pub use pressman::extract_coupling_findings;`):

```rust
mod inheritance;
pub use inheritance::{extract_class_records, RawBaseRef, RawClassRecord};
```

And in `src/metrics/complexity/pressman.rs:33` change `fn descendants(` to `pub(super) fn descendants(`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test metrics::complexity::inheritance`
Expected: FAIL — `same_file_extends`, `imported_named_base`, etc. panic on empty vec (the stub returns `Vec::new()`); `class_without_extends_yields_no_record` and `rust_files_yield_no_records` pass vacuously.

- [ ] **Step 3: Implement extraction**

Replace the stub body and add the helpers below it:

```rust
pub fn extract_class_records(path: &Path, content: &str) -> Vec<RawClassRecord> {
    let lang = detect_language(&path.to_string_lossy());
    if !matches!(lang, Language::JsTs) {
        return Vec::new();
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let Some(grammar) = grammar_for(lang, ext) else {
        return Vec::new();
    };
    let Some(tree) = parse(content, &grammar) else {
        return Vec::new();
    };
    let root = tree.root_node();
    let imports = import_bindings(root, content);
    descendants(root)
        .into_iter()
        .filter(|n| matches!(n.kind(), "class_declaration" | "class"))
        .filter_map(|class_node| class_record(class_node, content, &imports))
        .collect()
}

fn class_record(
    class_node: Node<'_>,
    content: &str,
    imports: &HashMap<String, (String, String)>,
) -> Option<RawClassRecord> {
    let heritage = child_of_kind(class_node, "class_heritage")?;
    let expr = base_expression(heritage)?; // None = no extends (e.g. implements-only)
    let base = if expr.kind() == "identifier" {
        let ident = text(expr, content).to_string();
        match imports.get(&ident) {
            Some((specifier, name)) => RawBaseRef::Specifier {
                specifier: specifier.clone(),
                name: name.clone(),
            },
            None => RawBaseRef::SameFile(ident),
        }
    } else {
        RawBaseRef::Unresolvable
    };
    let class_name = class_node
        .child_by_field_name("name")
        .map(|n| text(n, content).to_string())
        .unwrap_or_else(|| "default".to_string()); // `export default class extends X`
    Some(RawClassRecord {
        line: class_node.start_position().row + 1,
        class_name,
        base,
    })
}

/// The TS grammar wraps the extends target in an `extends_clause`
/// (field `value`); the JS grammar puts the expression directly under
/// `class_heritage`. TS `implements`-only heritage is not inheritance.
fn base_expression(heritage: Node<'_>) -> Option<Node<'_>> {
    if let Some(clause) = child_of_kind(heritage, "extends_clause") {
        return clause.child_by_field_name("value");
    }
    if child_of_kind(heritage, "implements_clause").is_some() {
        return None;
    }
    (0..heritage.named_child_count())
        .filter_map(|i| heritage.named_child(i as u32))
        .next()
}

/// local binding → (module specifier, exported name). Default imports map
/// to their local name (best-effort: a renamed default import terminates
/// the chain at resolution time instead — under-count, never over-count).
fn import_bindings(root: Node<'_>, content: &str) -> HashMap<String, (String, String)> {
    descendants(root)
        .into_iter()
        .filter(|n| n.kind() == "import_statement")
        .filter_map(|stmt| {
            let source = stmt.child_by_field_name("source")?;
            let specifier = text(source, content)
                .trim_matches(|c| c == '"' || c == '\'')
                .to_string();
            Some((stmt, specifier))
        })
        .flat_map(|(stmt, specifier)| {
            descendants(stmt)
                .into_iter()
                .filter_map(move |n| binding(n, content, &specifier))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn binding(
    n: Node<'_>,
    content: &str,
    specifier: &str,
) -> Option<(String, (String, String))> {
    match n.kind() {
        // `import A from './a'` — the clause's direct identifier child.
        "import_clause" => {
            let c = n.named_child(0).filter(|c| c.kind() == "identifier")?;
            let name = text(c, content).to_string();
            Some((name.clone(), (specifier.to_string(), name)))
        }
        // `import { A }` / `import { A as B }`.
        "import_specifier" => {
            let exported = text(n.child_by_field_name("name")?, content).to_string();
            let local = n
                .child_by_field_name("alias")
                .map(|a| text(a, content).to_string())
                .unwrap_or_else(|| exported.clone());
            Some((local, (specifier.to_string(), exported)))
        }
        _ => None,
    }
}

fn child_of_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    (0..node.child_count())
        .filter_map(|i| node.child(i as u32))
        .find(|c| c.kind() == kind)
}

fn text<'a>(node: Node<'_>, content: &'a str) -> &'a str {
    &content[node.byte_range()]
}
```

Note: if `Language` doesn't derive `PartialEq`, `matches!` still works (it's a pattern match, not `==`). If tree-sitter node-kind names differ from the above (test failures will show it), inspect with a scratch test printing `descendants(root).iter().map(|n| n.kind())` for a two-line class fixture — do not guess.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test metrics::complexity::inheritance`
Expected: PASS (all 11).

- [ ] **Step 5: Full check + commit**

```bash
RUSTFLAGS=-D warnings cargo test --lib
cargo clippy --all-targets -- -D warnings && cargo fmt
printf 'feat(complexity): extract TS/JS class records for inheritance coupling\n' > /tmp/claude-1000/msg.txt
git add src/metrics/complexity/
git commit -F /tmp/claude-1000/msg.txt
git cat-file commit HEAD | tail -3   # verify: no injected trailers
```

---

### Task 2: Snapshot model, cache bump, collector wiring

**Files:**
- Modify: `src/snapshot/mod.rs` (types after `CouplingFinding` ~line 166; field at ~line 230; `new()` at ~line 252)
- Modify: `src/cache/storage.rs:16` (CACHE_VERSION)
- Modify: `src/collector/import_resolver.rs` (new `pub(crate) fn resolve_specifier`)
- Modify: `src/collector/snapshot_builder.rs` (4-tuple pass, `resolve_class_records`, both `RepoSnapshot` literals, `ast_pass_at`, existing test at line 485)

**Interfaces:**
- Consumes: Task 1's `extract_class_records`, `RawClassRecord`, `RawBaseRef`; `resolve_single_import` (private in `import_resolver.rs`).
- Produces (used by Task 5):
  ```rust
  // src/snapshot/mod.rs
  pub struct ClassRecord { pub path: PathBuf, pub line: usize, pub class_name: String, pub base: BaseRef }
  pub enum BaseRef { SameFile(String), Resolved { path: PathBuf, name: String }, Unresolvable }
  // RepoSnapshot gains: pub class_records: Vec<ClassRecord>  (sorted by path, then line)
  ```

- [ ] **Step 1: Write the failing test**

In `src/collector/snapshot_builder.rs`'s `mod tests`, add:

```rust
#[test]
fn resolve_class_records_resolves_specifiers_and_sorts() {
    use crate::metrics::complexity::{RawBaseRef, RawClassRecord};
    use crate::snapshot::BaseRef;
    let files = vec![
        crate::metrics::testutil::make_file("src/a.ts"),
        crate::metrics::testutil::make_file("src/b.ts"),
    ];
    let mut raw = HashMap::new();
    raw.insert(
        PathBuf::from("src/b.ts"),
        vec![
            RawClassRecord {
                line: 9,
                class_name: "X".into(),
                base: RawBaseRef::Specifier { specifier: "react".into(), name: "Component".into() },
            },
            RawClassRecord {
                line: 2,
                class_name: "B".into(),
                base: RawBaseRef::Specifier { specifier: "./a".into(), name: "A".into() },
            },
        ],
    );
    let records = resolve_class_records(raw, &files);
    assert_eq!(records.len(), 2);
    // sorted by (path, line): B (line 2) before X (line 9)
    assert_eq!(records[0].class_name, "B");
    assert_eq!(
        records[0].base,
        BaseRef::Resolved { path: "src/a.ts".into(), name: "A".into() }
    );
    assert_eq!(
        records[1].base,
        BaseRef::Unresolvable,
        "external package must not resolve"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test resolve_class_records`
Expected: FAIL to compile — `resolve_class_records`, `ClassRecord`, `BaseRef` don't exist.

- [ ] **Step 3: Implement**

`src/snapshot/mod.rs`, directly after the `CouplingFinding` struct (~line 166):

```rust
/// A `class … extends …` site in a TS/JS file, with its base resolved as
/// far as static analysis allows. Produced by the collector; inheritance
/// depth (DIT) is computed at metric time so the threshold knob stays live
/// without re-collection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassRecord {
    pub path: PathBuf,
    /// 1-based declaration line.
    pub line: usize,
    pub class_name: String,
    pub base: BaseRef,
}

/// The extends-target of a class record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BaseRef {
    /// Base class assumed declared in the same file.
    SameFile(String),
    /// Base imported from a resolved project-local file.
    Resolved { path: PathBuf, name: String },
    /// External package, non-identifier extends expression, or unresolved
    /// specifier — terminates depth counting.
    Unresolvable,
}
```

`RepoSnapshot`: add `pub class_records: Vec<ClassRecord>,` after `coupling_findings` (~line 230), and `class_records: Vec::new(),` in `new()` (~line 252).

`src/cache/storage.rs:12-16` — extend the comment's history line and bump:

```rust
/// ... History: 1 = post-M1 shape (coupling_findings); 2 = M7 (class_records).
const CACHE_VERSION: u32 = 2;
```

`src/collector/import_resolver.rs`, after `resolve_single_import`:

```rust
/// Resolve one raw import specifier from `source` against the repo's known
/// files. Crate-visible for class-record base resolution (M7), which must
/// use the exact same candidate rules as the import graph.
pub(crate) fn resolve_specifier(
    raw: &str,
    source: &Path,
    known: &HashSet<&PathBuf>,
) -> Option<PathBuf> {
    resolve_single_import(raw, source, known)
}
```

`src/collector/snapshot_builder.rs`:

1. Imports — extend line 10 & 13 to:
   ```rust
   use crate::metrics::complexity::{self, RawBaseRef, RawClassRecord};
   use crate::snapshot::{
       BaseRef, ClassRecord, CouplingFinding, FileComplexity, FileEntry, RepoSnapshot, TimeWindow,
   };
   use super::import_resolver::{resolve_imports, resolve_specifier, RawImports};
   ```
   (Keep `complexity::` call sites working via the `self` import.)

2. `collect_file_metrics_with_progress` — return type gains a 4th element `HashMap<PathBuf, Vec<RawClassRecord>>`; the per-file closure adds
   ```rust
   let classes = complexity::extract_class_records(&entry.path, &content);
   ```
   and yields `Some((entry.path.clone(), metrics, imports, findings, classes))`. The accumulation loop becomes (note `path.clone()` — `path` was previously moved conditionally):
   ```rust
   let mut raw_classes = HashMap::new();
   for (path, metrics, imports, findings, classes) in results {
       file_metrics.insert(path.clone(), metrics);
       if !imports.is_empty() {
           raw_imports.insert(path.clone(), imports);
       }
       if !classes.is_empty() {
           raw_classes.insert(path, classes);
       }
       coupling_findings.extend(findings);
   }
   coupling_findings.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
   (file_metrics, raw_imports, coupling_findings, raw_classes)
   ```

3. New pure helper (place after `ast_pass_at`):
   ```rust
   /// Resolve raw class records' import specifiers against the repo's file
   /// set, producing the snapshot's `class_records` (sorted by path, line).
   fn resolve_class_records(
       raw: HashMap<PathBuf, Vec<RawClassRecord>>,
       files: &[FileEntry],
   ) -> Vec<ClassRecord> {
       let known: std::collections::HashSet<&PathBuf> =
           files.iter().map(|f| &f.path).collect();
       let mut records: Vec<ClassRecord> = raw
           .into_iter()
           .flat_map(|(path, recs)| {
               recs.into_iter()
                   .map(|r| {
                       let base = match r.base {
                           RawBaseRef::SameFile(name) => BaseRef::SameFile(name),
                           RawBaseRef::Unresolvable => BaseRef::Unresolvable,
                           RawBaseRef::Specifier { specifier, name } => {
                               match resolve_specifier(&specifier, &path, &known) {
                                   Some(target) => BaseRef::Resolved { path: target, name },
                                   None => BaseRef::Unresolvable,
                               }
                           }
                       };
                       ClassRecord {
                           path: path.clone(),
                           line: r.line,
                           class_name: r.class_name,
                           base,
                       }
                   })
                   .collect::<Vec<_>>()
           })
           .collect();
       records.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
       records
   }
   ```

4. Working-tree call site (~line 233): destructure the 4-tuple, then next to `resolve_imports`:
   ```rust
   let (file_metrics, raw_imports, coupling_findings, raw_classes) =
       self.collect_file_metrics_with_progress(&files, complexity_progress);
   ...
   let import_graph = resolve_imports(&raw_imports, &files);
   let class_records = resolve_class_records(raw_classes, &files);
   ```
   and add `class_records,` to the `RepoSnapshot` literal (~line 262).

5. `ast_pass_at` (~line 370): return type gains `Vec<ClassRecord>`; the loop adds
   ```rust
   let classes = complexity::extract_class_records(&entry.path, content);
   if !classes.is_empty() {
       raw_classes.insert(entry.path.clone(), classes);
   }
   ```
   (declare `let mut raw_classes: HashMap<PathBuf, Vec<RawClassRecord>> = HashMap::new();` with the other accumulators) and ends:
   ```rust
   let import_graph = resolve_imports(&raw_imports, files);
   let class_records = resolve_class_records(raw_classes, files);
   Ok((file_metrics, import_graph, coupling_findings, class_records))
   ```

6. `collect_snapshot_at` (~line 333): destructure 4, with the ADR-005 else-arm `(HashMap::new(), HashMap::new(), Vec::new(), Vec::new())`, and add `class_records,` to its `RepoSnapshot` literal (~line 355).

7. Fix the existing test at line 485: `let (_, _, findings)` → `let (_, _, findings, _)`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `RUSTFLAGS=-D warnings cargo test --lib`
Expected: PASS, including `resolve_class_records_resolves_specifiers_and_sorts` and all pre-existing collector/cache tests.

- [ ] **Step 5: Commit**

```bash
cargo clippy --all-targets -- -D warnings && cargo fmt
printf 'feat(snapshot): carry resolved class records; bump cache to v2\n' > /tmp/claude-1000/msg.txt
git add src/snapshot/mod.rs src/cache/storage.rs src/collector/
git commit -F /tmp/claude-1000/msg.txt
git cat-file commit HEAD | tail -3
```

---

### Task 3: `CouplingKind::Inheritance` — variant + every integration arm

Inserting the variant breaks every exhaustive match at once, so this task lands the variant **with its final logic everywhere**, compiler-guided. Findings are hand-built in tests (`CouplingFinding { kind: Inheritance, … }`) — no detector needed yet.

**Files:**
- Modify: `src/snapshot/mod.rs:147-154` (enum)
- Modify: `src/metrics/coupling/mod.rs` (`score_pressman` ~line 328, `pressman_metric` labels ~line 360, `pressman_finding_counts` ~line 305)
- Modify: `src/scorer/types.rs` (`CouplingFindingCounts` ~line 182, `HotspotFile` ~line 24, `HistoryCounts` ~line 198)
- Modify: `src/scorer.rs:44-51` (history fill) + tests ~line 465, 483
- Modify: `src/scorer/actions.rs` (advice const ~line 76, severity ~line 102, labels ~line 127, doc ~line 80)
- Modify: `src/scorer/builders.rs:67-76, 93-96, 107-109` (4-tuple counting + `HotspotFile` literal)
- Modify: `src/cmd/gate.rs:89-101` (two count literals), `:321-325` (increases)
- Test: `src/metrics/coupling/tests.rs`, plus inline tests in the files above

**Interfaces:**
- Produces (relied on by Tasks 5, 7):
  - `CouplingKind::Inheritance` declared **between `Common` and `Control`**.
  - `score_pressman(Inheritance, n)`: 0→100, 1..=2→70, 3..=6→55, _→40.
  - `CouplingFindingCounts { content, common, inheritance, control }` (all `usize`).
  - `HotspotFile.inheritance_findings: usize`; `HistoryCounts.inheritance_coupling: Option<usize>`.
  - Action label `"inheritance"`, advice const `INHERITANCE_ADVICE` containing `"composition"`.

- [ ] **Step 1: Write the failing tests**

`src/metrics/coupling/tests.rs` — extend the exact-band table in `score_pressman_bands_are_exact` (line 556) with:

```rust
        (Inheritance, 0, 100),
        (Inheritance, 1, 70),
        (Inheritance, 2, 70),
        (Inheritance, 3, 55),
        (Inheritance, 6, 55),
        (Inheritance, 7, 40),
```

and add a counts test next to the existing count tests:

```rust
#[test]
fn finding_counts_include_inheritance_kind() {
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![crate::metrics::testutil::make_file("src/c.ts")];
    snapshot
        .file_metrics
        .insert("src/c.ts".into(), Default::default());
    snapshot.coupling_findings = vec![CouplingFinding {
        path: "src/c.ts".into(),
        line: Some(2),
        kind: CouplingKind::Inheritance,
        evidence: "class C extends B → A (depth 2)".into(),
    }];
    let counts =
        pressman_finding_counts(&snapshot, &CouplingThresholds::default()).unwrap();
    assert_eq!(
        (counts.content, counts.common, counts.inheritance, counts.control),
        (0, 0, 1, 0)
    );
}
```

`src/scorer/actions.rs` tests — extend the advice table test (~line 505) with the row `(CouplingKind::Inheritance, "composition"),` and add (reusing the file's existing `finding`/`snap_with` helpers):

```rust
#[test]
fn inheritance_ranks_between_common_and_control() {
    let s = snap_with(vec![
        finding("src/ctrl.rs", CouplingKind::Control),
        finding("src/deep.ts", CouplingKind::Inheritance),
        finding("src/glob.rs", CouplingKind::Common),
    ]);
    let actions =
        generate_coupling_actions(&s, &crate::config::CouplingThresholds::default());
    let texts: Vec<&str> = actions.iter().map(|a| a.text.as_str()).collect();
    assert!(texts[0].contains("src/glob.rs") && texts[0].contains("worst: common"));
    assert!(texts[1].contains("src/deep.ts") && texts[1].contains("worst: inheritance"));
    assert!(texts[2].contains("src/ctrl.rs") && texts[2].contains("worst: control"));
}
```

(If `snap_with` returns `let mut s`-style — match the neighboring tests' exact setup.)

`src/scorer/builders.rs` tests — next to `hotspot_rows_carry_per_kind_finding_counts` (line 1124):

```rust
#[test]
fn hotspot_rows_carry_inheritance_counts_without_score_boost() {
    use crate::snapshot::{CouplingFinding, CouplingKind};
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![
        crate::metrics::testutil::make_file("src/deep.ts"),
        crate::metrics::testutil::make_file("src/clean.ts"),
    ];
    snapshot.coupling_findings = vec![CouplingFinding {
        path: "src/deep.ts".into(),
        line: Some(2),
        kind: CouplingKind::Inheritance,
        evidence: "class C extends B → A (depth 2)".into(),
    }];
    let cfg = crate::config::CouplingThresholds::default();
    let hotspots = build_hotspots(&snapshot, &cfg);
    let deep = hotspots.iter().find(|h| h.path == "src/deep.ts").unwrap();
    let clean = hotspots.iter().find(|h| h.path == "src/clean.ts").unwrap();
    assert_eq!(deep.inheritance_findings, 1);
    assert_eq!(clean.inheritance_findings, 0);
    // Ladder: only Content/Common multiply hotspot risk. Identical
    // churn/complexity ⇒ identical score despite the inheritance finding.
    assert_eq!(deep.hotspot_score, clean.hotspot_score);
}
```

`src/cmd/gate.rs` tests:

```rust
#[test]
fn ratchet_reports_inheritance_increase() {
    let baseline = CouplingFindingCounts { content: 0, common: 0, inheritance: 1, control: 0 };
    let head = CouplingFindingCounts { content: 0, common: 0, inheritance: 3, control: 0 };
    let verdict = ratchet_verdict(&baseline, &head, &[], &[], 0);
    assert!(verdict.increases.contains(&("inheritance", 1, 3)));
}
```

`src/scorer.rs` tests — extend `history_entry_carries_finding_counts` (line 480) with `assert_eq!(entry.counts.inheritance_coupling, Some(0));`, extend the counts literal in `report_embeds_finding_counts_when_detection_ran` (line 465) with `inheritance: 0,`, and add:

```rust
#[test]
fn history_counts_old_json_reads_inheritance_as_none() {
    let json = r#"{"commits":1,"files":2,"authors":1,"content_coupling":0,"common_coupling":0,"control_coupling":0}"#;
    let counts: crate::scorer::HistoryCounts = serde_json::from_str(json).unwrap();
    assert_eq!(counts.inheritance_coupling, None, "pre-M7 entry: not measured, never Some(0)");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib`
Expected: FAIL to compile — `Inheritance` variant and the three new fields don't exist yet.

- [ ] **Step 3: Implement — variant first, then follow the compiler**

`src/snapshot/mod.rs:147`:

```rust
pub enum CouplingKind {
    /// Reaching into another module's internals (e.g. `#[path]` imports).
    Content,
    /// Shared mutable global state (e.g. `static mut`, exported `let`).
    Common,
    /// Deep class-inheritance chain (project-local DIT ≥ threshold), TS/JS.
    Inheritance,
    /// Flag parameter steering a public function's internal control flow.
    Control,
}
```

`cargo build` now lists every non-exhaustive match. Fill each with its final logic:

- `src/metrics/coupling/mod.rs` `score_pressman` — insert between the Common and Control arms:
  ```rust
  CouplingKind::Inheritance => match count {
      // Maintainer decision (2026-07-15): between Common's harshness and
      // Control's leniency; the floor never triggers the ≤25 category cap.
      0 => 100,
      1..=2 => 70,
      3..=6 => 55,
      _ => 40,
  },
  ```
- `pressman_metric` labels (~line 360), between Common and Control:
  ```rust
  CouplingKind::Inheritance => ("Inheritance coupling", "deep class inheritance chains"),
  ```
- `pressman_finding_counts` (~line 305): add `inheritance: count_kind(CouplingKind::Inheritance),`.
- `src/scorer/types.rs` `CouplingFindingCounts`: add `pub inheritance: usize,` between `common` and `control`. `HotspotFile`: add `pub inheritance_findings: usize,` after `control_findings`. `HistoryCounts`: add after `control_coupling`:
  ```rust
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub inheritance_coupling: Option<usize>,
  ```
- `src/scorer.rs:48-50`: add `inheritance_coupling: report.coupling_finding_counts.map(|c| c.inheritance),`.
- `src/scorer/actions.rs`: doc comment ladder (~line 80) becomes `Content≻Common≻Inheritance≻Control`; new const after `CONTROL_ADVICE`:
  ```rust
  const INHERITANCE_ADVICE: &str =
      "Deep inheritance chain — favor composition over inheritance, or flatten the hierarchy.";
  ```
  severity match (~line 102): `Content => 0u8, Common => 1, Inheritance => 2, Control => 3`; label match (~line 127):
  ```rust
  0 => ("content", CONTENT_ADVICE),
  1 => ("common", COMMON_ADVICE),
  2 => ("inheritance", INHERITANCE_ADVICE),
  _ => ("control", CONTROL_ADVICE),
  ```
- `src/scorer/builders.rs:67-76`: the fold's value becomes `(usize, usize, usize, usize)` with `Inheritance => entry.2 += 1, Control => entry.3 += 1`; destructure (~line 93) as `(content_findings, common_findings, inheritance_findings, control_findings)` with `.unwrap_or((0, 0, 0, 0))`; add `inheritance_findings,` to the `HotspotFile` literal. **Do not touch** the boost condition at line 140 (`content_findings + common_findings > 0`).
- `src/cmd/gate.rs`: both `CouplingFindingCounts` fallback literals (~lines 89, 97) gain `inheritance: 0,`; the increases array (~line 321) gains `("inheritance", baseline.inheritance, head.inheritance),` between common and control.
- Any remaining compile errors are other `CouplingFindingCounts` literals (e.g. in tests) — add `inheritance: 0,` to each.

- [ ] **Step 4: Run tests to verify they pass**

Run: `RUSTFLAGS=-D warnings cargo test`
Expected: PASS — new tests green, no pre-existing test changed behavior (inheritance findings don't exist in any fixture yet).

- [ ] **Step 5: Commit**

```bash
cargo clippy --all-targets -- -D warnings && cargo fmt
printf 'feat(coupling): Inheritance rung — bands, counts, actions, gate, hotspots\n' > /tmp/claude-1000/msg.txt
git add src/snapshot/mod.rs src/metrics/coupling/ src/scorer.rs src/scorer/ src/cmd/gate.rs
git commit -F /tmp/claude-1000/msg.txt
git cat-file commit HEAD | tail -3
```

---

### Task 4: Config knob `inheritance_min_depth`

**Files:**
- Modify: `src/config/thresholds.rs:135-184` (field + default fn + `Default` impl)
- Modify: `src/config/mod.rs` (validation after the `corroboration_weight` block ending line 260; tests next to the corroboration tests at ~line 556)
- Modify: `src/init.rs:204-208` (TOML template)

**Interfaces:**
- Produces (used by Task 5): `CouplingThresholds.inheritance_min_depth: usize` (default **2**; `0` disables; `1` invalid).

- [ ] **Step 1: Write the failing tests**

In `src/config/mod.rs` tests, mirroring the corroboration-weight tests' setup (~line 556 — same config construction, same `validate(...)` call shape):

```rust
#[test]
fn inheritance_min_depth_of_one_is_rejected() {
    let mut cfg = base_valid_config();
    cfg.thresholds.coupling.inheritance_min_depth = 1;
    assert!(validate(&cfg).is_err(), "1 would flag every cross-file extends");
}

#[test]
fn inheritance_min_depth_zero_and_two_are_accepted() {
    let mut cfg = base_valid_config();
    cfg.thresholds.coupling.inheritance_min_depth = 0;
    assert!(validate(&cfg).is_ok(), "0 disables the rule");
    cfg.thresholds.coupling.inheritance_min_depth = 2;
    assert!(validate(&cfg).is_ok());
}
```

(`base_valid_config()` = whatever constructor the corroboration tests at line 556-580 use — reuse it verbatim; if they inline `RepoConfig::default()`, do the same.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test config::`
Expected: FAIL to compile — no `inheritance_min_depth` field.

- [ ] **Step 3: Implement**

`src/config/thresholds.rs` — add to `CouplingThresholds` after `corroboration_weight`:

```rust
    /// Minimum project-local inheritance depth (DIT) for a class to be
    /// flagged as inheritance coupling. 0 disables the rule; 1 is rejected
    /// by `validate()` (it would flag every cross-file `extends` — ordinary
    /// OO, not the deep-chain hazard this rung targets). Default 2.
    #[serde(default = "default_inheritance_min_depth")]
    pub inheritance_min_depth: usize,
```

with `fn default_inheritance_min_depth() -> usize { 2 }` next to the other defaults and `inheritance_min_depth: default_inheritance_min_depth(),` in the `Default` impl.

`src/config/mod.rs` — after the corroboration block (line 260):

```rust
    if config.thresholds.coupling.inheritance_min_depth == 1 {
        bail!("thresholds.coupling.inheritance_min_depth must be 0 (disabled) or >= 2, got 1");
    }
```

`src/init.rs:208` — the coupling block's last line becomes two lines (keep the `=` column aligned):

```rust
    out.push_str("hotspot_multiplier        = 1.25\n");
    out.push_str("inheritance_min_depth     = 2\n\n");
```

If an init test asserts the template text, update its expectation to include the new line.

- [ ] **Step 4: Run tests to verify they pass**

Run: `RUSTFLAGS=-D warnings cargo test config:: init`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo clippy --all-targets -- -D warnings && cargo fmt
printf 'feat(config): inheritance_min_depth threshold (0 disables, 1 rejected)\n' > /tmp/claude-1000/msg.txt
git add src/config/ src/init.rs
git commit -F /tmp/claude-1000/msg.txt
git cat-file commit HEAD | tail -3
```

---

### Task 5: Depth computation + metric row + `all_coupling_findings` chaining

**Files:**
- Create: `src/metrics/coupling/inheritance.rs`
- Modify: `src/metrics/coupling/mod.rs` (module decl; `compute_coupling` rows ~line 21-23; `all_coupling_findings` ~line 451)
- Test: inline in the new file + `src/metrics/coupling/tests.rs`

**Interfaces:**
- Consumes: `RepoSnapshot.class_records` (Task 2), `CouplingKind::Inheritance` + bands (Task 3), `thresholds.inheritance_min_depth` (Task 4).
- Produces: `pub(crate) fn inheritance_findings(snapshot: &RepoSnapshot, min_depth: usize) -> Vec<CouplingFinding>` — deterministic order (class_records are pre-sorted), evidence `class C extends B → A (depth 2)`, `line: Some(_)` always.

**Depth semantics (locked):** depth = number of *named, project-visible* ancestors. A `Resolved`/`SameFile` base with no record of its own (a plain root class) still counts as one ancestor; an `Unresolvable` base counts zero (can't be named); a cycle is cut before re-entering an in-progress class.

- [ ] **Step 1: Write the failing tests**

Create `src/metrics/coupling/inheritance.rs` with a stub + tests:

```rust
//! Inheritance-coupling depth (M7): pure, memoized DFS over the snapshot's
//! class records. Depth (DIT) counts only project-local edges; unresolvable
//! and external bases terminate a chain, cycles are cut.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::snapshot::{BaseRef, ClassRecord, CouplingFinding, CouplingKind, RepoSnapshot};

type Key<'a> = (&'a PathBuf, &'a str);

/// Every class whose project-local inheritance depth reaches `min_depth`,
/// as an Inheritance finding. `min_depth == 0` disables the rule.
pub(crate) fn inheritance_findings(
    snapshot: &RepoSnapshot,
    min_depth: usize,
) -> Vec<CouplingFinding> {
    let _ = (snapshot, min_depth);
    Vec::new() // implemented in Step 3
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(path: &str, line: usize, name: &str, base: BaseRef) -> ClassRecord {
        ClassRecord { path: path.into(), line, class_name: name.into(), base }
    }

    fn resolved(path: &str, name: &str) -> BaseRef {
        BaseRef::Resolved { path: path.into(), name: name.into() }
    }

    fn snap(records: Vec<ClassRecord>) -> RepoSnapshot {
        let mut s = crate::metrics::testutil::make_snapshot();
        s.class_records = records;
        s
    }

    fn chain_abc() -> Vec<ClassRecord> {
        vec![
            record("src/b.ts", 2, "B", resolved("src/a.ts", "A")),
            record("src/c.ts", 2, "C", resolved("src/b.ts", "B")),
        ]
    }

    #[test]
    fn depth_one_is_not_flagged() {
        let s = snap(vec![record("src/b.ts", 2, "B", resolved("src/a.ts", "A"))]);
        assert!(inheritance_findings(&s, 2).is_empty());
    }

    #[test]
    fn depth_two_is_flagged_with_line_and_chain_evidence() {
        let f = inheritance_findings(&snap(chain_abc()), 2);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].path, PathBuf::from("src/c.ts"));
        assert_eq!(f[0].line, Some(2));
        assert_eq!(f[0].kind, CouplingKind::Inheritance);
        assert_eq!(f[0].evidence, "class C extends B → A (depth 2)");
    }

    #[test]
    fn same_file_chain_counts() {
        let s = snap(vec![
            record("src/x.ts", 2, "B", BaseRef::SameFile("A".into())),
            record("src/x.ts", 3, "C", BaseRef::SameFile("B".into())),
        ]);
        assert_eq!(inheritance_findings(&s, 2).len(), 1);
    }

    #[test]
    fn unresolvable_base_terminates_chain() {
        // B extends mixin(...) — B's ancestor can't be named, so C's chain
        // is C → B, depth 1: not flagged.
        let s = snap(vec![
            record("src/b.ts", 2, "B", BaseRef::Unresolvable),
            record("src/c.ts", 2, "C", resolved("src/b.ts", "B")),
        ]);
        assert!(inheritance_findings(&s, 2).is_empty());
    }

    #[test]
    fn cycle_is_cut_without_hang_and_without_findings() {
        let s = snap(vec![
            record("src/a.ts", 1, "A", BaseRef::SameFile("B".into())),
            record("src/a.ts", 2, "B", BaseRef::SameFile("A".into())),
        ]);
        assert!(inheritance_findings(&s, 2).is_empty());
    }

    #[test]
    fn diamond_shares_memoized_ancestors() {
        // C and D both extend B (which extends A): both depth 2, flagged.
        let s = snap(vec![
            record("src/b.ts", 2, "B", resolved("src/a.ts", "A")),
            record("src/c.ts", 2, "C", resolved("src/b.ts", "B")),
            record("src/d.ts", 2, "D", resolved("src/b.ts", "B")),
        ]);
        assert_eq!(inheritance_findings(&s, 2).len(), 2);
    }

    #[test]
    fn every_qualifying_class_is_flagged_independently() {
        let mut records = chain_abc();
        records.push(record("src/d.ts", 2, "D", resolved("src/c.ts", "C")));
        // C is depth 2, D is depth 3 — both qualify at threshold 2.
        assert_eq!(inheritance_findings(&snap(records), 2).len(), 2);
    }

    #[test]
    fn min_depth_zero_disables() {
        assert!(inheritance_findings(&snap(chain_abc()), 0).is_empty());
    }

    #[test]
    fn min_depth_three_raises_the_bar() {
        let mut records = chain_abc();
        records.push(record("src/d.ts", 2, "D", resolved("src/c.ts", "C")));
        let f = inheritance_findings(&snap(records), 3);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].path, PathBuf::from("src/d.ts"));
        assert_eq!(f[0].evidence, "class D extends C → B → A (depth 3)");
    }
}
```

Register in `src/metrics/coupling/mod.rs` (top, next to the other private items):

```rust
mod inheritance;
pub(crate) use inheritance::inheritance_findings;
```

Also add the pipeline tests to `src/metrics/coupling/tests.rs`:

```rust
#[test]
fn all_coupling_findings_and_counts_include_inheritance() {
    use crate::snapshot::{BaseRef, ClassRecord};
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![
        crate::metrics::testutil::make_file("src/a.ts"),
        crate::metrics::testutil::make_file("src/b.ts"),
        crate::metrics::testutil::make_file("src/c.ts"),
    ];
    snapshot
        .file_metrics
        .insert("src/c.ts".into(), Default::default());
    snapshot.class_records = vec![
        ClassRecord {
            path: "src/b.ts".into(),
            line: 2,
            class_name: "B".into(),
            base: BaseRef::Resolved { path: "src/a.ts".into(), name: "A".into() },
        },
        ClassRecord {
            path: "src/c.ts".into(),
            line: 2,
            class_name: "C".into(),
            base: BaseRef::Resolved { path: "src/b.ts".into(), name: "B".into() },
        },
    ];
    let cfg = CouplingThresholds::default();
    let inh = all_coupling_findings(&snapshot, &cfg)
        .into_iter()
        .filter(|f| f.kind == CouplingKind::Inheritance)
        .count();
    assert_eq!(inh, 1);
    let counts = pressman_finding_counts(&snapshot, &cfg).unwrap();
    assert_eq!(counts.inheritance, 1);
}

#[test]
fn inheritance_metric_row_uses_bands() {
    use crate::snapshot::{BaseRef, ClassRecord};
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![
        crate::metrics::testutil::make_file("src/a.ts"),
        crate::metrics::testutil::make_file("src/b.ts"),
        crate::metrics::testutil::make_file("src/c.ts"),
    ];
    snapshot
        .file_metrics
        .insert("src/c.ts".into(), Default::default());
    snapshot.class_records = vec![
        ClassRecord {
            path: "src/b.ts".into(),
            line: 2,
            class_name: "B".into(),
            base: BaseRef::Resolved { path: "src/a.ts".into(), name: "A".into() },
        },
        ClassRecord {
            path: "src/c.ts".into(),
            line: 2,
            class_name: "C".into(),
            base: BaseRef::Resolved { path: "src/b.ts".into(), name: "B".into() },
        },
    ];
    let metrics = compute_coupling(&snapshot, &CouplingThresholds::default());
    let m = metrics
        .iter()
        .find(|m| m.name == "Inheritance coupling")
        .expect("metric row");
    assert_eq!(m.score, Some(70), "1 finding → 70 band");
    assert!(m.description.contains("1 finding(s)"));
}
```

(Match `compute_coupling`'s actual signature at `mod.rs:9` — if it takes extra args in the current tree, mirror how neighboring tests in `tests.rs` call it.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test metrics::coupling`
Expected: FAIL — the stub returns no findings (`depth_two…`, `all_coupling_findings…`, `inheritance_metric_row…` panic); disable/termination tests pass vacuously.

- [ ] **Step 3: Implement**

Replace the stub in `src/metrics/coupling/inheritance.rs`:

```rust
pub(crate) fn inheritance_findings(
    snapshot: &RepoSnapshot,
    min_depth: usize,
) -> Vec<CouplingFinding> {
    if min_depth == 0 {
        return Vec::new();
    }
    let by_key: HashMap<Key<'_>, &ClassRecord> = snapshot
        .class_records
        .iter()
        .map(|r| ((&r.path, r.class_name.as_str()), r))
        .collect();
    let mut memo: HashMap<Key<'_>, usize> = HashMap::new();
    snapshot
        .class_records
        .iter()
        .filter_map(|r| {
            let key = (&r.path, r.class_name.as_str());
            let depth = depth_of(key, &by_key, &mut memo, &mut Vec::new());
            (depth >= min_depth).then(|| CouplingFinding {
                path: r.path.clone(),
                line: Some(r.line),
                kind: CouplingKind::Inheritance,
                evidence: evidence(r, depth, &by_key),
            })
        })
        .collect()
}

fn parent_key<'a>(rec: &'a ClassRecord) -> Option<Key<'a>> {
    match &rec.base {
        BaseRef::SameFile(name) => Some((&rec.path, name.as_str())),
        BaseRef::Resolved { path, name } => Some((path, name.as_str())),
        BaseRef::Unresolvable => None,
    }
}

/// Depth = number of named, project-visible ancestors. A named base with no
/// record of its own (a plain root class) counts as one ancestor; an
/// unresolvable base counts zero; a cycle is cut before re-entering an
/// in-progress class. Memoized — diamonds cost each ancestor once.
fn depth_of<'a>(
    key: Key<'a>,
    by_key: &HashMap<Key<'a>, &'a ClassRecord>,
    memo: &mut HashMap<Key<'a>, usize>,
    in_progress: &mut Vec<Key<'a>>,
) -> usize {
    if let Some(&d) = memo.get(&key) {
        return d;
    }
    let Some(rec) = by_key.get(&key) else {
        return 0; // no record: a class without `extends` — chain root
    };
    let d = match parent_key(rec) {
        None => 0, // unresolvable base: the ancestor cannot be named
        Some(pk) if in_progress.contains(&pk) => 0, // cycle: cut before the edge
        Some(pk) if by_key.contains_key(&pk) => {
            in_progress.push(key);
            let parent_depth = depth_of(pk, by_key, memo, in_progress);
            in_progress.pop();
            parent_depth + 1
        }
        Some(_) => 1, // named base without a record: one countable ancestor
    };
    memo.insert(key, d);
    d
}

/// `class C extends B → A (depth 2)` — the named ancestor chain, cycle-safe.
fn evidence(
    rec: &ClassRecord,
    depth: usize,
    by_key: &HashMap<Key<'_>, &ClassRecord>,
) -> String {
    let mut names: Vec<String> = Vec::new();
    let mut seen: Vec<Key<'_>> = vec![(&rec.path, rec.class_name.as_str())];
    let mut cur = parent_key(rec);
    while let Some(k) = cur {
        if seen.contains(&k) {
            break;
        }
        names.push(k.1.to_string());
        seen.push(k);
        cur = by_key.get(&k).and_then(|r| parent_key(r));
    }
    format!(
        "class {} extends {} (depth {})",
        rec.class_name,
        names.join(" → "),
        depth
    )
}
```

`src/metrics/coupling/mod.rs`:

- `compute_coupling` (~line 14-23): compute `let inh = inheritance_findings(snapshot, thresholds.inheritance_min_depth);` next to `let barrel = …`, and insert the row between Common and Control:
  ```rust
  pressman_metric(snapshot, CouplingKind::Inheritance, inh, &corr, weight),
  ```
- `all_coupling_findings` (~line 451): add a second `.chain(…)`:
  ```rust
  snapshot
      .coupling_findings
      .iter()
      .cloned()
      .chain(gated_barrel_findings(snapshot, thresholds))
      .chain(inheritance_findings(snapshot, thresholds.inheritance_min_depth))
      .collect()
  ```
  and extend its doc comment: the complete set now includes metric-time inheritance findings, same rationale (metric, counts, hotspots, gate, actions must never disagree).

- [ ] **Step 4: Run tests to verify they pass**

Run: `RUSTFLAGS=-D warnings cargo test`
Expected: PASS — including every pre-existing gate/actions/hotspots test (their snapshots carry no `class_records`, so behavior is unchanged).

- [ ] **Step 5: Commit**

```bash
cargo clippy --all-targets -- -D warnings && cargo fmt
printf 'feat(coupling): inheritance depth metric wired through all_coupling_findings\n' > /tmp/claude-1000/msg.txt
git add src/metrics/coupling/
git commit -F /tmp/claude-1000/msg.txt
git cat-file commit HEAD | tail -3
```

---

### Task 6: Renderer + dashboard badges (`Ih n`)

**Files:**
- Modify: `src/renderer/templates/hotspots.js:298-305`
- Modify: `dashboard/src/types.ts:37-39`
- Modify: `dashboard/src/components/HotspotsView.tsx:168-173`
- Test: `dashboard/src/components/HotspotsView.test.tsx` (badge describe-block at line 65)

**Interfaces:**
- Consumes: `HotspotFile.inheritance_findings` (Task 3) via report JSON.
- Deliberate non-changes: red highlight (`cn + cm > 0`) in both renderers stays as-is.

- [ ] **Step 1: Write the failing dashboard test**

In the `HotspotsView coupling badge` describe-block of `dashboard/src/components/HotspotsView.test.tsx`:

```tsx
  it('shows the inheritance badge; reports without the field render unchanged', () => {
    const deep: HotspotFile = { ...file('src/deep.ts', 80), inheritance_findings: 2 }
    const old: HotspotFile = file('src/old.ts', 70) // pre-M7 report shape
    render(<HotspotsView files={[deep, old]} />)
    expect(screen.queryByText('Ih 2')).not.toBeNull()
    expect(screen.queryByTitle('src/old.ts')).not.toBeNull()
  })
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm -C dashboard test`
Expected: FAIL — `inheritance_findings` is not a known field (TS error) and/or `Ih 2` not found.

- [ ] **Step 3: Implement**

`dashboard/src/types.ts` — after `control_findings?: number`:

```ts
  inheritance_findings?: number
```

`dashboard/src/components/HotspotsView.tsx:168-173`:

```tsx
              const cn = f.content_findings ?? 0
              const cm = f.common_findings ?? 0
              const ih = f.inheritance_findings ?? 0
              const ct = f.control_findings ?? 0
              const badge = [cn > 0 && `Cn ${cn}`, cm > 0 && `Cm ${cm}`, ih > 0 && `Ih ${ih}`, ct > 0 && `Ct ${ct}`]
                .filter(Boolean)
                .join(' · ')
```

(The cell color condition at line 187 stays `cn + cm > 0`.)

`src/renderer/templates/hotspots.js:298-305`:

```js
        var cn = f.content_findings || 0;
        var cm = f.common_findings || 0;
        var ih = f.inheritance_findings || 0;
        var ct = f.control_findings || 0;
        if (cn + cm + ih + ct > 0) {
          var labels = [];
          if (cn) labels.push('Cn ' + cn);
          if (cm) labels.push('Cm ' + cm);
          if (ih) labels.push('Ih ' + ih);
          if (ct) labels.push('Ct ' + ct);
```

(The badge color at line 309 stays `(cn + cm > 0)`.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `pnpm -C dashboard test` — expected: PASS.
Run: `RUSTFLAGS=-D warnings cargo test renderer` — expected: PASS (templates are embedded via `include_str!`; this catches any renderer test asserting template text).

- [ ] **Step 5: Commit**

```bash
cargo fmt
printf 'feat(dashboard,html): Ih hotspot badge for inheritance findings\n' > /tmp/claude-1000/msg.txt
git add src/renderer/templates/hotspots.js dashboard/src/
git commit -F /tmp/claude-1000/msg.txt
git cat-file commit HEAD | tail -3
```

---

### Task 7: E2E milestone suite + full gates + dogfood

**Files:**
- Create: `tests/pressman_coupling_milestone_7.rs`

**Interfaces:**
- Consumes the whole pipeline through the real binary (`CARGO_BIN_EXE_barad-dur`), asserting the JSON contract: `coupling_finding_counts.inheritance`, `categories[].metrics[]` (`raw_value.List`), `coupling_actions[].text`, `file_hotspots[].inheritance_findings`.

- [ ] **Step 1: Write the E2E tests**

```rust
//! M7 milestone E2E: a TS fixture with a depth-2 inheritance chain surfaces
//! an Inheritance finding through counts, the metric row, actions, and
//! hotspots via `analyze --json`, while Rust trait impls produce nothing.
//! A warm-cache re-run pins the CACHE_VERSION bump (a stale pre-M7 snapshot
//! shape would silently drop class_records).

use std::path::Path;
use std::process::{Command, Output};

fn fixture_repo() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let git = |args: &[&str]| -> Output {
        let out = Command::new("git")
            .current_dir(dir.path())
            .args(args)
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "f@e.com"]);
    git(&["config", "user.name", "F"]);
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/a.ts"), "export class A {}\n").unwrap();
    std::fs::write(
        dir.path().join("src/b.ts"),
        "import { A } from './a';\nexport class B extends A {}\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/c.ts"),
        "import { B } from './b';\nexport class C extends B {}\n",
    )
    .unwrap();
    // Rust trait impls are interface inheritance — must yield no findings.
    std::fs::write(
        dir.path().join("src/lib.rs"),
        "pub trait T { fn f(&self); }\npub struct S;\nimpl T for S { fn f(&self) {} }\n",
    )
    .unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "fixture"]);
    dir
}

fn analyze_json(dir: &Path, extra: &[&str]) -> serde_json::Value {
    let out = Command::new(env!("CARGO_BIN_EXE_barad-dur"))
        .arg("analyze")
        .arg(dir)
        .arg("--json")
        .args(extra)
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "analyze failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("json")
}

#[test]
fn inheritance_finding_surfaces_end_to_end() {
    let dir = fixture_repo();
    let report = analyze_json(dir.path(), &["--no-cache"]);

    // Exactly one finding: C (depth 2); B (depth 1) stays clean; Rust silent.
    assert_eq!(report["coupling_finding_counts"]["inheritance"], 1);
    assert_eq!(report["coupling_finding_counts"]["content"], 0);
    assert_eq!(report["coupling_finding_counts"]["common"], 0);

    // Metric row: band score + real line + chain evidence.
    let coupling_cat = report["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "Coupling")
        .expect("coupling category");
    let metric = coupling_cat["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "Inheritance coupling")
        .expect("inheritance metric row");
    assert_eq!(metric["score"], 70, "1 finding → 70 band");
    let list = metric["raw_value"]["List"]
        .as_array()
        .expect("List raw_value");
    let entry = list[0].as_str().unwrap();
    assert!(
        entry.contains("src/c.ts:2") && entry.contains("class C extends B → A (depth 2)"),
        "evidence with line: {entry}"
    );

    // Action: ranked with the inheritance label + composition advice.
    let action = report["coupling_actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["text"].as_str().unwrap())
        .find(|t| t.contains("worst: inheritance"))
        .expect("inheritance action");
    assert!(action.contains("src/c.ts") && action.contains("composition"));

    // Hotspot badge counts: c.ts flagged, the Rust file clean.
    let hotspots = report["file_hotspots"].as_array().unwrap();
    let c = hotspots.iter().find(|h| h["path"] == "src/c.ts").unwrap();
    assert_eq!(c["inheritance_findings"], 1);
    let rs = hotspots.iter().find(|h| h["path"] == "src/lib.rs").unwrap();
    assert_eq!(rs["inheritance_findings"], 0);
}

#[test]
fn inheritance_finding_survives_warm_cache() {
    let dir = fixture_repo();
    let first = analyze_json(dir.path(), &[]); // collects + writes cache
    let second = analyze_json(dir.path(), &[]); // must serve the cache
    assert_eq!(first["coupling_finding_counts"]["inheritance"], 1);
    assert_eq!(
        second["coupling_finding_counts"]["inheritance"], 1,
        "cached snapshot must round-trip class_records (CACHE_VERSION 2)"
    );
}
```

- [ ] **Step 2: Run the new suite**

Run: `cargo test --test pressman_coupling_milestone_7`
Expected: PASS. If the metric-row path differs (e.g. category name casing), check with `tests/pressman_coupling_milestone_5.rs:60-67`, which asserts the same JSON shape — mirror it exactly.

- [ ] **Step 3: Full gates**

```bash
RUSTFLAGS=-D warnings cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
pnpm -C dashboard test
```
Expected: all green.

- [ ] **Step 4: Dogfood**

Run: `cargo run -- analyze . -v --no-cache`
Expected: completes; barad-dur (Rust + a class-free React dashboard) should report **0 inheritance findings** — the "Inheritance coupling" row shows score 100. Note the observed output in the task report.

- [ ] **Step 5: Commit**

```bash
printf 'test(coupling): M7 milestone — inheritance rung surfaced end-to-end\n' > /tmp/claude-1000/msg.txt
git add tests/pressman_coupling_milestone_7.rs
git commit -F /tmp/claude-1000/msg.txt
git cat-file commit HEAD | tail -3
```

---

### Task 8: Push + MR

- [ ] **Step 1: Push the branch**

```bash
git push -u origin feat/pressman-coupling-m7
```

- [ ] **Step 2: Open the MR** (lab.frogg.it — glab, never gh; main is protected)

```bash
GITLAB_HOST=lab.frogg.it glab mr create \
  --title "feat(coupling): inheritance rung (Pressman M7)" \
  --description "New CouplingKind::Inheritance — TS/JS class chains with project-local DIT >= inheritance_min_depth (default 2). Collector records class facts (cache v2); depth is a pure metric-time DFS; findings join all_coupling_findings so bands, counts, corroboration, actions, gate ratchet, trend, and hotspot badges pick them up. Design: docs/superpowers/specs/2026-07-10-pressman-coupling-m7-inheritance-design.md" \
  --source-branch feat/pressman-coupling-m7 --target-branch main
```

- [ ] **Step 3: Watch CI** — `GITLAB_HOST=lab.frogg.it glab ci status --live`; the `mutation-gate` job enforces ≥ 80% kill rate on the diff (the band/boundary tests exist for exactly this). Report the MR URL and CI outcome; merging is maintainer-gated — do not merge.

---

## Self-Review (done at authoring time)

- **Spec coverage:** decisions 1–7 → Tasks 1 (TS/JS-only, extraction), 2 (Approach A storage + cache bump), 3 (ladder position, counts, gate-no-Option, history `Option`), 4 (knob + validation), 5 (DIT ≥ 2, per-class findings, M6-seam chaining, metric row), 6 (badges + non-changes), 7 (E2E incl. warm-cache + Rust-silent). Renderer "zero changes" surfaces (actions text, metric rows) are covered by Task 3's baked strings — no renderer task needed for them, matching the spec.
- **Type consistency:** `RawClassRecord/RawBaseRef` (Task 1) consumed by name in Task 2; `ClassRecord/BaseRef` (Task 2) consumed in Task 5's tests; `inheritance_findings` name identical in Tasks 5 (fn) and 3/6 (`HotspotFile` field — different namespaces, both intended); `CouplingFindingCounts.inheritance` used in Tasks 3, 5, 7.
- **Known verify-at-execution points** (flagged inline, with fallbacks): tree-sitter node-kind names (Task 1 Step 3 note), `compute_coupling` call shape in tests (Task 5 note), config test constructor (Task 4 note), metric-row JSON path (Task 7 note → mirror milestone_5).
