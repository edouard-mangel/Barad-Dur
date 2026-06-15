# Technology Stack -- gitlab-pipeline-api

**Date**: 2026-03-25
**Author**: Morgan (Solution Architect)

---

## Overview

This feature requires no new technologies. It composes existing tools and platform capabilities. Every component listed below is either already in use or is a built-in GitLab CI feature.

---

## Stack

| Component | Technology | Version | License | Rationale | Status |
|-----------|-----------|---------|---------|-----------|--------|
| Analysis engine | barad-dur CLI | current (v0.7.0) | Project license | The tool being exposed; no changes needed | Exists |
| Container image | Docker (scratch-based) | ~31MB | Apache 2.0 | Minimal image with git + barad-dur + SSL certs | Exists |
| Container registry | Froggit Container Registry | GitLab instance version | GitLab EE (self-hosted) | Hosts the barad-dur Docker image | Exists |
| CI pipeline | GitLab CI | Froggit instance version | MIT (GitLab CE features only) | Pipeline trigger, job artifacts, rules | Exists |
| Trigger mechanism | GitLab Pipeline Trigger API | v4 | MIT (GitLab CE) | Free-tier feature; no premium required | Exists |
| Artifact storage | GitLab Job Artifacts | Froggit instance version | MIT (GitLab CE) | Built-in artifact storage with retention | Exists |
| CI runners | Froggit shared Docker runners | Instance-managed | N/A | Execute the analyze-api job | Exists |
| Caller dependencies | curl + jq | System packages | MIT / MIT | Available in all standard CI images | Exists |
| CI template | GitLab CI `include:project:` | v4 | MIT (GitLab CE) | Cross-project template sharing, free tier | Exists |

---

## New Files (not technologies)

| File | Purpose | Type |
|------|---------|------|
| `.gitlab-ci.yml` (modified) | Add `api` stage + `analyze-api` job | CI configuration |
| `ci/trigger-template.yml` | Reusable hidden job for caller projects | CI template |
| `docs/pipeline-api-setup.md` | Setup guide for DevOps engineers | Documentation |

---

## Rejected Alternatives

### Alternative: HTTP Server (e.g., Actix-web, Axum)

Wrapping barad-dur in an HTTP server to expose a REST API.

**Rejected**: Requires Rust code changes, a new deployment target (always-on server), TLS termination, authentication middleware, and operational burden (monitoring, restarts, scaling). Violates NFR-01 (no code changes) and the time-to-market priority. GitLab CI provides all of this for free.

### Alternative: Serverless Function (e.g., AWS Lambda, OpenFaaS)

Deploying barad-dur as a serverless function triggered by HTTP.

**Rejected**: Requires a serverless platform (not available on Froggit), packaging changes, cold-start latency, and external infrastructure. The 31MB Docker image with git is too large for typical serverless cold-start budgets. GitLab CI runners are already available.

### Alternative: GitLab CI Child Pipeline (instead of trigger)

Using `trigger:` keyword to spawn a child pipeline within the same project.

**Rejected**: Child pipelines are for decomposing a single project's CI, not for cross-project API invocation. The `trigger:` keyword with `strategy: depend` works project-internally but does not provide the external API contract that callers need. Pipeline Trigger API is the correct mechanism for cross-project triggering.

---

## Compatibility Constraints

- All GitLab CI features used are free-tier (CE). No premium/ultimate features required.
- `include:project:` requires projects to be on the same Froggit instance.
- Pipeline trigger tokens are per-project and require Maintainer access to create.
- Artifact storage counts against project storage quota (Froggit-managed).

---

## Architectural Enforcement

Since this feature is entirely CI configuration (no Rust code), traditional architecture enforcement tools (ArchUnit, import-linter) do not apply. Instead:

| Rule | Enforcement |
|------|-------------|
| analyze-api job only runs on trigger source | GitLab CI `rules:` syntax; CI lint validates on push |
| Existing jobs unaffected by trigger | GitLab CI implicit rules; verify via pipeline simulation (`gitlab-ci-lint`) |
| No secrets in artifact output | Code review checklist; script must not echo env vars to artifact file |
| Template validates required variables | Script-level `if [ -z "$VAR" ]` checks in template |
