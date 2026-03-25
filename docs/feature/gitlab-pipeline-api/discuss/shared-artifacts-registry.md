# Shared Artifacts Registry: gitlab-pipeline-api

## Artifact Inventory

| ID | Name | Format | Source | Consumers | Sensitivity |
|----|------|--------|--------|-----------|-------------|
| SA-01 | Pipeline Trigger Token | String (secret) | Froggit UI (barad-dur project) | Caller pipeline CI variable | **Secret** — must be masked |
| SA-02 | Barad-dur Project ID | Integer | Froggit project settings | Caller pipeline config | Public within instance |
| SA-03 | REPO_URL | String (HTTPS URL) | Caller pipeline variable | Trigger API, analyze-api job | Internal |
| SA-04 | REPO_BRANCH | String | Caller pipeline variable | Trigger API, analyze-api job | Internal |
| SA-05 | ANALYSIS_OPTIONS | String | Caller pipeline variable | analyze-api job | Internal |
| SA-06 | MIN_SCORE | Integer | Caller pipeline variable | analyze-api job | Internal |
| SA-07 | CATEGORIES | String (CSV) | Caller pipeline variable | analyze-api job | Internal |
| SA-08 | Pipeline ID | Integer | GitLab trigger API response | Status polling, artifact download | Internal |
| SA-09 | Job ID | Integer | GitLab pipeline jobs API | Artifact download | Internal |
| SA-10 | barad-dur-report.json | JSON file | barad-dur CLI output | Artifact download, report parsing | Internal |
| SA-11 | Docker image tag | String | CI_REGISTRY_IMAGE | analyze-api job (image reference) | Internal |

## Single Source of Truth

| Artifact | Authoritative Source | Never Duplicate To |
|----------|---------------------|--------------------|
| Trigger Token (SA-01) | Froggit > barad-dur > Settings > CI/CD > Triggers | Do not hardcode in .gitlab-ci.yml |
| Project ID (SA-02) | Froggit project page | Use CI variable, not magic number |
| Docker image (SA-11) | CI_REGISTRY_IMAGE variable | Do not hardcode registry URL |

## Data Flow

```
Caller Project                          Barad-dur Project
─────────────────                       ──────────────────
CI Variables:                           Pipeline Triggers:
  BARAD_DUR_TOKEN (SA-01) ──────────────> token validation
  BARAD_DUR_PROJECT_ID (SA-02) ─────────> project lookup

Trigger Job:                            analyze-api Job:
  REPO_URL (SA-03) ─────────────────────> git clone target
  REPO_BRANCH (SA-04) ──────────────────> branch checkout
  ANALYSIS_OPTIONS (SA-05) ─────────────> CLI flags
  MIN_SCORE (SA-06) ────────────────────> gate threshold
  CATEGORIES (SA-07) ───────────────────> category filter
                                          │
  Pipeline ID (SA-08) <─────────────────── trigger response
                                          │
  barad-dur-report.json (SA-10) <───────── artifact download
```

## Integration Checkpoints

| Checkpoint | Validates | Method |
|------------|-----------|--------|
| IC-01: Token validity | SA-01 is a valid trigger token for SA-02 | Trigger returns 201 (not 401) |
| IC-02: Repo accessibility | SA-03 is clonable from within the CI runner | Clone succeeds (not 404/403) |
| IC-03: Artifact produced | SA-10 exists after job completion | Artifact download returns 200 |
| IC-04: Report parseable | SA-10 contains valid JSON with expected schema | JSON parse + field check |
| IC-05: Image available | SA-11 resolves to a pullable Docker image | Job starts (not image pull error) |
