# Data Models -- gitlab-pipeline-api

**Date**: 2026-03-25
**Author**: Morgan (Solution Architect)

---

## Overview

This feature introduces no new application-level data models. The HTML report is unchanged -- it is the existing `barad-dur analyze --html` output (a self-contained single-file interactive report with embedded CSS, JS, and D3 visualizations). This document specifies the API contract data: trigger variables, artifact format, and response structures.

---

## Trigger Variables (Input Contract)

| Variable | Type | Required | Default | Validation | Example |
|----------|------|----------|---------|------------|---------|
| `REPO_URL` | String (HTTPS URL) | Yes | -- | Must start with `https://`; git clone must succeed | `https://froggit.example.com/fintech/payment-gateway.git` |
| `REPO_BRANCH` | String | No | `main` | Must be an existing branch in target repo | `feature/new-payment-flow` |
| `ANALYSIS_OPTIONS` | String (CLI flags) | No | empty | Shell-safe quoting; no eval | `--skip-blame --since 3months` |
| `MIN_SCORE` | String (integer) | No | -- | Must parse as integer 0-100 | `70` |
| `CATEGORIES` | String (CSV) | No | all categories | Valid: health, team, evolution, hygiene (case-insensitive) | `health,hygiene` |

---

## Artifact Output (HTML Report)

The artifact `barad-dur-report.html` is the unchanged output of `barad-dur analyze --html -o barad-dur-report.html`. It is a self-contained single-file interactive HTML report with:
- Embedded CSS + D3.js visualizations
- 5 tabs: Overview (radar chart, gauges), Hotspots (scatter plot), Coupling (pairs table), Ownership (stacked bars), Age (timeline)
- Dark theme, works offline, no external dependencies
- The format is owned by the existing Rust codebase and is not modified by this feature.

Key characteristics for callers:
- Self-contained: can be opened directly in a browser, no server needed
- Interactive: D3-powered charts and drill-down tables
- Downloadable via GitLab Artifacts API as a single file

---

## GitLab API Structures (External, Not Owned)

### Trigger Response (POST /trigger/pipeline)

```json
{
  "id": 58432,
  "iid": 12,
  "status": "created",
  "web_url": "https://froggit.example.com/devops/barad-dur/-/pipelines/58432"
}
```

Key fields: `id` (for polling), `web_url` (for human-readable link in error messages).

### Pipeline Status Response (GET /pipelines/:id)

```json
{
  "id": 58432,
  "status": "success",
  "web_url": "https://froggit.example.com/devops/barad-dur/-/pipelines/58432"
}
```

Terminal statuses: `success`, `failed`, `canceled`, `skipped`.

### Job Listing Response (GET /pipelines/:id/jobs)

```json
[
  {
    "id": 92341,
    "name": "analyze-api",
    "status": "success",
    "artifacts": [
      { "filename": "barad-dur-report.html", "file_type": "archive" }
    ]
  }
]
```

Key field: `id` (job ID for artifact download).

---

## Error Messages (Job Log Output)

The analyze-api job writes structured messages to the job log for each error class:

| Error Class | Message Format | Exit Code |
|-------------|---------------|-----------|
| Missing REPO_URL | `ERROR: REPO_URL is required` | 1 |
| Invalid MIN_SCORE | `ERROR: MIN_SCORE must be a positive integer (got: '<value>')` | 1 |
| Clone failure | `ERROR: Failed to clone repository: <git error>` | 1 |
| Branch not found | `ERROR: Branch '<branch>' not found in repository` | 1 |
| Gate failure | `FAIL: overall score <N> < threshold <M>` | 1 |
| Gate pass | `PASS: overall score <N> >= threshold <M>` | 0 |

---

## No New Persistent Data

This feature creates no new databases, files, or persistent state. Each triggered pipeline:
- Clones the target repo to `/tmp` (ephemeral, container-local)
- Writes `barad-dur-report.html` to the job workspace (uploaded as artifact, then discarded)
- Leaves no trace after the job completes (aside from the artifact in GitLab storage)
