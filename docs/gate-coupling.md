# Coupling ratchet (`gate --no-new-coupling` / `--max-new-coupling`)

The coupling ratchet is a `barad-dur gate` mode that fails a build when a
branch *introduces* new Pressman coupling findings (content, common, control
— see `docs/superpowers/specs/2026-07-02-pressman-coupling-design.md`)
relative to a baseline commit. It is a ratchet, not a score threshold: it
doesn't care what your coupling score is today, only whether today is worse
than the baseline.

## What it guarantees

- **No regressions, not zero findings.** A repo can carry any amount of
  pre-existing coupling debt and still pass the gate — the ratchet only
  blocks *new* findings introduced since the baseline. Existing debt is a
  separate cleanup effort (see `--max-new-coupling` below).
- **Per-kind counts, not just a total.** The gate compares content/common/
  control counts individually between HEAD and the baseline, and reports
  each kind that increased.
- **Identity-stable diffing.** A finding is identified by `(path, kind,
  evidence)` — not file:line. Moving a `static mut` to a different line in
  the same file is not a new finding; renaming or relocating the underlying
  global is (see `ratchet_ignores_line_number_shifts` /
  `ratchet_removed_findings_do_not_mask_new_ones` in `src/cmd/gate.rs`'s test
  suite for the exact semantics).
- **The gate always computes coupling itself.** It never reads coupling
  counts back out of `.repository-analysis/trends.json` history — the
  ratchet re-collects and re-detects at both HEAD and the baseline ref on
  every run. One consequence: filtered/excluded-file `analyze` runs recorded
  in history never influence the ratchet, because the ratchet doesn't
  consult history at all.

## Why the baseline is an explicit `--baseline-ref`, never a history file

`--no-new-coupling` and `--max-new-coupling` both `requires = "baseline_ref"`
in the CLI (`src/cli/mod.rs`) — there is no implicit or default baseline.
This is deliberate, fail-loud design, not an oversight:

`.repository-analysis/` (where `trends.json` lives) is gitignored. A fresh
CI clone has no history file. If the ratchet silently fell back to "no
history means pass," the gate's broken state (misconfigured CI, first run
on a repo) would be indistinguishable from its passing state — exactly the
kind of quiet failure a quality gate must not have. A hybrid mode (history
locally, explicit ref in CI) was considered and rejected too: two different
baseline semantics behind one flag means a change can pass on a developer's
machine and fail in CI (or vice versa) for reasons that have nothing to do
with the code.

The full rationale, along with the other rejected alternatives, is recorded
in [`docs/superpowers/specs/2026-07-02-pressman-coupling-design.md`](superpowers/specs/2026-07-02-pressman-coupling-design.md),
"Resolved design questions," item 3. Short version: **explicit ref, always**
— an unresolvable ref is a hard error, not a silent pass.

## Recommended baseline: the MR merge base

Use the merge request's diff base, not the target branch's moving tip:

- GitLab: `$CI_MERGE_REQUEST_DIFF_BASE_SHA`
- Rationale: this measures exactly what *this branch* adds. If you instead
  diff against `origin/main`, every commit someone else merges to `main`
  while your MR is open changes your gate's baseline out from under you —
  new findings on `main` would show up as "your" new findings, and fixes on
  `main` would silently shrink your allowance.

## GitLab CI job example

```yaml
coupling-gate:
  stage: test
  variables:
    GIT_DEPTH: 0
  script:
    - barad-dur gate . --min-score 0 --no-new-coupling
      --baseline-ref "$CI_MERGE_REQUEST_DIFF_BASE_SHA"
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
```

`GIT_DEPTH: 0` is required: GitLab's default shallow clone hides the commit
history the baseline ref needs to resolve. Without it, `git2::Repository::
revparse_single` fails to find the ref and the gate exits with the
unresolvable-ref error (see the error catalogue below) — which names
`GIT_DEPTH: 0` directly in its hint text.

## Local usage

Locally there's no `$CI_MERGE_REQUEST_DIFF_BASE_SHA`; use your remote-tracking
branch instead:

```bash
# fail if this branch introduces coupling findings absent from origin/main
barad-dur gate . --min-score 0 --no-new-coupling --baseline-ref origin/main

# any branch, tag, or SHA works as a baseline ref
barad-dur gate . --no-new-coupling --baseline-ref HEAD~1
barad-dur gate . --no-new-coupling --baseline-ref v0.17.0
```

`make gate-coupling` wraps the `origin/main` form (see `Makefile`).

## `--max-new-coupling <N>`: cleanup mode

`--no-new-coupling` is shorthand for `--max-new-coupling 0`. During a
deliberate coupling cleanup, set a non-zero allowance so the gate doesn't
block unrelated work while the team pays down existing debt:

```bash
# allow up to 3 new findings total (summed across content+common+control)
# while a cleanup is in progress
barad-dur gate . --max-new-coupling 3 --baseline-ref origin/main
```

The allowance is a total across all three kinds, not per-kind — 3 new
`common` findings and 0 new `content`/`control` findings consume the same
budget as 1 of each. Ratchet the number down over time (3 → 1 → 0) as debt
is paid off, then drop back to `--no-new-coupling` once it reaches zero.

## Cost note

`--no-new-coupling` / `--max-new-coupling` collect a **second** snapshot at
the baseline ref, in addition to the normal HEAD analysis. That second
collection:

- reads file blobs directly from git's object database at the baseline
  commit (`collect_snapshot_at` → `ast_pass_at` in
  `src/collector/snapshot_builder.rs`) — it does **not** check out the
  working tree, so it's safe to run without disturbing local state;
- always runs the AST/tree-sitter pass needed to detect coupling findings
  (`run_ast = true`), unlike `backfill`'s historical sweep, which skips AST
  entirely per ADR-005;
- is **not cached**. There is no per-commit-hash snapshot cache for
  baseline collection — every gate invocation re-parses the baseline tree
  from scratch. This is a real, repeated cost on every ratchet run, not a
  one-time cost amortized by a cache.
- runs single-threaded (no rayon), on the reasoning that a gate run
  collects the baseline once, not once per historical commit the way
  backfill's sampling does, so the added complexity of parallelizing it
  wasn't worth it.

In practice this roughly doubles gate wall-clock time on a repository of
any size, since it's a full uncached AST pass over every non-binary file at
the baseline ref. There is no cache to warm — every run pays this cost.

## Error-message catalogue

Exact strings, quoted from `src/cli/mod.rs` and `src/cmd/gate.rs`, so you
can recognize them in CI logs.

**1. `--no-new-coupling` or `--max-new-coupling` without `--baseline-ref`**
(clap usage error, exit code 2 — checked before any analysis runs):

```
error: the following required arguments were not provided:
  --baseline-ref <REF>

Usage: barad-dur gate --baseline-ref <REF> --min-score <MIN_SCORE> --no-new-coupling <TARGET>

For more information, try '--help'.
```

**2. Unresolvable baseline ref** (typo, or a shallow clone that never
fetched the ref — exit code 1, from `resolve_baseline_ref` in
`src/cmd/gate.rs`):

```
Error: cannot resolve baseline ref '<ref>': <git2 error>. On CI, shallow clones hide history — set GIT_DEPTH: 0 (GitLab) or fetch the ref first (git fetch origin <ref>).
```

The `<git2 error>` segment is whatever `git2::Repository::revparse_single`
reports (e.g. `revspec 'does-not-exist' not found; class=Reference (4);
code=NotFound (-3)` for a typo, or a similar not-found error for a ref a
shallow clone never fetched) — the hint after it is always the same,
pointing at the two most common causes.

**3. Ratchet failure** (the check ran successfully and found regressions —
exit code 1, from `print_ratchet` in `src/cmd/gate.rs`):

```
RATCHET FAIL: <N> new coupling finding(s) vs <baseline_ref> (allowed <max_new>)
  <kind>: <baseline_count> -> <head_count>
  <path>:<line> — <evidence>
```

The per-kind increase lines only appear for kinds that actually increased;
the per-finding lines list every new finding (line omitted when the
detector didn't attribute one, e.g. barrel-bypass findings). On success, one
of:

```
RATCHET PASS: no new coupling findings vs <baseline_ref>
```

or, when running with a non-zero `--max-new-coupling` allowance:

```
RATCHET PASS: <N> new <= allowed <max_new> vs <baseline_ref>
```

## Not yet wired up

Per the design's milestone plan, the ratchet (M3) ships before hotspot
cross-referencing (M4), history corroboration (M5), and refactoring actions
(M6). Coupling findings used by the ratchet are not yet cross-referenced
against hotspots or co-change history, and there are no auto-generated
refactoring suggestions tied to ratchet failures — those land in later
milestones.
