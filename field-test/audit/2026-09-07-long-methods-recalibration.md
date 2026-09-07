# P2c decision-surface audit — Long Methods recalibration (BD-006)

Merge under audit: `feat/bd-006-long-methods` (Long Methods recalibration).
Corpus: all 11 entries, full local sweep (the private entries were present, so
this is not the CI public subset).

**Baseline compared against: `origin/main` at `34c9caf`, not the committed
baselines on this branch.** This matters, and it is the reason a worksheet was
missing before. `field-test audit` compares the current run against whatever
`field-test/baselines` holds in the working tree, and this branch's
`test(field): accept long method recalibration` commit had already rewritten
those files. Run as-is on the branch, the audit reports *nothing changed* and
emits only the five rotation rows. The changed decision surfaces only become
visible when the pre-merge baselines are restored first:

```
git checkout origin/main -- field-test/baselines
cargo run --release --features field-test --bin field-test -- audit
git checkout HEAD -- field-test/baselines
```

Rows marked `[withdrawn]` are recommendations this merge REMOVES. They are
sampled because a merge that stops giving advice changes the decision surface
as much as one that starts; the selector did not surface them until this merge
fixed it.

---

## barad-dur

Rotation rows: pre-existing recommendations, unrelated to this merge, sampled
because nothing has been audited before. Pin `dc23fbd6`.

For each recommendation below, answer all three. Any **Safe** failure blocks the merge; **True** and **Actionable** failures become tickets.

| # | Recommendation | True? | Safe? | Actionable? | Notes |
|---|---|---|---|---|---|
| 1 | [Coupling] Change coupling smells (score: 25) — Decouple cross-boundary co-changing files by introducing interfaces or shared abstractions | yes | yes | yes | Score reflects real co-change pairs. Generic advice, but the Coupling tab names the pairs, so a maintainer has somewhere to start. |
| 2 | [Health] God objects (score: 30) — Break down large files by extracting responsibilities into smaller modules | yes | yes | **partial** | The score is real, but its per-file evidence is inflated by the defect in rows 4 and 5. Splitting is safe advice; the sizing that justifies it is overstated. |
| 3 | [Health] src/cmd/analyze.rs — 510 loc; structural hub — 10 connections (threshold 8) — consider splitting by responsibility: build_* (6), compute_* (2) | yes | yes | yes | 615 raw lines at the pin, so 510 code LOC is consistent. `build_*`/`compute_*` are production functions here; the hub claim is independent of the test-counting defect below. |
| 4 | [Health] src/config/mod.rs — 677 loc — consider splitting by responsibility: validate_* (22) | yes | yes | **NO** | **All 22 `validate_*` are test functions.** At `dc23fbd6`, `mod tests` opens at line 332 and every `fn validate_*` sits between lines 458 and 795. Production has exactly one, `pub fn validate` at line 224. The file is large largely *because* of its tests, and "split by validate_* responsibility" would mean splitting the test module. A maintainer cannot act on this as written. |
| 5 | [Health] src/renderer/cli/mod.rs — 642 loc — consider splitting by responsibility: render_* (22) | yes | yes | **NO** | Same defect. `mod tests` opens at line 322; production `render_*` are at lines 14, 100, 148, 204 and 256, while 14 of the 22 counted are test functions from line 421 onward. |

**Ticket (Actionable failure, rows 4 and 5).** The god-objects prefix
clustering counts functions inside a file's own `#[cfg(test)] mod tests`.
Test *files* are already excluded by `FileRole::Test`, but an inline test
module is not, so a source file's test suite inflates both its LOC and its
apparent responsibility clusters. Not a defect of this merge; pre-existing and
independent of the Long Methods rule.

---

## evolutionary-architecture-by-example

For each recommendation below, answer all three. Any **Safe** failure blocks the merge; **True** and **Actionable** failures become tickets.

| # | Recommendation | True? | Safe? | Actionable? | Notes |
|---|---|---|---|---|---|
| 1 | [withdrawn] [Health] Long methods (score: 75) — Extract smaller functions from the longest methods to improve readability | yes | yes | n/a | Measured, not assumed — see below. |

**Was it safe to stop saying this?** Yes, on measurement. Analysed at pin
`536af586` with deliberately permissive thresholds to enumerate what the
shipped rule now lets through:

- `cc = 100000, cc_floor = 0, loc = 40, ui_loc = 40` (flag anything with
  CC ≥ 1 over 40 LOC): **0 of 783 functions**.
- `cc = 100000, cc_floor = 0, loc = 0, ui_loc = 0` (flag every function with
  CC ≥ 1): 72 of 783, **max LOC 35, max CC 2**, and **zero functions above
  CC 5** — so the complexity floor is not what removed them.

The remaining 711 functions report CC 0, i.e. no control flow at all. The two
findings the legacy `LOC > 40 OR CC > 10` rule produced here were therefore
length-only, branch-free members — exactly the class BD-006 set out to stop
flagging. Nothing complex is now hidden in this repository.

**Recorded as a decision-surface concern, not a Safe failure.** With this
action gone the repository reports **overall 100/100 with an empty action
list**, on 874 files where only 4 of roughly 30 metrics are computable
(`total_commits` 0, so Team and Evolution are entirely unscored, and Health's
own Bus factor and Churn-ownership risk are null). The score is correct under
the scoring rules and no recommendation is wrong, so this does not block the
merge. But "flawless, nothing to do" is the strongest possible claim on the
weakest available evidence, and it is a presentation question the unscored-
category renormalisation (`34c9caf`) owns rather than this merge. Ticket, not
a blocker.

---

## payp-app-front

Private corpus entry; measured locally, not reproducible in CI.

For each recommendation below, answer all three. Any **Safe** failure blocks the merge; **True** and **Actionable** failures become tickets.

| # | Recommendation | True? | Safe? | Actionable? | Notes |
|---|---|---|---|---|---|
| 1 | [Health] Long methods (score: 50) — Extract smaller functions from the longest methods to improve readability | yes | yes | yes | 28 of 305 functions flagged (9.2%). CC ranges from 6 (one above the floor, so the floor binds as designed) to 62. Worst: `LeadsTable` CC 62 / 316 LOC, `DetailsStep` CC 33 / 326, `LeadsTabTable` CC 28 / 187. Every sampled finding is a genuinely branch-heavy component. |
| 2 | [withdrawn] [Health] Long methods (score: 25) — Extract smaller functions from the longest methods to improve readability | yes | yes | n/a | Not a withdrawal of advice: the same recommendation with its score moved 25 → 50 as findings fell 87 → 28. The advice still stands and still applies; only its severity changed. |

---

## Outcome

- **Safe failures: none.** The merge is not blocked.
- **True failures: none.**
- **Actionable failures: two**, both pre-existing and both about the
  god-objects prefix clustering counting inline test functions (barad-dur rows
  4 and 5). Ticket raised above; unrelated to the Long Methods rule.
- **Decision-surface concern, ticket not blocker:** the 100/100-with-no-actions
  presentation on `evolutionary-architecture-by-example`.
- **Process defect found while producing this worksheet:** running
  `make field-audit` *after* `make field-test-accept` shows nothing, because
  the audit compares against the committed baselines the accept step just
  rewrote. The audit must be run against the pre-merge baseline. Worth
  encoding in the harness rather than in reviewer memory.
