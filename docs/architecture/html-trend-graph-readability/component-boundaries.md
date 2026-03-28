# Component Boundaries: html-trend-graph-readability

## Bounded Context

This feature operates entirely within **one bounded context**: the Trends tab rendering logic inside `src/renderer/html.rs`.

No other modules, files, or systems are modified.

---

## Components Modified

### `src/renderer/html.rs`

| Component | Type | Lines (approx) | Change |
|-----------|------|----------------|--------|
| `CSS_CONST` | Rust `const &str` | ~581–610 | Add `.tr-legend` and `.tr-legend-sep` CSS rules |
| `buildTrendsTab()` | Inline JS in Rust string | ~2587–2745 | Add `hasBackfill` check + `buildLegend()` call |
| `renderChart()` | Nested JS fn | ~2706–2715 | Conditional `fill`/`stroke` per entry source |
| `mouseenter` handler | JS closure | ~2718–2738 | Add `sourceLabel` line to tooltip text |

---

## Dependency Boundaries

```
src/renderer/html.rs
  ├── reads: window.R.history[i].source  (already serialized by scorer.rs — no change)
  ├── uses:  el(), svgEl(), txt()         (existing DOM helpers — no change)
  ├── uses:  scoreColor()                 (existing — no change)
  └── adds:  .tr-legend CSS              (scoped to Trends tab CSS class)
```

### What is NOT touched

| File | Why untouched |
|------|--------------|
| `src/scorer.rs` | `entry.source` already serialized in `HistoryEntry` |
| `src/backfill/mod.rs` | Source field already set to `"backfill"` |
| `src/cache/history.rs` | No history read/write changes |
| `src/renderer/json.rs` | Different renderer, out of scope |
| `src/renderer/cli.rs` | Text output, out of scope |
| `src/cli.rs` | No new flags |
| `dashboard/` | React dashboard has its own trend rendering; separate scope |

---

## Interface Contracts

### Input: `window.R.history[i]` (unchanged)

```typescript
interface HistoryEntry {
  source?: string;        // "backfill" | undefined | null
  overall_score: number;
  timestamp: string;      // ISO 8601
  head: string;           // 40-char SHA
  counts: { commits: number; files: number; authors: number; };
  categories: Record<string, number>;
  metrics?: Record<string, number>;
}
```

The `source` field is already present when set. Legacy entries (pre-source-field) have `source === undefined`.

### Guard contract (D-05)

```javascript
entry.source === 'backfill'   // → hollow, "Backfill" label
// any other value             // → filled, "Live analysis" label
```

---

## Rendering Pipeline (unchanged except starred)

```
buildTrendsTab()
  ├─ Build controls row: [metric <select>] [legend★]
  ├─ Build chart container
  ├─ renderChart(metric)  ← called on init + metric change
  │   ├─ Compute scores[] from history[i]
  │   ├─ Draw SVG axes, grid, labels (unchanged)
  │   ├─ Draw polyline (unchanged)
  │   └─ Draw circles★: fill="none"|scoreColor based on source
  └─ Wire events
      ├─ select.onchange → renderChart(newMetric) (unchanged)
      └─ .tr-dot mouseenter★ → tooltip with source label
```

★ = modified by this feature
