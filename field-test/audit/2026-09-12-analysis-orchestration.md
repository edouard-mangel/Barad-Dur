# P2 decision-surface audit — Analysis orchestration

Branch: `refactor/analysis-orchestration`; source revision `67325d4`, implementation base `9c4a273`.
Corpus: all 11 configured repositories, including the private local entries (`.corpus` linked from the main checkout).

`make field-test` exited 0: `field test clean across 11 repositories`. Both passes matched the committed decision surfaces and each other. No baseline, pin, or history-window change was accepted. `make field-audit` exited 0 and emitted the same five rotation rows for barad-dur at pin `dc23fbd6` as the two previous audits — the driver still has no rotation state, so this is the harness-selected sample, not a fresh one.

## Sampled recommendations

| # | Recommendation | True? | Safe? | Actionable? | Evidence and notes |
|---|---|---|---|---|---|
| 1 | [Coupling] Change coupling smells (score: 25) — Decouple cross-boundary co-changing files by introducing interfaces or shared abstractions | yes | yes | yes | Archived report: 774 qualifying cross-boundary pairs, 282 cross-community, 101/140 affected source files — unchanged from the previous audit, as expected on an unchanged pin. The advice names no destructive edit. |
| 2 | [Health] God objects (score: 30) — Break down large files by extracting responsibilities into smaller modules | yes | yes | partial | Archived report: 22/139 source files oversized or structurally overconnected. Signal and advice hold; the per-file clusters in rows 3–5 still count inline tests. |
| 3 | [Health] src/cmd/analyze.rs — 510 loc; structural hub — 10 connections (threshold 8) — consider splitting by responsibility: build_* (6), compute_* (2) | yes | yes | partial | Evidence is at the pin, where the previous audit already showed five of the six `build_*` to be tests. At this branch head the file is 621 lines with one production `compute_*` (`compute_trend_and_update_history`) and one production `build_*` (`build_ecosystem_reports`); `compute_selected_metrics` moved to `src/analysis/`. The hub observation stays true; the cluster wording is still inflated by tests. |
| 4 | [Health] src/config/mod.rs — 677 loc — consider splitting by responsibility: validate_* (22) | yes | yes | no | Unchanged pin, unchanged finding: all 22 `validate_*` are test functions; production has one `validate`. |
| 5 | [Health] src/renderer/cli/mod.rs — 642 loc — consider splitting by responsibility: render_* (22) | yes | yes | no | Unchanged pin, unchanged finding: eight production `render_*`, fourteen tests inflate the cluster. |

## Recurring follow-up: inline-test responsibility clustering

Same pre-existing Actionable defect as the [September 7](2026-09-07-long-methods-recalibration.md) and [September 10](2026-09-10-calculation-boundaries-time.md) audits, reproduced on the unchanged pinned corpus. Not introduced or worsened here; the clustering calculation is outside M02's ownership, which must preserve formulas and baselines.

Fifth consecutive audit to file this row with a per-occurrence "outside ownership" line and no owner. Under the minors policy's recurrence rule this is no longer a minor: **escalated to a tracker issue** (responsibility clustering must count production functions only, or label the test share), so the next audit references the issue instead of re-filing the row.

Safe failures: none. No new or withdrawn recommendations across the corpus (the field test compares full decision surfaces, not overall scores).

Evidence inspected: `field-test/archive/barad-dur-0.json` (generated, ignored), `src/cmd/analyze.rs` at the branch head, and the worksheet emitted by `make field-audit`.
