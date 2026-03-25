# Acceptance Criteria: gitlab-pipeline-api

## Summary Matrix

| Story | Criteria | Scenarios | Status |
|-------|----------|-----------|--------|
| US-01 | 6 | 4 | Draft |
| US-02 | 5 | 3 | Draft |
| US-03 | 6 | 3 | Draft |
| US-04 | 5 | 2 | Draft |
| US-05 | 5 | 2 | Draft |
| US-06 | 4 | 3 | Draft |
| US-07 | 5 | 2 | Draft |
| US-08 | 4 | 2 | Draft |
| US-09 | 4 | 2 | Draft |

## US-01: Analyze-API Job

| # | Criterion | Source Scenario | Verification |
|---|-----------|-----------------|--------------|
| AC-01.1 | analyze-api job exists in .gitlab-ci.yml and runs only on pipeline triggers | Happy path | Rule: `$CI_PIPELINE_SOURCE == "trigger"` |
| AC-01.2 | Job accepts REPO_URL as a required trigger variable | Happy path | Job fails if REPO_URL is missing |
| AC-01.3 | Job uses the existing Docker image from Froggit container registry | Happy path | `image: $CI_REGISTRY_IMAGE:latest` |
| AC-01.4 | Job produces barad-dur-report.json as artifact (expire_in: 1 month) | Happy path | Artifact downloadable via API |
| AC-01.5 | Job fails with clear error when REPO_URL is missing or invalid | Error path | Error message in job log |
| AC-01.6 | Job succeeds (exit 0) when target repo has no recent commits | Edge case | Empty window produces warning, not error |

## US-02: Caller Pipeline Example

| # | Criterion | Source Scenario | Verification |
|---|-----------|-----------------|--------------|
| AC-02.1 | Example includes curl commands for trigger, poll, download | Happy path | Documented and runnable |
| AC-02.2 | Example handles success, failure, and auth error cases | All scenarios | Branch logic in script |
| AC-02.3 | Example uses masked CI variables | Happy path | Token never in logs |
| AC-02.4 | Example extracts overall_score from JSON | Happy path | jq parse succeeds |
| AC-02.5 | Example tested end-to-end on Froggit | Happy path | Pipeline run succeeds |

## US-03: Options Pass-Through and Gate

| # | Criterion | Source Scenario | Verification |
|---|-----------|-----------------|--------------|
| AC-03.1 | ANALYSIS_OPTIONS passed through to barad-dur command | Happy path | Job log shows flags |
| AC-03.2 | Shell injection prevented in ANALYSIS_OPTIONS | Security | Dangerous characters rejected |
| AC-03.3 | MIN_SCORE triggers gate check after analysis | Happy path | Gate output in job log |
| AC-03.4 | Gate uses >= comparison | Edge case | Score at threshold passes |
| AC-03.5 | Artifact produced regardless of gate result | Error path | `artifacts:when: always` |
| AC-03.6 | Invalid MIN_SCORE rejected with clear error | Error path | Non-integer caught before use |

## US-04: Caller Template

| # | Criterion | Source Scenario | Verification |
|---|-----------|-----------------|--------------|
| AC-04.1 | Template at ci/trigger-template.yml | Happy path | File exists in repo |
| AC-04.2 | Hidden job .barad-dur-analysis with defaults | Happy path | Extends works |
| AC-04.3 | Required CI variables validated before trigger | Error path | Early failure on missing vars |
| AC-04.4 | Usable via include: project: directive | Happy path | Cross-project include works |
| AC-04.5 | Variable overrides supported | Edge case | ANALYSIS_OPTIONS, MIN_SCORE |

## US-05: Setup Documentation

| # | Criterion | Source Scenario | Verification |
|---|-----------|-----------------|--------------|
| AC-05.1 | Guide at docs/pipeline-api-setup.md | Happy path | File exists |
| AC-05.2 | Covers: prerequisites, token, variables, config, verify | Happy path | All sections present |
| AC-05.3 | Troubleshooting for top 5 errors | Error paths | 401, 404, missing vars, clone fail, timeout |
| AC-05.4 | References CI template (US-04) | Happy path | Link to template |
| AC-05.5 | Specifies required permissions | Edge case | Maintainer for token |

## US-06: Branch Selection

| # | Criterion | Source Scenario | Verification |
|---|-----------|-----------------|--------------|
| AC-06.1 | REPO_BRANCH accepted with default "main" | Default | Variable declaration |
| AC-06.2 | Job checks out specified branch | Happy path | Branch in report metadata |
| AC-06.3 | Nonexistent branch produces clear error | Error path | Job log message |
| AC-06.4 | Default works when omitted | Edge case | Analysis runs on main |

## US-07: Category Filter

| # | Criterion | Source Scenario | Verification |
|---|-----------|-----------------|--------------|
| AC-07.1 | CATEGORIES accepts comma-separated names | Happy path | Variable parsing |
| AC-07.2 | Valid names: health, team, evolution, hygiene | Happy path | Mapped to CLI flags |
| AC-07.3 | Invalid names warned but not fatal | Edge case | Warning in log |
| AC-07.4 | Empty/missing runs all categories | Default | Full report |
| AC-07.5 | Case-insensitive matching | Edge case | "Health" == "health" |

## US-08: Timeout Configuration

| # | Criterion | Source Scenario | Verification |
|---|-----------|-----------------|--------------|
| AC-08.1 | Default timeout: 30 minutes | Default | Job config |
| AC-08.2 | Overridable via template extension | Happy path | timeout: keyword |
| AC-08.3 | Timeout kill produces no artifact | Error path | GitLab native behavior |
| AC-08.4 | Docs recommend --skip-blame for large repos | Documentation | Guide section |

## US-09: Concurrency Safeguards

| # | Criterion | Source Scenario | Verification |
|---|-----------|-----------------|--------------|
| AC-09.1 | Docs describe resource_group vs parallel | Documentation | Section exists |
| AC-09.2 | Template includes commented resource_group | Template | Commented line |
| AC-09.3 | Jobs are fully isolated (no shared state) | Happy path | No persistent cache |
| AC-09.4 | Staggering guidance for >10 concurrent | Documentation | Recommendation |
