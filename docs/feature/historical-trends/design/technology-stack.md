# Technology Stack — Historical Trends

Feature: `historical-trends`
Wave: DESIGN
Date: 2026-03-18

---

## Principle

This feature adds zero new runtime dependencies. All required capabilities already exist in the project's dependency tree or the Rust standard library.

---

## Existing dependencies in use (no version change required)

| Dependency | License | Use in this feature |
|---|---|---|
| `serde` + `serde_json` | MIT/Apache-2.0 | NDJSON serialisation/deserialisation of `HistoryEntry` |
| `chrono` | MIT/Apache-2.0 | Timestamps in `HistoryEntry` and `SparklinePoint` |
| `colored` | MPL-2.0 | Coloured delta output in CLI renderer |
| `anyhow` | MIT/Apache-2.0 | Error propagation in `cache/history.rs` |
| `std::collections::HashMap` | stdlib | `categories` and `metrics` maps in `HistoryEntry` |

---

## No new dependencies required

### Velocity computation
A rolling mean over at most 8 `f64` values. Pure arithmetic; no numerical library needed.

### Sparkline rendering
Unicode block character mapping via a lookup array `['▁','▂','▃','▄','▅','▆','▇','█']`. Pure Rust `char` manipulation; no crate needed.

### NDJSON persistence
`serde_json::to_string()` per entry + `writeln!`. Already implemented in `cache::history`. No streaming JSON library needed.

### HTML Trends tab
The trend data is embedded in `window.R.history` (already the case after this feature). The frontend D3 chart and vanilla JS DOM manipulation are already part of the HTML renderer's bundled output. The Trends tab is a new tab section added to the existing inline JavaScript, not a new bundle.

---

## Rejected alternatives

### SQLite (via `rusqlite` crate)

Rejected. Introduces a C FFI dependency, complicates cross-compilation (important for CI), and is significant complexity overhead for what is essentially an append log with sequential reads. SQLite is the correct tool when you need ad-hoc queries, transactions, or indexed lookups. None of those requirements exist here. The trend store is written once per run and read in full once per render. NDJSON is simpler, debuggable with any text editor, and zero-dependency.

### Bincode (already used for snapshot cache)

Rejected for trend store. Bincode is not human-readable. The trend file should be inspectable by users and parseable by external tools (grep, jq, scripts). It is appropriate for the snapshot cache because that file is an internal performance optimisation not meant for external consumption. Trend data is user-visible state.

### MessagePack (via `rmp-serde`)

Rejected for the same reasons as bincode, plus it adds a dependency.

---

## Build and tooling impact

| Concern | Assessment |
|---|---|
| Compile time | No change (no new crates) |
| Binary size | +3–5 KB estimated (new `trend.rs` module) |
| `cargo deny` | No new licenses to review |
| CI pipeline | No changes needed |
| Cross-compilation | No change (all pure Rust) |
