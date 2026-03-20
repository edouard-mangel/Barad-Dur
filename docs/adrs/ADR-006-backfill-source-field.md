# ADR-006: HistoryEntry gains optional source field for backfill provenance

**Status**: Accepted
**Date**: 2026-03-19
**Feature**: adaptive-trends-period
**Deciders**: Morgan (Solution Architect)

---

## Context

`trends.json` stores a flat array of `HistoryEntry` records. Before this feature, every entry in the store was produced by `barad-dur analyze` running at HEAD — all entries were homogeneous.

The `backfill` subcommand introduces a second kind of entry: retroactive analysis at a historical SHA. These backfill entries have structural differences that affect correctness of downstream consumers:

1. **Absent complexity scores**: backfill entries carry `file_metrics = {}` (see ADR-005). A dashboard rendering complexity trends would show a misleading flat line for the backfill period unless it knows these entries lack complexity data.

2. **Re-backfill targeting**: if a user runs `barad-dur backfill` twice with different `sample_count` values, or if the backfill feature is changed to include complexity data in a future release, it must be possible to identify and replace only the backfill entries without touching live entries.

3. **Audit and debugging**: support tooling (e.g., `jq`-based inspection of `trends.json`) benefits from being able to filter entries by their origin.

Without a provenance field, `trends.json` is opaque about entry origin after the fact.

---

## Decision

Add `pub source: Option<String>` to `HistoryEntry` with serde annotation `#[serde(skip_serializing_if = "Option::is_none")]`.

- **Live `analyze` entries**: `source` is `None`. The field is omitted from serialized JSON entirely. Existing `trends.json` files are unaffected.
- **Backfill entries**: `source` is `Some("backfill".to_string())`. The field appears in JSON as `"source": "backfill"`.

The string value `"backfill"` is a stable constant owned by `backfill::run`. Future sources (e.g., `"import"`, `"migration"`) can use different string values without schema changes.

---

## Alternatives Considered

### Alternative 1: Schema version bump (`schema_version: 2`)

Increment `schema_version` to signal that entries may have absent complexity data.

**Rejected**: `schema_version` is a document-level marker for structural breaking changes, not a per-entry content marker. A version bump would require migrating all existing entries in `trends.json` and would not convey which specific entries are backfill entries vs live entries. The field granularity is wrong.

### Alternative 2: Separate file for backfill entries (`backfill.json`)

Store backfill entries in a separate file, keep `trends.json` for live entries only.

**Rejected**: DISCUSS decision D-05 explicitly requires writing to the existing `trends.json` store. Splitting the store would require changes to `compute_trend`, `load_history`, and all dashboard consumers that read the single store. It also prevents the dashboard from rendering a unified timeline with both backfill and live entries interleaved.

### Alternative 3: Boolean flag (`is_backfill: bool`)

Add `pub is_backfill: bool` instead of `Option<String>`.

**Rejected**: A boolean cannot express future provenance categories without a schema change. `Option<String>` with `skip_serializing_if` is strictly more expressive and equally backward-compatible. The string `"backfill"` is self-documenting in the JSON file; `true` is not.

### Alternative 4: `Option<String>` with `skip_serializing_if = "Option::is_none"` (this ADR)

Add an optional string field that is absent from JSON when `None`, present as `"source": "backfill"` for backfill entries.

**Accepted**: Backward-compatible (existing entries without the field deserialize to `source = None`), self-documenting, extensible to future sources, and satisfies the provenance requirements without a schema version bump.

---

## Consequences

### Positive

- Existing `trends.json` files without a `source` field remain valid: serde deserializes missing optional fields as `None`
- No `schema_version` bump required
- The dashboard can render backfill entries with distinct styling (hollow dots, tooltip) using `source === "backfill"` in JavaScript
- Safe targeted re-backfill: a future `barad-dur backfill --replace` can identify and remove only entries where `source == "backfill"` before re-seeding
- JSON inspection via `jq` is straightforward: `jq '[.[] | select(.source == "backfill")]' trends.json`
- The field name `source` is generic enough to accommodate future values (`"import"`, `"ci-migration"`) without schema changes

### Negative

- Live `analyze` entries must now be kept consistent: callers of `build_history_entry` must explicitly pass `source = None` (or equivalent) to avoid accidentally tagging live entries
- The string value `"backfill"` is a stringly-typed constant; a typo in a future caller would silently produce an unrecognized source value. Mitigation: define the constant in `backfill::run` and reference it by name

### No action required for

- Existing `trends.json` files (backward-compatible deserialization)
- `compute_trend` and velocity computation (both operate on `HistoryEntry.overall_score` and branch, unaffected by `source`)
- `load_history` and `append_if_new_head` (transparent to the `source` field)
