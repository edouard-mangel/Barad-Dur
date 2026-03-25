# ADR-007: GitLab CI Pipeline Trigger over HTTP Server for Pipeline API

**Status**: Accepted
**Date**: 2026-03-25
**Feature**: gitlab-pipeline-api
**Deciders**: Morgan (Solution Architect)

---

## Context

Teams on Froggit want to integrate barad-dur repository health analysis into their CI/CD workflows. This requires exposing barad-dur as a service that other pipelines can invoke programmatically, passing a target repository URL and receiving a structured JSON report.

Three architectural approaches were evaluated:
1. An always-on HTTP server wrapping the CLI
2. A serverless function deployment
3. GitLab CI Pipeline Trigger API as the service boundary

The decision must optimize for: time-to-market (highest priority), maintainability (solo developer), zero Rust code changes (NFR-01), and use of existing Froggit infrastructure.

---

## Decision

Use GitLab CI Pipeline Trigger API as the service interface. A new `analyze-api` job in `.gitlab-ci.yml` runs only when triggered via the pipeline trigger API with a `REPO_URL` variable. The existing Docker image serves as the execution environment. The JSON report is published as a pipeline artifact downloadable via the GitLab Job Artifacts API.

---

## Alternatives Considered

### Alternative 1: HTTP Server (Actix-web or Axum)

Wrap barad-dur in an HTTP framework. Expose `POST /analyze` endpoint. Return JSON in response body.

**Evaluation**:
- (+) Standard REST API semantics; familiar to all developers
- (+) Synchronous request/response; no polling needed
- (-) Requires Rust code changes (violates NFR-01)
- (-) Requires deployment infrastructure: always-on server, TLS termination, process supervisor, health checks, monitoring
- (-) Authentication and authorization must be implemented from scratch
- (-) Scaling requires load balancer or manual instance management
- (-) Operational burden for a solo developer: uptime, restarts, security patches

**Rejected**: Violates NFR-01 (no code changes). Introduces significant operational complexity for a solo developer. Time-to-market: weeks instead of days.

### Alternative 2: Serverless Function (OpenFaaS / Knative)

Package barad-dur as a serverless function triggered by HTTP.

**Evaluation**:
- (+) No always-on server; pay-per-invocation
- (+) Auto-scaling built in
- (-) Requires a serverless platform (not available on Froggit)
- (-) 31MB Docker image with git causes cold-start latency (10-30 seconds)
- (-) Analysis runs 1-30 minutes, exceeding typical serverless timeouts (15 min max on most platforms)
- (-) Requires code changes for function handler wrapper
- (-) Requires external infrastructure outside Froggit

**Rejected**: No serverless platform available on Froggit. Long-running analysis (up to 30+ minutes) exceeds serverless timeout limits. Requires external infrastructure and code changes.

### Alternative 3: GitLab CI Child Pipeline (trigger: keyword)

Use GitLab CI `trigger:` keyword to spawn a child pipeline from within the same project.

**Evaluation**:
- (+) Native GitLab CI feature
- (+) No code changes
- (-) Child pipelines are intra-project only; cannot be invoked cross-project via API
- (-) `trigger:` keyword creates a downstream pipeline in the same or another project, but the calling mechanism is pipeline YAML, not an external API
- (-) Does not provide the external trigger token authentication that callers need

**Rejected**: Child pipelines do not provide a cross-project API interface. The Pipeline Trigger API is the correct GitLab mechanism for external invocation.

### Alternative 4: GitLab CI Pipeline Trigger API (this ADR)

Use the built-in Pipeline Trigger API with a new `analyze-api` job.

**Evaluation**:
- (+) Zero Rust code changes (NFR-01 satisfied)
- (+) Zero new infrastructure (uses existing runners, registry, artifact storage)
- (+) Authentication built in (trigger tokens, masked variables)
- (+) Isolation built in (fresh container per trigger)
- (+) Artifact management built in (retention, download API)
- (+) Time-to-market: days, not weeks
- (+) Solo developer can maintain (single YAML file)
- (-) Asynchronous: callers must poll for completion (adds ~15-30 seconds latency)
- (-) Not a standard REST API; requires GitLab-specific client code

**Accepted**: Best fit for all quality attributes and constraints.

---

## Consequences

### Positive

- No changes to Rust source code; feature is entirely CI configuration + documentation
- No new services to deploy, monitor, or maintain
- Authentication, isolation, and artifact management provided by GitLab for free
- Deliverable in ~5 days across 3 releases
- Uses only GitLab CE (free tier) features; no premium/ultimate dependencies
- Each trigger runs in a fresh container with no shared state (NFR-04)

### Negative

- Callers must implement polling loop (trigger -> poll -> download) instead of a single HTTP request/response
- Mitigation: reusable CI template (`ci/trigger-template.yml`) encapsulates the polling logic
- Pipeline trigger API is GitLab-specific; callers outside GitLab CI would need to implement the 3-step flow manually
- Analysis latency includes pipeline scheduling overhead (typically 5-30 seconds on Froggit shared runners)
- Artifact retention counts against project storage quota

### Risks

- If Froggit upgrades GitLab and the Trigger API changes behavior, the feature may break
- Mitigation: Pin to API v4; document smoke test for post-upgrade verification
