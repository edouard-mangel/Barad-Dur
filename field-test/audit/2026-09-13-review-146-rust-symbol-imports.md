# P2 decision-surface audit — Rust symbol-import resolution (review !146)

Branch: `refactor/snapshot-assembly`, review-fix commits after `da05eca5`; source revision `2f55956` (`fix(collector): resolve a Rust symbol import to the module that declares it`), audited at `0276e4a`.
Corpus: all 11 configured repositories, including the private local entries, at their pins under `~/WS`.

`make field-test` exited 1 on the first run: **2 repositories differ from baseline**, both Rust (`barad-dur` at pin `dc23fbd6`, `starship` at `e939a19a`); `ripgrep`, `helix` and the eight non-Rust entries are unchanged. Both passes matched each other (deterministic). The differences are the intended effect of the resolver change: a `use crate::a::b;` line where `b` is a symbol declared in `src/a.rs` or `src/a/mod.rs` now yields an import edge to that file; before, it yielded none. Baselines accepted with `make field-test-accept` in their own commit; `make field-test` is clean afterwards.

## What moved, and why it is true

| Repository | Change | Evidence |
|---|---|---|
| starship | Coupling 82 -> 80; Circular dependencies 86 -> 69 | `src/utils/mod.rs` imports `crate::context::Context` and `Shell` (lines 14-15) while `src/context/mod.rs` imports `crate::utils::{CommandOutput, PathExt, create_command, exec_timeout, read_file}` (line 4): a real module cycle, invisible before because every one of those lines names symbols. 21 files under `src/` import a `utils` symbol (`grep -rlE '^use crate::utils::[a-z_]+' src`), 7 import a `context` symbol. |
| starship | `src/utils/mod.rs` hub 17 -> 50 connections; `src/context/mod.rs` 11 -> 26; `src/modules/git_status.rs` newly a hub at 8 | Same imports counted as edges; the hotspot texts change their connection counts only, the recommendation itself (split by responsibility) is unchanged. |
| barad-dur (pin) | `src/cmd/analyze.rs` 10 -> 16, `src/scorer.rs` 9 -> 17, `src/config/mod.rs` newly a hub at 17; threshold 8 -> 11 | Symbol imports such as `use crate::scorer::build_report;` now resolve. The hub threshold is the p90 of the file degree distribution with a floor of 8 (`hub_threshold`, god_objects.rs), so it rises with the denser graph; the same three files stay flagged. |

## Sampled recommendations (barad-dur at pin)

| # | Recommendation | True? | Safe? | Actionable? | Evidence and notes |
|---|---|---|---|---|---|
| 1 | [Coupling] Change coupling smells (score: 25) — Decouple cross-boundary co-changing files by introducing interfaces or shared abstractions | yes | yes | yes | Unchanged pin, unchanged text and score. The advice names no destructive edit. |
| 2 | [Health] God objects (score: 30) — Break down large files by extracting responsibilities into smaller modules | yes | yes | partial | Unchanged; the per-file clusters in rows 3-5 still count inline tests (recurring follow-up, see previous audits). |
| 3 | [Health] src/cmd/analyze.rs — 510 loc; structural hub — 16 connections (threshold 11) — consider splitting by responsibility: build_* (6), compute_* (2) | yes | yes | partial | Connection count rose from 10: the file's `use crate::...::symbol` lines are now edges. The count is truer than before; the cluster still includes test functions. |
| 4 | [Health] src/config/mod.rs — 677 loc; structural hub — 17 connections (threshold 11) — consider splitting by responsibility: validate_* (22) | yes | yes | no | Newly a hub: 10 files at the pin import a symbol from it (grep for `^use crate::config::` at dc23fbd6 under src/), and its own outbound imports make up the rest of the 17 (degree counts both directions). The `validate_*` cluster is still all test functions (pre-existing Actionable defect). |
| 5 | [Health] src/renderer/cli/mod.rs — 642 loc — consider splitting by responsibility: render_* (22) | yes | yes | no | Unchanged; fourteen test functions inflate the cluster. |

## Starship, changed rows

| # | Recommendation | True? | Safe? | Actionable? | Notes |
|---|---|---|---|---|---|
| 6 | Coupling 80, Circular dependencies 69 | yes | yes | yes | The `utils` <-> `context` cycle is real (see evidence above) and is exactly what the metric exists to report. |
| 7 | src/utils/mod.rs — structural hub — 50 connections | yes | yes | yes | 21 importing files plus its own imports; a `utils` module imported from everywhere is the textbook hub. |
| 8 | src/modules/git_status.rs — structural hub — 8 connections (threshold 8) | yes | yes | partial | At the threshold exactly; the split advice stands, the hub label adds little. |

Safe failures: none. No recommendation was withdrawn; three barad-dur and three starship hotspot rows changed their connection counts, and starship's coupling category moved by two points on a newly visible cycle.

## Recurring follow-up: inline-test responsibility clustering

Same pre-existing Actionable defect as the previous audits (rows 2-5). Not introduced or worsened here.

Evidence inspected: the `make field-test` diff output, the two baseline diffs (`git diff -- field-test/baselines`), `grep` over the starship checkout at its pin, and the worksheet emitted by `make field-audit`.
