# Journey: Pipeline Trigger API

## Overview

A DevOps engineer (Fatima Benali) configures her team's CI pipeline to call
barad-dur on Froggit via GitLab's pipeline trigger API. The analysis runs as a
triggered pipeline, and the JSON report is downloaded as a pipeline artifact.

## Actors

| Actor | Type | Context |
|-------|------|---------|
| Fatima Benali | Human | DevOps engineer configuring the calling pipeline |
| nightly-quality pipeline | Automated | Fatima's team pipeline that triggers barad-dur |
| barad-dur pipeline | Automated | The triggered pipeline on the barad-dur project |

## Journey Flow (Happy Path)

```
  Fatima (one-time setup)             nightly-quality pipeline              barad-dur pipeline
  ========================            ===========================           ======================

  1. Create trigger token
     on barad-dur project
     (Settings > CI/CD > Triggers)
         |
  2. Store token as CI variable
     in her project
     ($BARAD_DUR_TRIGGER_TOKEN)
         |
  3. Write .gitlab-ci.yml job
     that calls trigger API
     ───────────────────────────────> 4. POST /api/v4/projects/:id/trigger/pipeline
                                         variables[REPO_URL]=https://...
                                         variables[REPO_BRANCH]=main
                                         variables[ANALYSIS_OPTIONS]=--skip-blame
                                         token=$BARAD_DUR_TRIGGER_TOKEN
                                         ref=main
                                                                           |
                                                                      5. Pipeline starts
                                                                         "analyze-api" job runs
                                                                         |
                                                                      6. barad-dur analyze $REPO_URL
                                                                         --json --pretty
                                                                         > barad-dur-report.json
                                                                         |
                                                                      7. Report saved as artifact
                                                                         (barad-dur-report.json)
                                                                           |
                                      8. Poll pipeline status    <─────────┘
                                         (GET /api/v4/projects/:id/pipelines/:pipeline_id)
                                         |
                                      9. Download artifact
                                         (GET /api/v4/projects/:id/jobs/:job_id/artifacts)
                                         |
                                     10. Parse JSON, extract score,
                                         fail if below threshold
```

## Emotional Arc (Lightweight)

```
Confidence
    ^
    |
  5 |                                                          *──────* (10. done)
    |                                                     *───*
  4 |                                           *────*───*
    |                                      *───*  (7-9. polling, downloading)
  3 |              *────────*─────────────*
    |         *───*                (5-6. waiting for pipeline)
  2 |    *───*
    |   * (1-2. setup: "will this work?")
  1 |
    +──────────────────────────────────────────────────────────────> time
     setup        trigger          running         retrieve
```

- **Setup (steps 1-3)**: Moderate uncertainty — "Am I using the right project ID? Is the token scoped correctly?"
- **Trigger (step 4)**: Brief anxiety — "Did it accept my request?"
- **Running (steps 5-7)**: Passive wait — confidence builds as pipeline shows "running"
- **Retrieve (steps 8-10)**: Satisfaction — "I have the report, my pipeline can use the score"

## Error Paths

```
  Error Point              What Goes Wrong                  User Sees
  ─────────────────────────────────────────────────────────────────────
  Step 4: Trigger          Invalid token                    HTTP 401 from GitLab API
  Step 4: Trigger          Wrong project ID                 HTTP 404 from GitLab API
  Step 4: Trigger          Missing ref (branch)             HTTP 400 from GitLab API
  Step 6: Analysis         Invalid REPO_URL                 Job fails, "fatal: repo not found"
  Step 6: Analysis         Private repo, no clone access    Job fails, "authentication failed"
  Step 6: Analysis         Repo too large / timeout         Job fails (CI timeout, default 1h)
  Step 6: Analysis         Empty/corrupt repository         Job exits with warning, empty report
  Step 9: Download         Artifact expired                 HTTP 404 on artifact download
  Step 9: Download         Job failed (no artifact)         No artifact to download
```

## TUI Mockups (Caller Pipeline Output)

### Successful trigger and retrieval

```
$ # In the calling pipeline job log:
[trigger-analysis] Triggering barad-dur pipeline...
[trigger-analysis]   Project: barad-dur (ID: 4217)
[trigger-analysis]   Repo URL: https://froggit.example.com/team/my-service.git
[trigger-analysis]   Branch: main
[trigger-analysis]   Options: --skip-blame
[trigger-analysis] Pipeline created: #58432 (status: pending)
[trigger-analysis] Waiting for pipeline to complete...
[trigger-analysis]   Status: running (45s elapsed)
[trigger-analysis]   Status: running (90s elapsed)
[trigger-analysis]   Status: success (127s elapsed)
[trigger-analysis] Downloading artifact: barad-dur-report.json
[trigger-analysis] Overall score: 74/100
[trigger-analysis] Health: 68 | Team: 71 | Evolution: 79 | Hygiene: 82
[trigger-analysis] PASS: score 74 >= threshold 60
```

### Failed analysis (bad repo URL)

```
$ # In the barad-dur triggered pipeline job log:
[analyze-api] Repository URL: https://froggit.example.com/nonexistent/repo.git
[analyze-api] Cloning repository...
[analyze-api] Error: Failed to clone: repository not found
[analyze-api] Exit code: 1

$ # In the calling pipeline job log:
[trigger-analysis] Pipeline #58433 failed (status: failed)
[trigger-analysis] ERROR: barad-dur analysis failed. Check pipeline #58433 logs.
```

## Integration Points

| Point | System | Protocol | Notes |
|-------|--------|----------|-------|
| Trigger | GitLab Pipeline Trigger API | HTTPS POST | `/api/v4/projects/:id/trigger/pipeline` |
| Status polling | GitLab Pipeline API | HTTPS GET | `/api/v4/projects/:id/pipelines/:id` |
| Artifact download | GitLab Job Artifacts API | HTTPS GET | `/api/v4/projects/:id/jobs/:id/artifacts` |
| Repo clone | Git over HTTPS | HTTPS | From within the triggered job |
| Docker registry | Froggit Container Registry | HTTPS | `$CI_REGISTRY_IMAGE:latest` |
