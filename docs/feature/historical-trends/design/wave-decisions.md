# Wave Decisions — Historical Trends (DESIGN)

Feature: `historical-trends`
Wave: DESIGN
Date: 2026-03-18
Author: Morgan (Solution Architect)

---

## Inherited decisions from DISCUSS (read-only)

| ID  | Decision |
|-----|----------|
| D-01 | Auto-record on every `analyze` run — no opt-in flag |
| D-02 | Forward-only — no re-analysis of past commits (HARD CONSTRAINT) |
| D-03 | `--trend` flag for full history view; default shows compact delta only |
| D-04 | Branch isolation — deltas only for same-branch entries |
| D-05 | `--json` backward compat non-negotiable — trend key only when `--trend` specified |
| D-06 | Deduplication by commit SHA — idempotent consecutive runs |

---

## DESIGN decisions — open questions resolved

### DA-01: Deduplication override — `--force-trend` flag?

**Decision: Not implemented.**

Rationale: D-06 deduplicated by HEAD SHA is the only meaningful deduplication axis. A `--force-trend` flag would let callers inject duplicate data under the same commit, which corrupts velocity computations and violates idempotency. The only valid reason to override deduplication would be re-running a corrected analysis on the same commit, which D-02 already prohibits. The flag adds complexity without a safe use case. If a user needs to correct a record, they delete or archive the trend file manually and re-run.

Rejected alternative: Allow `--force-trend` but require `--no-cache` simultaneously. Rejected because two-flag combinations create undiscoverable UX and the use case remains unsafe for velocity integrity.

---

### DA-02: Trend store pruning — `max_trend_entries` config key?

**Decision: Not implemented in this release. Reserved in config schema as a commented-out no-op key.**

Rationale: The trend store uses NDJSON (one JSON object per line). File growth is bounded: one entry per unique HEAD commit. On a busy repo committing 20 times/day for a year, that is ~7 300 entries at ~400 bytes each = ~3 MB. This is negligible. Pruning introduces a write-amplification risk (rewriting the entire file to trim the head) and a data-loss risk (pruned entries cannot be recovered). The config schema will document the key for future use with a comment explaining it is not yet honoured. This avoids a breaking schema change later.

Rejected alternative: Implement pruning by keeping the most recent N entries. Rejected because it complicates velocity computation (the window must not extend past the oldest retained entry) and there is no evidence of a real storage constraint.

---

### DA-03: HTML trend tab behaviour — requires `--trend` or always shows if data exists?

**Decision: Always rendered when history data exists, regardless of `--trend`.**

Rationale: The HTML report is a self-contained artefact. If trend data has been collected it is already available in `report.history` (loaded in `main.rs` unconditionally before render). Hiding the tab based on a CLI flag creates asymmetry: the same HTML output file would silently omit a data tab that exists. The `--trend` flag controls the *CLI text output* and the *JSON `trend` key* — it does not control what the HTML embeds. This is consistent with how the HTML report already embeds all analysis data regardless of which CLI flags were set.

Rejected alternative: Gate HTML trend tab on `--trend`. Rejected because it requires threading an additional boolean into `renderer::html::render()` signature, coupling render to CLI flags, without user benefit.

---

### DA-04: Velocity computation — full history or rolling window?

**Decision: Rolling 8-entry window.**

Rationale: Full-history velocity is skewed by ancient baseline entries recorded during early project phases where the score moved significantly from first capture. An 8-entry window covers approximately 8 unique commits, which on most active repos spans days to weeks — long enough to show genuine trends, short enough to stay actionable. The window size 8 is consistent with common Agile rolling averages and sparklines that fit in a terminal line (FR-02 sparkline). The constant is defined as `VELOCITY_WINDOW: usize = 8` in `src/trend.rs` so it can be changed without searching callers.

Rejected alternative: Rolling 4-entry window. Rejected as too volatile; a single outlier commit dominates. Rejected: full history. Rejected because ancient baselines distort current trajectory.

Velocity formula: `(last_score - first_score_in_window) / (window_size - 1)`. Sign convention: positive = improving. Units: score points per run. Presented in CLI as `+N.N pts/run` or `-N.N pts/run`.

---

### DA-05: `schema_version` upgrade path

**Decision: Archive-and-replace on unrecognised `schema_version`.**

Rationale: The NDJSON trend file stores one `HistoryEntry` JSON object per line. There is no file-level schema version in the current `history.json` (only per-entry fields). Adding a new field to `HistoryEntry` is backward-compatible via `#[serde(default)]`. For breaking structural changes (renaming a field, changing a type), the archive-and-replace strategy from FR-08 is reused: rename the existing file to `history.json.bak`, create a fresh file, and emit a warning. This matches the corrupt-file recovery pattern already specified.

Implementation: `src/trend.rs` reads entries and skips malformed lines (already done in `cache::history::load_history` with the silent `if let Ok` pattern). An explicit `schema_version` u32 field is added to `HistoryEntry` (default = 0 via `#[serde(default)]`). The current schema is version 1. When the loader encounters entries with `schema_version > CURRENT_SCHEMA_VERSION` it triggers the archive-and-replace path.

---

## Development paradigm decision

**Decision: Functional-first Rust.**

Rationale: Rust is multi-paradigm but its type system, ownership model, and standard library idioms lean heavily functional: iterators, `Option`/`Result` chaining, `map`/`filter`/`fold`, immutable-by-default bindings. The trend feature is predominantly a data transformation pipeline: load entries → filter by branch → compute delta → compute sparkline → compute velocity → render. This decomposes naturally into pure functions with no mutable shared state. The `TrendSummary` struct is an immutable value computed once and passed by reference to renderers. No trait objects or inheritance hierarchies are warranted.

The existing codebase already uses this style throughout (`scorer.rs`, `metrics/`, `renderer/`). Introducing an OOP-style `TrendService` with mutable internal state would break with the established codebase conventions.

Concrete guidance for software-crafter: prefer iterator chains over mutable loops, prefer `Result<T, E>` propagation via `?` over explicit match blocks, keep all functions in `src/trend.rs` pure (no I/O side effects — I/O stays in `cache/trend_store.rs`).
