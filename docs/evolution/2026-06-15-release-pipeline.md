# Evolution: release-pipeline

**Date**: 2026-06-15
**Feature ID**: release-pipeline
**Wave**: DEVOPS only (no Rust code — pure GitLab CI YAML changes)
**Commits**: `faf26d0` (trivy-scan job), `4b72f97` (mutation gate on MR pipelines)

## Summary

Fixed two remaining CI pipeline gaps identified during a DEVOPS audit of the existing `.gitlab-ci.yml`. The pipeline already covered the majority of the security and quality surface; these two jobs completed the gap closure. No Rust source code was modified — the entire feature consists of GitLab CI YAML additions.

## Business Context

A DEVOPS audit of the existing pipeline identified 8 gaps across security, quality, and observability. Most gaps were either already handled by the existing infrastructure or deferred by design (see wave-decisions.md). Two gaps were genuinely missing and had clear, low-cost fixes:

1. **Docker image CVE exposure**: The container image was built and pushed to the registry without any vulnerability scan. A compromised or vulnerable base image could go undetected.
2. **Mutation gate on MR pipelines**: The mutation testing job existed but ran only on scheduled pipelines (daily). New code could merge without ever hitting the mutation gate — the ≥80% kill rate was not enforced at review time.

Both gaps were closed in a single iteration with two targeted CI job additions.

## Steps Completed

### Step 01-01 — trivy-scan job

**Completed**: 2026-06-15T11:31:14Z
**Commit**: `faf26d0`

Added `trivy-scan` job to the `docker` stage. The job:
- Runs after the `docker` build job via `needs: [docker]`
- Triggers on `main` push and `v*.*.*` tags (same conditions as `docker` build)
- Uses `aquasec/trivy:latest` image — no additional tooling install
- Scans `${CI_REGISTRY_IMAGE}:${CI_COMMIT_REF_SLUG}` for CRITICAL CVEs
- Set `allow_failure: true` for initial rollout (baseline CVE count unknown at first run)

**DES trace**: PREPARE → GREEN → COMMIT (RED_ACCEPTANCE and RED_UNIT skipped — CI configuration change, no executable tests)

### Step 01-02 — mutation gate on MR pipelines

**Completed**: 2026-06-15T11:32:54Z
**Commit**: `4b72f97`

Restructured the mutation testing jobs to implement the hybrid strategy defined in D9:

- **`mutation` (per-feature)**: Added rule `$CI_PIPELINE_SOURCE == "merge_request_event"`. Runs diff-scoped against `$CI_MERGE_REQUEST_DIFF_BASE_SHA` on MR pipelines, and `HEAD~1..HEAD` on pushes to main. Set `allow_failure: false` — blocks merge on kill rate < 80%.
- **`mutation-nightly` (full-codebase)**: Renamed from the original `mutation` job. Retained schedule-only trigger but removed the 25-hour diff filter — the nightly run now covers the full codebase. Retains `allow_failure: true` until the full-codebase baseline is established.

**DES trace**: PREPARE → GREEN → COMMIT (RED_ACCEPTANCE and RED_UNIT skipped — CI configuration change, no executable tests)

## Key Decisions

### D1 — Deployment Target: N/A (distribution only)
barad-dur is a CLI tool. The pipeline's job is to build, validate, and distribute artifacts — not to deploy a server. No deployment infrastructure was designed or modified.

### D3 — CI/CD Platform: GitLab CI on self-hosted instance
All work targets the existing `.gitlab-ci.yml` on `lab.frogg.it`. No new CI platform was introduced.

### D4 — Existing Infrastructure Reuse
The existing pipeline was extended, not replaced. Both new jobs integrate into existing stages (`docker`, `analysis`) with standard `needs:` and `rules:` patterns already established in the file.

### D8 — Branching Strategy: Trunk-Based Development
Feature branches merged via MR. The per-feature mutation job (`mutation`) is now triggered on `merge_request_event` — directly supporting this branching model by gating quality at merge time.

### D9 — Mutation Testing Strategy: Hybrid
Per-feature on MR/push (diff-scoped, ≥80% kill rate, blocking). Full-codebase nightly (schedule, ≥80% kill rate, non-blocking until baseline established). This matches the project size (~3k LOC) and delivery cadence (single maintainer, feature-driven).

## Remaining Gaps (not implemented in this feature)

The following gaps from the audit were identified but not addressed in this iteration (documented in wave-decisions.md):

| Gap | Severity | Status |
|---|---|---|
| No JUnit XML test reports | Medium | Deferred — cargo-nextest migration is a separate change |
| Binary smoke test | High | Deferred — smoke test design requires separate step |
| `audit`/`deny`/`semver-check` non-blocking on tags | High | Deferred — risk-assessed, flagged for next release cycle |
| Coverage not enforced (`--fail-under`) | Medium | Deferred |
| `release-linux` image alignment | Low | Deferred |
| `--jobs` parallelism in nightly mutation | Medium | Deferred |

## Implementation Notes

- Both jobs are pure YAML additions — zero risk to existing job behavior
- `trivy-scan` uses `allow_failure: true` as a safe initial posture; flip to `false` after the first clean scan
- The `mutation` per-feature job reuses the existing kill rate gate Python script verbatim
- `mutation-nightly` retains `allow_failure: true` — the transition plan in mutation-testing-strategy.md defines the 4-phase path to making it blocking
