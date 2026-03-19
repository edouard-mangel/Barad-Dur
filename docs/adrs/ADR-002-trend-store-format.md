# ADR-002: Trend Store Format

**Status**: Accepted
**Date**: 2026-03-18
**Feature**: historical-trends
**Deciders**: Morgan (Solution Architect)

---

## Context

The `historical-trends` feature requires persistent storage of scored analysis snapshots, one per unique commit HEAD, to enable delta computation, sparkline generation, and velocity tracking.

**Constraints from DISCUSS wave**:
- D-02: Forward-only (no re-analysis)
- D-06: Deduplication by commit SHA
- NFR-05: `.repository-analysis/` already gitignored — the store lives there
- NFR-01: Trend recording ≤0.5s overhead, no additional git calls

**Pre-existing state**: `src/cache/history.rs` already implements a working NDJSON append-only store in `.repository-analysis/history.json` with SHA deduplication. This was written before the `historical-trends` feature specification was formalised.

**Access pattern**: write once per run (append one line), read once per run (sequential full scan). No random access, no update-in-place, no cross-entry joins. File growth is bounded by unique commit count: a busy repo at 20 commits/day for 3 years = ~21 900 entries × ~400 bytes = ~8.8 MB maximum.

---

## Decision

**Use NDJSON (Newline-Delimited JSON) in `.repository-analysis/trends.json`.**

One JSON object per line. The file is append-only. Reading is a sequential line scan. The existing `cache::history` module implements this; the only change is renaming the constant from `"history.json"` to `"trends.json"`.

Justification:
- The implementation already exists and is tested (4 unit tests in `cache/history.rs`)
- NDJSON is human-readable: `cat .repository-analysis/trends.json | jq .` works out of the box
- NDJSON is streamable: tools like `jq`, `grep`, Python's `json` module, and shell scripts can process it line by line without loading the entire file
- Appending a single line is an O(1) file operation with no read-modify-write cycle
- Corrupt or malformed lines are skipped per-line, not per-file — one bad entry does not lose the entire history
- Zero new dependencies

---

## Alternatives Considered

### SQLite (via `rusqlite` crate)

**Rejected.**

Pros: structured queries, indexed lookups, atomic transactions, schema versioning via `PRAGMA user_version`.
Cons:
- Introduces a C FFI dependency (rusqlite links to libsqlite3 or bundles it). This complicates cross-compilation targets (MUSL, Windows, ARM) that are part of the existing CI matrix.
- A relational schema is over-engineering for an append log with no joins. There is only one table and all reads are `SELECT * FROM entries WHERE branch = ? ORDER BY timestamp`.
- SQLite is 2–5 MB of bundled C when compiled statically, increasing binary size meaningfully.
- The access pattern (append + full scan) offers no benefit from B-tree indices.
- Requires a migration system for schema evolution (which NDJSON handles with `#[serde(default)]`).

### Bincode (matching the existing snapshot cache format)

**Rejected.**

Pros: compact, fast serialisation, zero text overhead.
Cons:
- Not human-readable. Users cannot inspect their trend history without a custom tool.
- Bincode does not support appending: each write requires deserialising the full file, appending in memory, and rewriting — O(N) write cost that grows with file size.
- Schema evolution requires explicit versioning (Bincode has no built-in skip-unknown-fields).
- Appropriate for the snapshot cache (internal, ephemeral, large object); inappropriate for user-visible persistent state.

### MessagePack (via `rmp-serde`)

**Rejected** for the same reasons as Bincode, plus it adds a new crate dependency.

### CSV

**Rejected.** CSV is adequate for scalar-only data but the `categories` and `metrics` fields are maps (variable keys), which require encoding as multi-column wide format or serialised sub-strings. This complexity eliminates CSV's readability advantage while retaining its limitations (no schema, no types, quoting ambiguity).

---

## Consequences

**Positive**:
- Zero new dependencies
- Existing implementation reused (no new code for persistence layer)
- Human-readable and tool-friendly
- Graceful single-line error recovery

**Negative**:
- Full sequential scan on every render (mitigated: bounded file size, modern storage makes this negligible at expected scale)
- No indexed queries (not required by current feature set)
- `trends.json` file name differs from the existing `history.json`. **Migration rule**: on first run after upgrade, if `.repository-analysis/history.json` exists and `.repository-analysis/trends.json` does not, `cache::history` copies `history.json` to `trends.json` before appending. The original `history.json` is retained (non-destructive). This one-time forward migration preserves existing trend data for users who ran the pre-release version.
