# P2 decision-surface audit — Snapshot assembly

Branch: `refactor/snapshot-assembly`; source revision `42aecac` (implementation `ba006f7`), implementation base `fa02644`.
Corpus: all 11 configured repositories, including the private local entries (`.corpus` linked from the main checkout).

`make field-test` exited 0: `field test clean across 11 repositories`. Both passes matched the committed decision surfaces and each other. No baseline, pin, or history-window change was accepted. `make field-audit` exited 0 and emitted the same five rotation rows for barad-dur at pin `dc23fbd6` as the September 7, 10 and 12 (M02) audits — the driver still has no rotation state.

## Sampled recommendations

| # | Recommendation | True? | Safe? | Actionable? | Evidence and notes |
|---|---|---|---|---|---|
| 1 | [Coupling] Change coupling smells (score: 25) — Decouple cross-boundary co-changing files by introducing interfaces or shared abstractions | yes | yes | yes | Unchanged pin, unchanged surface: the sweep compares the full decision surface and reported no difference. The advice names no destructive edit. |
| 2 | [Health] God objects (score: 30) — Break down large files by extracting responsibilities into smaller modules | yes | yes | partial | Unchanged; the per-file clusters in rows 3–5 still count inline tests. |
| 3 | [Health] src/cmd/analyze.rs — 510 loc; structural hub — 10 connections (threshold 8) — consider splitting by responsibility: build_* (6), compute_* (2) | yes | yes | partial | Evidence is at the pin, where five of the six `build_*` are tests (September 10 audit). This branch does not touch `analyze.rs`. |
| 4 | [Health] src/config/mod.rs — 677 loc — consider splitting by responsibility: validate_* (22) | yes | yes | no | Unchanged: all 22 `validate_*` are test functions. |
| 5 | [Health] src/renderer/cli/mod.rs — 642 loc — consider splitting by responsibility: render_* (22) | yes | yes | no | Unchanged: fourteen test functions inflate the cluster. |

## Recurring follow-up: inline-test responsibility clustering

Same pre-existing Actionable defect as the previous audits, reproduced on the unchanged pinned corpus. Not introduced or worsened here; the clustering calculation is a metric concern outside M03's collector refactor, which must preserve every channel byte for byte — and the sweep says it did.

Safe failures: none. No new or withdrawn recommendations across the corpus.

Evidence inspected: the worksheet emitted by `make field-audit`, `git status` after the sweep (no baseline modified), and the branch diff (no file under `src/cmd`, `src/config`, `src/renderer`, `src/metrics`).
