# Prioritization: gitlab-pipeline-api

## Prioritization Criteria

| Criterion | Weight | Description |
|-----------|--------|-------------|
| User Outcome Impact | 40% | Does this story deliver a verifiable behavior? |
| Dependency | 30% | Does it unblock other stories? |
| Effort | 20% | Smaller effort = faster feedback loop |
| Risk Reduction | 10% | Does it validate a risky assumption? |

## Release 1: Walking Skeleton (Priority: MUST)

| Story | Impact | Dependency | Effort | Risk | Score | Order |
|-------|--------|------------|--------|------|-------|-------|
| US-01: analyze-api job | High (core) | Unblocks all | Small (1d) | Validates trigger mechanism | 95 | 1 |
| US-02: Caller example | High (proves E2E) | Needs US-01 | Small (0.5d) | Validates artifact retrieval | 90 | 2 |

**Rationale**: These two stories prove the entire concept. US-01 is the foundation.
Everything else is enhancement.

## Release 2: Enhanced API (Priority: SHOULD)

| Story | Impact | Dependency | Effort | Risk | Score | Order |
|-------|--------|------------|--------|------|-------|-------|
| US-03: Options + gate | Medium | Needs US-01 | Small (0.5d) | Low | 72 | 3 |
| US-06: Branch selection | Medium | Needs US-01 | Tiny (0.25d) | Low | 70 | 4 |
| US-07: Category filter | Low-Medium | Needs US-01 | Tiny (0.25d) | Low | 60 | 5 |
| US-04: Caller template | Medium | Needs US-02 | Small (0.5d) | Low | 58 | 6 |
| US-05: Setup docs | Medium | Needs US-01 | Small (0.5d) | Low | 55 | 7 |

**Rationale**: US-03 (options/gate) delivers the most value after MVP — it enables
quality gates in calling pipelines. Branch and category filter are trivial additions.
Docs and template are important but not blocking.

## Release 3: Robustness (Priority: COULD)

| Story | Impact | Dependency | Effort | Risk | Score | Order |
|-------|--------|------------|--------|------|-------|-------|
| US-08: Timeout config | Low | Needs US-01 | Tiny (0.25d) | Low | 40 | 8 |
| US-09: Concurrency | Low | Needs US-01 | Small (0.5d) | Medium | 35 | 9 |

**Rationale**: Timeout is a simple CI variable. Concurrency is a "nice to have" —
GitLab handles parallel pipelines natively; this is about resource management.

## Delivery Sequence

```
  Day 1          Day 2          Day 3          Day 4          Day 5
  ─────          ─────          ─────          ─────          ─────
  US-01          US-02          US-03          US-04          US-08
  (analyze-api   (caller E2E    (options +     US-06          US-09
   job)           example)       gate)         US-07
                                               US-05
```

## Walking Skeleton Validation Criteria

The walking skeleton (R1) is validated when:
1. A pipeline trigger on the barad-dur project starts the analyze-api job
2. The job clones a target repo and produces barad-dur-report.json
3. A separate pipeline can download and parse that artifact
4. The JSON contains overall_score and category breakdowns
