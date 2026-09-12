# Stable Metric Identity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Let metric and category wording change without changing advice, navigation, weighting, or historical identity.

**Architecture:** Rust owns stable metric/category IDs and their label/history-key catalog. Behavioral consumers select by ID. Reports carry IDs alongside labels; presentation-specific advice/tooltips retain their owners and receive explicit catalog coverage checks.

**Tech Stack:** Existing Rust/Serde, M01-generated TypeScript and decoder, HTML JavaScript, existing test tooling.

**Spec:** [M04 finding](../../reviews/2026-09-07-structure-maintainability.md#m04--give-metrics-stable-identities-independent-of-display-names) and the design below.

## Global constraints

- Planning only; no code, probes, implementation, or history migration is authorized.
- Preserve current labels, scores, advice wording, configuration names, CLI flags, and history continuity during this refactor.
- Report JSON follows the user's current-only policy; no older-report adapters are required.
- That JSON policy does not authorize deleting, archiving, or resetting existing trend histories.
- Keep identity separate from calculation, display wording, and persistence compatibility.

## Design decisions

Use stable Rust identities with explicitly fixed serialized strings, not enum ordinals or strings derived from current display labels. Category IDs are lowercase identifiers; metric IDs are category-qualified identifiers to avoid accidental name collisions. Once released, an ID must not be recycled for a different meaning.

A domain catalog owns each built-in identity, its default label, category association, and stable persisted-history key. Metric descriptions may remain calculation-specific when they contain result-dependent detail. Advice stays with scorer actions; frontend-specific tooltip copy stays with its renderer, keyed by generated IDs and checked for complete dispositions.

Do not create a universal metadata service or runtime plugin registry. Exhaustive Rust mappings and producer/consumer coverage tests are sufficient for the current built-in catalog.

### History policy: preserve existing persisted keys

For existing metrics/categories, freeze the current historical key as an explicit catalog value. History writers use that value rather than the mutable display label. Readers continue reading existing keys; internal comparisons can resolve known keys to IDs without rewriting stored files. New metrics receive a stable history key at introduction.

This plan intentionally does not migrate historical maps to new ID strings. Unknown historical keys remain preserved and must not be guessed from fuzzy label matching. No history-schema reset is needed solely for adding identity; scoring-formula version changes retain their existing separate policy.

## Task 1: Inventory identity-dependent behavior

**Files:** src/metrics/ constructors, src/scorer/actions.rs, src/scorer.rs, src/config/, src/cmd/gate.rs, src/cache/history.rs, src/trend.rs, renderer templates, dashboard consumers.

- [ ] Inventory every built-in category/metric, current label, persisted key, and selection/advice/navigation/tooltip consumer.
- [ ] Identify duplicate labels and explicitly distinguish same-identity reuse from different metrics sharing wording.
- [ ] Classify string comparisons as identity-dependent, human-readable output, or historical/configuration compatibility.
- [ ] Record intentional absence of special advice or tooltip content; do not treat every generic fallback as a defect.

**Acceptance:** Every built-in metric has an explicit identity and consumer disposition; no identity is inferred from mutable text during migration.

## Task 2: Introduce the Rust catalog

**Files:** New src/metrics/identity.rs; src/metrics/mod.rs and metric constructors; M02 selection types where present.

**Contract:** Metric/category identity is stable; labels are presentation data; persisted keys are a separate explicit mapping.

- [ ] Define and serialize fixed identities, preserving public labels as catalog metadata.
- [ ] Assign identities at metric/category construction, not by inspecting completed display strings.
- [ ] Check unique IDs and persisted keys where uniqueness is required, category membership, and complete catalog coverage.
- [ ] Add generated-type annotations through M01's optional feature without changing default build requirements.
- [ ] Keep registry-like discovery and user-defined dynamic metrics outside scope.

**Acceptance:** Adding a built-in metric requires an explicit identity/catalog decision and cannot silently fall through an unreviewed string path.

## Task 3: Migrate Rust behavioral consumers

**Files:** src/scorer/actions.rs, src/scorer.rs, src/config/, src/cmd/gate.rs, M02 analysis selection, relevant tests.

- [ ] Select advice, category-specific additions, navigation targets, and weighting by ID.
- [ ] Translate existing configuration keys and CLI flags at their boundaries; retain their public names and validation behavior.
- [ ] Preserve intentional generic advice by expressing it as a catalog disposition rather than an accidental unmatched label.
- [ ] Test a changed display label with the same ID: scores, weighting, advice selection, and targets must remain unchanged.

**Acceptance:** Wording is not a behavioral switch. No user-visible scoring or CLI/configuration change is bundled into the extraction.

## Task 4: Preserve history independently of labels

**Files:** src/scorer.rs or M02 history-summary builder, src/cache/history.rs, src/trend.rs, backfill/trend tests.

- [ ] Write existing stable persisted keys from catalog metadata, not category/metric display text.
- [ ] Compare known historical values using their explicit key-to-ID relationship where needed.
- [ ] Cover pre-refactor history, changed current labels, unknown keys, null metrics, duplicate-head handling, and configured category selection.
- [ ] Verify history output remains equivalent and existing files are neither rewritten nor archived by the identity change.

**Acceptance:** A label rename retains trend continuity without older-report compatibility code, a history migration, or a schema reset.

## Task 5: Carry identity through reports and presentation

**Files:** src/metrics/mod.rs, generated dashboard report declarations, dashboard/src/report/decode.ts, src/renderer/templates/chrome.js and overview consumers, relevant frontend tests/fixtures.

- [ ] Serialize current IDs alongside labels and regenerate M01 declarations/fixtures during implementation.
- [ ] Require valid current IDs in the dashboard decoder; reject incompatible older shapes without introducing fallback label matching.
- [ ] Key HTML tooltip and other identity-dependent presentation lookup by IDs while continuing to display labels.
- [ ] Add coverage between generated identities and tooltip/advice dispositions; keep renderer-specific copy outside calculation modules.
- [ ] Coordinate touched JavaScript modules with M05 so behavior is migrated once, whether before or after bundling.

**Acceptance:** Renaming displayed text does not lose tooltip lookup or navigation. Current reports remain producer/consumer checked.

## Task 6: Sweep identities and release guidance

- [ ] Enumerate every remaining metric/category string comparison and record why it is presentation, compatibility, or still a defect.
- [ ] Run full Rust tests, generated contract checks, dashboard tests/build, HTML smoke, and required P1/P2 gates during implementation.
- [ ] Compare recommendation text and history keys before/after; no baseline change is expected merely from introducing IDs.
- [ ] Document ID stability, new-metric registration, and the deliberate separation between current-only report JSON and preserved history keys.

**Completion:** Identity survives a label rename across every behavioral consumer, with exhaustive catalog coverage and unchanged existing history.

## Evidence and dependencies

This plan follows M01 and should use M02's selection boundary if already delivered. M06's neutral type ownership keeps IDs in metrics, not the report orchestrator. Source inspection established name-keyed consumers; catalog completeness, serialization changes, and label-rename parity remain future checks under the [review process](../../review-process.md).
