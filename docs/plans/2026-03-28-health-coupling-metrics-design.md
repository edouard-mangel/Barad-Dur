# Health & Coupling Metrics Redesign

## Context

The current health metrics category contains 5 metrics with conceptual issues:
- **Stale code** is backwards — stable code is healthy, not a risk
- **Churn hotspots** measures churn in isolation, which is meaningless without complexity context
- **Temporal coupling** is a relationship signal, not an individual-file signal — misplaced in health
- **File complexity** uses file size in bytes, a poor language-agnostic proxy
- **Bus factor** uses the minimum across all files, so a single trivially-owned file tanks the score

This redesign removes stale code, replaces weak proxies with `file_metrics` (tree-sitter AST data already in the snapshot), and extracts coupling signals into a dedicated category.

## Design

### Two categories

**Health** — individual-file maintainability signals (weight: 0.25)

| Metric | Signal | Data source |
|---|---|---|
| `bus_factor` | % of files with single-author dominance | `blame_map` |
| `god_objects` | Files with LOC > 300 AND public_methods > 15, or LOC > 500 | `file_metrics` |
| `complex_hotspots` | Files in top 25% of both cyclomatic complexity AND churn count | `file_metrics` + `commits_by_file` |

**Coupling** — inter-file relationship signals (weight: 0.20)

| Metric | Signal | Data source |
|---|---|---|
| `temporal_coupling` | File pairs co-changing > 70% of the time | `file_change_pairs` |
| `fan_out_coupling` | Files co-changing with > 5 distinct partners | `file_change_pairs` |
| `demeter_violations` | Method chains of depth ≥ 3 | `file_metrics` (new tree-sitter query) |

### Weight redistribution

| Category | Old weight | New weight |
|---|---|---|
| Health | 0.40 | 0.25 |
| Coupling | — | 0.20 |
| Team | 0.15 | 0.10 |
| Evolution | 0.25 | 0.25 |
| Git Hygiene | 0.20 | 0.20 |

### Scoring formulas

**bus_factor** — ratio of single-author dominated files:
- < 10% → 100
- < 25% → 75
- < 50% → 50
- ≥ 50% → 25

**god_objects** — count of god object files:
- 0 → 100
- 1–2 → 75
- 3–5 → 50
- > 5 → 25

**complex_hotspots** — count of files in top 25% of both CC and churn:
- 0 → 100
- 1–2 → 75
- 3–5 → 50
- > 5 → 25

**temporal_coupling** — unchanged from current implementation

**fan_out_coupling** — count of high-fan-out files (> 5 distinct partners):
- 0 → 100
- 1–2 → 75
- 3–5 → 50
- > 5 → 25

**demeter_violations** — total violation count across all files with `file_metrics`:
- 0 → 100
- 1–5 → 75
- 6–15 → 50
- > 15 → 25

### Demeter tree-sitter queries

Detect method chains of depth ≥ 3 (e.g. `obj.getA().getB().doSomething()`).

**Rust** (`field_expression` chaining):
```
(field_expression
  value: (call_expression
    function: (field_expression
      value: (call_expression)
    )
  )
) @demeter
```

**JS/TS** (`member_expression` chaining):
```
(member_expression
  object: (call_expression
    function: (member_expression
      object: (call_expression)
    )
  )
) @demeter
```

Python, Go, and other supported languages receive analogous queries on their respective chain node types (`attribute`, `selector_expression`).

The count is stored in a new `demeter_violations: u32` field on `FileComplexity`, populated by a new `count_demeter_violations()` function in `treesitter.rs`.

## File changes

| File | Change |
|---|---|
| `src/metrics/health.rs` | Remove `stale_code`, `churn_hotspots`, `temporal_coupling`, `file_complexity`; add `god_objects`, `complex_hotspots`; fix `bus_factor` aggregation |
| `src/metrics/coupling.rs` | New file: `temporal_coupling` (moved), `fan_out_coupling`, `demeter_violations` |
| `src/metrics/mod.rs` | Register `coupling` module |
| `src/metrics/complexity/queries.rs` | Add Demeter queries for all supported languages |
| `src/metrics/complexity/treesitter.rs` | Add `count_demeter_violations()`, extend `FileComplexity` |
| `src/snapshot.rs` | Add `demeter_violations` field to `FileComplexity` |
| `src/scorer.rs` | Register Coupling category, update `WEIGHTS` constant |
| `src/renderer/cli.rs` | Update action hints for new metric names |
| `src/renderer/html.rs` | No change (data-driven) |

## Testing

- `src/metrics/health.rs` — unit tests for `god_objects` and `complex_hotspots` with crafted snapshot fixtures; `bus_factor` test updated for new aggregation
- `src/metrics/coupling.rs` — unit tests for `fan_out_coupling` and `demeter_violations`; `temporal_coupling` test migrated from health
- `src/metrics/complexity/treesitter.rs` — inline test per language for the Demeter query against a depth-3 chain fixture
- `src/scorer.rs` — weight sum assertion updated to include Coupling

Mutation kill rate gate: ≥ 80%.
