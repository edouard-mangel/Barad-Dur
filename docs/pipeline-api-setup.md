# barad-dur Pipeline API Setup Guide

Analyze any public git repository via the GitLab Pipeline Trigger API and get an interactive HTML report.

## Prerequisites

- barad-dur project hosted on Froggit with CI/CD enabled
- Docker image built and pushed to registry (automatic on main branch pushes)
- Maintainer access to the barad-dur project (for trigger token creation)

## Quick Start

### 1. Create a Pipeline Trigger Token

In the barad-dur project on Froggit:
**Settings > CI/CD > Pipeline trigger tokens > Add trigger**

Copy the token — you'll need it for API calls.

### 2. Trigger an Analysis (curl)

```bash
curl --request POST \
  "https://lab.music-music.com/api/v4/projects/<PROJECT_ID>/trigger/pipeline" \
  --form "token=<TRIGGER_TOKEN>" \
  --form "ref=main" \
  --form "variables[REPO_URL]=https://github.com/torvalds/linux.git"
```

Response:
```json
{
  "id": 58432,
  "web_url": "https://lab.music-music.com/devops/barad-dur/-/pipelines/58432"
}
```

### 3. Wait for Completion

Poll the pipeline status:
```bash
curl -s "https://lab.music-music.com/api/v4/projects/<PROJECT_ID>/pipelines/58432" \
  --header "PRIVATE-TOKEN: <YOUR_TOKEN>" \
  | jq '.status'
```

### 4. Download the HTML Report

```bash
# Get the job ID
JOB_ID=$(curl -s "https://lab.music-music.com/api/v4/projects/<PROJECT_ID>/pipelines/58432/jobs" \
  --header "PRIVATE-TOKEN: <YOUR_TOKEN>" \
  | jq -r '.[] | select(.name == "analyze-api") | .id')

# Download the report
curl -o report.html \
  "https://lab.music-music.com/api/v4/projects/<PROJECT_ID>/jobs/${JOB_ID}/artifacts/barad-dur-report.html" \
  --header "PRIVATE-TOKEN: <YOUR_TOKEN>"
```

Open `report.html` in your browser — it's a self-contained interactive report with D3 visualizations.

## CI/CD Integration (Template)

For automated analysis from another project's pipeline:

### 1. Set CI Variables

In your project: **Settings > CI/CD > Variables**

| Variable | Value | Options |
|----------|-------|---------|
| `BARAD_DUR_TRIGGER_TOKEN` | The trigger token from step 1 | Masked |
| `BARAD_DUR_PROJECT_ID` | barad-dur's project ID | — |

### 2. Include the Template

```yaml
include:
  - project: 'devops/barad-dur'
    file: 'ci/trigger-template.yml'

analyze-my-repo:
  extends: .barad-dur-analysis
  variables:
    REPO_URL: "https://github.com/my-org/my-repo.git"
```

### 3. Advanced Options

```yaml
analyze-my-repo:
  extends: .barad-dur-analysis
  variables:
    REPO_URL: "https://github.com/my-org/my-repo.git"
    REPO_BRANCH: "develop"           # Default: main
    MIN_SCORE: "70"                   # Fail if score < 70
    CATEGORIES: "health,hygiene"      # Analyze specific categories
    BARAD_DUR_TIMEOUT: "3600"         # Wait up to 1 hour (default: 30min)
```

## Trigger Variables Reference

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `REPO_URL` | Yes | — | HTTPS URL of a public git repository |
| `REPO_BRANCH` | No | `main` | Branch to analyze |
| `MIN_SCORE` | No | — | Quality gate threshold (0-100) |
| `CATEGORIES` | No | all | Comma-separated: health, team, evolution, hygiene |

## Output

The pipeline produces `barad-dur-report.html` as an artifact:
- Self-contained single-file interactive report
- 5 tabs: Overview, Hotspots, Coupling, Ownership, Age
- D3-powered charts and drill-down tables
- Dark theme, works offline
- Retained for 1 week (available even if quality gate fails)

## Performance Tips

- For large repositories (>10K commits), consider analyzing a shorter time window:
  add `--since 3months` via the analysis options
- For repositories with more than 50,000 commits, add `--skip-blame` to `ANALYSIS_OPTIONS` to significantly reduce analysis time.
- The `--skip-blame` flag significantly speeds up analysis at the cost of some metrics accuracy
- Default timeout is 30 minutes — increase `BARAD_DUR_TIMEOUT` for very large repos

## Troubleshooting

| Problem | Cause | Fix |
|---------|-------|-----|
| HTTP 401 on trigger | Invalid or expired trigger token | Regenerate token in project settings |
| HTTP 404 on trigger | Wrong project ID | Verify `BARAD_DUR_PROJECT_ID` matches the barad-dur project |
| Clone failure | Private repo or invalid URL | Only public HTTPS URLs are supported |
| Timeout | Large repository | Increase `BARAD_DUR_TIMEOUT` or use `--skip-blame` |
| Empty report | Analysis error | Check the pipeline job log for error details |

## Concurrency

By default, multiple triggered analyses run in parallel. If runner capacity is limited, add `resource_group` to serialize:

```yaml
analyze-my-repo:
  extends: .barad-dur-analysis
  resource_group: barad-dur-analysis
  variables:
    REPO_URL: "..."
```

**Parallel vs resource_group trade-offs**: The default (parallel) lets all triggered jobs run concurrently, giving faster total throughput but higher peak runner load. Adding `resource_group` forces jobs to run sequentially, which lowers peak runner load at the cost of longer wall-clock time for callers. Use the default parallel mode unless runner queue depth becomes a bottleneck; switch to `resource_group` only when you observe jobs waiting significantly longer than the analysis itself takes.

**Staggering schedules for large rollouts**: For company-wide rollouts with more than 10 teams triggering simultaneously, consider offsetting cron schedules (e.g., stagger by 5–10 minutes per team) to distribute runner load without requiring `resource_group`. This preserves parallel throughput within each team's window while smoothing aggregate demand across teams.
