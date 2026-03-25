# User Story Map: gitlab-pipeline-api

## Backbone (User Activities)

```
  SETUP                    TRIGGER                  ANALYZE                RETRIEVE              CONSUME
  ─────────────────────    ──────────────────────   ────────────────────   ────────────────────   ──────────────────
  Create trigger token     Call trigger API          Clone target repo      Poll pipeline status   Parse JSON report
  Store as CI variable     Pass repo URL + opts      Run barad-dur          Download artifact      Extract scores
  Write caller pipeline    Receive pipeline ID       Produce JSON artifact  Handle failure         Gate / display
```

## Walking Skeleton (Minimum End-to-End Slice)

The thinnest possible slice that proves the entire flow works:

```
  [Setup]                  [Trigger]                [Analyze]              [Retrieve]
  Add analyze-api job      Trigger via curl         Clone + analyze        Download artifact
  to .gitlab-ci.yml        with REPO_URL only       output JSON            verify JSON
```

**Walking Skeleton stories:**
1. US-01: analyze-api CI job accepts REPO_URL and produces JSON artifact
2. US-02: Caller pipeline example triggers and downloads report

## Story Map

```
                    Walking Skeleton (R1)         Enhanced (R2)              Robust (R3)
                    ─────────────────────         ─────────────              ──────────────
  SETUP             US-01: analyze-api job        US-04: Caller template     --
                                                  US-05: Setup docs

  TRIGGER           (built into US-01)            US-06: Branch variable     --

  ANALYZE           US-01: basic analyze          US-03: Options pass-thru   US-08: Timeout config
                                                  US-07: Category filter     US-09: Concurrency

  GATE              --                            US-03: MIN_SCORE gate      --

  RETRIEVE          US-02: artifact download      --                         --

  CONSUME           US-02: parse + display        --                         --
```

## Stories by Release

### Release 1: Walking Skeleton (MVP)
- **US-01**: analyze-api job — accept REPO_URL, run analysis, produce JSON artifact
- **US-02**: Caller pipeline example — trigger, poll, download, parse

### Release 2: Enhanced API
- **US-03**: Options pass-through — ANALYSIS_OPTIONS, MIN_SCORE gate
- **US-04**: Caller pipeline template — reusable .gitlab-ci.yml snippet
- **US-05**: Setup documentation — step-by-step guide for token + variable setup
- **US-06**: Branch selection — REPO_BRANCH variable support
- **US-07**: Category filter — CATEGORIES variable for selective analysis

### Release 3: Robustness
- **US-08**: Timeout configuration — configurable job timeout for large repos
- **US-09**: Concurrency handling — prevent resource exhaustion from parallel triggers

## Scope Assessment: PASS -- 9 stories, 2 contexts (CI config, documentation), estimated 5 days

The feature is right-sized. All stories are thin end-to-end slices. No split needed.
