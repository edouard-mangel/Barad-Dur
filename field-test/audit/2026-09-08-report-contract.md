# P2c decision-surface audit — Report contract

Merge under audit: `refactor/report-contract`.
Corpus: all 11 entries, full local sweep.

This refactor changes the generated dashboard contract, runtime decoding, and
loading boundaries. It does not change Rust calculations, scores, evidence, or
recommendation generation. `make field-test` confirmed no decision-surface
regression and deterministic output across all 11 pinned repositories.

## barad-dur

The audit driver emitted five rotation rows. They are the same pre-existing
recommendations reviewed in
`field-test/audit/2026-09-07-long-methods-recalibration.md`.

| # | Recommendation | True? | Safe? | Actionable? | Notes |
|---|---|---|---|---|---|
| 1 | [Coupling] Change coupling smells (score: 25) — Decouple cross-boundary co-changing files by introducing interfaces or shared abstractions | yes | yes | yes | The Coupling evidence names the qualifying pairs, so maintainers can identify boundaries to investigate. |
| 2 | [Health] God objects (score: 30) — Break down large files by extracting responsibilities into smaller modules | yes | yes | partial | The finding is supported and the advice is safe, but two per-file responsibility clusters remain inflated by inline test functions. |
| 3 | [Health] src/cmd/analyze.rs — 510 loc; structural hub — 10 connections (threshold 8) — consider splitting by responsibility: build_* (6), compute_* (2) | yes | yes | yes | The size, hub degree, and production function clusters support the recommendation. |
| 4 | [Health] src/config/mod.rs — 677 loc — consider splitting by responsibility: validate_* (22) | yes | yes | no | The `validate_*` cluster consists predominantly of inline test functions, so the proposed production responsibility is misleading. Pre-existing Actionable failure. |
| 5 | [Health] src/renderer/cli/mod.rs — 642 loc — consider splitting by responsibility: render_* (22) | yes | yes | no | The cluster mixes production renderers with many inline test functions. Pre-existing Actionable failure. |

## Outcome

- Safe failures: none; the refactor is not blocked.
- True failures: none.
- Actionable failures: two pre-existing god-object clustering defects, already
  recorded by the prior audit and unaffected by this change.
- Decision-surface changes from this refactor: none.
