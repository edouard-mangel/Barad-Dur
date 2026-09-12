# P2 decision-surface audit — Rust toolchain and dependency refresh

Branch: `chore/rust-dependency-refresh`; base: `9c4a273`.
Corpus: all 11 configured repositories, including private local entries.

`make field-test` exited 0: `field test clean across 11 repositories`.
Both passes matched all committed decision surfaces and each other. No baseline,
pin, scoring formula, or history window was changed. `make field-audit` exited 0
and emitted these five existing rotation rows for barad-dur at `dc23fbd6`.

| # | Recommendation | True? | Safe? | Actionable? | Evidence and notes |
|---|---|---|---|---|---|
| 1 | Change coupling smells (25): decouple cross-boundary co-changing files | yes | yes | yes | Current archived report: 774 qualifying cross-boundary pairs, 282 cross-community pairs, 101/140 affected source files. Coupling rows identify concrete investigation targets. |
| 2 | God objects (30): extract responsibilities from large files | yes | yes | partial | Current report identifies 22/139 oversized or structurally overconnected files. Some responsibility clusters include inline tests, as detailed below. |
| 3 | src/cmd/analyze.rs: 510 LOC, 10 connections; build_* (6), compute_* (2) | yes | yes | partial | Re-read pinned source: test module starts at line 428; build_* contains one production function and five tests; compute_* contains two production functions. |
| 4 | src/config/mod.rs: 677 LOC; validate_* (22) | yes | yes | no | Re-read pinned source: test module starts at line 332; all 22 validate_* functions are tests. The suggested production responsibility is misleading. |
| 5 | src/renderer/cli/mod.rs: 642 LOC; render_* (22) | yes | yes | no | Re-read pinned source: test module starts at line 322; eight render_* functions are production and fourteen are tests. |

Safe failures: none. No recommendations were added, changed, or withdrawn.
The actionability limitations are the same inline-test clustering defect
documented in the [previous audit](2026-09-10-calculation-boundaries-time.md).
That recorded follow-up remains explicit: exclude inline tests from suggested
production responsibility clusters, cover these pinned examples, and review
any resulting baseline changes separately. This dependency refresh preserves
the existing calculations rather than incorporating that scoring change.

Evidence re-read: `field-test/archive/barad-dur-0.json`, the three source files
via `git show dc23fbd6:<path>`, and the emitted worksheet. The driver has no
persistent rotation state, so these are repeated harness-selected rows.
