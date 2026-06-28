# Upstream Changes — manifest-hotspot-exclusion (DESIGN → DISCUSS back-propagation)

DESIGN took a different direction than DISCUSS specified. Per nWave back-propagation
rules, the affected DISCUSS assumptions are recorded here for product-owner (Luna)
review. **DISCUSS documents are not edited** — they remain historical record.

## What changed

| Aspect | DISCUSS assumption | DESIGN decision |
|--------|--------------------|-----------------|
| Mechanism | Snapshot-level default exclusion of manifests (`DEFAULT_EXCLUDE_PATTERNS` in `exclude.rs`) | Presentation-layer **manual dismiss** of any hotspot row (mirror coupling-pair dismissal) |
| Manifest specificity | Core-ecosystem manifest glob list, auto-excluded | **No manifest heuristic** — generic per-row dismiss; manifests are just the motivating example |
| Data impact | Manifests dropped from snapshot before metrics | Data unchanged; rows filtered at render time only |
| Surfaces | Collector (one place) | HTML report (`hotspots.js`) **and** React dashboard (`HotspotsView.tsx`) |
| Persistence | n/a | Ephemeral client state (lost on reload), v1 |

## Quoted original assumptions now superseded

- requirements.md **FR-1**: "each core-ecosystem manifest is excluded from the
  analyzed file set" → **superseded**. Files are no longer excluded from analysis;
  they are dismissible from the hotspots *view*.
- requirements.md **FR-2/FR-3**, **NFR-1** (deps/coupling safety), **C-1/C-2/C-3** →
  **moot**. Collection is untouched, so the deps/coupling safety invariant is
  trivially satisfied and the opt-out-granularity question (C-1) disappears
  ("Reset dismissed" + not-dismissing-it *is* the opt-out).
- acceptance-criteria.md **AC-1..5** (unit tests on `is_excluded`) → **replaced**.
  See new acceptance criteria below.
- story-map.md S1/S2/S3 framing (manifest exclusion + toggle + richer parsing) →
  S1 redefined; the richer-parsing idea (S3) is unaffected and still in `Ideas.md`.

## Replacement acceptance criteria (DESIGN-level, pending Luna's ratification)

- **AC-D1**: In the HTML report hotspots table, each row has a dismiss (`×`) control;
  clicking it hides that row and updates a "N dismissed" status line.
- **AC-D2**: A "Reset dismissed" control restores all dismissed rows.
- **AC-D3**: Dismissal is keyed by file path — sorting/filtering the table keeps the
  correct rows dismissed.
- **AC-D4**: The React dashboard hotspots table provides the same dismiss + reset
  behavior.
- **AC-D5**: No change to `report.json` / `HotspotFile` data or any metric output.
- **AC-D6** (Rust-testable): rendered hotspots HTML contains the dismiss control
  markers (class + "Reset dismissed" label).

## Naming note
The feature id `manifest-hotspot-exclusion` is now a mild misnomer — the feature is
*generic hotspot-row dismissal*. Recommend keeping the id to avoid directory churn,
but Luna may rename to e.g. `hotspot-row-dismissal` if preferred.

## Action for product owner
- [ ] Ratify the mechanism pivot (presentation dismiss vs snapshot exclusion).
- [ ] Accept replacement acceptance criteria AC-D1..D6 (or amend).
- [ ] Decide on feature-id rename (optional).
