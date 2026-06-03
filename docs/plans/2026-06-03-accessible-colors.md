# Accessible Colors (CBF Toggle) — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a GUI toggle in the HTML report that switches between the default palette and a color-blind-friendly (CBF) palette, persisted in `localStorage`.

**Architecture:** CSS custom properties define semantic color tokens in `:root`. A `body.cbf` class overrides the green tokens with sky-blue. All JS inline-style color references switch from hardcoded hex to `var(--c-*)`. A toggle button in the page header flips the class.

**Tech Stack:** Pure CSS custom properties + vanilla JS. All changes in `src/renderer/html/`.

---

### Task 1: Add CSS custom property tokens

**Files:**
- Modify: `src/renderer/html/css.rs` (top of the CSS string, after the reset block)

**Step 1: Write the failing test**

In `src/renderer/html/tests.rs`, add after the existing `html_is_valid_document` test:

```rust
#[test]
fn html_has_cbf_css_tokens() {
    let html = render(&make_report()).unwrap();
    assert!(html.contains("--c-good:"), "--c-good token must be in CSS");
    assert!(html.contains("--c-warn:"), "--c-warn token must be in CSS");
    assert!(html.contains("--c-danger:"), "--c-danger token must be in CSS");
    assert!(html.contains("body.cbf"), "body.cbf override block must exist");
    assert!(html.contains("#38bdf8"), "CBF block must contain sky-blue for --c-good");
}
```

**Step 2: Run the test to verify it fails**

```bash
cargo test html_has_cbf_css_tokens -- --nocapture
```

Expected: FAIL (`--c-good:` not found).

**Step 3: Insert the `:root` and `body.cbf` blocks in `css.rs`**

In `src/renderer/html/css.rs`, insert the following immediately after the `*, *::before, *::after` reset line (before the `body {` rule):

```css
:root {
  --c-good:        #10b981;
  --c-good-bg:     rgba(16,185,129,0.13);
  --c-good-lo:     #22c55e;
  --c-warn:        #f59e0b;
  --c-warn-bg:     rgba(245,158,11,0.13);
  --c-danger:      #ef4444;
  --c-danger-bg:   rgba(239,68,68,0.13);
  --c-age-mid:     #eab308;
  --c-age-mid-bg:  rgba(234,179,8,0.13);
}
body.cbf {
  --c-good:        #38bdf8;
  --c-good-bg:     rgba(56,189,248,0.13);
  --c-good-lo:     #38bdf8;
}
```

**Step 4: Run the test to verify it passes**

```bash
cargo test html_has_cbf_css_tokens -- --nocapture
```

Expected: PASS.

**Step 5: Run the full test suite**

```bash
cargo test
```

Expected: all tests pass.

**Step 6: Commit**

```bash
git add src/renderer/html/css.rs src/renderer/html/tests.rs
git commit -m "feat(html): add CSS custom property tokens for CBF palette"
```

---

### Task 2: Convert `scoreColor()` and `defaultScoreHints` in `js_shared.rs`

**Files:**
- Modify: `src/renderer/html/js_shared.rs`
- Modify: `src/renderer/html/tests.rs` (replace `score_color_thresholds`, add new test)
- Modify: `src/renderer/html/html.rs` (remove the `#[cfg(test)] fn score_color` Rust stub)

**Step 1: Write the failing test**

In `tests.rs`, **replace** the existing `score_color_thresholds` test (lines 74–80) with:

```rust
#[test]
fn html_score_color_uses_css_vars() {
    let html = render(&make_report()).unwrap();
    assert!(html.contains("var(--c-good)"), "scoreColor must return var(--c-good)");
    assert!(html.contains("var(--c-warn)"), "scoreColor must return var(--c-warn)");
    assert!(html.contains("var(--c-danger)"), "scoreColor must return var(--c-danger)");
}
```

**Step 2: Run test to verify it fails**

```bash
cargo test html_score_color_uses_css_vars -- --nocapture
```

Expected: FAIL (`var(--c-good)` not found in JS output).

**Step 3: Update `scoreColor()` in `js_shared.rs`**

In `src/renderer/html/js_shared.rs`, replace:

```js
function scoreColor(s) {
  return s >= 71 ? '#10b981' : s >= 41 ? '#f59e0b' : '#ef4444';
}
```

with:

```js
function scoreColor(s) {
  return s >= 71 ? 'var(--c-good)' : s >= 41 ? 'var(--c-warn)' : 'var(--c-danger)';
}
```

**Step 4: Update `defaultScoreHints` in `js_shared.rs`**

Replace the three-line `defaultScoreHints` array (near the bottom of the file):

```js
var defaultScoreHints = [
  { color: '#ef4444', label: '0–39 Critical' },
  { color: '#f59e0b', label: '40–69 Needs work' },
  { color: '#22c55e', label: '70–100 Healthy' }
];
```

with:

```js
var defaultScoreHints = [
  { color: 'var(--c-danger)', label: '0–39 Critical' },
  { color: 'var(--c-warn)',   label: '40–69 Needs work' },
  { color: 'var(--c-good-lo)', label: '70–100 Healthy' }
];
```

**Step 5: Remove the Rust `score_color` stub in `html.rs`**

In `src/renderer/html/html.rs`, delete the entire `#[cfg(test)] fn score_color` block (lines 46–55):

```rust
#[cfg(test)]
fn score_color(score: u32) -> &'static str {
    if score >= 71 {
        "#10b981"
    } else if score >= 41 {
        "#f59e0b"
    } else {
        "#ef4444"
    }
}
```

**Step 6: Run tests to verify**

```bash
cargo test
```

Expected: all tests pass (including the new `html_score_color_uses_css_vars`).

**Step 7: Commit**

```bash
git add src/renderer/html/js_shared.rs src/renderer/html/html.rs src/renderer/html/tests.rs
git commit -m "feat(html): convert scoreColor() and defaultScoreHints to CSS vars"
```

---

### Task 3: Update `js_age.rs` — legend + `ageBand()`

**Files:**
- Modify: `src/renderer/html/js_age.rs`

The age chip uses `band.color + '22'` for a semi-transparent background. Once `band.color` is a CSS variable, string concatenation produces invalid CSS. Fix: return a `bg` field from `ageBand()`.

**Step 1: No new test needed** — the existing `html_tabs_have_info_banners` test covers that tabs render. The change is purely cosmetic.

**Step 2: Replace the legend color array in `js_age.rs`**

Replace:

```js
      [
        { color: '#10b981', label: 'Fresh (<90 days) — actively maintained' },
        { color: '#eab308', label: '3–6 months — aging, review periodically' },
        { color: '#f59e0b', label: '6–12 months — stale, check if still relevant' },
        { color: '#ef4444', label: '>1 year — potentially abandoned' }
      ]
```

with:

```js
      [
        { color: 'var(--c-good)',    label: 'Fresh (<90 days) — actively maintained' },
        { color: 'var(--c-age-mid)', label: '3–6 months — aging, review periodically' },
        { color: 'var(--c-warn)',    label: '6–12 months — stale, check if still relevant' },
        { color: 'var(--c-danger)',  label: '>1 year — potentially abandoned' }
      ]
```

**Step 3: Replace `ageBand()` to include a `bg` field**

Replace:

```js
    function ageBand(days) {
      if (days > 365) return { color: '#ef4444', label: '> 1y' };
      if (days > 180) return { color: '#f59e0b', label: '> 6mo' };
      if (days > 90)  return { color: '#eab308', label: '> 3mo' };
      return { color: '#10b981', label: 'Fresh' };
    }
```

with:

```js
    function ageBand(days) {
      if (days > 365) return { color: 'var(--c-danger)',  bg: 'var(--c-danger-bg)',  label: '> 1y' };
      if (days > 180) return { color: 'var(--c-warn)',    bg: 'var(--c-warn-bg)',    label: '> 6mo' };
      if (days > 90)  return { color: 'var(--c-age-mid)', bg: 'var(--c-age-mid-bg)', label: '> 3mo' };
      return            { color: 'var(--c-good)',    bg: 'var(--c-good-bg)',    label: 'Fresh' };
    }
```

**Step 4: Fix the chip rendering to use `band.bg`**

Replace:

```js
      var bandChip = el('span', { className: 'chip', style: { background: band.color + '22', color: band.color } });
```

with:

```js
      var bandChip = el('span', { className: 'chip', style: { background: band.bg, color: band.color } });
```

**Step 5: Run tests**

```bash
cargo test
```

Expected: all tests pass.

**Step 6: Commit**

```bash
git add src/renderer/html/js_age.rs
git commit -m "feat(html): convert js_age color references to CSS vars"
```

---

### Task 4: Update `js_coupling.rs` — legend + inline color ternaries

**Files:**
- Modify: `src/renderer/html/js_coupling.rs`

**Step 1: Replace the legend color array**

Replace:

```js
        { color: '#22c55e', label: '<30% — Normal co-change' },
        { color: '#f59e0b', label: '30–60% — Worth investigating' },
        { color: '#ef4444', label: '>60% — Strongly coupled, refactor candidate' }
```

with:

```js
        { color: 'var(--c-good-lo)', label: '<30% — Normal co-change' },
        { color: 'var(--c-warn)',    label: '30–60% — Worth investigating' },
        { color: 'var(--c-danger)',  label: '>60% — Strongly coupled, refactor candidate' }
```

**Step 2: Replace the pct span ternary color**

Replace:

```js
        var pctSpan = el('span', { style: { fontWeight: '700', color: p.coupling_pct > 70 ? '#ef4444' : p.coupling_pct > 40 ? '#f59e0b' : '#10b981' } });
```

with:

```js
        var pctSpan = el('span', { style: { fontWeight: '700', color: p.coupling_pct > 70 ? 'var(--c-danger)' : p.coupling_pct > 40 ? 'var(--c-warn)' : 'var(--c-good)' } });
```

**Step 3: Replace the cross-boundary badge color**

Replace:

```js
          var cbBadge = el('span', { style: { color: '#f59e0b', fontWeight: '600', fontSize: '0.75rem' } });
```

with:

```js
          var cbBadge = el('span', { style: { color: 'var(--c-warn)', fontWeight: '600', fontSize: '0.75rem' } });
```

**Step 4: Replace the `inlineBar` color ternary**

Replace:

```js
        barCell.append(inlineBar(p.coupling_pct, p.coupling_pct > 70 ? '#ef4444' : p.coupling_pct > 40 ? '#f59e0b' : '#10b981'));
```

with:

```js
        barCell.append(inlineBar(p.coupling_pct, p.coupling_pct > 70 ? 'var(--c-danger)' : p.coupling_pct > 40 ? 'var(--c-warn)' : 'var(--c-good)'));
```

**Step 5: Run tests**

```bash
cargo test
```

Expected: all tests pass.

**Step 6: Commit**

```bash
git add src/renderer/html/js_coupling.rs
git commit -m "feat(html): convert js_coupling color references to CSS vars"
```

---

### Task 5: Update `js_hotspots.rs` and `js_ownership.rs` legend arrays

**Files:**
- Modify: `src/renderer/html/js_hotspots.rs`
- Modify: `src/renderer/html/js_ownership.rs`

**Step 1: `js_hotspots.rs` — replace legend array**

Replace:

```js
        { color: '#22c55e', label: 'Low risk — simple + rarely changed' },
        { color: '#f59e0b', label: 'Medium — monitor these files' },
        { color: '#ef4444', label: 'High risk — complex + frequently changed' }
```

with:

```js
        { color: 'var(--c-good-lo)', label: 'Low risk — simple + rarely changed' },
        { color: 'var(--c-warn)',    label: 'Medium — monitor these files' },
        { color: 'var(--c-danger)',  label: 'High risk — complex + frequently changed' }
```

**Step 2: `js_ownership.rs` — replace legend array**

Replace:

```js
        { color: '#22c55e', label: 'Shared — multiple contributors, low bus-factor risk' },
        { color: '#f59e0b', label: 'Concentrated — one author >70%, knowledge silo risk' },
        { color: '#ef4444', label: 'Sole owner — single author >90%, critical bus-factor' }
```

with:

```js
        { color: 'var(--c-good-lo)', label: 'Shared — multiple contributors, low bus-factor risk' },
        { color: 'var(--c-warn)',    label: 'Concentrated — one author >70%, knowledge silo risk' },
        { color: 'var(--c-danger)',  label: 'Sole owner — single author >90%, critical bus-factor' }
```

**Step 3: Run tests**

```bash
cargo test
```

Expected: all tests pass.

**Step 4: Commit**

```bash
git add src/renderer/html/js_hotspots.rs src/renderer/html/js_ownership.rs
git commit -m "feat(html): convert js_hotspots and js_ownership legend colors to CSS vars"
```

---

### Task 6: Update `js_authors.rs` — legends + color functions

**Files:**
- Modify: `src/renderer/html/js_authors.rs`

**Step 1: Replace the authors legend array**

Replace:

```js
        { color: '#10b981', label: 'Active — committed in last 30 days' },
        { color: '#f59e0b', label: 'Aging — 30–90 days since last commit' },
        { color: '#ef4444', label: 'Stale — 90+ days since last commit' }
```

with:

```js
        { color: 'var(--c-good)',   label: 'Active — committed in last 30 days' },
        { color: 'var(--c-warn)',   label: 'Aging — 30–90 days since last commit' },
        { color: 'var(--c-danger)', label: 'Stale — 90+ days since last commit' }
```

**Step 2: Replace `activityColor()` function body**

Replace:

```js
    function activityColor(days) {
      if (days <= 30) return '#10b981';
      if (days <= 90) return '#f59e0b';
      return '#ef4444';
    }
```

with:

```js
    function activityColor(days) {
      if (days <= 30) return 'var(--c-good)';
      if (days <= 90) return 'var(--c-warn)';
      return 'var(--c-danger)';
    }
```

**Step 3: Replace `qualityColor()` function body**

Replace:

```js
    function qualityColor(q) {
      if (q >= 70) return '#10b981';
      if (q >= 40) return '#f59e0b';
      return '#ef4444';
    }
```

with:

```js
    function qualityColor(q) {
      if (q >= 70) return 'var(--c-good)';
      if (q >= 40) return 'var(--c-warn)';
      return 'var(--c-danger)';
    }
```

**Step 4: Run tests**

```bash
cargo test
```

Expected: all tests pass.

**Step 5: Commit**

```bash
git add src/renderer/html/js_authors.rs
git commit -m "feat(html): convert js_authors color references to CSS vars"
```

---

### Task 7: Update `js_trends.rs` — local `scoreColor()` + live dot

**Files:**
- Modify: `src/renderer/html/js_trends.rs`

**Step 1: Replace the local `scoreColor()` function**

In `js_trends.rs` there is a second, local `scoreColor()` definition (not the shared one). Replace:

```js
    function scoreColor(s) {
      if (s >= 71) return '#10b981';
      if (s >= 41) return '#f59e0b';
      return '#ef4444';
    }
```

with:

```js
    function scoreColor(s) {
      if (s >= 71) return 'var(--c-good)';
      if (s >= 41) return 'var(--c-warn)';
      return 'var(--c-danger)';
    }
```

**Step 2: Replace the live dot background in the legend**

Replace:

```js
      dotLive.style.cssText = 'background:#10b981;';
```

with:

```js
      dotLive.style.cssText = 'background:var(--c-good);';
```

**Step 3: Run tests**

```bash
cargo test
```

Expected: all tests pass.

**Step 4: Commit**

```bash
git add src/renderer/html/js_trends.rs
git commit -m "feat(html): convert js_trends color references to CSS vars"
```

---

### Task 8: Add the CBF toggle button to the page header

The header is built in `renderApp()` inside `src/renderer/html/js_authors.rs`.

**Files:**
- Modify: `src/renderer/html/js_authors.rs`
- Modify: `src/renderer/html/tests.rs`

**Step 1: Write the failing test**

In `tests.rs`, add:

```rust
#[test]
fn html_has_cbf_toggle_button() {
    let html = render(&make_report()).unwrap();
    assert!(html.contains("cbf-palette"), "CBF toggle must use localStorage key cbf-palette");
    assert!(html.contains("cbf-btn"), "CBF toggle button must have cbf-btn id or class");
}
```

**Step 2: Run test to verify it fails**

```bash
cargo test html_has_cbf_toggle_button -- --nocapture
```

Expected: FAIL.

**Step 3: Add localStorage restore at the top of `renderApp()`**

In `js_authors.rs`, inside `renderApp()`, add the following as the very first line of the function body (before `var app = ...`):

```js
    if (localStorage.getItem('cbf-palette')) document.body.classList.add('cbf');
```

**Step 4: Add the toggle button after the chips are built**

In `renderApp()`, after the line:

```js
    if (R.time_window_months && R.time_window_months > 0) {
      chips.append(chip(R.time_window_months + 'mo window', '#2a1f0a', '#fcd34d'));
    }
```

add:

```js
    var cbfBtn = el('button', {
      style: {
        background: 'none',
        border: '1px solid #334155',
        borderRadius: '6px',
        color: '#64748b',
        fontSize: '11px',
        padding: '2px 8px',
        cursor: 'pointer',
        fontFamily: 'inherit',
        marginLeft: '4px'
      }
    });
    cbfBtn.id = 'cbf-btn';
    function updateCbfBtn() {
      var on = document.body.classList.contains('cbf');
      cbfBtn.textContent = on ? '◐ Default' : '◑ CBF';
      cbfBtn.style.color = on ? 'var(--c-good)' : '#64748b';
      cbfBtn.style.borderColor = on ? 'var(--c-good)' : '#334155';
    }
    updateCbfBtn();
    cbfBtn.addEventListener('click', function() {
      document.body.classList.toggle('cbf');
      localStorage.setItem('cbf-palette', document.body.classList.contains('cbf') ? '1' : '');
      updateCbfBtn();
    });
    chips.append(cbfBtn);
```

**Step 5: Run tests to verify**

```bash
cargo test
```

Expected: all tests pass including `html_has_cbf_toggle_button`.

**Step 6: Commit**

```bash
git add src/renderer/html/js_authors.rs src/renderer/html/tests.rs
git commit -m "feat(html): add CBF palette toggle button to report header"
```

---

### Task 9: Smoke-test the full rendered report

**Step 1: Generate a report**

```bash
make analyze
```

This writes `dashboard/report.json`. Then generate the HTML:

```bash
cargo run -- analyze . --html -o /tmp/barad-dur-test.html
```

**Step 2: Open in a browser and verify**

Open `/tmp/barad-dur-test.html` in a browser. Check:
- The `◑ CBF` button is visible in the top-right of the header.
- Clicking it turns scores blue (sky-blue replaces green throughout all tabs).
- Clicking again restores the default green.
- Refreshing the page preserves the last-chosen palette.

**Step 3: Run the full test suite one final time**

```bash
cargo test
```

Expected: all tests pass.

**Step 4: Final commit if any polish was needed**

```bash
git add -p   # stage only what changed
git commit -m "feat(html): accessible color toggle — final polish"
```
