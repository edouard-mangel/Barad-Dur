# P2 decision-surface audit — Calculation boundaries and time

Branch: `refactor/calculation-boundaries-time`; source revision `3b33f14`, implementation base `47b6c26`.
Corpus: all 11 configured repositories, including the private local entries.

`make field-test` exited 0: `field test clean across 11 repositories`. Both passes matched the committed decision surfaces and each other. No baseline, pin, or history-window change was accepted. `make field-audit` exited 0 and emitted only these five existing rotation rows for barad-dur at pin `dc23fbd6`.

## Sampled recommendations

| # | Recommendation | True? | Safe? | Actionable? | Evidence and notes |
|---|---|---|---|---|---|
| 1 | [Coupling] Change coupling smells (score: 25) — Decouple cross-boundary co-changing files by introducing interfaces or shared abstractions | yes | yes | yes | The current archived report records 774 qualifying cross-boundary pairs, 282 cross-community pairs and 101/140 affected source files. The Coupling view supplies concrete pairs to investigate; the advice does not prescribe a destructive edit. |
| 2 | [Health] God objects (score: 30) — Break down large files by extracting responsibilities into smaller modules | yes | yes | partial | The report identifies 22/139 oversized or structurally overconnected source files. The signal and conservative advice are supported, but the per-file responsibility clusters below include inline tests. |
| 3 | [Health] src/cmd/analyze.rs — 510 loc; structural hub — 10 connections (threshold 8) — consider splitting by responsibility: build_* (6), compute_* (2) | yes | yes | partial | The size/hub evidence is unchanged. At the pin, `compute_*` functions at lines 216 and 304 are production, but only one `build_*` function is production (line 349); five are tests at lines 546–611, after `mod tests` at 428. The earlier audits overstated this cluster's production relevance. |
| 4 | [Health] src/config/mod.rs — 677 loc — consider splitting by responsibility: validate_* (22) | yes | yes | no | `git show dc23fbd6:src/config/mod.rs` places all 22 `validate_*` functions inside the test module (starts at line 332; functions at 458–795). Production has one `validate`, at line 224. The proposed production responsibility is misleading. |
| 5 | [Health] src/renderer/cli/mod.rs — 642 loc — consider splitting by responsibility: render_* (22) | yes | yes | no | At the pin, eight production `render_*` functions precede `mod tests` at line 322; fourteen test functions at 421–614 inflate the reported cluster to 22. The suggested split needs production-only evidence. |

## Recurring follow-up: inline-test responsibility clustering

This is the pre-existing Actionable defect recorded in [the September 7 audit](2026-09-07-long-methods-recalibration.md), reproduced again on the unchanged pinned corpus. It remains an explicit follow-up, not a newly introduced regression or an unreported minor. Row 3 documents an additional affected recommendation from the same defect and corrects the prior audit's assertion that its named clusters were production functions.

Follow-up acceptance: exclude inline test functions when calculating suggested production responsibility clusters; cover the pinned config, CLI-renderer and analyze examples; review and explicitly accept any resulting decision-surface changes in that feature's own commit. Changing that calculation is outside M06's ownership/time refactor, which is required to preserve formulas and baselines.

Safe failures: none. No new or withdrawn recommendations. Two fully non-actionable rows and two partially actionable rows remain due to this known clustering defect. The audit does not claim every existing recommendation is satisfactory.

Evidence inspected: `field-test/archive/barad-dur-0.json` (generated, ignored), the three pinned source files through `git show`, and the actual worksheet from `make field-audit`. The driver currently has no persistent rotation state, so these repeated rows are the harness-selected sample, not a fresh random sample across all repositories.
