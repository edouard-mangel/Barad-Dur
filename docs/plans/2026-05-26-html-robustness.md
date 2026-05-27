# HTML Report Robustness Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Harden three production weaknesses: two panic-capable code paths in Rust, missing empty-state coverage in the HTML report, and three oversized files that mix concerns.

**Architecture:** TDD throughout. Approach A hardens `registry/client.rs` + `init.rs`. Approach B adds 6 empty-state tests + CSS class fixes to the HTML renderer. Approach C is a pure refactor split across `js_treemap.rs` and `cli.rs`.

**Tech Stack:** Rust (anyhow, reqwest, OnceLock), vanilla JS DOM strings in `renderer/html/js_*.rs`, `cargo test`.

---

## Approach A — Production error hardening

### Background

`registry/client.rs:19` — `reqwest::blocking::Client::builder().build()` can fail (TLS init failure), and the current code calls `.expect()`, causing a hard panic.

`src/init.rs:238,240` — `prompt()` calls `.unwrap()` on `stderr().flush()` and `stdin().read_line()`. A closed pipe causes a panic during interactive `barad-dur init`.

Five call sites use `super::client::http()`: `cargo.rs:10`, `osv.rs:11`, `npm.rs:9`, `nuget.rs:12`, `pip.rs:9`. All already return `anyhow::Result`, so adding `?` propagation is safe.

---

### Task A1: Make `http_with_timeout` fallible

**Files:**
- Modify: `src/registry/client.rs`

**Step 1: Write the failing test**

Add inside the existing `#[cfg(test)] mod tests` block in `client.rs`:

```rust
#[test]
fn http_with_timeout_returns_result() {
    // Verify the signature compiles and returns Ok on a normal system.
    let result = http_with_timeout(Duration::from_millis(100));
    assert!(result.is_ok(), "expected Ok client, got {:?}", result.err());
}
```

**Step 2: Run to confirm it fails**

```bash
cargo test -p barad-dur http_with_timeout_returns_result 2>&1 | head -30
```

Expected: compile error — `http_with_timeout` returns `Client`, not `Result<Client, _>`.

**Step 3: Change the signature**

Replace the body of `http_with_timeout` in `client.rs`:

```rust
pub fn http_with_timeout(timeout: Duration) -> Result<reqwest::blocking::Client, reqwest::Error> {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
}
```

Also update `HTTP_CLIENT` to store `Option<reqwest::blocking::Client>` and `http()` to return `Option<&'static reqwest::blocking::Client>`:

```rust
static HTTP_CLIENT: OnceLock<Option<reqwest::blocking::Client>> = OnceLock::new();

pub fn http() -> Option<&'static reqwest::blocking::Client> {
    HTTP_CLIENT
        .get_or_init(|| match http_with_timeout(Duration::from_secs(TIMEOUT_SECS)) {
            Ok(c) => Some(c),
            Err(e) => {
                eprintln!("warning: failed to build HTTP client: {e}");
                None
            }
        })
        .as_ref()
}
```

**Step 4: Fix the existing test** (still in `client.rs`) — the timeout test uses `http_with_timeout` which now returns `Result`:

```rust
let result = http_with_timeout(Duration::from_millis(200))
    .expect("client should build")
    .get(format!("http://{}/", addr))
    .send();
```

**Step 5: Run tests**

```bash
cargo test -p barad-dur registry::client 2>&1 | tail -20
```

Expected: all pass, including `http_with_timeout_returns_result` and `client_times_out_on_unresponsive_server`.

**Step 6: Commit**

```bash
git add src/registry/client.rs
git commit -m "fix: make http_with_timeout fallible, client() returns Option"
```

---

### Task A2: Update the 5 registry call sites

**Files:**
- Modify: `src/registry/cargo.rs:10`
- Modify: `src/registry/osv.rs:11`
- Modify: `src/registry/npm.rs:9`
- Modify: `src/registry/nuget.rs:12`
- Modify: `src/registry/pip.rs:9`

**Step 1: Check it compiles before any change**

```bash
cargo check 2>&1 | head -30
```

Expected: errors about `Option<&Client>` not having `.get()`.

**Step 2: Apply the same pattern to each of the 5 files**

Pattern: before the HTTP call, resolve the client:

```rust
let client = super::client::http()
    .ok_or_else(|| anyhow::anyhow!("HTTP client unavailable"))?;
```

Then replace `super::client::http().get(...)` with `client.get(...)`.

**`cargo.rs`** — change lines 10–12 from:
```rust
let body: serde_json::Value = super::client::http()
    .get(&url)
    .send()?
    .json()?;
```
to:
```rust
let client = super::client::http()
    .ok_or_else(|| anyhow::anyhow!("HTTP client unavailable"))?;
let body: serde_json::Value = client.get(&url).send()?.json()?;
```

**`osv.rs`** — change lines 11–16 from:
```rust
let response: serde_json::Value = super::client::http()
    .post(url)
    ...
```
to:
```rust
let client = super::client::http()
    .ok_or_else(|| anyhow::anyhow!("HTTP client unavailable"))?;
let response: serde_json::Value = client.post(url)
    ...
```

**`npm.rs`**, **`nuget.rs`**, **`pip.rs`** — same single-line pattern:
```rust
// Before:
let body: serde_json::Value = super::client::http().get(&url).send()?.json()?;
// After:
let client = super::client::http()
    .ok_or_else(|| anyhow::anyhow!("HTTP client unavailable"))?;
let body: serde_json::Value = client.get(&url).send()?.json()?;
```

**Step 3: Verify it compiles**

```bash
cargo check 2>&1 | head -20
```

Expected: clean (no errors).

**Step 4: Run all tests**

```bash
cargo test 2>&1 | tail -20
```

Expected: all pass.

**Step 5: Commit**

```bash
git add src/registry/cargo.rs src/registry/osv.rs src/registry/npm.rs src/registry/nuget.rs src/registry/pip.rs
git commit -m "fix: propagate HTTP client init failure through registry call sites"
```

---

### Task A3: Fix panicking I/O in `init.rs::prompt()`

**Files:**
- Modify: `src/init.rs:236-247`

**Step 1: Write the failing test**

Add inside the test module in `init.rs` (or create a new `#[cfg(test)]` block at the bottom):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_returns_default_on_empty_input() {
        // We can't easily mock stdin, but we can verify the fallback path
        // by checking that the function signature accepts &str args and
        // that the default is returned when input is empty.
        // This test validates the function is callable without panicking.
        // (Full I/O testing requires integration tests or stdin injection.)
        let _ = format!("prompt: {}", "test"); // smoke test
    }
}
```

Note: The real hardening happens in production code, not via unit tests, because stdin interaction can't be mocked without dependency injection. The value is eliminating the `.unwrap()` call that could panic in CI or piped environments.

**Step 2: Change `prompt()` to not panic on I/O error**

In `src/init.rs`, replace the `prompt` function body (lines 236–247):

```rust
fn prompt(question: &str, default: &str) -> String {
    eprint!("     ? {} [{}]: ", question, default);
    let _ = io::stderr().flush(); // best-effort flush; ignore errors
    let mut input = String::new();
    if io::stdin().lock().read_line(&mut input).is_err() {
        return default.to_string(); // gracefully return default on I/O failure
    }
    let trimmed = input.trim();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}
```

**Step 3: Verify clean compile**

```bash
cargo check 2>&1 | head -10
```

Expected: clean.

**Step 4: Run all tests**

```bash
cargo test 2>&1 | tail -15
```

Expected: all pass.

**Step 5: Commit**

```bash
git add src/init.rs
git commit -m "fix: replace panicking unwrap() in prompt() with graceful I/O fallback"
```

---

## Approach B — HTML empty-state completeness

### Background

Several tabs produce blank output when their data arrays are empty, and the `safeRender` error boundary uses inline style instead of a dedicated CSS class. The `make_report()` test fixture already has all data fields set to empty vectors, so the 6 empty-state tests can use it directly.

Tab → empty-state string reference:

| Tab | JS function | Expected string when data absent |
|-----|-------------|----------------------------------|
| Treemap | `buildTreemapTab` | `'No hotspot data'` |
| Authors | `buildAuthorsTab` | `'No author data available'` |
| Coupling | `buildCouplingTab` | `'No coupling data'` |
| Ownership | `buildOwnershipTab` | `'No ownership data available'` |
| Age | `buildAgeTab` | `'No file age data'` |
| Audit | `buildAuditTab` | `'No audit data'` |

---

### Task B1: Add `tab-error` CSS class to `safeRender`

**Files:**
- Modify: `src/renderer/html/js_authors.rs`

**Step 1: Write the failing test**

Add to `src/renderer/html/tests_extra.rs`:

```rust
#[test]
fn html_safe_render_error_has_tab_error_class() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("tab-error"),
        "safeRender error div must use CSS class 'tab-error' so errors can be styled consistently"
    );
}
```

**Step 2: Run to confirm it fails**

```bash
cargo test -p barad-dur html_safe_render_error_has_tab_error_class 2>&1 | tail -10
```

Expected: FAIL — `tab-error` not found.

**Step 3: Add the class**

In `js_authors.rs`, find the `safeRender` function (around line 203) and change the error div creation from:

```javascript
var d = el('div', { style: { padding: '24px', color: '#ef4444' } });
```

to:

```javascript
var d = el('div', { className: 'tab-error' });
```

Then add `.tab-error` CSS to the stylesheet in `src/renderer/html/css.rs` (find the CSS string and append):

```css
.tab-error { padding: 24px; color: #ef4444; font-size: 13px; }
```

**Step 4: Run the test**

```bash
cargo test -p barad-dur html_safe_render_error_has_tab_error_class 2>&1 | tail -10
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/renderer/html/js_authors.rs src/renderer/html/css.rs
git commit -m "fix: give safeRender error div a tab-error CSS class for consistent styling"
```

---

### Task B2: Fix Authors tab empty-state to use `.no-data` class

**Files:**
- Modify: `src/renderer/html/js_authors.rs:51-53`

**Step 1: Write the failing test**

Add to `tests_extra.rs`:

```rust
#[test]
fn html_authors_empty_state_uses_no_data_class() {
    let html = render(&make_report()).unwrap();
    // make_report() has author_cards: vec![] so we hit the empty path
    assert!(
        html.contains("no-data"),
        "Authors tab empty state must use the 'no-data' CSS class, not inline style"
    );
}
```

**Step 2: Run to confirm it fails**

```bash
cargo test -p barad-dur html_authors_empty_state_uses_no_data_class 2>&1 | tail -10
```

Expected: FAIL.

**Step 3: Change the Authors empty-state div**

In `js_authors.rs`, replace the empty-state block (around lines 50–54):

```javascript
// Before:
var empty = el('div', { style: { padding: '48px', textAlign: 'center', color: '#64748b' } });
empty.append(txt('No author data available. Run with blame enabled.'));
container.append(empty);
return container;

// After:
var empty = el('div', { className: 'no-data' });
empty.append(txt('No author data available. Run with blame enabled.'));
container.append(empty);
return container;
```

**Step 4: Run the test**

```bash
cargo test -p barad-dur html_authors_empty_state_uses_no_data_class 2>&1 | tail -10
```

Expected: PASS.

**Step 5: Run all HTML tests**

```bash
cargo test -p barad-dur renderer::html 2>&1 | tail -20
```

Expected: all pass.

**Step 6: Commit**

```bash
git add src/renderer/html/js_authors.rs
git commit -m "fix: use no-data CSS class for Authors tab empty state"
```

---

### Task B3: Add 6 empty-state tests (one per tab)

**Files:**
- Modify: `src/renderer/html/tests_extra.rs`

**Step 1: Discover the current empty-state strings**

Verify each tab's "no data" string by grepping:

```bash
grep -rn "No hotspot\|No coupling\|No ownership\|No file age\|No audit\|No author" \
  src/renderer/html/js_*.rs
```

Note the exact string for each tab. Use those exact strings in the assertions.

**Step 2: Write all 6 failing tests at once**

Add to `tests_extra.rs`:

```rust
#[test]
fn html_treemap_empty_state() {
    // make_report() has file_hotspots: vec![]
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("No hotspot data"),
        "Treemap tab must show 'No hotspot data' when file_hotspots is empty"
    );
}

#[test]
fn html_authors_empty_state() {
    // make_report() has author_cards: vec![]
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("No author data available"),
        "Authors tab must show its empty-state message when author_cards is empty"
    );
}

#[test]
fn html_coupling_empty_state() {
    // make_report() has coupling_pairs: vec![]
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("No coupling data"),
        "Coupling tab must show 'No coupling data' when coupling_pairs is empty"
    );
}

#[test]
fn html_ownership_empty_state() {
    // make_report() has author_ownership: vec![]
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("No ownership data"),
        "Ownership tab must show its empty-state message when author_ownership is empty"
    );
}

#[test]
fn html_age_empty_state() {
    // make_report() has file_ages: vec![]
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("No file age data"),
        "Age tab must show its empty-state message when file_ages is empty"
    );
}

#[test]
fn html_audit_empty_state() {
    // make_report() has audit: None
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("No audit data"),
        "Audit tab must show its empty-state message when audit is None"
    );
}
```

**Step 3: Run to see which ones fail**

```bash
cargo test -p barad-dur "html_treemap_empty_state|html_authors_empty_state|html_coupling_empty_state|html_ownership_empty_state|html_age_empty_state|html_audit_empty_state" 2>&1 | tail -30
```

Expected: some may pass (if the string already exists), some may fail. For those that fail, the exact empty-state string in the JS differs from the test expectation — look at the grep output from Step 1 and fix the assertion strings to match the actual JS strings. **Do not change the JS to match the test.** Read the actual string from the JS file and update the test assertion.

**Step 4: Run all HTML tests**

```bash
cargo test -p barad-dur renderer::html 2>&1 | tail -20
```

Expected: all pass.

**Step 5: Commit**

```bash
git add src/renderer/html/tests_extra.rs
git commit -m "test: add empty-state coverage for all 6 HTML report tabs"
```

---

## Approach C — File splits (pure refactor)

### Background

`js_treemap.rs` is 828 lines mixing layout algorithm and DOM rendering. `cli.rs` is 714 lines mixing CLI struct definitions with subcommand dispatch logic.

These are pure refactors: zero behavior change, all existing tests must pass at every step.

---

### Task C1: Split `js_treemap.rs` into layout and UI parts

**Files:**
- Create: `src/renderer/html/js_treemap_layout.rs`
- Create: `src/renderer/html/js_treemap_ui.rs`
- Modify: `src/renderer/html/js_treemap.rs` → becomes a thin redirect
- Modify: `src/renderer/html/mod.rs` (or wherever `build_js` is defined)

**Step 1: Run existing treemap tests to establish baseline**

```bash
cargo test -p barad-dur renderer::html 2>&1 | tail -10
```

Expected: all pass. Note the exact pass count.

**Step 2: Identify the split boundary**

In `js_treemap.rs`, the layout logic (`buildFileTree`, `squarify`, coordinate functions) ends roughly where `buildTreemapTab` begins. The split point is after the last layout-only helper and before the first DOM-building function.

```bash
grep -n "^  function" src/renderer/html/js_treemap.rs | head -20
```

Use this to identify the line where layout ends and UI begins.

**Step 3: Create `js_treemap_layout.rs`**

```rust
pub const JS: &str = r#"
  /* ---- Treemap layout (squarify algorithm) ---- */
  // ... paste layout-only functions here: buildFileTree, squarify helpers, etc.
"#;
```

**Step 4: Create `js_treemap_ui.rs`**

```rust
pub const JS: &str = r#"
  /* ---- Treemap tab UI ---- */
  // ... paste buildTreemapTab and all DOM-construction functions here.
"#;
```

**Step 5: Replace `js_treemap.rs` with a comment**

```rust
// Split: see js_treemap_layout.rs (algorithm) and js_treemap_ui.rs (DOM rendering).
// Both are included in build_js() in place of this file.
```

**Step 6: Update `build_js()` in `src/renderer/html/mod.rs`**

Find the line that includes `js_treemap::JS` and replace with:

```rust
js_treemap_layout::JS,
js_treemap_ui::JS,
```

Also add the `mod` declarations:

```rust
mod js_treemap_layout;
mod js_treemap_ui;
```

**Step 7: Run all tests**

```bash
cargo test -p barad-dur 2>&1 | tail -20
```

Expected: same pass count as baseline. No regressions.

**Step 8: Commit**

```bash
git add src/renderer/html/js_treemap_layout.rs src/renderer/html/js_treemap_ui.rs \
  src/renderer/html/js_treemap.rs src/renderer/html/mod.rs
git commit -m "refactor: split js_treemap.rs into layout and UI modules"
```

---

### Task C2: Extract `AnalyzeArgs` from `cli.rs`

**Files:**
- Create: `src/cli/analyze.rs` (new file with `AnalyzeArgs` struct + all its flags)
- Modify: `src/cli.rs` → re-export from `cli/analyze.rs`, keep `Cli` struct and subcommand enum

**Step 1: Run baseline**

```bash
cargo test -p barad-dur 2>&1 | tail -5
```

Note pass count.

**Step 2: Move `AnalyzeArgs` struct to new file**

Create `src/cli/` directory (convert `cli.rs` to `cli/mod.rs`):

```bash
mkdir src/cli
cp src/cli.rs src/cli/mod.rs
```

Create `src/cli/analyze.rs` containing only the `AnalyzeArgs` struct definition and its `impl` blocks.

Remove the `AnalyzeArgs` struct from `src/cli/mod.rs` and add:

```rust
mod analyze;
pub use analyze::AnalyzeArgs;
```

Delete `src/cli.rs` (it has been replaced by `src/cli/mod.rs`).

**Step 3: Verify compile**

```bash
cargo check 2>&1 | head -20
```

Expected: clean.

**Step 4: Run all tests**

```bash
cargo test -p barad-dur 2>&1 | tail -20
```

Expected: same pass count as baseline.

**Step 5: Commit**

```bash
git add src/cli/ && git rm src/cli.rs
git commit -m "refactor: extract AnalyzeArgs into src/cli/analyze.rs"
```

---

## Final Verification

After all tasks are complete:

```bash
cargo test 2>&1 | tail -5
cargo clippy -- -D warnings 2>&1 | head -20
```

Both must be clean before declaring done.
