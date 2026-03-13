# Cross-Tab Drill-Through Links Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Overview tab recommendations clickable — each recommendation links to the relevant tab and pre-filters/highlights the related data.

**Architecture:** Pure JS + minimal Rust change. Replace `top_actions: Vec<String>` with `top_actions: Vec<ActionItem>` in `src/scorer.rs` (struct carries optional `target_tab` and `sort_by` hints). Update JS `buildActions()` to render clickable links that call a new `switchToTab()` helper. No new crates, no new files.

**Tech Stack:** Rust (serde), vanilla JS (DOM)

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src/scorer.rs` | Modify | Add `ActionItem` struct, change `top_actions` type, update `generate_top_actions` |
| `src/renderer/html.rs` | Modify | Update JS `buildActions()` + add `switchToTab()` helper, update tests |

---

## Chunk 1: ActionItem Struct + generate_top_actions

### Task 1: Add ActionItem struct and update top_actions type

**Files:**
- Modify: `src/scorer.rs:17-83` (structs + AnalysisReport)
- Modify: `src/scorer.rs:342-394` (generate_top_actions + suggest_action)

- [ ] **Step 1: Write failing test — ActionItem has target_tab field**

In `src/scorer.rs` tests section, add after existing `top_actions_picks_worst` test:

```rust
#[test]
fn top_actions_include_target_tab() {
    let categories = vec![
        CategoryResult {
            name: "Health".to_string(),
            score: 50,
            metrics: vec![
                MetricValue {
                    name: "Bus factor".to_string(),
                    description: "bad".to_string(),
                    raw_value: RawValue::Integer(1),
                    score: 20,
                },
                MetricValue {
                    name: "Churn hotspots".to_string(),
                    description: "bad".to_string(),
                    raw_value: RawValue::Count(0),
                    score: 30,
                },
            ],
        },
    ];

    let actions = generate_top_actions(&categories);
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0].target_tab.as_deref(), Some("ownership"));
    assert_eq!(actions[0].sort_by.as_deref(), Some("authors"));
    assert_eq!(actions[1].target_tab.as_deref(), Some("hotspots"));
    assert_eq!(actions[1].sort_by.as_deref(), Some("churn"));
}
```

Run: `cargo test --lib scorer::tests::top_actions_include_target_tab`

Expected: **FAIL** — `ActionItem` does not exist, `top_actions` is `Vec<String>`.

- [ ] **Step 2: Add ActionItem struct**

In `src/scorer.rs`, after the `RemoteMeta` struct (line ~64), add:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ActionItem {
    pub text: String,
    pub target_tab: Option<String>,
    pub sort_by: Option<String>,
}
```

- [ ] **Step 3: Change top_actions type in AnalysisReport**

In `src/scorer.rs`, change `AnalysisReport`:

```rust
// Before:
pub top_actions: Vec<String>,
// After:
pub top_actions: Vec<ActionItem>,
```

- [ ] **Step 4: Add target_tab_for_metric helper and update generate_top_actions**

Replace the `generate_top_actions` function with:

```rust
fn generate_top_actions(categories: &[CategoryResult]) -> Vec<ActionItem> {
    let mut low_metrics: Vec<(&str, &str, u32)> = Vec::new();

    for cat in categories {
        for metric in &cat.metrics {
            low_metrics.push((&cat.name, &metric.name, metric.score));
        }
    }

    // Sort by score ascending (worst first)
    low_metrics.sort_by_key(|m| m.2);

    // Take top 3 worst metrics and generate suggestions
    low_metrics
        .iter()
        .take(3)
        .filter(|m| m.2 < 80) // Only suggest for metrics below 80
        .map(|(cat, metric, score)| {
            let (target_tab, sort_by) = target_tab_for_metric(metric);
            ActionItem {
                text: format!(
                    "[{}] {} (score: {}) — {}",
                    cat, metric, score,
                    suggest_action(metric)
                ),
                target_tab: target_tab.map(String::from),
                sort_by: sort_by.map(String::from),
            }
        })
        .collect()
}

fn target_tab_for_metric(metric_name: &str) -> (Option<&'static str>, Option<&'static str>) {
    match metric_name {
        "Bus factor" => (Some("ownership"), Some("authors")),
        "Churn hotspots" => (Some("hotspots"), Some("churn")),
        "Temporal coupling" => (Some("coupling"), None),
        "Stale code" => (Some("age"), Some("oldest")),
        "File complexity" => (Some("hotspots"), Some("complexity")),
        "Knowledge distribution" => (Some("ownership"), None),
        "Contributor activity" => (None, None),
        "Ownership clarity" => (Some("ownership"), None),
        "Collaboration patterns" => (Some("ownership"), None),
        "Code age" => (Some("age"), None),
        "Commit message quality" => (None, None),
        "Gitignore coverage" => (None, None),
        "Merge patterns" => (None, None),
        "Growth trend" => (Some("trends"), None),
        "Refactoring ratio" => (Some("hotspots"), None),
        "History cleanliness" => (None, None),
        "Commit cadence" => (Some("trends"), None),
        _ => (None, None),
    }
}
```

- [ ] **Step 5: Fix existing tests that use Vec<String> for top_actions**

In `src/scorer.rs` test `top_actions_picks_worst`, update the assertion:

```rust
// Before:
assert!(actions[0].contains("Knowledge distribution"));
// After:
assert!(actions[0].text.contains("Knowledge distribution"));
```

In `src/renderer/html.rs` test helper `make_report()`, update `top_actions` field:

```rust
// Before:
top_actions: vec!["Improve test coverage".into()],
// After:
top_actions: vec![ActionItem {
    text: "Improve test coverage".into(),
    target_tab: Some("hotspots".into()),
    sort_by: None,
}],
```

Add the import at the top of the html.rs test module:

```rust
use crate::scorer::ActionItem;
```

- [ ] **Step 6: Run all tests — confirm green**

Run: `cargo test --lib`

Expected: **PASS** — all existing tests pass, new test passes.

---

## Chunk 2: JS — clickable action items + switchToTab helper

### Task 2: Add switchToTab() helper function

**Files:**
- Modify: `src/renderer/html.rs` (JS block, around line 2540-2590)

- [ ] **Step 1: Add switchToTab() function**

In the JS code, just before the `renderApp()` call (line ~2593), insert a module-level helper. This needs to be placed inside the IIFE but outside `renderApp`, so the tab-switching closure captures the DOM references. The cleanest approach: define `switchToTab` as a function on `window` that `renderApp` populates, or inline it into `renderApp` after the tabs are built.

Insert after line 2589 (`contentDivs[0].dataset.rendered = '1';`) and before `app.replaceChildren(...)`:

```javascript
    // Expose tab-switching for drill-through links
    window.__switchToTab = function(tabName, sortBy) {
      var idx = tabNames.indexOf(tabName.charAt(0).toUpperCase() + tabName.slice(1));
      if (idx < 0) return;
      var allTabs = tabs.querySelectorAll('.tab');
      allTabs.forEach(function(tb) { tb.className = 'tab'; });
      contentDivs.forEach(function(cd) { cd.className = 'tab-content'; });
      allTabs[idx].className = 'tab active';
      contentDivs[idx].className = 'tab-content active';
      if (contentDivs[idx].dataset.rendered !== '1') {
        contentDivs[idx].replaceChildren(tabContents[idx]());
        contentDivs[idx].dataset.rendered = '1';
      }
      if (sortBy) {
        var sortBtn = contentDivs[idx].querySelector('[data-sort="' + sortBy + '"]');
        if (sortBtn) sortBtn.click();
      }
      contentDivs[idx].scrollIntoView({ behavior: 'smooth', block: 'start' });
    };
```

- [ ] **Step 2: Run tests — confirm no regressions**

Run: `cargo test --lib renderer::html`

Expected: **PASS**

### Task 3: Update buildActions() to render clickable links

**Files:**
- Modify: `src/renderer/html.rs` (JS block, lines ~840-863)

- [ ] **Step 1: Update buildActions to handle ActionItem objects**

Replace the `buildActions` function body. The data is now serialized as `{ text, target_tab, sort_by }` objects instead of plain strings:

```javascript
  function buildActions(actions) {
    var section = el('div', { className: 'actions-section' });
    var heading = el('div', { style: { marginBottom: '8px' } });
    var h = el('span', { className: 'label' });
    h.append(txt('Top Recommendations'));
    heading.append(h);
    section.append(heading);
    if (!actions || actions.length === 0) {
      var none = el('div', { style: { color: '#64748b', padding: '8px 0', fontSize: '13px' } });
      none.append(txt('No recommendations — all metrics look good!'));
      section.append(none);
      return section;
    }
    actions.forEach(function(a, i) {
      var item = el('div', { className: 'action-item' });
      var num = el('div', { className: 'action-num' });
      num.append(txt(String(i + 1)));
      var actionObj = typeof a === 'string' ? { text: a } : a;
      var text = el('div');
      if (actionObj.target_tab) {
        var link = el('a', {
          href: '#',
          className: 'action-link',
          style: { color: '#38bdf8', textDecoration: 'underline', cursor: 'pointer' }
        });
        link.dataset.targetTab = actionObj.target_tab;
        if (actionObj.sort_by) link.dataset.sortBy = actionObj.sort_by;
        link.append(txt(actionObj.text));
        link.addEventListener('click', function(e) {
          e.preventDefault();
          if (window.__switchToTab) {
            window.__switchToTab(actionObj.target_tab, actionObj.sort_by || null);
          }
        });
        text.append(link);
      } else {
        text.append(txt(actionObj.text));
      }
      item.append(num, text);
      section.append(item);
    });
    return section;
  }
```

- [ ] **Step 2: Run tests — confirm no regressions**

Run: `cargo test --lib renderer::html`

Expected: **PASS**

### Task 4: Add CSS for action links

**Files:**
- Modify: `src/renderer/html.rs` (CSS block)

- [ ] **Step 1: Add .action-link styles**

Find the `.action-item` CSS rules and add after them:

```css
.action-link { color: #38bdf8; text-decoration: underline; cursor: pointer; transition: color 0.2s; }
.action-link:hover { color: #7dd3fc; }
```

- [ ] **Step 2: Run tests — confirm no regressions**

Run: `cargo test --lib renderer::html`

Expected: **PASS**

---

## Chunk 3: HTML Tests

### Task 5: Add tests for drill-through attributes

**Files:**
- Modify: `src/renderer/html.rs` (test module, after existing tests)

- [ ] **Step 1: Add test — action items with target_tab render as links**

```rust
#[test]
fn html_actions_render_drill_through_links() {
    let mut report = make_report();
    report.top_actions = vec![
        ActionItem {
            text: "Fix bus factor".into(),
            target_tab: Some("ownership".into()),
            sort_by: Some("authors".into()),
        },
        ActionItem {
            text: "No link action".into(),
            target_tab: None,
            sort_by: None,
        },
    ];
    let html = render(&report).unwrap();
    // First action should have drill-through attributes
    assert!(html.contains("action-link"));
    assert!(html.contains("__switchToTab"));
}
```

- [ ] **Step 2: Add test — switchToTab function exists in output**

```rust
#[test]
fn html_contains_switch_to_tab_function() {
    let report = make_report();
    let html = render(&report).unwrap();
    assert!(html.contains("__switchToTab"));
}
```

- [ ] **Step 3: Run full test suite**

Run: `cargo test --lib`

Expected: **PASS** — all tests green.

---

## Chunk 4: Commit

- [ ] **Step 1: Verify all tests pass**

Run: `cargo test`

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`

- [ ] **Step 3: Run fmt**

Run: `cargo fmt`

- [ ] **Step 4: Commit**

```
feat(html): add cross-tab drill-through links for recommendations

ActionItem struct replaces plain strings in top_actions, carrying
target_tab and sort_by hints. Overview recommendations are now
clickable links that switch to the relevant tab.
```

---

## Metric-to-Tab Mapping Reference

| Metric | target_tab | sort_by | Tab Index |
|--------|-----------|---------|-----------|
| Bus factor | ownership | authors | 3 |
| Churn hotspots | hotspots | churn | 1 |
| Temporal coupling | coupling | — | 2 |
| Stale code | age | oldest | 4 |
| File complexity | hotspots | complexity | 1 |
| Knowledge distribution | ownership | — | 3 |
| Contributor activity | — | — | — |
| Ownership clarity | ownership | — | 3 |
| Collaboration patterns | ownership | — | 3 |
| Code age | age | — | 4 |
| Growth trend | trends | — | 6 |
| Refactoring ratio | hotspots | — | 1 |
| Commit cadence | trends | — | 6 |
| Merge patterns | — | — | — |
| Commit message quality | — | — | — |
| History cleanliness | — | — | — |
| Gitignore coverage | — | — | — |
