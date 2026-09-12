# Maintainability Refactor Plans

Date: 2026-09-07

Source: [structure review](../reviews/2026-09-07-structure-maintainability.md).

Status: all six prose-only plans are written. Implementation, probes, commits, and releases are not authorized. Plan completion does not mean implementation verification has passed.

## Plan index

| Finding | Plan | Intended result |
|---|---|---|
| M01 Report boundary | [Report contract](../superpowers/plans/2026-09-07-report-contract.md) | Rust-generated TypeScript, current-only runtime decoding, explicit report thresholds |
| M02 Analysis orchestration | [Shared analysis](../superpowers/plans/2026-09-07-analysis-orchestration.md) | Reusable calculations/evidence and summaries separate from display enrichment |
| M03 Snapshot assembly | [Named snapshot assembly](../superpowers/plans/2026-09-07-snapshot-assembly.md) | Shared aggregation/resolution/construction with explicit live/historical readers |
| M04 Stable identity | [Metric and category identity](../superpowers/plans/2026-09-07-stable-metric-identity.md) | Behavior keyed by stable IDs, labels free to change, existing history keys preserved |
| M05 HTML modules | [Embedded report modules](../superpowers/plans/2026-09-07-html-report-modules.md) | Explicit source dependencies and a committed offline bundle |
| M06 Calculation boundaries/time | [Calculation ownership and time](../superpowers/plans/2026-09-07-calculation-boundaries-time.md) | Domain-owned results, neutral score policy, explicit reference time |

Each plan contains design choices, constraints, affected files, independently reviewable tasks, concrete acceptance cases, dependencies, and future verification requirements. Implementation snippets have been removed from M01.

## Confirmed user decisions

- Generate TypeScript declarations from Rust, with a separate runtime decoder.
- No backward compatibility for older report JSON.
- Finish plans only: no implementation code or further probes.

Current nullable scores and fields deliberately omitted by the current serializer remain valid. An older report already matching the current consumed shape can load; incompatible shapes receive regeneration guidance. This does not authorize deleting reports or resetting snapshot caches/history.

## Planning choices

The following are explicit conservative design recommendations, not additional product decisions attributed to the user.

| Area | Planned choice | Reason |
|---|---|---|
| Analysis | Shared immutable calculation results; commands retain I/O and policy | Removes duplication without mixing collection/network/history modes |
| Snapshot | Collector-internal named records; unchanged serialized snapshot | Reduces propagation work without cache migration |
| Identity | Stable Rust IDs and explicit metadata; retain current persisted history keys | Wording changes no longer alter behavior or require rewriting history |
| HTML | Explicit modules plus an explicitly generated, committed single-script bundle | Preserves offline HTML and ordinary Cargo builds without Node |
| Time | Explicit invocation reference; preserve current historical scoring semantics | Improves testability without silently changing what age scores mean |

As-of-commit historical scoring, history-key migration, cache-format changes, new dashboard tabs, and public CLI/configuration redesign are outside these refactors.

## Recommended implementation order

1. **M01:** repair the report boundary and establish producer/consumer checks.
2. **M06:** establish neutral calculation ownership and explicit time inputs.
3. **M02:** reuse those boundaries while sharing orchestration and evidence.
4. **M03:** consolidate collector assembly without changing the snapshot contract.
5. **M04:** introduce stable identities through the established analysis/report boundaries.
6. **M05:** modularize HTML consumers after any identity-key changes are settled.

This order minimizes overlapping edits; it is not a hard dependency chain. M03 can proceed independently. M05 can also be prepared separately, provided M04's tooltip/lookup migration is reconciled once. M06's type moves preserve public re-exports and M01 generation attributes.

## Ownership across plans

- M01 owns wire generation, runtime decoding, dashboard formatting, and loading.
- M02 owns shared selection/orchestration and reuse of derived evidence.
- M03 owns collector intermediates, acquisition provenance, and final assembly.
- M04 owns stable IDs, catalog coverage, and stable historical-key mapping.
- M05 owns embedded JavaScript modules, bundle production, and distribution.
- M06 owns calculation-result placement, score-band policy placement, and reference-time inputs.

Do not move the same type in M01, M02, and M06. Do not merge historical JSON compatibility with report-upload compatibility. Do not make the HTML module refactor depend on the React dashboard at runtime.

## Verification required during future implementation

Follow the [review process](../review-process.md):

- P0: before implementation readiness, verify external API, grammar, and type-shape claims; record actual output or mark a claim unverified. No new probes were performed for this prose-only planning delivery.
- P1: enumerate every consumer of each plan's invariants, not only edited files.
- P2: run required corpus regression/determinism and recommendation audits before merge; a Safe failure blocks merging.
- Run full Rust suites where applicable, never a library-only substitute; include formatting/Clippy and feature-gated targets.
- Run affected dashboard, generated-contract, HTML smoke, bundle, and packaging checks.
- Explain behavior/decision-surface changes; do not silently accept baselines, migrate data, or relax compatibility tests.
- Preserve unrelated user work and document any remaining verification limits.

Earlier M01 scratch checks are historical evidence summarized in its [design](../superpowers/specs/2026-09-07-report-contract-design.md). They are not substitutes for application verification. Proposed bundle configuration, new helper interfaces, parity, and packaging remain unverified until an authorized implementation phase.

## Completion of the planning request

All six findings now have separate prose-only plans. No implementation steps are marked complete. The next phase requires a separate request to implement; these documents do not start it automatically.
