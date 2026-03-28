# Walking Skeleton: html-trend-graph-readability

## Driving Port

`renderer::html::render(&report: &AnalysisReport) -> Result<String>`

All acceptance tests call this function. It is the sole entry point for the entire feature.

---

## Walking Skeleton: Backfill Entry Produces Hollow Circle JS Logic

**Minimum observable outcome**: When the renderer receives a history entry with `source = "backfill"`, the output HTML contains JavaScript that will render that point as a hollow circle (`fill="none"`) and the `window.R` history data includes the `source` field.

This single slice proves:
1. The `source` field is correctly serialized into `window.R.history`
2. The JS code contains the conditional fill logic keyed on `entry.source === 'backfill'`

### Scenario: Walking Skeleton — backfill entry wires through to hollow circle JS

```gherkin
Given a report whose history contains exactly one entry with source "backfill" and overall_score 58
When render(&report) is called
Then the rendered HTML contains the JS guard `entry.source === 'backfill'`
And the rendered HTML contains the string `fill="none"` in the circle-rendering JS
And the window.R JSON embedded in the HTML contains `"source":"backfill"`
```

### Rust test stub

```rust
#[test]
#[ignore]  // Enable first: AC-TG-01 walking skeleton
fn html_trends_backfill_source_wired_through() {
    let mut report = make_report();
    report.history = vec![make_history_entry(58, Some("backfill"))];
    let html = render(&report).unwrap();
    assert!(
        html.contains("source === 'backfill'"),
        "JS must guard on entry.source === 'backfill'"
    );
    assert!(
        html.contains(r#""source":"backfill""#),
        "window.R history entry must carry source field"
    );
}
```

---

## Why This Is the Walking Skeleton

The story map walking skeleton is: "When `entry.source === 'backfill'`, the SVG circle renders with `fill='none'`."

The Rust test above is the thinnest verifiable slice of this: it confirms the data flows from `HistoryEntry.source` into `window.R` and that the JS code references `entry.source`. If this test passes, the core wiring is done. The visual output (actual hollow circle in browser) is confirmed by the milestone tests and manual browser verification.
