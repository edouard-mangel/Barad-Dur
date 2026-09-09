# P2 decision-surface audit — Entity-history correctness

Merge under audit: `feat/per-entity-trend-history`.
Corpus: all 11 configured repositories.

`make field-test` reported no decision-surface regression. `make field-audit`
therefore emitted only the five existing barad-dur rotation rows below; the
entity-history fingerprint and pair-key changes introduced no recommendation
changes.

## barad-dur

| # | Recommendation | True? | Safe? | Actionable? | Notes |
|---|---|---|---|---|---|
| 1 | [Coupling] Change coupling smells (score: 25) — Decouple cross-boundary co-changing files by introducing interfaces or shared abstractions | yes | yes | yes | The score reflects measured cross-boundary co-change pairs; the Coupling report identifies the pairs to investigate. |
| 2 | [Health] God objects (score: 30) — Break down large files by extracting responsibilities into smaller modules | yes | yes | partial | The large-file signal is real, but rows 4 and 5 show that inline tests inflate some responsibility clusters. |
| 3 | [Health] src/cmd/analyze.rs — 510 loc; structural hub — 10 connections (threshold 8) — consider splitting by responsibility: build_* (6), compute_* (2) | yes | yes | yes | The LOC and structural-hub evidence are factual; the named production responsibilities provide a starting point. |
| 4 | [Health] src/config/mod.rs — 677 loc — consider splitting by responsibility: validate_* (22) | yes | yes | no | All 22 `validate_*` functions are tests. The recommendation's proposed responsibility does not exist in production code. |
| 5 | [Health] src/renderer/cli/mod.rs — 642 loc — consider splitting by responsibility: render_* (22) | yes | yes | no | Inline test functions inflate this cluster too, so the proposed split is not actionable as written. |

## Outcome

- Safe failures: none.
- True failures: none.
- Actionable failures: two pre-existing rotation findings, both caused by the
  god-object prefix clustering counting functions inside inline test modules.
  They are independent of this merge and already documented in the
  2026-09-07 audit.
