# Requirements: gitlab-pipeline-api

## Problem Statement

Teams on Froggit want to integrate repository health analysis into their CI/CD
workflows. Currently, to run barad-dur on a repository, someone must either
install the CLI locally or manually trigger a Docker run. There is no way for
one pipeline to request an analysis of another repository and consume the
structured results programmatically.

## Functional Requirements

### FR-01: Triggered Analysis Job

The barad-dur project must include a CI job ("analyze-api") that:
- Runs only when the pipeline is triggered via GitLab's pipeline trigger API
- Accepts a target repository URL via the `REPO_URL` trigger variable
- Clones the target repository and runs `barad-dur analyze` on it
- Produces a JSON report as a downloadable pipeline artifact
- Uses the existing Docker image from the Froggit container registry

### FR-02: Variable-Based Configuration

The analyze-api job must accept configuration via trigger variables:
- `REPO_URL` (required): HTTPS URL of the target repository
- `REPO_BRANCH` (optional, default "main"): Branch to analyze
- `ANALYSIS_OPTIONS` (optional): Additional CLI flags (e.g., `--skip-blame`)
- `MIN_SCORE` (optional): If set, the job fails when score is below threshold
- `CATEGORIES` (optional): Comma-separated category filter

### FR-03: Report Artifact

The analysis report must be:
- Saved as `barad-dur-report.json` in the job's artifact path
- Available for download via GitLab's job artifacts API
- Retained for at least 1 month (configurable via `expire_in`)
- Valid JSON matching the existing `barad-dur --json` output schema

### FR-04: Score Gate (Optional)

When `MIN_SCORE` is provided:
- The job must compare the overall score against the threshold
- Exit code 0 if score >= MIN_SCORE
- Exit code 1 if score < MIN_SCORE
- The report artifact must still be produced even on failure (for debugging)

### FR-05: Error Reporting

When analysis fails, the job must:
- Exit with a non-zero code
- Write a clear error message to the job log (not just a stack trace)
- Distinguish between: clone failure, analysis failure, gate failure

## Non-Functional Requirements

### NFR-01: No Code Changes

The feature must be implementable purely through CI configuration changes
(`.gitlab-ci.yml`) and documentation. No changes to the Rust source code.

### NFR-02: Security

- Trigger tokens must never appear in job logs (GitLab masks them by default)
- The analyze-api job must not expose credentials in artifact outputs
- The Docker image must remain minimal (scratch-based)

### NFR-03: Performance

- Analysis of a typical repository (< 10,000 commits, < 5,000 files) should
  complete within 10 minutes
- The `--skip-blame` option should be documented as a way to reduce runtime
  for large repositories

### NFR-04: Isolation

- Each triggered pipeline runs in its own CI job with a fresh container
- No shared state between analysis runs (no persistent cache across triggers)
- Multiple simultaneous triggers must not interfere with each other

### NFR-05: Compatibility

- Must work with Froggit's GitLab instance (self-hosted)
- Must use only standard GitLab CI features (no premium/ultimate features)
- Caller pipelines can use any CI image (curl is sufficient for triggering)

## Constraints

| Constraint | Description |
|------------|-------------|
| C-01 | GitLab pipeline trigger API limits apply (Froggit instance defaults) |
| C-02 | Artifact storage counts against project storage quota |
| C-03 | CI runners must have network access to clone target repos |
| C-04 | The Docker image must already be built and pushed before trigger use |
| C-05 | No caching between triggered runs (each is a clean analysis) |

## Dependencies

| Dependency | Status | Impact |
|------------|--------|--------|
| Docker image in registry | Exists | analyze-api job uses it as `image:` |
| Pipeline trigger token | Must create | Required for callers to authenticate |
| CI runner with Docker | Exists | Standard Froggit runner |
| Network access from runner | Exists | Required to clone target repos |
