# Pressman Coupling Pre-M4 Hygiene Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the reviewer-triaged follow-up ledger from M1–M3: fix seven known detector false negatives/positives, derive the severity-cap triggers from the band table, add an explicit cache schema version, extract the gate's inline finding-set assembly into a testable pure function, and land the small test/config/docs items — before M4 builds on the detectors.

**Architecture:** No new subsystems. Everything modifies existing modules: `src/metrics/complexity/pressman.rs` (detectors), `src/metrics/coupling/mod.rs` (cap derivation), `src/cmd/gate.rs` (pure extraction), `src/cache/storage.rs` (versioned cache), plus config/docs touch-ups. All detector changes stay pure `(path, content) → Vec<CouplingFinding>`.

**Tech Stack:** Rust, tree-sitter (rust + typescript grammars), bincode, clap, serde.

## Global Constraints

- CI runs `RUSTFLAGS=-D warnings cargo test` — warnings are errors. Run tests that way.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt -- --check` must pass (pre-push hook enforces).
- Functional paradigm: pure functions, iterator chains, immutable bindings, `?` for errors (project CLAUDE.md).
- TDD: every behavior change starts with a failing test.
- Commit messages: conventional commits; NEVER mention Claude, AI, or the assistant (no Co-Authored-By).
- Score-band thresholds are defined once in `src/scorer/types.rs` — never re-hardcode 71/41.
- Branch: `fix/pressman-coupling-hygiene` off `main`.

## Explicitly deferred (do NOT do these)

- `trends.js:153` `innerHTML` refactor — ledger says separate branch (security-hook implications).
- ImportGraph type alias, barrel double-scan memoization, `has_detectable_files` recompute, `TimeWindow` trim for baseline walk, `RawValue::NotCollected` — micro perf/style, YAGNI for this MR.
- Tooltip JS string-assert test — brittle, low value.

---

### Task 1: Rust control — `mut` patterns and match-guard branches

Two false negatives in `rust_control` (src/metrics/complexity/pressman.rs:185):
`pub fn f(mut flag: bool)` — the parameter's `pattern` field is a `mut_pattern`
node, so `text(pat)` yields `"mut flag"` and never matches a condition.
`match n { _ if flag => … }` — the guard lives in a `match_pattern` node's
`condition` field, which the branch walk doesn't inspect.

**Files:**
- Modify: `src/metrics/complexity/pressman.rs` (fn `rust_control`, ~lines 185–224; tests module)

**Interfaces:**
- Consumes: existing helpers `text`, `same_scope_descendants`, `contains_word`
- Produces: private helper `fn pattern_identifier<'a>(pat: Node<'_>, content: &'a str) -> &'a str` (used only within `rust_control`)

- [ ] **Step 1: Write the failing tests** (append to the "Rust control coupling" section of the tests module)

```rust
#[test]
fn pub_fn_with_mut_bool_param_branched_is_control_coupling() {
    let src = "pub fn go(mut fast: bool) {\n    if fast {\n        sprint();\n    }\n}\n";
    assert_eq!(
        findings_for("src/a.rs", src).len(),
        1,
        "mut_pattern params must look through the mut"
    );
}

#[test]
fn pub_fn_with_match_guard_on_bool_is_control_coupling() {
    let src = "pub fn pick(fast: bool, n: u32) {\n    match n {\n        _ if fast => sprint(),\n        _ => walk(),\n    }\n}\n";
    assert_eq!(
        findings_for("src/a.rs", src).len(),
        1,
        "guard arms branch on the flag just like if-expressions"
    );
}

#[test]
fn match_guard_on_local_not_param_is_not_flagged() {
    let src = "pub fn pick(fast: bool, n: u32) {\n    let faster = n > 1;\n    match n {\n        _ if faster => sprint(),\n        _ => walk(),\n    }\n    store(fast);\n}\n";
    assert!(
        findings_for("src/a.rs", src).is_empty(),
        "guard on a local must not be attributed to the param"
    );
}
```

- [ ] **Step 2: Run tests to verify the two positives fail**

Run: `cargo test metrics::complexity::pressman -- mut_bool match_guard`
Expected: `pub_fn_with_mut_bool_param_branched_is_control_coupling` and `pub_fn_with_match_guard_on_bool_is_control_coupling` FAIL (0 findings); the negative passes.

- [ ] **Step 3: Implement both fixes**

Add the helper next to `rust_control`:

```rust
/// The identifier text of a parameter pattern, looking through `mut`
/// (`mut flag: bool` parses as a `mut_pattern` wrapping the identifier).
fn pattern_identifier<'a>(pat: Node<'_>, content: &'a str) -> &'a str {
    if pat.kind() == "mut_pattern" {
        (0..pat.child_count())
            .filter_map(|i| pat.child(i as u32))
            .find(|c| c.kind() == "identifier")
            .map(|c| text(c, content))
            .unwrap_or_else(|| text(pat, content))
    } else {
        text(pat, content)
    }
}
```

In `rust_control`, change the pattern extraction:

```rust
        .filter_map(|p| {
            p.child_by_field_name("pattern")
                .map(|pat| pattern_identifier(pat, content))
        })
```

And extend the condition match inside the `branched` walk with a guard arm:

```rust
            let cond = match n.kind() {
                "if_expression" | "while_expression" => n.child_by_field_name("condition"),
                "match_expression" => n.child_by_field_name("value"),
                // `_ if flag => …` — the guard is the match_pattern's condition field
                "match_pattern" => n.child_by_field_name("condition"),
                _ => None,
            };
```

- [ ] **Step 4: Run the module tests**

Run: `cargo test metrics::complexity::pressman`
Expected: all PASS (if the guard test still fails, dump the AST with a scratch `eprintln!("{}", tree.root_node().to_sexp())` in the test to confirm the guard node kind/field for the pinned tree-sitter-rust version, and adjust the arm — the guard is part of `match_pattern` in current grammars).

- [ ] **Step 5: Commit**

```bash
git add src/metrics/complexity/pressman.rs
git commit -m "fix(coupling): detect mut-pattern params and match guards in Rust control"
```

---

### Task 2: Rust common — `lazy_static!` look-through

`lazy_static! { static ref M: Mutex<…> = …; }` parses as a `macro_invocation`
with an opaque token tree — no `static_item` nodes, so the Common detector is
blind to the most classic mutable-global idiom. Apply the look-through rule to
each `static ref` entry's type text (between `:` and `=`), same
`INTERIOR_MUTABILITY` markers and left-boundary rule.

**Files:**
- Modify: `src/metrics/complexity/pressman.rs` (fn `rust_findings` ~line 116; new fn `rust_lazy_static`; tests)

**Interfaces:**
- Consumes: `INTERIOR_MUTABILITY`, `contains_marker_with_left_boundary`, `finding`, `text`
- Produces: private `fn rust_lazy_static(node, content, path) -> Option<CouplingFinding>` — one finding per macro invocation (not per entry; the invocation is the reportable unit and its first line is the evidence)

- [ ] **Step 1: Write the failing tests** (append to "Rust common coupling" section)

```rust
#[test]
fn lazy_static_mutex_is_common_coupling() {
    let src = "lazy_static::lazy_static! {\n    static ref REGISTRY: Mutex<Vec<u32>> = Mutex::new(Vec::new());\n}\n";
    let f = findings_for("src/a.rs", src);
    assert_eq!(f.len(), 1, "lazy_static wrapping Mutex is a mutable global");
    assert_eq!(f[0].kind, CouplingKind::Common);
}

#[test]
fn lazy_static_pure_value_is_not_flagged() {
    let src = "lazy_static::lazy_static! {\n    static ref KEYWORDS: Vec<&'static str> = build_keywords();\n}\n";
    assert!(
        findings_for("src/a.rs", src).is_empty(),
        "write-once pure lazy_static is not common coupling"
    );
}

#[test]
fn lazy_static_marker_in_initializer_only_is_not_flagged() {
    let src = "lazy_static::lazy_static! {\n    static ref N: usize = Cell::new(0).get();\n}\n";
    assert!(
        findings_for("src/a.rs", src).is_empty(),
        "only the type segment (between ':' and '=') is scanned, not the initializer"
    );
}

#[test]
fn other_macros_are_not_flagged() {
    let src = "thread_local! {\n    static FOO: RefCell<u32> = RefCell::new(0);\n}\n";
    assert!(
        findings_for("src/a.rs", src).is_empty(),
        "thread-locals are per-thread, not shared globals — and only lazy_static is matched"
    );
}
```

- [ ] **Step 2: Run tests to verify the positive fails**

Run: `cargo test metrics::complexity::pressman -- lazy_static other_macros`
Expected: `lazy_static_mutex_is_common_coupling` FAILS with 0 findings; the three negatives pass vacuously.

- [ ] **Step 3: Implement**

In `rust_findings`, add the arm:

```rust
            "macro_invocation" => rust_lazy_static(n, content, path),
```

Add the detector (near `rust_common`):

```rust
/// `lazy_static! { static ref X: Mutex<…> = …; }` hides its statics from the
/// `static_item` detector (the macro body is an opaque token tree). Apply the
/// look-through rule to each `static ref` entry's type text — the segment
/// between `:` and `=` — so initializer expressions can't false-positive.
/// One finding per invocation: the macro block is the reportable unit.
fn rust_lazy_static(node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let is_lazy_static = node
        .child_by_field_name("macro")
        .is_some_and(|m| text(m, content).ends_with("lazy_static"));
    if !is_lazy_static {
        return None;
    }
    let hit = text(node, content).split(';').any(|entry| {
        entry.contains("static ref")
            && entry
                .split_once(':')
                .and_then(|(_, rest)| rest.split_once('=').map(|(ty, _)| ty))
                .is_some_and(|ty| {
                    INTERIOR_MUTABILITY
                        .iter()
                        .any(|m| contains_marker_with_left_boundary(ty, m))
                })
    });
    hit.then(|| finding(path, node, CouplingKind::Common, content))
}
```

Note the `ends_with("lazy_static")`: the macro field text is the full path
(`lazy_static::lazy_static`) when invoked qualified, or bare `lazy_static`.
`ends_with` covers both; `thread_local` and everything else fall through.
(If `ends_with` feels loose: `x_lazy_static` would match — acceptable; a
macro with that name shadowing the idiom is not a case worth code.)

- [ ] **Step 4: Run the module tests**

Run: `cargo test metrics::complexity::pressman`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add src/metrics/complexity/pressman.rs
git commit -m "fix(coupling): look through lazy_static macro for interior mutability"
```

---

### Task 3: Flag-use matching — property-access false positive + exact `boolean` type

Two matching bugs, both languages' control detectors:

1. `contains_word` (pressman.rs:100) treats `.` as a boundary, so
   `if (settings.verbose)` matches an unused param named `verbose`
   (same for Rust: `if self.flag` matches param `flag`). Fix: a preceding
   `.` disqualifies the match — `x.flag` is a field/property access, never
   the parameter.
2. `js_control`'s TS branch (pressman.rs:313) does
   `text(annotation).contains("boolean")`, so `flags: boolean[]`,
   `flag: boolean | undefined`, and custom types containing the substring
   all count as flag params. Fix: only an annotation whose type is exactly
   the `predefined_type` `boolean` qualifies (maintainer-visible decision:
   unions/arrays are data shapes, not flags — document in the code).

**Files:**
- Modify: `src/metrics/complexity/pressman.rs` (fns `contains_word`, `js_control`; tests)

**Interfaces:**
- Produces: private `fn annotation_is_exact_boolean(annotation: Node<'_>, content: &str) -> bool`
- Consumes: nothing new

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn property_access_does_not_count_as_flag_use_js() {
    let src = "export function log(verbose: boolean) {\n  if (settings.verbose) {\n    console.debug('x');\n  }\n}\n";
    assert!(
        findings_for("src/l.ts", src).is_empty(),
        "settings.verbose is a property access, not the parameter"
    );
}

#[test]
fn field_access_does_not_count_as_flag_use_rust() {
    let src = "pub fn render(compact: bool, s: &Settings) {\n    if s.compact {\n        short();\n    }\n    store(compact);\n}\n";
    assert!(
        findings_for("src/a.rs", src).is_empty(),
        "s.compact is a field access, not the parameter"
    );
}

#[test]
fn ts_boolean_array_param_is_not_a_flag() {
    let src = "export function f(flags: boolean[]) {\n  if (flags) {\n    a();\n  }\n}\n";
    assert!(
        findings_for("src/f.ts", src).is_empty(),
        "boolean[] is data, not a control flag"
    );
}

#[test]
fn ts_boolean_union_param_is_not_a_flag() {
    let src = "export function f(flag: boolean | undefined) {\n  if (flag) {\n    a();\n  }\n}\n";
    assert!(
        findings_for("src/f.ts", src).is_empty(),
        "only an exact boolean annotation qualifies (documented decision)"
    );
}
```

- [ ] **Step 2: Run tests to verify all four fail**

Run: `cargo test metrics::complexity::pressman -- property_access field_access boolean_array boolean_union`
Expected: all four FAIL (each currently produces 1 finding).

- [ ] **Step 3: Implement both fixes**

`contains_word` — add `.` to the disqualifying set on the left side only:

```rust
/// True when `word` appears in `hay` with non-identifier characters (or
/// string boundaries) on both sides — and not preceded by `.`, which would
/// make it a field/property access (`settings.verbose`), never the parameter.
fn contains_word(hay: &str, word: &str) -> bool {
    hay.match_indices(word).any(|(i, _)| {
        let before_ok = !hay[..i]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.');
        let after_ok = !hay[i + word.len()..]
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '_');
        before_ok && after_ok
    })
}
```

`js_control` — replace the substring check:

```rust
            "required_parameter" | "optional_parameter" => {
                let is_bool = (0..p.child_count())
                    .filter_map(|i| p.child(i as u32))
                    .filter(|c| c.kind() == "type_annotation")
                    .any(|c| annotation_is_exact_boolean(c));
                let pat = p.child_by_field_name("pattern")?;
                (is_bool && pat.kind() == "identifier").then(|| text(pat, content))
            }
```

with the helper (near `js_control`):

```rust
/// True when a `type_annotation`'s type is exactly the predefined `boolean` —
/// not `boolean[]`, not a union, not a look-alike named type. Unions and
/// arrays are data shapes, not control flags (maintainer decision, this MR).
fn annotation_is_exact_boolean(annotation: Node<'_>, content: &str) -> bool {
    (0..annotation.named_child_count())
        .filter_map(|i| annotation.named_child(i as u32))
        .any(|t| t.kind() == "predefined_type" && text(t, content) == "boolean")
}
```

For `boolean[]` the annotation's named child is an `array_type` (whose own
child is the predefined type — deliberately not recursed into); for unions
it's a `union_type`; both fail the top-level check, which is the decision.

- [ ] **Step 4: Run the full module — regressions matter here**

Run: `cargo test metrics::complexity::pressman`
Expected: all PASS, including the existing positives
(`ts_exported_fn_with_branched_boolean_is_control_coupling`,
`pub_fn_with_branched_bool_is_control_coupling`,
`similarly_named_variable_does_not_false_positive`).

- [ ] **Step 5: Commit**

```bash
git add src/metrics/complexity/pressman.rs
git commit -m "fix(coupling): reject property-access and non-exact boolean flag matches"
```

---

### Task 4: JS control — exported arrow/function-expression consts + generator scopes

`export const render = (compact: boolean) => {…}` is the dominant modern
export style and is completely invisible to `js_control` — `js_export`
(pressman.rs:241) only routes `function_declaration` to the control check.
Also `JS_SCOPE_BOUNDARIES` omits generator kinds, so a generator's shadowing
params leak into the enclosing function's walk.

**Files:**
- Modify: `src/metrics/complexity/pressman.rs` (const `JS_SCOPE_BOUNDARIES` ~line 50; fn `js_export` ~line 241; tests)

**Interfaces:**
- Consumes: `js_control` unchanged — arrow/function-expression nodes have the same `parameters`/`body` fields it already reads
- Produces: no new names; `js_export`'s `lexical_declaration` arm restructured

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn ts_exported_arrow_with_branched_boolean_is_control_coupling() {
    let src = "export const render = (compact: boolean) => {\n  if (compact) {\n    short();\n  }\n};\n";
    let f = findings_for("src/r.ts", src);
    assert_eq!(f.len(), 1, "exported arrow functions are exported functions");
    assert_eq!(f[0].kind, CouplingKind::Control);
}

#[test]
fn ts_exported_function_expression_is_control_coupling() {
    let src = "export const render = function (compact: boolean) {\n  if (compact) {\n    short();\n  }\n};\n";
    assert_eq!(findings_for("src/r.ts", src).len(), 1);
}

#[test]
fn ts_exported_const_arrow_without_flags_is_not_flagged() {
    let src = "export const add = (a: number, b: number) => a + b;\n";
    assert!(findings_for("src/a.ts", src).is_empty());
}

#[test]
fn generator_shadowing_bool_param_is_not_flagged() {
    let src = "export function outer(flag: boolean) {\n  function* gen(flag: boolean) {\n    if (flag) {\n      yield 1;\n    }\n  }\n  gen(true);\n}\n";
    assert!(
        findings_for("src/g.ts", src).is_empty(),
        "generator's own shadowed param must not be attributed to outer fn"
    );
}
```

- [ ] **Step 2: Run tests to verify the positives + generator negative fail**

Run: `cargo test metrics::complexity::pressman -- exported_arrow function_expression generator_shadowing const_arrow_without`
Expected: arrow + function-expression tests FAIL (0 findings), generator test FAILS (1 finding), the no-flag negative passes.

- [ ] **Step 3: Implement**

`JS_SCOPE_BOUNDARIES`:

```rust
const JS_SCOPE_BOUNDARIES: &[&str] = &[
    "arrow_function",
    "function_expression",
    "function_declaration",
    "method_definition",
    "generator_function",
    "generator_function_declaration",
];
```

`js_export` — restructure the match:

```rust
fn js_export(node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let decl = node.child_by_field_name("declaration")?;
    match decl.kind() {
        "lexical_declaration" if is_let_declaration(decl, content) => {
            Some(finding(path, node, CouplingKind::Common, content))
        }
        // `export const f = (…) => {…}` — a function export in const
        // clothing; run the control check on each declarator's function value.
        "lexical_declaration" => (0..decl.named_child_count())
            .filter_map(|i| decl.named_child(i as u32))
            .filter(|d| d.kind() == "variable_declarator")
            .filter_map(|d| d.child_by_field_name("value"))
            .filter(|v| matches!(v.kind(), "arrow_function" | "function_expression"))
            .find_map(|v| js_control(v, content, path)),
        "variable_declaration" => Some(finding(path, node, CouplingKind::Common, content)),
        "function_declaration" => js_control(decl, content, path),
        _ => None,
    }
}
```

(Arrow functions with a single unparenthesized param can't carry a type
annotation or default, so `js_control`'s `parameters` field lookup returning
`None` for them is correct, not a gap.)

- [ ] **Step 4: Run the module tests**

Run: `cargo test metrics::complexity::pressman`
Expected: all PASS — especially `ts_export_const_is_not_flagged` (plain const still exempt from Common) and both existing nested-shadowing tests.

- [ ] **Step 5: Commit**

```bash
git add src/metrics/complexity/pressman.rs
git commit -m "fix(coupling): detect exported arrow/function-expression flags, generator scopes"
```

---

### Task 5: JS common — subscript, nested, and augmented global writes

`js_global_write` (pressman.rs:261) only matches
`assignment_expression` with a one-level `member_expression` whose object is
the bare identifier. Misses: `window["x"] = 1` (subscript),
`globalThis.app.state = {}` (nested chain), `window.count += 1` (augmented
assignment — a different node kind entirely).

**Files:**
- Modify: `src/metrics/complexity/pressman.rs` (fns `js_findings` ~line 228, `js_global_write` ~line 261; tests)

**Interfaces:**
- Produces: private `fn member_chain_root(node: Node<'_>) -> Option<Node<'_>>`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn js_subscript_global_write_is_common_coupling() {
    let f = findings_for("src/boot.js", "window[\"cache\"] = new Map();\n");
    assert_eq!(f.len(), 1, "computed-key global writes are still global writes");
    assert_eq!(f[0].kind, CouplingKind::Common);
}

#[test]
fn js_nested_global_write_is_common_coupling() {
    assert_eq!(
        findings_for("src/boot.js", "globalThis.app.state = {};\n").len(),
        1,
        "writing through a chain rooted at globalThis mutates global state"
    );
}

#[test]
fn js_augmented_global_write_is_common_coupling() {
    assert_eq!(
        findings_for("src/boot.js", "window.count += 1;\n").len(),
        1,
        "+= is a write"
    );
}

#[test]
fn js_local_subscript_write_is_not_flagged() {
    assert!(findings_for("src/a.js", "arr[0] = 1;\n").is_empty());
}

#[test]
fn js_reading_global_subscript_is_not_flagged() {
    assert!(findings_for("src/a.js", "const v = window[\"x\"];\n").is_empty());
}
```

- [ ] **Step 2: Run tests to verify the three positives fail**

Run: `cargo test metrics::complexity::pressman -- global_write subscript_write`
Expected: subscript, nested, augmented tests FAIL (0 findings); the two negatives pass.

- [ ] **Step 3: Implement**

`js_findings` — route augmented assignments too:

```rust
            "assignment_expression" | "augmented_assignment_expression" => {
                js_global_write(n, content, path)
            }
```

`js_global_write` + helper:

```rust
/// Assignment (plain or augmented) whose target chain is rooted at
/// `globalThis` / `window` → Common. Covers `window.x = …`,
/// `window["x"] = …`, `globalThis.a.b = …`, `window.count += 1`.
fn js_global_write(node: Node<'_>, content: &str, path: &Path) -> Option<CouplingFinding> {
    let left = node.child_by_field_name("left")?;
    let root = member_chain_root(left)?;
    let is_global =
        root.kind() == "identifier" && matches!(text(root, content), "globalThis" | "window");
    is_global.then(|| finding(path, node, CouplingKind::Common, content))
}

/// Walk `a.b.c` / `a["b"]` chains down to the leftmost object node.
/// Non-chain targets return None — a plain identifier write is not a
/// member write (shadowing `window` itself is a different sin).
fn member_chain_root(node: Node<'_>) -> Option<Node<'_>> {
    match node.kind() {
        "member_expression" | "subscript_expression" => {
            let obj = node.child_by_field_name("object")?;
            match obj.kind() {
                "member_expression" | "subscript_expression" => member_chain_root(obj),
                _ => Some(obj),
            }
        }
        _ => None,
    }
}
```

- [ ] **Step 4: Run the module tests**

Run: `cargo test metrics::complexity::pressman`
Expected: all PASS, including `js_reading_window_is_not_flagged`.

- [ ] **Step 5: Commit**

```bash
git add src/metrics/complexity/pressman.rs
git commit -m "fix(coupling): catch subscript, nested, and augmented global writes"
```

---

### Task 6: Detector unit-test gaps (tests only)

Four triaged gaps from M1 reviews — no production code changes. If any of
these tests FAILS, stop and report; that's a real bug this task just
surfaced, not a reason to change the test.

**Files:**
- Modify: `src/metrics/complexity/pressman.rs` (tests module only)

- [ ] **Step 1: Add the four tests** (each in its matching section)

```rust
#[test]
fn rust_compact_path_attribute_is_content_coupling() {
    let src = "#[path=\"../other/impl.rs\"]\nmod stolen;\n";
    assert_eq!(
        findings_for("src/a.rs", src).len(),
        1,
        "no-space #[path=…] variant must match (whitespace is normalized)"
    );
}

#[test]
fn rust_path_prefixed_attribute_is_not_flagged() {
    let src = "#[path2 = \"x\"]\nmod m;\n";
    assert!(
        findings_for("src/a.rs", src).is_empty(),
        "#[path2=…] must not match the #[path= prefix"
    );
}

#[test]
fn rust_nested_fn_shadowing_bool_param_is_not_flagged() {
    let src = "pub fn outer(flag: bool) {\n    fn inner(flag: bool) {\n        if flag {\n            do_it();\n        }\n    }\n    inner(true);\n}\n";
    assert!(
        findings_for("src/a.rs", src).is_empty(),
        "nested fn's own bool param must not be attributed to the outer fn"
    );
}

#[test]
fn js_class_expression_singleton_is_flagged() {
    let src = "const Db = class {\n  static instance = null;\n};\n";
    assert_eq!(
        findings_for("src/db.js", src).len(),
        1,
        "the bare `class` node kind arm must work, not just class_declaration"
    );
}
```

- [ ] **Step 2: Run them**

Run: `cargo test metrics::complexity::pressman -- compact_path path_prefixed nested_fn_shadowing class_expression`
Expected: all PASS (these pin existing behavior). Any FAIL → stop, report to controller.

- [ ] **Step 3: Commit**

```bash
git add src/metrics/complexity/pressman.rs
git commit -m "test(coupling): pin path-attr variants, nested-fn shadowing, class-expression singleton"
```

---

### Task 7: Severity-cap triggers derived from the band table

`apply_severity_cap` (src/metrics/coupling/mod.rs:463) hardcodes trigger
limits 50 and 25 — duplicating values that already live in `score_pressman`'s
band table ten lines up. Make `score_pressman` a `const fn` and derive the
triggers, so re-tuning the bands can never desynchronize the cap.

**Files:**
- Modify: `src/metrics/coupling/mod.rs` (fns `score_pressman` ~line 313, `apply_severity_cap` ~line 463)
- Modify: `src/metrics/coupling/tests.rs` (one new contract test)

**Interfaces:**
- Produces: `score_pressman` becomes `pub(crate) const fn` (same signature); private consts `CONTENT_CAP_TRIGGER: u32`, `COMMON_CAP_TRIGGER: u32`

- [ ] **Step 1: Write the contract test** (in `src/metrics/coupling/tests.rs`)

```rust
#[test]
fn cap_triggers_track_the_band_table() {
    // A single content finding must trigger the cap; common needs the 4+ band.
    // These assert the *linkage*, not the values — the 16-case band table
    // test pins the values.
    assert_eq!(CONTENT_CAP_TRIGGER, score_pressman(CouplingKind::Content, 1));
    assert_eq!(COMMON_CAP_TRIGGER, score_pressman(CouplingKind::Common, 4));
}
```

Add `CONTENT_CAP_TRIGGER, COMMON_CAP_TRIGGER` to the existing
`use super::…` imports at the top of tests.rs.

- [ ] **Step 2: Run to verify it fails to compile** (consts don't exist yet)

Run: `cargo test metrics::coupling`
Expected: compile error — `CONTENT_CAP_TRIGGER` not found.

- [ ] **Step 3: Implement**

Change the signature (body unchanged — `match` on enums with range patterns
is valid in `const fn` on stable):

```rust
pub(crate) const fn score_pressman(kind: CouplingKind, count: usize) -> u32 {
```

Add below `SEVERITY_CAP`:

```rust
/// Cap triggers derive from the band table itself: Content triggers on any
/// finding (its count-1 band), Common on its 4+ band. Re-tuning
/// `score_pressman` moves the cap automatically — no second edit to forget.
const CONTENT_CAP_TRIGGER: u32 = score_pressman(CouplingKind::Content, 1);
const COMMON_CAP_TRIGGER: u32 = score_pressman(CouplingKind::Common, 4);
```

In `apply_severity_cap`, replace the literals:

```rust
            let limit = match m.name.as_str() {
                "Content coupling" => CONTENT_CAP_TRIGGER,
                "Common coupling" => COMMON_CAP_TRIGGER,
                _ => return false,
            };
```

- [ ] **Step 4: Run the coupling tests — the cap behavior tests must not move**

Run: `cargo test metrics::coupling`
Expected: all PASS (pure refactor plus one new test).

- [ ] **Step 5: Commit**

```bash
git add src/metrics/coupling/mod.rs src/metrics/coupling/tests.rs
git commit -m "refactor(coupling): derive severity-cap triggers from the band table"
```

---

### Task 8: Extract ratchet finding-set assembly into a pure function

The barrel-toggle gating in `run_gate` (src/cmd/gate.rs:107-136) is a 30-line
inline block whose contract ("must mirror `pressman_finding_counts`'s gating
exactly") has no direct test — the M3 triage flagged the toggle-off branch as
untested. Extract it; test both branches with synthetic snapshots.

**Files:**
- Modify: `src/cmd/gate.rs` (fn `run_gate` ~lines 107–136; new pub(crate) fn; tests module)

**Interfaces:**
- Consumes: `coupling::barrel_bypass_findings(snapshot, component_depth)` (already imported), `CouplingThresholds { content_barrel_rule, component_depth, … }`
- Produces: `pub(crate) fn ratchet_finding_sets(coupling_cfg: &crate::config::CouplingThresholds, base: &RepoSnapshot, head: &RepoSnapshot) -> (Vec<CouplingFinding>, Vec<CouplingFinding>)`

- [ ] **Step 1: Write the failing tests** (in gate.rs's tests module; check its existing imports — `RepoSnapshot`, `TimeWindow`, `FileEntry` may need adding)

```rust
fn snapshot_with_barrel_bypass() -> crate::snapshot::RepoSnapshot {
    use crate::snapshot::{FileEntry, RepoSnapshot, TimeWindow};
    use std::path::PathBuf;
    let mut s = RepoSnapshot::new(
        PathBuf::from("/tmp/x"),
        "x".into(),
        "main".into(),
        TimeWindow::default(),
    );
    for p in ["src/a/index.ts", "src/a/impl.ts", "src/b/user.ts"] {
        s.files.push(FileEntry {
            path: PathBuf::from(p),
            size_bytes: 1,
            is_binary: false,
            depth: 3,
            blob_oid: String::new(),
        });
    }
    // Cross-component import that bypasses src/a's barrel.
    s.import_graph.insert(
        PathBuf::from("src/b/user.ts"),
        vec![PathBuf::from("src/a/impl.ts")],
    );
    s
}

#[test]
fn ratchet_sets_include_barrel_findings_when_toggle_on() {
    let cfg = crate::config::RepoConfig::default().thresholds.coupling;
    assert!(cfg.content_barrel_rule, "default toggle must be on");
    let base = crate::snapshot::RepoSnapshot::new(
        std::path::PathBuf::from("/tmp/x"),
        "x".into(),
        "main".into(),
        crate::snapshot::TimeWindow::default(),
    );
    let head = snapshot_with_barrel_bypass();
    let (base_set, head_set) = ratchet_finding_sets(&cfg, &base, &head);
    assert!(base_set.is_empty());
    assert_eq!(head_set.len(), 1, "barrel bypass must join the head set");
    assert!(head_set[0].evidence.contains("barrel"));
}

#[test]
fn ratchet_sets_exclude_barrel_findings_when_toggle_off() {
    let mut cfg = crate::config::RepoConfig::default().thresholds.coupling;
    cfg.content_barrel_rule = false;
    let head = snapshot_with_barrel_bypass();
    let (_, head_set) = ratchet_finding_sets(&cfg, &head.clone(), &head);
    assert!(
        head_set.is_empty(),
        "toggle off: barrel findings must not enter the ratchet diff"
    );
}
```

(If `RepoSnapshot` doesn't derive `Clone`, build the base with a second
`snapshot_with_barrel_bypass()` call instead of `.clone()`.)

- [ ] **Step 2: Run to verify compile failure** (function doesn't exist)

Run: `cargo test cmd::gate`
Expected: compile error — `ratchet_finding_sets` not found.

- [ ] **Step 3: Implement — extract, then replace the inline block**

Add near `resolve_baseline_ref`:

```rust
/// Assemble the base/head finding sets the ratchet diffs. Barrel-bypass
/// findings only join when the toggle is on — this must mirror
/// `pressman_finding_counts`'s gating exactly, or the counts (used for the
/// increase summary) and the finding set (used for the new-finding diff)
/// disagree about what "content coupling" means.
pub(crate) fn ratchet_finding_sets(
    coupling_cfg: &crate::config::CouplingThresholds,
    base: &RepoSnapshot,
    head: &RepoSnapshot,
) -> (Vec<CouplingFinding>, Vec<CouplingFinding>) {
    let with_barrel = |snap: &RepoSnapshot| -> Vec<CouplingFinding> {
        let barrel = if coupling_cfg.content_barrel_rule {
            coupling::barrel_bypass_findings(snap, coupling_cfg.component_depth)
        } else {
            Vec::new()
        };
        snap.coupling_findings.iter().cloned().chain(barrel).collect()
    };
    (with_barrel(base), with_barrel(head))
}
```

In `run_gate`, replace lines 102–136 (the comment + the whole
`let (base_findings, head_findings) … };` block) with:

```rust
        let (base_findings, head_findings) =
            ratchet_finding_sets(&cfg.thresholds.coupling, &base_snapshot, &snapshot);
```

Check gate.rs's `use` lines: `RepoSnapshot` may not be imported at top level
(the inline block used it via the collector's return type). Add what the
compiler asks for; keep paths consistent with the file's existing style.

- [ ] **Step 4: Run gate + milestone tests**

Run: `cargo test cmd::gate && cargo test --test pressman_coupling_milestone_3`
Expected: all PASS (the E2E suite proves the extraction didn't change run_gate behavior).

- [ ] **Step 5: Commit**

```bash
git add src/cmd/gate.rs
git commit -m "refactor(gate): extract ratchet finding-set assembly, test barrel toggle"
```

---

### Task 9: `ast_pass_at` skip-path tests (tests only)

The blob-based AST pass (src/collector/snapshot_builder.rs:370) silently
skips entries with unparseable oids, missing blobs, and non-UTF8 content —
all three `continue`/`else` branches untested (M3 triage).

**Files:**
- Modify: `src/collector/snapshot_builder.rs` (tests module at ~line 408)

- [ ] **Step 1: Write the test**

```rust
#[test]
fn ast_pass_at_skips_bad_oid_missing_blob_and_non_utf8() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    let good = repo.blob(b"static mut CACHE: usize = 0;\n").unwrap();
    let non_utf8 = repo.blob(&[0xff, 0xfe, 0x9f, 0x00]).unwrap();
    let entry = |path: &str, oid: String| FileEntry {
        path: PathBuf::from(path),
        size_bytes: 1,
        is_binary: false,
        depth: 2,
        blob_oid: oid,
    };
    let files = vec![
        entry("src/good.rs", good.to_string()),
        entry("src/bad_oid.rs", "not-a-sha".to_string()),
        // Well-formed oid that exists in no ODB entry:
        entry(
            "src/missing.rs",
            "0123456789abcdef0123456789abcdef01234567".to_string(),
        ),
        entry("src/non_utf8.rs", non_utf8.to_string()),
    ];
    let (metrics, _imports, findings) = ast_pass_at(&repo, &files).unwrap();
    assert_eq!(
        findings.len(),
        1,
        "only the parseable blob contributes findings; the rest skip silently"
    );
    assert!(metrics.contains_key(Path::new("src/good.rs")));
    assert!(!metrics.contains_key(Path::new("src/bad_oid.rs")));
    assert!(!metrics.contains_key(Path::new("src/missing.rs")));
    assert!(!metrics.contains_key(Path::new("src/non_utf8.rs")));
}
```

Match the tests module's existing imports (it likely already has
`tempfile`/`git2`; add `FileEntry`, `Path`, `PathBuf` if missing).

- [ ] **Step 2: Run it**

Run: `cargo test collector::snapshot_builder`
Expected: PASS. If the non-UTF8 blob is classified differently (e.g. findings ≠ 1), inspect which branch actually fired before touching anything — the test documents current skip semantics.

- [ ] **Step 3: Commit**

```bash
git add src/collector/snapshot_builder.rs
git commit -m "test(collector): cover ast_pass_at skip paths"
```

---

### Task 10: Explicit cache schema version + evidence roundtrip assert

The !50 post-merge audit note: mid-struct field additions to `RepoSnapshot`
invalidate old `snapshot.bin` caches only because bincode's positional format
happens to misparse — a future addition that garbage-parses cleanly would
silently serve stale data. Version the cache explicitly. Also add the
missing `evidence` assert to the findings roundtrip test (M1 triage).

**Files:**
- Modify: `src/cache/storage.rs`

**Interfaces:**
- Produces: private `const CACHE_VERSION: u32`; `save`/`load` signatures unchanged

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn load_rejects_mismatched_cache_version() {
    let dir = TempDir::new().unwrap();
    let cache_dir = dir.path().join(CACHE_DIR);
    fs::create_dir_all(&cache_dir).unwrap();
    let stale = bincode::serialize(&(0u32, make_test_snapshot())).unwrap();
    fs::write(cache_dir.join(CACHE_FILE), stale).unwrap();
    assert!(load(dir.path()).unwrap().is_none());
    assert!(
        !cache_dir.join(CACHE_FILE).exists(),
        "stale-version cache must be deleted like a corrupt one"
    );
}

#[test]
fn load_rejects_unversioned_legacy_cache() {
    let dir = TempDir::new().unwrap();
    let cache_dir = dir.path().join(CACHE_DIR);
    fs::create_dir_all(&cache_dir).unwrap();
    let legacy = bincode::serialize(&make_test_snapshot()).unwrap();
    fs::write(cache_dir.join(CACHE_FILE), legacy).unwrap();
    assert!(load(dir.path()).unwrap().is_none());
}
```

And in `snapshot_roundtrips_coupling_findings`, add after the `line` assert:

```rust
        assert_eq!(
            loaded.coupling_findings[0].evidence,
            "static mut CACHE: usize = 0;"
        );
```

- [ ] **Step 2: Run to verify the two new tests fail**

Run: `cargo test cache::storage`
Expected: both version tests FAIL (`load` currently accepts whatever deserializes); roundtrip passes.

- [ ] **Step 3: Implement**

```rust
/// Bumped whenever `RepoSnapshot`'s serialized shape changes. Bincode is
/// positional: a mid-struct field addition can garbage-parse instead of
/// failing, silently serving stale data. The explicit version makes
/// invalidation deterministic. History: 1 = post-M1 shape (coupling_findings).
const CACHE_VERSION: u32 = 1;
```

`save`:

```rust
    let data = bincode::serialize(&(CACHE_VERSION, snapshot))?;
```

`load`:

```rust
    match bincode::deserialize::<(u32, RepoSnapshot)>(&data) {
        Ok((CACHE_VERSION, snapshot)) => Ok(Some(snapshot)),
        // Wrong version or corrupt — delete and re-collect.
        Ok(_) | Err(_) => {
            let _ = fs::remove_file(&cache_file);
            Ok(None)
        }
    }
```

(A legacy unversioned cache reads its first bytes as the version — a
`RepoSnapshot`'s leading path-length u64 low bytes — which only
accidentally equals `CACHE_VERSION` for a path of length 1; the tuple's
snapshot parse then fails anyway. Both guards land in the same arm.)

- [ ] **Step 4: Run the cache tests + one full analyze smoke**

Run: `cargo test cache:: && cargo run --quiet -- analyze . --no-cache > /dev/null && cargo run --quiet -- analyze . > /dev/null`
Expected: tests PASS; the two analyze runs succeed (second exercises save→load of the new format).

- [ ] **Step 5: Commit**

```bash
git add src/cache/storage.rs
git commit -m "fix(cache): version snapshot.bin explicitly, assert evidence roundtrip"
```

---

### Task 11: HistoryCounts write-side serde test + loud milestone-test skip

Two M2 triage leftovers: (a) the `skip_serializing_if` behavior on the three
coupling count fields has no write-side test — a serde attribute typo would
ship `"content_coupling": null` into every history file; (b)
`tests/pressman_coupling_milestone_2.rs:19` silently `return`s when the test
repo can't open, so a broken `BARAD_DUR_TEST_REPO` yields a green test that
tested nothing.

**Files:**
- Modify: `src/scorer/types.rs` (add a tests module if absent — check the end of the file first)
- Modify: `tests/pressman_coupling_milestone_2.rs:19-21`

- [ ] **Step 1: Write the serde tests** (in `src/scorer/types.rs`; if the file has no `#[cfg(test)] mod tests`, add one at the end)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_counts_omit_absent_coupling_fields() {
        let counts = HistoryCounts {
            commits: 1,
            files: 2,
            authors: 3,
            content_coupling: None,
            common_coupling: None,
            control_coupling: None,
        };
        let json = serde_json::to_value(&counts).unwrap();
        assert!(json.get("content_coupling").is_none(), "None must serialize as absent, not null");
        assert!(json.get("common_coupling").is_none());
        assert!(json.get("control_coupling").is_none());
    }

    #[test]
    fn history_counts_serialize_present_coupling_fields() {
        let counts = HistoryCounts {
            commits: 1,
            files: 2,
            authors: 3,
            content_coupling: Some(0),
            common_coupling: Some(4),
            control_coupling: Some(7),
        };
        let json = serde_json::to_value(&counts).unwrap();
        assert_eq!(json["content_coupling"], 0);
        assert_eq!(json["common_coupling"], 4);
        assert_eq!(json["control_coupling"], 7);
    }
}
```

- [ ] **Step 2: Run them**

Run: `cargo test scorer::types`
Expected: PASS (pins existing attributes — a regression guard, not a bug fix).

- [ ] **Step 3: Make the milestone-2 skip loud**

In `tests/pressman_coupling_milestone_2.rs`, replace:

```rust
    let Ok(collector) = Collector::open(&test_repo_path(), TimeWindow::default()) else {
        return;
    };
```

with:

```rust
    let collector = Collector::open(&test_repo_path(), TimeWindow::default())
        .expect("test repo must open — check BARAD_DUR_TEST_REPO");
```

- [ ] **Step 4: Run the milestone suite**

Run: `cargo test --test pressman_coupling_milestone_2`
Expected: PASS (both tests).

- [ ] **Step 5: Commit**

```bash
git add src/scorer/types.rs tests/pressman_coupling_milestone_2.rs
git commit -m "test(scorer): pin HistoryCounts write-side serde, make milestone skip loud"
```

---

### Task 12: Config, CLI, Makefile, and docs batch

Seven small triaged items, none behavioral beyond CLI arg validation.

**Files:**
- Modify: `src/init.rs` (~line 203, after the hygiene block)
- Modify: `src/cli/mod.rs` (~line 139)
- Modify: `src/cmd/gate.rs` (~line 164)
- Modify: `Makefile` (~line 45)
- Modify: `docs/gate-coupling.md`
- Modify: `docs/superpowers/specs/2026-07-02-pressman-coupling-design.md` (~line 124 and the "Resolved design questions" section)

- [ ] **Step 1: init template gains `[thresholds.coupling]`** — in `src/init.rs` after the `[thresholds.hygiene]` lines (~line 203), insert:

```rust
    out.push_str("[thresholds.coupling]\n");
    out.push_str("component_depth           = 2\n");
    out.push_str("change_coupling_min_ratio = 0.30\n");
    out.push_str("content_barrel_rule       = true\n\n");
```

Run: `cargo test init`
Expected: PASS — `generate_toml_is_valid` parses the new section against `CouplingThresholds`'s serde names; if it fails, the field names above are wrong — check `src/config/thresholds.rs`, don't bend the test.

- [ ] **Step 2: ratchet flags become mutually exclusive** — in `src/cli/mod.rs:139`:

```rust
    #[arg(long, requires = "baseline_ref", conflicts_with = "max_new_coupling")]
    pub no_new_coupling: bool,
```

Add next to the existing `gate_ratchet_requires_baseline_ref` test (~line 344, same assertion style — copy its structure):

```rust
#[test]
fn gate_ratchet_flags_conflict() {
    let err = Cli::try_parse_from([
        "barad-dur",
        "gate",
        ".",
        "--no-new-coupling",
        "--max-new-coupling",
        "3",
        "--baseline-ref",
        "origin/main",
    ])
    .unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}
```

Run: `cargo test cli::`
Expected: PASS.

- [ ] **Step 3: friendlier peel error** — in `src/cmd/gate.rs:164`:

```rust
    let commit = obj.peel_to_commit().map_err(|e| {
        anyhow::anyhow!("baseline ref '{r}' does not point at a commit: {e}")
    })?;
```

- [ ] **Step 4: Makefile ratchet target is pure** — in the `gate-coupling` target (~line 45):

```makefile
gate-coupling:
	cargo run --quiet -- gate . --min-score 0 --no-new-coupling --baseline-ref origin/main
```

(`--min-score` defaults to 60; without the override the make target
conflates the score gate with the ratchet.)

- [ ] **Step 5: docs — rename-detection + filtered-runs notes** — in `docs/gate-coupling.md`, read the file first, then add to its caveats/notes section (create a `## Caveats` section at the end if none exists):

```markdown
- **Renames count as new.** Finding identity is `(path, kind, evidence)`;
  the ratchet does not follow git renames. Moving a file with existing
  findings makes them "new at the new path" — pair the move with the fix,
  or use `--max-new-coupling` for the transition MR.
- **Counts ignore `--category` filters.** `coupling_finding_counts` (and the
  history entry counts) derive from the snapshot, not the report's category
  list — an `analyze --category health` run still records coupling counts.
  Deliberate: the counts describe the repository, not the report.
```

- [ ] **Step 6: spec corrections** — in `docs/superpowers/specs/2026-07-02-pressman-coupling-design.md`:

(a) Line ~123-124, fix the M2 lead contradiction. Replace:

```markdown
`content_coupling`, `common_coupling`, `control_coupling`. Every
`analyze`/`backfill` run records them; trend deltas and the dashboard history
```

with:

```markdown
`content_coupling`, `common_coupling`, `control_coupling`. Every `analyze`
run records them; trend deltas and the dashboard history
```

(b) In the "Resolved design questions" section, append:

```markdown
7. **`pub(crate)` counts as public** (recorded 2026-07-06, pre-M4 hygiene).
   `rust_control` treats any `visibility_modifier` — including `pub(crate)` —
   as exported. Rationale: control coupling is inter-module, and
   `pub(crate)` items are exactly the cross-module surface inside a crate.
   Only truly private (no modifier) functions are exempt.
8. **Exact `boolean` only for TS flag params** (recorded 2026-07-06).
   `boolean[]`, unions (`boolean | undefined`), and look-alike named types
   are data shapes, not flags. Optional params (`flag?: boolean`) still
   qualify — the annotation itself is exactly `boolean`.
```

- [ ] **Step 7: full verification sweep**

Run: `RUSTFLAGS=-D warnings cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check`
Expected: everything green.

- [ ] **Step 8: Commit**

```bash
git add src/init.rs src/cli/mod.rs src/cmd/gate.rs Makefile docs/gate-coupling.md docs/superpowers/specs/2026-07-02-pressman-coupling-design.md
git commit -m "chore(gate): init coupling thresholds, flag conflict, docs and spec notes"
```

---

## Final verification (after all tasks)

- [ ] `RUSTFLAGS=-D warnings cargo test` — full suite green
- [ ] `cargo clippy --all-targets -- -D warnings && cargo fmt -- --check`
- [ ] Dogfood: `cargo run --quiet -- analyze . --no-cache -v` — inspect the Coupling category. The new detectors may surface REAL findings in this repo (e.g. property-access FPs disappearing, new lazy_static/global hits appearing). New findings in barad-dur's own code are a report to the controller, not necessarily a blocker — M1 precedent: real findings got fixed in a separate authorized commit.
- [ ] `make gate-coupling` (after the Makefile change) passes against `origin/main` — expect it to FAIL if the detector fixes changed finding counts vs the baseline (the baseline is collected with the NEW detectors via blob AST, so counts should actually match — if it fails, read the diff it prints and report).
