# Walking Skeleton — gitlab-pipeline-api

## Upstream Issue: JSON → HTML Artifact

DISCUSS acceptance criteria (acceptance-criteria.md, AC-01.4, AC-02.4) reference `barad-dur-report.json` throughout.
DESIGN wave decision (architecture-design.md, component-boundaries.md) changed the artifact to `barad-dur-report.html` — a self-contained interactive HTML report.

All walking skeleton scenarios use `barad-dur-report.html`. The JSON reference in DISCUSS is superseded.

---

## Walking Skeleton Definition

The walking skeleton answers: "Can a DevOps engineer trigger barad-dur analysis from their CI pipeline and receive a downloadable report?"

It covers US-01 + US-02 only — the thinnest vertical slice that delivers observable value end-to-end:

```
[Caller Pipeline]
    |
    | POST /trigger/pipeline (REPO_URL)
    v
[analyze-api job in .gitlab-ci.yml]
    |
    | git clone + barad-dur analyze --html
    v
[barad-dur-report.html artifact]
    |
    | GET /jobs/:id/artifacts/barad-dur-report.html
    v
[Caller receives interactive HTML report]
```

---

## Implementation Status: DONE

Both walking skeleton stories (US-01 + US-02) are fully implemented.

### Component 1: analyze-api job

**File**: `.gitlab-ci.yml`, lines 596–661

What is verified as implemented:
- Job `analyze-api` exists in stage `api` (line 596)
- Rule: `$CI_PIPELINE_SOURCE == "trigger" && $REPO_URL` (line 600) — activates only on triggers
- Image: `${CI_REGISTRY_IMAGE}:latest` (line 598) — uses existing Docker image
- REPO_URL validation: empty check (lines 607–611) + HTTPS prefix check (lines 612–616)
- MIN_SCORE validation: non-integer rejected (lines 617–621)
- Clone: `git clone --branch "${REPO_BRANCH}" --depth 0 "${REPO_URL}" /tmp/target` (line 625)
- CATEGORIES mapping to CLI flags: valid names → `--health/team/evolution/hygiene`, unknown → warning (lines 631–639)
- Analysis: `barad-dur analyze /tmp/target --html -o barad-dur-report.html` (line 643)
- Gate: `barad-dur gate /tmp/target --min-score "${MIN_SCORE}"` (lines 646–654)
- Artifact: `barad-dur-report.html`, `expire_in: 1 week`, `when: always` (lines 655–659)
- Timeout: `30 minutes` (line 660)

**Confirmed gap**: `ANALYSIS_OPTIONS` variable is NOT passed through to the analyze command (line 643 shows no `${ANALYSIS_OPTIONS:-}` in the command). This is US-03 scope and must be implemented in DELIVER.

### Component 2: Caller template

**File**: `ci/trigger-template.yml`, lines 1–142

What is verified as implemented:
- Hidden job `.barad-dur-analysis` defined (line 26)
- Required variable validation: `BARAD_DUR_TRIGGER_TOKEN`, `BARAD_DUR_PROJECT_ID`, `REPO_URL` (lines 33–47)
- Trigger POST with variable pass-through: REPO_URL, REPO_BRANCH, MIN_SCORE, CATEGORIES (lines 49–76)
- Polling loop: 15-second intervals, configurable `BARAD_DUR_TIMEOUT` (default 1800s) (lines 78–108)
- Artifact download: `GET /jobs/:job_id/artifacts/barad-dur-report.html` (lines 110–127)
- Empty report detection (lines 123–126)
- Artifact: `barad-dur-report.html`, `when: always` (lines 135–139)
- Commented `resource_group` for concurrency (lines 140–141)

**Confirmed gap**: `ANALYSIS_OPTIONS` is NOT forwarded in the trigger body (lines 49–76 show REPO_BRANCH, MIN_SCORE, CATEGORIES but not ANALYSIS_OPTIONS). This aligns with the gap in analyze-api job.

### Component 3: Setup documentation

**File**: `docs/pipeline-api-setup.md`, 148 lines

Sections confirmed present: Prerequisites, Quick Start (token, trigger curl, poll, download), CI/CD Integration (template), Trigger Variables Reference, Output, Performance Tips, Troubleshooting, Concurrency.

**Gap**: No explicit mention of `--skip-blame` recommendation specifically for repos with >50,000 commits (AC-08.4). The Performance Tips section mentions `--skip-blame` generally but does not give the commit threshold. This must be addressed in DELIVER.

**Gap**: No section dedicated to concurrency options explaining `resource_group` vs parallel trade-offs with staggering guidance for >10 concurrent triggers (AC-09.1, AC-09.4). The Concurrency section shows how to use `resource_group` but does not explain the trade-offs or staggering.

---

## Walking Skeleton Litmus Test

"Can Fatima trigger barad-dur analysis from her nightly pipeline and download an interactive HTML report?"

- Given: Trigger token created, BARAD_DUR_TRIGGER_TOKEN + BARAD_DUR_PROJECT_ID stored as CI variables
- When: Her pipeline job includes `ci/trigger-template.yml` and extends `.barad-dur-analysis` with her REPO_URL
- Then: The `analyze-api` job runs, produces `barad-dur-report.html`, and her pipeline downloads it

Answer: **Yes — demonstrable to stakeholders today.**

---

## Driving Ports

These are the entry points tests invoke (never internal components):

| Port | Description |
|------|-------------|
| GitLab Trigger API `POST /api/v4/projects/:id/trigger/pipeline` | Activates the analyze-api job |
| `analyze-api` job script in `.gitlab-ci.yml` | Processes variables, clones, analyzes |
| `.barad-dur-analysis` hidden job in `ci/trigger-template.yml` | Caller-side orchestration |
| `docs/pipeline-api-setup.md` file + section headers | Documentation existence and completeness |
