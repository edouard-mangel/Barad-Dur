# Mutation Testing Strategy — release-pipeline

## Strategy: Hybrid

| Mode | Scope | Trigger | Kill Rate Gate | Timeout |
|---|---|---|---|---|
| Per-feature | Files changed in current MR / push | MR push + push to `main` | ≥ 80% (blocking) | 45 min |
| Nightly full-codebase | All source files | Daily schedule | ≥ 80% (blocking once baseline set) | 6 hours |

## Current Implementation Analysis

### What exists

The `mutation` job in `.gitlab-ci.yml` currently:
- Runs only on `schedule` (`$CI_PIPELINE_SOURCE == "schedule"`)
- Detects commits in the last 25 hours: `git log --since="25 hours ago"`
- If commits found, diffs from before the oldest recent commit to HEAD
- Passes `--in-diff recent.diff` to `cargo-mutants` to scope mutations
- Gates at ≥ 80% kill rate via inline Python script
- Exits 0 (skip) if no commits in the last 25 hours
- `allow_failure: true` — does not block pipeline

### Gaps

| Gap | Impact |
|---|---|
| Only runs on schedule, not on MR/push | Per-feature feedback delayed until next schedule run (up to 24h) |
| Schedule + diff-scope = wrong combination | Nightly run should test the full codebase, not just last-25h diff |
| `allow_failure: true` | Kill rate gate never blocks anything today |
| No per-feature job on MR | New code can merge without mutation gate |

## Recommended Design

### Job 1: `mutation` (per-feature, on push/MR)

Trigger: MR pipelines + push to `main` (not schedules).
Scope: files changed between current commit and its merge base with `main`, or the last N commits on a push.

```yaml
mutation:
  stage: analysis
  <<: *rust-base
  timeout: 45 minutes
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH && $CI_PIPELINE_SOURCE != "schedule"
  script:
    - cargo install cargo-mutants --locked
    - |
      # Compute diff against merge base (MR) or last commit (push)
      if [ -n "$CI_MERGE_REQUEST_DIFF_BASE_SHA" ]; then
        git diff "$CI_MERGE_REQUEST_DIFF_BASE_SHA"..HEAD > recent.diff
      else
        git diff HEAD~1..HEAD > recent.diff
      fi
      if [ ! -s recent.diff ]; then
        echo "Empty diff — no Rust source changes, skipping"
        exit 0
      fi
      cargo mutants --in-diff recent.diff --timeout 60 2>&1 | tee mutants.log
    - <kill rate gate — same Python script as current>
  artifacts:
    paths:
      - mutants.out/
    when: always
    expire_in: 1 week
  allow_failure: false  # Blocks merge
```

Using `$CI_MERGE_REQUEST_DIFF_BASE_SHA` (available in MR pipelines) gives precise scope — only mutations on lines that changed in this MR. On push to main, `HEAD~1..HEAD` covers the single commit landed.

### Job 2: `mutation-nightly` (full codebase, on schedule)

Trigger: schedule only.
Scope: full codebase (no `--in-diff`).

```yaml
mutation-nightly:
  stage: analysis
  <<: *rust-base
  timeout: 6 hours
  rules:
    - if: $CI_PIPELINE_SOURCE == "schedule"
  script:
    - cargo install cargo-mutants --locked
    - cargo mutants --timeout 60 --jobs 4 2>&1 | tee mutants.log
    - <kill rate gate — same Python script, no "skipping" escape path>
  artifacts:
    paths:
      - mutants.out/
    when: always
    expire_in: 1 week
  allow_failure: true  # Remove once full-codebase baseline ≥ 80% is achieved
```

`--jobs 4` parallelizes mutation testing across available cores. The full-codebase run is slow — parallel execution is essential within the 6h timeout.

## Kill Rate Gate Implementation

The existing Python gate script is correct. The only change for `mutation-nightly` is removing the early-exit path for "no commits in last 25 hours" — the full-codebase job always has work to do.

Gate logic (both jobs):
```
viable = total - unviable
if viable == 0: skip (no mutations generated — likely no Rust logic changed)
rate = caught / viable * 100
if rate < 80: FAIL
```

## Transition Plan

| Phase | State | Action |
|---|---|---|
| Now | `mutation` runs on schedule + 25h diff filter, `allow_failure: true` | No change to existing job yet |
| Step 1 | Add `mutation` per-feature job (MR + push, `allow_failure: false`) | New job, separate from existing |
| Step 2 | Rename existing schedule job to `mutation-nightly`, remove 25h filter, keep `allow_failure: true` | Rename + simplify existing job |
| Step 3 | Run nightly for 2 weeks, establish kill rate baseline | Observe |
| Step 4 | Set `mutation-nightly: allow_failure: false` once baseline ≥ 80% | Flip the flag |

## cargo-mutants Scope Behavior

`--in-diff <file>` filters mutations to lines present in the provided unified diff. This means:
- Functions not touched by the MR are not mutated
- A 5-line change in a 1000-line file generates a small, focused mutation set
- Fast feedback: per-feature run typically completes in 5–15 minutes for typical changes

For the nightly run without `--in-diff`, cargo-mutants generates mutations for every function in every `src/` file. For barad-dur's current size (~3k LOC), this is manageable within 6 hours. At >50k LOC, revisit the nightly strategy.

## Artifact Output

`mutants.out/` directory contains:
- `missed.txt` — list of mutations that survived (tests did not catch them)
- `caught.txt` — mutations killed by tests
- `timeout.txt` — mutations that timed out
- `unviable.txt` — mutations that did not compile

The `missed.txt` file is the actionable artifact — each entry identifies a specific code location where test coverage is insufficient to catch a behavioral change.
