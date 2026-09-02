# Review process redesign — design

Date: 2026-09-01
Status: proposed

## Goal

Reduce **escapes** — defects that reach `main` or ship to users despite
the review process. The maintainer's explicit steer: favour quality
over speed; spending more review effort is acceptable.

This design is derived backwards from escapes that actually happened on
this project, recorded in `.superpowers/sdd/progress.md` and
`docs/sovrium-feedback-tickets.md`. Nothing here is added on instinct.

## Scope

In scope:

- The **in-session SDD review** — per-task review and final review.
- The **product-output review** — validating what Barad-dûr recommends
  against real foreign repositories.

Out of scope, deliberately, and recorded in project memory:

- The **CI / MR gate** (mutation kill rate, coverage, SAST) and review
  of the CI/release configuration itself.
- The **pre-implementation steps** (brainstorm → spec → plan quality).
  P0 below is a backstop for plan defects; improving how plans are
  written is separate work.

## The escape taxonomy

Every known escape falls into one of four classes. Each has a distinct
reason nobody caught it, and therefore needs a distinct gate.

| Class | Evidence | Why it escaped |
|---|---|---|
| **E1 Emergent** | `OnceCell`/`Cell<` substring FP (Critical, M1); barrel-gating divergent across 4 call sites (M4); ratchet nondeterminism (Important, M3) | Correct within its own diff; wrong only as a set or against a corpus |
| **E2 Plan-claim** | `mut_pattern` wrong for tree-sitter-rust 0.24.2; `;`-windowing carried an FP *and* an FN; `FileComplexity` is `#[non_exhaustive]`; borrow bug in the plan's own sketch | The plan asserted a fact about a grammar/API and nobody probed it |
| **E3 Output quality** | BD-001 (advice to delete compiling source), BD-002, BD-003 — all P0 | Self-dogfooding on a tidy 860-commit Rust repo never exercised it |
| **E4 Process** | Scratch AST-dump committed over `src/main.rs`, masked by a `--lib`-only run; TDD skipped twice | A verdict was asserted with no evidence behind it |

Known limitation: the taxonomy is backward-looking. A fifth class never
encountered gets no gate, and by construction would not be noticed.

## Gates

### P0 — Plan-claim verification (at plan freeze)

Every factual claim in a plan about an external grammar, crate API, or
type shape is extracted into a checklist and probed against reality: a
tree-sitter parse dump, a `cargo check` on a scratch snippet, the actual
struct definition.

- The probe records its **output**, not a tick. A checklist of assertions
  that probing occurred is a rubber stamp.
- A claim too expensive to probe is marked **unverified** in the plan, so
  the implementer treats it as suspect rather than gospel.
- Plans gain a required **"invariants this feature introduces"** section.
  It seeds P1's sweep list. A plan that cannot state its own invariants
  is a plan that cannot be swept — which is also the clearest signal for
  the deferred pre-implementation work.

Rationale: all four historical plan corrections were probe-sized, minutes
each. Discovering them at task time instead forced mid-task redesign,
because the plan had already committed to a design built on the false
claim.

### P1 — Invariant sweep (at final review, before P2)

The reviewer enumerates every rule the feature introduces, then finds
**all** call sites of each and verifies identical treatment. Output is a
table: rule → sites found → verdict, committed with the review.

Rationale: M4's review did exactly this on instinct and found the
four-site barrel-gating divergence. This makes it procedure.

Known limitation, stated plainly: the pass is only as complete as the
reviewer's rule list. A rule nobody enumerates is never swept, and
nothing detects the omission. This converts luck into procedure; it does
not make the search exhaustive. Grep-based automation was rejected — the
rules are not syntactically uniform, and a grep that finds three of four
sites is worse than a human who knows there should be four, because it
reads as complete.

### P2 — Corpus sweep (at final review)

Three modes. Regression and determinism are mechanical; audit is not, and
audit is the one with the track record.

#### P2a — Regression mode

Analyse every corpus repo at its pinned SHA and diff the **decision
surface** against a committed baseline. Any diff fails the run.

#### P2b — Determinism mode

Analyse every corpus repo **twice** and diff the two outputs against each
other. Any difference is a nondeterminism bug.

Rationale: M3's barrel nondeterminism and the multiset ratchet diff were
both ordering bugs, and one was an Important. The check is mechanical,
requires no judgement, and roughly doubles a ~3.1 minute sweep.

#### P2c — Audit mode (every final review)

A reviewer reads actual recommendations produced against real repos and
judges them. This is not a diff.

**Why it is mandatory and cannot be replaced by P2a**: BD-001 was found on
a first-ever run of a repository that had no baseline. Under regression
mode alone, that first run *creates* the baseline, the bad advice goes
into it, and every later run passes clean. The gate would have frozen the
P0 rather than caught it. All three Sovrium P0s came from inspection, not
comparison.

**Sampling** — bounded at roughly 10 items per merge:

- **All** recommendations new or changed by this merge (usually few, and
  highest-risk).
- Plus a **rotating slice** of up to **5** pre-existing recommendations, cycling across the
  corpus so untouched recommendations are eventually all seen. This is
  what makes audit a net rather than a ratchet: it can find advice that
  was already wrong.

**Rubric** — three questions per recommendation:

| Question | Fails when | Example |
|---|---|---|
| **True?** | The evidence does not support the claim | BD-002 (exclusions ignored → order-of-magnitude wrong), BD-003 (fabricated source/test pairs) |
| **Safe?** | Following it would break something | **BD-001** — directionally reasonable, deletes compiling source |
| **Actionable?** | A maintainer cannot act on it, or it should not apply here | BD-005 (team metrics on a solo repo) |

The rubric was derived backwards from the ten Sovrium tickets; every P0
lands cleanly in one column. BD-001 is the load-bearing demonstration: it
passes **True** and fails only **Safe**. A rubric asking merely "is this
correct?" waves it through — which is approximately what the existing
process did.

**Any `Safe` failure blocks the merge.** `True` and `Actionable` failures
become tickets.

Completed worksheets are committed under `field-test/audit/`.

### Evidence contract (all reports, all levels)

Reports state evidence, not verdicts: which call sites were enumerated,
which corpus samples inspected, the RED→GREEN transcript, full-suite
output.

Honest assessment: this is the weakest of the gates because it is
self-attested — a reviewer can paste output without reading it and
nothing detects that. Its documented value is making *omissions* visible:
the `--lib` masking incident was catchable precisely because the report
displayed a `--lib` invocation where a full-suite run belonged. It is
cheap and has one documented save.

### Minors policy

Minors no longer default to deferred. Each gets one of three dispositions:

1. **Fix now.**
2. **Corpus-test it** — does this false positive actually fire across the
   corpus? The corpus turns "is this minor real?" into an empirical
   question rather than a judgement call.
3. **Retire explicitly**, with a written rationale.

A minor that **reappears in a later milestone auto-escalates**.

Rationale, from the ledger:

- M1 Task 10 logged "cap threshold 70 is a magic number duplicating
  `SCORE_GOOD_MIN-1`" as a *Minor*. The M1 final review fixed it as one
  of two **Importants** in `816f666`. A deferred minor was a real defect.
- Barrel duplication was a *Minor* in M2 at **2 sites**, a *Minor* in M4
  at **3 sites**, and **4 sites** by M4's final review — still deferred.
  Deferred three times, larger each time.

The second case is the reason for auto-escalation: severity was assessed
per-occurrence, so a signal that got louder each time still read as
"minor" every time. Recurrence is information and was being discarded.

## The harness

### Layout

```
field-test/
  corpus.toml                    # committed: name, pinned SHA, language
  baselines/<name>.surface.json  # committed: the decision surface
  audit/rotation.json            # committed: audit coverage state
  audit/<date>-<branch>.md       # committed: completed worksheets
  archive/                       # gitignored: full normalized reports
```

### Corpus

11 repositories, all local, pinned. Measured with
`target/release/barad-dur analyze <path> --json --no-cache`.

| Repo | Lang | Commits | Pin | Wall | JSON | Shape it contributes |
|---|---|---:|---|---:|---:|---|
| `barad-dur` | Rust | 860 | `73ebdf3e` | 5.3s | 496K | Self-dogfood; the tidy baseline |
| `ripgrep` | Rust | 2,287 | `3fce3b5b` | 2.0s | 156K | Statics/`OnceLock`-heavy — exercises the look-through rule behind the M1 Critical |
| `helix` | Rust | 7,689 | `079a789e` | 13.4s | 1.3M | Large multi-crate workspace |
| `starship` | Rust | 4,387 | `e939a19a` | 5.3s | 248K | Highly modular, many contributors and bots — team metrics, BD-004/005 |
| `dotnet-starter-kit` | C# | 1,684 | `b21bdd93` | 8.4s | 360K | Mainstream modular monolith |
| `evolutionary-architecture-by-example` | C# | 1,471 | `536af586` | 14.8s | 704K | Explicitly about architecture evolving — coupling detectors |
| `eShopModernizing` | C# | 435 | `63bc9ec4` | 13.2s | 680K | Legacy .NET Framework idiom; detectors tuned on modern C# should misfire here |
| `App-Serveat` | C# | 190 | `6fcfa756` | 11.8s | 524K | Inverted ratio: 1,191 files, 190 commits. Shallow history (BD-009), generated-code noise |
| `payp-app-front` | TS | 840 | `2260b980` | 3.5s | 288K | Real product front end |
| `kairis-crm` | TS | 558 | `663493ef` | 5.1s | 672K | Mid-size product code |
| `mautic` | PHP | 24,379 | `181701cd` | 104.4s | 5.4M | Largest history; PHP has no import resolver → the *unscored* path |

Total: **~3.1 minutes** for one pass, **~6.2 minutes** with the determinism
double-run. Every figure above is measured, not estimated.

`openjdk` was evaluated and **excluded**: it is a 2.7 GB source drop with
no `.git`, so there is no history to analyse.

Corpus repos are the maintainer's real working repositories. The harness
therefore analyses a **throwaway `git worktree`** at the pinned SHA and
removes it, making it structurally incapable of touching a branch or a
dirty tree.

This is a hard requirement, not hygiene. Measured during this design:
`barad-dur analyze <path>` **mutates the analysed repository** — it
creates `.repository-analysis/` inside it and appends
`.repository-analysis/` to its `.gitignore` (creating the file if absent).
Running the sweep directly against the corpus dirtied all ten repos and
required a scripted cleanup. Any harness that skips worktree isolation
will do the same on every run.

Follow-up candidate for the product backlog, outside this spec's scope:
an analyser that writes into the repository it is reading is surprising,
and under this spec's own audit rubric it is a **Safe** failure — the
tool modifies your working tree without being asked. A `--no-write` mode,
or writing to a cache directory outside the target, deserves a ticket.

### The decision surface

Committed baselines record only:

- Overall score and per-category scores
- Per metric: name, score **or `unscored`**, band
- Action suggestions (the recommendation text), normalized
- Finding counts by kind
- Top-20 hotspot paths in rank order
- Counts: `total_files`, `total_commits`, `total_authors`

Concretely, from the measured JSON: `overall_score`, `categories[]`
(`name`, `score`, and each `metrics[]` entry's `name` + `score`, where
`score: null` means unscored), `top_actions[]` and `coupling_actions[]`
(`text`, `target_tab`), `coupling_finding_counts` (`common`, `content`,
`control`, `inheritance`), `score_thresholds`, and the first 20
`file_hotspots[].path` in rank order.

`raw_value` and `description` on metrics are excluded: they restate the
score in prose and would make the surface diff on wording changes.

Normalization: paths relative to repo root and forward-slashed,
deterministic ordering, no timestamps, no absolute paths, no durations.

The volatile fields are known, not guessed. Measured on a real report:
absolute paths do not appear (`/home/` occurs zero times; `file_hotspots[].path`
is already repo-relative), but the top-level **`history`** array carries a
run `timestamp` and is regenerated on every run from the trend store, which
is empty in a fresh worktree. `history` is therefore **excluded from the
surface entirely**. Excluding it is what makes fail-on-diff viable at all —
left in, every run would diff.

Recording **scored-vs-unscored** is deliberate: the 0.21.0 lesson was that
reporting a fabricated `100` instead of *unscored* is exactly this class
of silent regression.

Full normalized reports are written to the gitignored `archive/` and are
**not committed**. Committing them was considered and rejected on
measurement: regeneration costs between 2 and 104 seconds, so a committed
archive buys almost nothing while costing real bloat (mautic alone is
5.4 MB per revision). Historical drill-down still works — the SHA is
pinned and the binary is rebuildable from git.

### Commands

```
make field-test         # P2a + P2b: analyse twice, diff surface vs baseline
                        # exits non-zero on any surface diff or nondeterminism
make field-test-accept  # rewrite baselines — its own reviewed commit
make field-audit        # emit the P2c worksheet: new/changed recommendations
                        # plus the rotating slice, with the True/Safe/Actionable rubric
```

`field-test-accept` must produce a separate reviewed commit showing the
diff, so every deliberate change to what the tool recommends is in git
history. Warn-only was rejected: informational checks become invisible.

## Escape accounting

One ledger line per escape found, recording **which gate caught it — or
that none did**.

This is what makes the process falsifiable. In six months it answers
whether P1 earns its cost, or whether audit mode finds everything and the
rest is theatre. The existing `progress.md` already works this way, which
is the only reason a false-negative rate for the *current* process could
be computed at all.

## Rejected alternatives

| Rejected | Why |
|---|---|
| Deepen per-task review | Every escape was structurally invisible inside one task's diff. The `OnceCell` FP is correct in its own hunk; barrel-gating is right at each individual site. More attention on a unit that cannot contain the defect buys only ceremony. |
| Raw JSON baselines | Timestamps, absolute paths and ordering churn every run; fail-on-diff becomes fail-always, people blanket-accept, the gate dies. |
| Committed full reports | Overturned by measurement — see above. |
| Two-tier corpus | Overturned by measurement: the largest repo is 104s, the whole corpus ~3.5 min. Tiering solved a problem that does not exist, and its language-triggered-promotion rule existed only to patch the gap tiering created. |
| Bounding `mautic` with `--since` | Truncating history changes results, and BD-009 is the ticket stating that truncated history distorts metrics. It would validate a configuration nobody runs. |
| Grep-automated invariant sweep | Rules are not syntactically uniform; partial results read as complete. |
| Warn-only field test | No failure mode because no effect. |

## Risks

- **P1 completeness ceiling** — unenumerated rules are never swept.
- **Audit fatigue** — P2c costs reviewer attention on every merge. Bounded
  sampling (~10 items) is the mitigation; escape accounting is the
  detector if it degrades.
- **Corpus staleness** — pins age away from upstream. Refreshing is manual.
- **Evidence contract is self-attested** — it surfaces omissions, not
  diligence.
- **Backward-looking taxonomy** — no gate for an unencountered class.

## Implementation shape

Two separable pieces, in order:

1. **The harness** — `corpus.toml`, worktree runner, surface extraction and
   normalization, baseline diff, determinism double-run, audit worksheet
   generation, `make` targets. Real code, TDD as usual.
2. **The process documents** — P0/P1 definitions, evidence contract,
   minors policy, escape accounting; referenced from `CLAUDE.md` so every
   session sees them.
