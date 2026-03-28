# Test Scenarios: html-trend-graph-readability

## Driving Port

`renderer::html::render(&report: &AnalysisReport) -> Result<String>`

All tests call this function. Test setup varies only the `report.history` field.

## Test Framework

Rust native `#[test]` in `src/renderer/html.rs` test module. All stubs carry `#[ignore]` — enable one at a time per Outside-In TDD discipline. Walking skeleton is enabled first.

## Testing Approach

The SVG circles are built by JavaScript at browser runtime, not by Rust at render time. Rust tests verify:
1. **JS code correctness**: The rendered HTML contains the expected JS logic (guard conditions, attribute assignments, string literals)
2. **Data correctness**: The embedded `window.R` JSON carries the expected `source` fields

Behavioral tests (actual hollow/filled rendering, legend DOM visibility, tooltip on hover) are covered by the manual browser verification checklist at the end of this document.

---

## Helper Function (to be added to test module)

```rust
fn make_history_entry(score: u32, source: Option<&str>) -> HistoryEntry {
    HistoryEntry {
        timestamp: chrono::Utc::now(),
        head: "a".repeat(40),
        overall_score: score,
        categories: std::collections::HashMap::new(),
        metrics: std::collections::HashMap::new(),
        counts: crate::scorer::HistoryCounts { commits: 10, files: 5, authors: 2 },
        branch: "main".into(),
        schema_version: 1,
        source: source.map(|s| s.to_string()),
    }
}
```

---

## Walking Skeleton

### WS-1: Backfill entry source wires through to JS guard

**AC**: AC-TG-01.1 (partial — structural check)

```gherkin
Given a report with one history entry (source: "backfill", score: 58)
When render(&report) is called
Then the HTML contains JS guard `entry.source === 'backfill'`
And window.R JSON contains `"source":"backfill"`
```

```rust
#[test]
#[ignore]  // Walking skeleton — enable first
fn html_trends_backfill_source_wired_through() { ... }
```

---

## Milestone 1: Circle Visual Encoding

### M1-1: Hollow circle JS uses fill="none"

**AC**: AC-TG-01.1

```gherkin
Given a report with at least one backfill history entry
When render(&report) is called
Then the rendered JS contains the string literal `fill="none"` for hollow circle encoding
And the rendered JS contains the `pointer-events:all` attribute for hollow circles
```

```rust
#[test]
#[ignore]  // M1-1
fn html_trends_hollow_circle_js_fill_none() { ... }
```

### M1-2: Live circle JS uses scoreColor fill

**AC**: AC-TG-01.2

```gherkin
Given a report with at least one live history entry (source: None)
When render(&report) is called
Then the rendered JS assigns `scoreColor(scores[i])` to fill for live entries
```

```rust
#[test]
#[ignore]  // M1-2
fn html_trends_live_circle_js_uses_score_color() { ... }
```

### M1-3: Backfill circle JS sets stroke to scoreColor

**AC**: AC-TG-01.3

```gherkin
Given a report with at least one backfill history entry
When render(&report) is called
Then the rendered JS assigns `scoreColor(scores[i])` to stroke for hollow circles
```

```rust
#[test]
#[ignore]  // M1-3
fn html_trends_hollow_circle_stroke_is_score_color() { ... }
```

### M1-4: Legacy entry (no source) treated as live

**AC**: AC-TG-01.5

```gherkin
Given a report with a history entry where source is None (legacy entry)
When render(&report) is called
Then the window.R JSON for that entry does NOT contain a "source" key
And the JS guard `entry.source === 'backfill'` evaluates to false for undefined source
```

```rust
#[test]
#[ignore]  // M1-4
fn html_trends_legacy_entry_has_no_source_in_window_r() { ... }
```

---

## Milestone 2: Legend

### M2-1: Legend labels present in JS when backfill entries exist

**AC**: AC-TG-02.1, AC-TG-02.2

```gherkin
Given a report with at least one backfill history entry
When render(&report) is called
Then the rendered HTML contains the string "Backfill" as a JS text literal
And the rendered HTML contains the string "Live analysis" as a JS text literal
```

```rust
#[test]
#[ignore]  // M2-1
fn html_trends_legend_labels_in_js() { ... }
```

### M2-2: Legend uses DOM APIs (no innerHTML)

**AC**: AC-TG-02.4 / CC-1

```gherkin
Given any report
When render(&report) is called
Then the rendered JS for the legend uses el()/svgEl()/txt() helper calls
And no innerHTML assignment appears in the legend-building code path
```

```rust
#[test]
#[ignore]  // M2-2: code-review test — checks JS code in HTML for absence of innerHTML in legend
fn html_trends_legend_no_innerHTML() { ... }
```

### M2-3: Legend CSS class present for styling

**AC**: AC-TG-02.1 (visual legibility on dark background)

```gherkin
Given any report
When render(&report) is called
Then the rendered HTML contains the CSS class `.tr-legend`
```

```rust
#[test]
#[ignore]  // M2-3
fn html_trends_legend_css_class_present() { ... }
```

---

## Milestone 3: Tooltip Source Label

### M3-1: Tooltip JS contains "Source: Backfill" string literal

**AC**: AC-TG-03.1

```gherkin
Given any report
When render(&report) is called
Then the rendered JS mouseenter handler contains the literal string "Source: Backfill"
```

```rust
#[test]
#[ignore]  // M3-1
fn html_trends_tooltip_source_backfill_label() { ... }
```

### M3-2: Tooltip JS contains "Source: Live analysis" string literal

**AC**: AC-TG-03.2, AC-TG-03.3

```gherkin
Given any report
When render(&report) is called
Then the rendered JS mouseenter handler contains the literal string "Source: Live analysis"
```

```rust
#[test]
#[ignore]  // M3-2
fn html_trends_tooltip_source_live_label() { ... }
```

### M3-3: Tooltip uses textContent (no innerHTML)

**AC**: AC-TG-03.5 / CC-1

```gherkin
Given any report
When render(&report) is called
Then the tooltip assignment in the JS uses textContent, not innerHTML
```

```rust
#[test]
#[ignore]  // M3-3: checks for absence of innerHTML in tooltip handler
fn html_trends_tooltip_no_innerHTML() { ... }
```

---

## Milestone 4: Edge Cases and Regression

### M4-1: Existing `html_trends_has_chart` test still passes (regression)

**AC**: CC-5

No new test needed — this is the existing test. Must continue to pass throughout.

### M4-2: Zero-backfill history — all entries serialized without source field

**AC**: AC-TG-04.1

```gherkin
Given a report with 3 history entries all with source None
When render(&report) is called
Then the window.R JSON contains no entries with a "source" key
```

```rust
#[test]
#[ignore]  // M4-2
fn html_trends_zero_backfill_window_r_has_no_source_field() { ... }
```

### M4-3: All-backfill history — all entries serialized with source "backfill"

**AC**: AC-TG-04.2

```gherkin
Given a report with 3 history entries all with source "backfill"
When render(&report) is called
Then the window.R JSON contains 3 entries each with `"source":"backfill"`
```

```rust
#[test]
#[ignore]  // M4-3
fn html_trends_all_backfill_window_r_all_have_source() { ... }
```

### M4-4: Pointer-events fix present on hollow circle string

**AC**: D-DESIGN-02 (hover area fix for hollow circles)

```gherkin
Given a report with at least one backfill entry
When render(&report) is called
Then the rendered JS contains `pointer-events:all` for hollow circles
```

```rust
#[test]
#[ignore]  // M4-4
fn html_trends_hollow_dot_pointer_events_all() { ... }
```

---

## Manual Browser Verification Checklist

The following behavioral tests require opening the HTML in a browser (Chrome DevTools / Firefox):

| Check | Steps | Expected |
|-------|-------|----------|
| Hollow circles render for backfill | Open report with mixed history; inspect SVG | Backfill circles have `fill="none"` attribute |
| Filled circles render for live | Same report | Live circles have solid fill color |
| Legend visible in mixed report | Same report | Legend row visible above chart |
| Legend absent in all-live report | Open report with no backfill history | No legend row present |
| Tooltip shows "Source: Backfill" | Hover a hollow dot | Tooltip contains "Source: Backfill" |
| Tooltip shows "Source: Live analysis" | Hover a filled dot | Tooltip contains "Source: Live analysis" |
| Hollow dot hover area works | Hover over interior of hollow circle | Tooltip appears (not just on stroke border) |
| Encoding survives metric change | Change selector to any metric | Hollow dots remain hollow, filled remain filled |
| No JS errors | Open DevTools console | Zero errors in any config |

---

## Story-to-Test Traceability

| User Story | Scenarios | ACs Covered |
|-----------|-----------|-------------|
| US-TG-01 (hollow/filled encoding) | WS-1, M1-1, M1-2, M1-3, M1-4 | AC-TG-01.1–01.5 |
| US-TG-02 (legend) | M2-1, M2-2, M2-3 | AC-TG-02.1–02.5 |
| US-TG-03 (tooltip source label) | M3-1, M3-2, M3-3 | AC-TG-03.1–03.5 |
| US-TG-04 (edge case robustness) | M4-1, M4-2, M4-3, M4-4 | AC-TG-04.1–04.5 |
