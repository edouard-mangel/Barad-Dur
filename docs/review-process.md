# Review process

Date: 2026-09-01

Five gates reduce **escapes** — defects that reach `main` or ship to users
despite review. Full rationale, the escape taxonomy each gate answers to,
and the rejected alternatives live in
[`docs/superpowers/specs/2026-09-01-review-process-design.md`](superpowers/specs/2026-09-01-review-process-design.md).
This document is the operational reference: what each gate is, when it
runs, and what evidence it must emit.

## P0 — Plan-claim verification

**Runs at:** plan freeze, before implementation starts.

Every factual claim a plan makes about an external grammar, crate API, or
type shape is extracted into a checklist and probed against reality — a
tree-sitter parse dump, a `cargo check` on a scratch snippet, the actual
struct definition. The probe's **output** is recorded in the plan, not a
checkmark; a checklist that only asserts "probed: yes" is a rubber stamp.

A claim too expensive to probe is marked **`unverified`** in the plan, so
the implementer treats it as suspect rather than gospel, instead of the
claim silently reading the same as a verified one.

Plans also carry a required **"invariants this feature introduces"**
section. It seeds P1's sweep list below. A plan that cannot state its own
invariants is a plan that cannot be swept.

Every historical plan-claim escape (a tree-sitter node kind that turned
out wrong for the pinned grammar version, a `;`-windowing rule that carried
both a false positive and a false negative, a struct assumed exhaustive
that was `#[non_exhaustive]`, a borrow bug in the plan's own sketch) was
probe-sized — minutes of work. Catching it at plan freeze avoids the
mid-task redesign that finding it later forces.

## P1 — Invariant sweep

**Runs at:** final review.

The reviewer enumerates every rule the feature introduces, then finds
**all** call sites of each and verifies identical treatment. Output is a
table — rule → call sites found → verdict — committed with the review.

**Completeness ceiling, stated rather than hidden:** the sweep is only as
complete as the reviewer's rule list. A rule nobody enumerates is never
swept, and nothing detects the omission. This turns luck into procedure —
it does not make the search exhaustive. Grep-based automation was
considered and rejected: the rules are not syntactically uniform, and a
grep that finds three of four sites is worse than a human who knows there
should be four, because a partial automated result reads as complete.

## P2 — Corpus sweep

**Runs at:** final review, every merge.

Three modes against 11 pinned real repositories in `field-test/corpus.toml`
(`field-test/baselines/<name>.surface.json` holds each repo's committed
decision surface). All three are driven by `make field-test` (regression +
determinism) and `make field-audit` (the True/Safe/Actionable worksheet).

**Where it runs.** CI runs regression + determinism on every merge-request
and `main` pipeline (`field-test` job) over the **public subset**: the
entries in `corpus.toml` that carry a `url`, cloned in full under
`$CI_PROJECT_DIR/.corpus/<manifest-hash>` and cached across pipelines. The
runtime hash is computed after the merge-result checkout, so repositories from
different merged manifests never share a directory. The job blocks the
merge on any diff. The entries without a `url` are private repositories
that exist only on the maintainer's machine; they are covered only by the
maintainer's local `make field-test` (`BARAD_DUR_CORPUS_SCOPE` unset), which
therefore still runs before every merge. `BARAD_DUR_CORPUS_SCOPE=public`
reproduces the CI subset locally. Merge-request pipelines run on the
**merged result**, not the branch head, so a branch that forked before an
unrelated scoring change still meets the current baselines.

### P2a — Regression mode

Analyse every corpus repo at its pinned SHA and diff the **decision
surface** (scores, per-metric scored-or-`unscored` state, action
suggestions, finding counts, top-20 hotspots — never raw JSON, never
timestamps or absolute paths) against the committed baseline. Any diff
fails the run.

### P2b — Determinism mode

Analyse every corpus repo **twice** and diff the two outputs against each
other. Any difference is a nondeterminism bug.

### P2c — Audit mode

A reviewer reads actual recommendations produced against real repos and
judges them — this is not a diff, and it is the one mode with a track
record: every known output-quality escape was found by inspection, not
comparison, because on the run that creates a repo's first baseline,
regression mode has nothing to diff against and the bad advice goes
straight into the baseline clean.

Sampling is bounded at roughly 10 items per merge: all recommendations new
or changed by this merge, plus a rotating slice of up to 5 pre-existing
ones, cycling across the corpus so untouched recommendations eventually
all get seen. The 5 is one allowance shared by the whole corpus, not 5 per
repository — a per-repository budget would emit ~50 rows a run, which is
audit fatigue rather than the mitigation for it. A repository with no
baseline at all is audited against an empty one: every recommendation is
then new, which is the first-ever-run case this mode exists for.

Each sampled recommendation is scored on three questions:

| Question | Fails when |
|---|---|
| **True?** | The evidence does not support the claim |
| **Safe?** | Following it would break something |
| **Actionable?** | A maintainer cannot act on it, or it should not apply here |

**Any `Safe` failure blocks the merge.** `True` and `Actionable` failures
become tickets. Completed worksheets are committed under
`field-test/audit/`.

### What the harness costs and produced, measured

One `make field-test` invocation runs two passes over all 11 repos. First
measured run: **5:51**. Second run: **5:47**. Budget roughly six minutes,
not the ~3 minutes a single pass would suggest.

On the first real sweep it reported **zero nondeterminism across 44
analysis passes** — the surfaces were reproducible run-to-run.

Baselines are committed and total about **76 KB** for the 11 repos.
`field-test/archive/` holds the full normalized reports; it is **gitignored**
and regenerated per run — not committed.

`make field-test-accept` must produce its **own reviewed commit** showing
the baseline diff. That commit is what puts every deliberate change to what
the tool recommends into git history where a human sees it before it
becomes the new normal.

### Reproducible analysis window

Every corpus analysis passes the committed lower bound `--since 2026-03-01`.
Combined with the pinned commit, this makes the report independent of today's
date: the same binary and pin see the same history on every run. The explicit
boundary retains the window used to seed the baselines without making large
repositories pay the prohibitive cost of a complete-history scan.

Adding or replacing a corpus entry requires an explicit
`make field-test-accept`; a normal regression run fails when a committed
baseline is missing and never manufactures review evidence.

### Follow-ups (not yet done)

- **Rotation state is not implemented.** `field-test/audit/rotation.json`
  is specified but not built. `select_for_audit` already takes an
  `already_seen` set — the driver currently passes an empty one on every
  run. The slice of 5 pre-existing recommendations is a **corpus-wide**
  budget spent in corpus order, so with no persisted state it is spent on
  the first repositories in `corpus.toml` every run, and the repositories
  after that contribute no pre-existing rows at all. The bound itself is
  correct — that is what keeps the worksheet at roughly ten items — but
  until the seen-set is persisted the rotation does not actually rotate.
  Wiring up persistence is additive, not a redesign.
- **A category with every metric unscored still scores 100.** Both the
  `barad-dur` and `mautic` baselines show a "Team" category with all 7
  metrics `null` and a category score of 100. This is pre-existing scorer
  behaviour, surfaced for the first time by these baselines. Under P2c's
  own rubric this reads as a **True** failure — 100 presented as "perfect"
  when it actually means "no data" — and it is the same class of defect as
  the 0.21.0 lesson about reporting a fabricated score instead of
  *unscored*. Flagged here as a candidate audit ticket against the
  product, not fixed by this document.

## Evidence contract

Reports state **evidence**, not verdicts: which call sites were
enumerated, which corpus samples were inspected, the RED→GREEN transcript,
full-suite test output. Full suite, **never `--lib` alone** — a `--lib`-only
run has previously masked a broken `src/main.rs`.

Honest assessment: this is the weakest of the gates because it is
self-attested — a reviewer can paste output without reading it and nothing
detects that. Its documented value is making *omissions* visible, not
diligence: the `--lib` masking incident was catchable precisely because
the report displayed a `--lib` invocation where a full-suite run belonged.

## Minors policy and escape accounting

Minors no longer default to deferred. Each gets one of three dispositions:

1. **Fix now.**
2. **Corpus-test it** — does this false positive actually fire across the
   corpus? The corpus turns "is this minor real?" into an empirical
   question instead of a judgement call.
3. **Retire explicitly**, with a written rationale.

A minor that **reappears in a later milestone auto-escalates**. Severity
assessed per-occurrence let a signal that got louder every time still read
as "minor" every time; recurrence is information that was being discarded.

Every escape found is logged with **which gate caught it, or that none
did**. This is what makes the process falsifiable: over time the ledger
answers whether a given gate earns its cost, or whether one gate (audit
mode, historically) finds everything and the rest is theatre.
