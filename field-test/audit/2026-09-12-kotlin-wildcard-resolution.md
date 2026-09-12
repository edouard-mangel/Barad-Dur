# P2 recommendation audit — Kotlin imports and wildcard lookup

Branch: `perf/kotlin-wildcard-resolution`; base: `cbadbe0`.
Corpus: all 11 configured repositories, including private local entries.

`make field-test` exited 0: `field test clean across 11 repositories`.
All 22 analysis passes matched their committed decision surfaces and each other.
No corpus baselines or pins changed.

The worksheet below records the recurring barad-dur recommendations at
`dc23fbd6`. Evidence was re-read from the current archived report and pinned
source, independently of the prior audit. `make field-audit` exited 0 and
emitted exactly these five existing rows; no recommendations were new, changed,
or withdrawn.

| # | Recommendation | True? | Safe? | Actionable? | Evidence |
|---|---|---|---|---|---|
| 1 | Change coupling smells (25): decouple cross-boundary co-changing files | yes | yes | yes | 774 qualifying cross-boundary pairs; 282 cross-community; score based on 101/140 affected files. The coupling rows identify concrete investigation targets. |
| 2 | God objects (30): extract responsibilities from large files | yes | yes | partial | Report lists 22/139 oversized or overconnected source files. Some suggested clusters contain inline test functions. |
| 3 | src/cmd/analyze.rs: 510 LOC, 10 connections; build_* (6), compute_* (2) | yes | yes | partial | Pinned test module starts at line 428. build_* includes one production function and five tests; both compute_* functions are production. |
| 4 | src/config/mod.rs: 677 LOC; validate_* (22) | yes | yes | no | Pinned test module starts at line 332. All 22 validate_* functions are tests, so the suggested production responsibility is misleading. |
| 5 | src/renderer/cli/mod.rs: 642 LOC; render_* (22) | yes | yes | no | Pinned test module starts at line 322. Eight render_* functions are production and fourteen are tests. |

No Safe failure was found in these inspected recommendations. The recurring
actionability defect is corpus-tested again and escalated under the minors
policy: responsibility suggestions should exclude inline test functions, with
these three pinned examples as acceptance cases. It is the same recorded defect
as the [dependency-refresh audit](2026-09-12-rust-dependency-refresh.md) and
[calculation-time audit](2026-09-10-calculation-boundaries-time.md); this Kotlin
change neither introduces nor changes it. Its correction requires separately
reviewed recommendation/baseline changes.

The harness has no persistent rotation state, which is why it selects these
previously inspected rows again. The Ktor source benchmark and the limitation
on multiplatform source-root resolution are documented in the
[Kotlin review evidence](../../docs/reviews/2026-09-12-kotlin-wildcard-resolution.md).
