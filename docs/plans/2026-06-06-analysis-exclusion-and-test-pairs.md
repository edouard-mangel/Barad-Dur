# Analysis Exclusion Expansion + Test Pair Indicator Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Expand default file exclusions to cover generated files and build directories, and flag test↔production file pairs in the temporal coupling tab.

**Architecture:** Feature 1 is a pure expansion of two constants in `src/collector/exclude.rs`. Feature 2 adds a `bool` field to `CouplingPair`, computes it in `build_coupling_pairs()`, and renders a badge in the HTML coupling tab.

**Tech Stack:** Rust, `glob_match` crate (already used), vanilla JS string in `js_coupling.rs`.

**Working directory:** `/home/edouard/WS/barad-dur/.worktrees/afferent-efferent-coupling`

---

## Task 1: Expand default excluded extensions

**Files:**
- Modify: `src/collector/exclude.rs` — `DEFAULT_EXCLUDE_EXTENSIONS` constant (lines 5–10)
- Test: `src/collector/exclude.rs` — `#[cfg(test)]` block at bottom

**Step 1: Write the failing tests**

Add to the test module in `src/collector/exclude.rs`:

```rust
#[test]
fn is_excluded_matches_generated_extensions() {
    // Protocol Buffers
    assert!(is_excluded(Path::new("proto/user.pb.go"), &[], &[], true));
    assert!(is_excluded(Path::new("proto/user.pb.h"), &[], &[], true));
    assert!(is_excluded(Path::new("proto/user.pb.cc"), &[], &[], true));
    assert!(is_excluded(Path::new("proto/user.pb.swift"), &[], &[], true));
    assert!(is_excluded(Path::new("proto/user_pb2.py"), &[], &[], true));
    // C# generated
    assert!(is_excluded(Path::new("src/Api/Client.g.cs"), &[], &[], true));
    assert!(is_excluded(Path::new("src/Api/Client.generated.cs"), &[], &[], true));
    // TypeScript declarations
    assert!(is_excluded(Path::new("types/index.d.ts"), &[], &[], true));
    // Minified assets
    assert!(is_excluded(Path::new("dist/app.min.js"), &[], &[], true));
    assert!(is_excluded(Path::new("dist/styles.min.css"), &[], &[], true));
    // Regular source should still pass
    assert!(!is_excluded(Path::new("src/main.rs"), &[], &[], true));
    assert!(!is_excluded(Path::new("src/user.go"), &[], &[], true));
    assert!(!is_excluded(Path::new("src/client.ts"), &[], &[], true));
}
```

**Step 2: Run to confirm failure**

```bash
RUSTFLAGS="-D warnings" cargo test is_excluded_matches_generated_extensions 2>&1 | tail -5
```
Expected: `FAILED` — extensions not yet in the list.

**Step 3: Expand `DEFAULT_EXCLUDE_EXTENSIONS`**

In `src/collector/exclude.rs`, add to the array:

```rust
const DEFAULT_EXCLUDE_EXTENSIONS: &[&str] = &[
    // Translation / resource files
    "resx", "po", "pot", "xlf", "xliff", "strings", "arb", "lproj",
    // Documentation files
    "md", "txt", "rst", "adoc", "textile",
    // Protocol Buffers generated
    "pb.go", "pb.h", "pb.cc", "pb.swift",
    // Python protobuf generated (compound: ends with "_pb2.py" — handled as pattern below)
    // C# generated
    "g.cs", "generated.cs",
    // TypeScript declarations
    "d.ts",
    // Minified assets
    "min.js", "min.css",
];
```

Note: `_pb2.py` is a suffix pattern, not a plain extension — add it to `DEFAULT_EXCLUDE_PATTERNS` instead (next step covers this).

**Step 4: Run and confirm pass**

```bash
RUSTFLAGS="-D warnings" cargo test is_excluded_matches_generated_extensions 2>&1 | tail -5
```
Expected: `ok`

**Step 5: Commit**

```bash
git add src/collector/exclude.rs
git commit -m "feat(exclude): add generated file extensions to defaults"
```

---

## Task 2: Expand default excluded path patterns

**Files:**
- Modify: `src/collector/exclude.rs` — `DEFAULT_EXCLUDE_PATTERNS` constant (lines 14–47)
- Test: `src/collector/exclude.rs` — test module

**Step 1: Write the failing tests**

```rust
#[test]
fn is_excluded_matches_generated_directories() {
    assert!(is_excluded(Path::new("node_modules/lodash/index.js"), &[], &[], true));
    assert!(is_excluded(Path::new("vendor/github.com/foo/bar.go"), &[], &[], true));
    assert!(is_excluded(Path::new("src/__pycache__/utils.cpython-311.pyc"), &[], &[], true));
    assert!(is_excluded(Path::new("myapp.egg-info/PKG-INFO"), &[], &[], true));
    assert!(is_excluded(Path::new("target/debug/build/barad-dur/out/main.rs"), &[], &[], true));
    assert!(is_excluded(Path::new(".next/server/pages/index.js"), &[], &[], true));
    assert!(is_excluded(Path::new(".nuxt/components.d.ts"), &[], &[], true));
    assert!(is_excluded(Path::new("out/Release/chrome"), &[], &[], true));
    assert!(is_excluded(Path::new("src/gen/proto/user.go"), &[], &[], true));
    assert!(is_excluded(Path::new("src/generated/api/client.ts"), &[], &[], true));
    assert!(is_excluded(Path::new(".gradle/caches/foo"), &[], &[], true));
    assert!(is_excluded(Path::new(".mvn/wrapper/maven-wrapper.jar"), &[], &[], true));
    assert!(is_excluded(Path::new("build/outputs/apk/debug.apk"), &[], &[], true));
    assert!(is_excluded(Path::new("proto/user_pb2.py"), &[], &[], true));
    // Regular source dirs must NOT be excluded
    assert!(!is_excluded(Path::new("src/main.rs"), &[], &[], true));
    assert!(!is_excluded(Path::new("dist/published.js"), &[], &[], true)); // dist intentionally allowed
}
```

**Step 2: Run to confirm failure**

```bash
RUSTFLAGS="-D warnings" cargo test is_excluded_matches_generated_directories 2>&1 | tail -5
```
Expected: `FAILED`

**Step 3: Expand `DEFAULT_EXCLUDE_PATTERNS`**

Append to the existing patterns array in `src/collector/exclude.rs`:

```rust
    // Generated build artefact directories
    "**/node_modules/**",
    "**/vendor/**",
    "**/__pycache__/**",
    "**/*.egg-info/**",
    "**/target/**",
    "**/.next/**",
    "**/.nuxt/**",
    "**/out/**",
    "**/gen/**",
    "**/generated/**",
    "**/.gradle/**",
    "**/.mvn/**",
    "**/build/**",
    // Python protobuf generated files (compound suffix, not plain extension)
    "**/*_pb2.py",
```

**Step 4: Run and confirm pass**

```bash
RUSTFLAGS="-D warnings" cargo test is_excluded_matches_generated_directories 2>&1 | tail -5
```
Expected: `ok`

**Step 5: Run full suite — check no regressions**

```bash
RUSTFLAGS="-D warnings" cargo test 2>&1 | grep -E "^test result"
```
Expected: all `ok`

**Step 6: Commit**

```bash
git add src/collector/exclude.rs
git commit -m "feat(exclude): add generated directories and pb2 pattern to defaults"
```

---

## Task 3: Add `is_test_pair` field to `CouplingPair`

**Files:**
- Modify: `src/scorer/types.rs` — `CouplingPair` struct (line 21)
- Modify: `src/scorer/builders.rs` — `build_coupling_pairs()` construction (line 103)

**Step 1: Write the failing test**

In `src/scorer/builders.rs` test module:

```rust
#[test]
fn coupling_pair_is_test_pair_field_defaults_false() {
    // build_coupling_pairs with a non-test pair must set is_test_pair = false
    let mut snapshot = RepoSnapshot::default();
    // Use the test helpers already present to build a minimal snapshot
    // with one pair: src/foo.rs <-> src/bar.rs (not a test pair)
    // Then assert is_test_pair == false on the result
    // (exact snapshot construction follows existing test patterns in this file)
}
```

Look at existing tests in `src/scorer/builders.rs` for the snapshot construction pattern, then write a test that asserts `pair.is_test_pair == false` for a non-test pair.

**Step 2: Run to confirm compile failure**

```bash
RUSTFLAGS="-D warnings" cargo test coupling_pair_is_test_pair 2>&1 | tail -10
```
Expected: compile error — `is_test_pair` field doesn't exist yet.

**Step 3: Add the field to `CouplingPair`**

In `src/scorer/types.rs`:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct CouplingPair {
    pub file_a: String,
    pub file_b: String,
    pub co_changes: usize,
    pub coupling_pct: f64,
    pub cross_boundary: bool,
    pub is_test_pair: bool,   // ← add this
}
```

**Step 4: Fix all construction sites**

Search for all places `CouplingPair {` is constructed:

```bash
grep -rn "CouplingPair {" src/
```

For each one, add `is_test_pair: false` as a placeholder (the real computation comes in Task 4). There should be at least one in `src/scorer/builders.rs` around line 103.

**Step 5: Run and confirm tests pass**

```bash
RUSTFLAGS="-D warnings" cargo test 2>&1 | grep -E "^test result"
```
Expected: all `ok`

**Step 6: Commit**

```bash
git add src/scorer/types.rs src/scorer/builders.rs
git commit -m "feat(scorer): add is_test_pair field to CouplingPair"
```

---

## Task 4: Implement `is_test_pair()` detection

**Files:**
- Modify: `src/scorer/builders.rs` — add `is_test_pair()` function and wire it

**Step 1: Write the failing tests**

Add to `src/scorer/builders.rs` test module:

```rust
#[test]
fn is_test_pair_detects_suffix_test() {
    assert!(is_test_pair("src/UserService.java", "tests/UserServiceTest.java"));
    assert!(is_test_pair("src/UserService.java", "tests/UserServiceTests.java"));
    // symmetric
    assert!(is_test_pair("tests/UserServiceTest.java", "src/UserService.java"));
}

#[test]
fn is_test_pair_detects_dot_test_spec() {
    assert!(is_test_pair("src/parser.ts", "src/parser.test.ts"));
    assert!(is_test_pair("src/parser.ts", "src/parser.spec.ts"));
    assert!(is_test_pair("src/parser.test.ts", "src/parser.ts"));
}

#[test]
fn is_test_pair_detects_underscore_test_spec() {
    assert!(is_test_pair("user.go", "user_test.go"));
    assert!(is_test_pair("user.go", "user_spec.go"));
    assert!(is_test_pair("user_test.go", "user.go"));
}

#[test]
fn is_test_pair_detects_test_prefix() {
    assert!(is_test_pair("user.py", "test_user.py"));
    assert!(is_test_pair("test_user.py", "user.py"));
}

#[test]
fn is_test_pair_case_insensitive() {
    assert!(is_test_pair("UserService.cs", "USERSERVICETEST.cs"));
}

#[test]
fn is_test_pair_rejects_unrelated_pairs() {
    assert!(!is_test_pair("src/user.rs", "src/order.rs"));
    assert!(!is_test_pair("src/user.rs", "src/user_handler.rs"));
}
```

**Step 2: Run to confirm failure**

```bash
RUSTFLAGS="-D warnings" cargo test is_test_pair 2>&1 | tail -10
```
Expected: compile error — `is_test_pair` function not defined.

**Step 3: Implement `is_test_pair()`**

Add to `src/scorer/builders.rs` (before `build_coupling_pairs`):

```rust
fn is_test_pair(a: &str, b: &str) -> bool {
    let stem_a = file_stem(a).to_lowercase();
    let stem_b = file_stem(b).to_lowercase();
    is_test_of(&stem_a, &stem_b) || is_test_of(&stem_b, &stem_a)
}

/// Returns true if `test_stem` looks like a test file for `prod_stem`.
fn is_test_of(prod_stem: &str, test_stem: &str) -> bool {
    test_stem == format!("{}test", prod_stem)
        || test_stem == format!("{}tests", prod_stem)
        || test_stem == format!("{}.test", prod_stem)
        || test_stem == format!("{}.spec", prod_stem)
        || test_stem == format!("{}_test", prod_stem)
        || test_stem == format!("{}_spec", prod_stem)
        || test_stem == format!("test_{}", prod_stem)
}

/// Extract the full filename stem (everything before the first dot in the filename).
fn file_stem(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    // Take everything up to the first '.' to handle compound extensions like .test.ts
    name.split('.').next().unwrap_or(name).to_string()
}
```

**Step 4: Wire into `build_coupling_pairs()`**

In `build_coupling_pairs()`, update the `CouplingPair` construction:

```rust
CouplingPair {
    file_a: a.to_string_lossy().to_string(),
    file_b: b.to_string_lossy().to_string(),
    co_changes: *co,
    coupling_pct,
    cross_boundary,
    is_test_pair: is_test_pair(
        &a.to_string_lossy(),
        &b.to_string_lossy(),
    ),
}
```

**Step 5: Run and confirm all pass**

```bash
RUSTFLAGS="-D warnings" cargo test 2>&1 | grep -E "^test result"
```
Expected: all `ok`

**Step 6: Commit**

```bash
git add src/scorer/builders.rs
git commit -m "feat(scorer): implement is_test_pair detection for coupling pairs"
```

---

## Task 5: Render `🧪` badge in HTML coupling tab

**Files:**
- Modify: `src/renderer/html/js_coupling.rs` — add badge after `cross_boundary` badge
- Test: `src/renderer/html/tests_extra.rs` — add two tests

**Step 1: Write the failing tests**

Add to `src/renderer/html/tests_extra.rs`:

```rust
#[test]
fn coupling_tab_shows_test_pair_badge_when_is_test_pair() {
    let mut report = make_report();
    report.coupling_pairs = vec![CouplingPair {
        file_a: "src/user.rs".into(),
        file_b: "src/user_test.rs".into(),
        co_changes: 10,
        coupling_pct: 80.0,
        cross_boundary: false,
        is_test_pair: true,
    }];
    let html = render(&report).unwrap();
    assert!(
        html.contains("\u{1f9ea}"),  // 🧪 emoji
        "coupling tab must show 🧪 badge for test pairs"
    );
    assert!(
        html.contains("Expected coupling"),
        "badge must have tooltip explaining expected coupling"
    );
}

#[test]
fn coupling_tab_no_test_pair_badge_for_regular_pairs() {
    let mut report = make_report();
    report.coupling_pairs = vec![CouplingPair {
        file_a: "src/user.rs".into(),
        file_b: "src/order.rs".into(),
        co_changes: 5,
        coupling_pct: 60.0,
        cross_boundary: false,
        is_test_pair: false,
    }];
    let html = render(&report).unwrap();
    assert!(
        !html.contains("Expected coupling"),
        "no test pair badge for regular coupling pairs"
    );
}
```

Note: you'll need to import `CouplingPair` at the top of `tests_extra.rs` — check existing imports in that file for the pattern.

**Step 2: Run to confirm failure**

```bash
RUSTFLAGS="-D warnings" cargo test coupling_tab_shows_test_pair_badge 2>&1 | tail -5
```
Expected: `FAILED` — badge not rendered yet.

**Step 3: Add badge rendering to `js_coupling.rs`**

In `src/renderer/html/js_coupling.rs`, find the `cross_boundary` badge block (around line 142–147) and add the test pair badge immediately after it:

```javascript
if (p.is_test_pair) {
  var tpBadge = el('span', { title: 'Expected coupling — production file and its test file naturally change together.', style: { marginLeft: '4px', cursor: 'default' } });
  tpBadge.append(txt('🧪'));
  cbCell.append(tpBadge);
}
```

In Rust string syntax (inside the existing JS string in `js_coupling.rs`), this will be written as a raw string segment consistent with how `cross_boundary` is written in that file.

**Step 4: Run and confirm all pass**

```bash
RUSTFLAGS="-D warnings" cargo test 2>&1 | grep -E "^test result"
```
Expected: all `ok`

**Step 5: Commit**

```bash
git add src/renderer/html/js_coupling.rs src/renderer/html/tests_extra.rs
git commit -m "feat(html): show test pair badge in coupling tab"
```

---

## Task 6: Push and open MR

```bash
git push -u origin feat/afferent-efferent-coupling
```

Then open MR targeting `main` on lab.frogg.it.

---

## Running all tests (reference)

```bash
RUSTFLAGS="-D warnings" cargo test
```

All test suites must be green before each commit.
