# Component Boundaries -- gitlab-pipeline-api

**Date**: 2026-03-25
**Author**: Morgan (Solution Architect)

---

## Overview

This feature has three components. All are CI configuration artifacts -- no application code. Each has a clear responsibility boundary and a well-defined interface.

---

## Component 1: analyze-api Job

**Location**: `.gitlab-ci.yml` (new stage `api`, new job `analyze-api`)

**Responsibility**: Accept triggered pipeline requests, validate inputs, clone public target repository, run barad-dur analysis, optionally enforce score gate, publish HTML report artifact.

**Boundary**: This job is the "server side" of the pipeline API. It owns:
- Input validation (REPO_URL required, MIN_SCORE is integer, CATEGORIES are valid)
- Shell injection prevention for ANALYSIS_OPTIONS
- Clone orchestration (git clone with branch selection)
- CLI invocation (barad-dur analyze + optional gate)
- Artifact production (barad-dur-report.html — self-contained interactive HTML report)
- Error discrimination (input error vs clone error vs gate error)

**Does NOT own**:
- The barad-dur CLI behavior (existing Rust code, unchanged)
- The Docker image build (existing `docker` stage, unchanged)
- Caller-side logic (trigger, poll, download)

**Interface**:
- Input: GitLab trigger variables (REPO_URL, REPO_BRANCH, ANALYSIS_OPTIONS, MIN_SCORE, CATEGORIES)
- Output: Pipeline artifact `barad-dur-report.html` (interactive HTML report) + exit code (0 or 1)
- Activation: `rules: - if: $CI_PIPELINE_SOURCE == "trigger" && $REPO_URL`

**Isolation from existing CI**: The `analyze-api` job uses a `rules:` clause that activates only on `trigger` pipeline source with `REPO_URL` present. All existing jobs either have no rules (default push/MR) or explicit rules for `$CI_COMMIT_TAG`, `$CI_COMMIT_BRANCH`, or `$CI_PIPELINE_SOURCE == "schedule"`. The two sets never overlap.

---

## Component 2: Caller Template

**Location**: `ci/trigger-template.yml` (new file in barad-dur repo)

**Responsibility**: Provide a reusable, includable CI job definition that caller projects extend to trigger barad-dur analysis without writing curl scripts.

**Boundary**: This template owns:
- Hidden job `.barad-dur-analysis` with sensible defaults
- Required variable validation (BARAD_DUR_TRIGGER_TOKEN, BARAD_DUR_PROJECT_ID)
- Trigger + poll + download + parse script logic
- Commented `resource_group` for concurrency control
- Default timeout (30 minutes)

**Does NOT own**:
- Caller-specific variables (REPO_URL value, MIN_SCORE threshold)
- Trigger token creation (manual setup step)
- Froggit infrastructure (runners, registry)

**Interface**:
- Input: Caller extends `.barad-dur-analysis` with project-specific variables
- Output: Downloads `barad-dur-report.html` into caller job workspace; prints score summary
- Activation: `include: project: 'devops/barad-dur'` + `extends: .barad-dur-analysis`

**Dependency**: Requires `BARAD_DUR_TRIGGER_TOKEN` (masked) and `BARAD_DUR_PROJECT_ID` as CI variables in the caller project.

---

## Component 3: Documentation

**Location**: `docs/pipeline-api-setup.md` (new file)

**Responsibility**: Guide DevOps engineers through end-to-end setup: token creation, variable configuration, template inclusion, verification, and troubleshooting.

**Boundary**: This document owns:
- Prerequisites and permissions
- Step-by-step setup instructions
- Troubleshooting for top 5 errors (401, 404, missing vars, clone fail, timeout)
- Performance guidance (--skip-blame for large repos)
- Concurrency guidance (resource_group, staggering)

**Does NOT own**:
- barad-dur CLI documentation (existing README)
- GitLab CI syntax reference (external docs)

---

## Component Interaction Map

```
Caller Project                          barad-dur Project
+----------------------------+          +-----------------------------+
| .gitlab-ci.yml             |          | .gitlab-ci.yml              |
|                            |          |   [existing stages: lint,   |
| include:                   |          |    build, test, analysis,   |
|   project: devops/barad-dur|--------->|    deploy, release, docker] |
|   file: ci/trigger-template|          |                             |
|                            |          |   [new stage: api]          |
| quality-check:             |          |     analyze-api job         |
|   extends: .barad-dur-     |  trigger |       |                     |
|            analysis        |--------->|       v                     |
|   variables:               |          |   clone + analyze + gate    |
|     REPO_URL: ...          |          |       |                     |
|                            |  artifact|       v                     |
|   (downloads report) <-----|<---------|   barad-dur-report.html     |
+----------------------------+          +-----------------------------+
```

---

## Traceability: Requirements to Components

| Requirement | Component |
|-------------|-----------|
| FR-01: Triggered Analysis Job | analyze-api job |
| FR-02: Variable-Based Configuration | analyze-api job |
| FR-03: Report Artifact | analyze-api job |
| FR-04: Score Gate | analyze-api job |
| FR-05: Error Reporting | analyze-api job |
| NFR-01: No Code Changes | All (CI config + docs only) |
| NFR-02: Security | analyze-api job + caller template |
| NFR-03: Performance | analyze-api job (timeout) + documentation (--skip-blame) |
| NFR-04: Isolation | analyze-api job (container isolation) |
| NFR-05: Compatibility | All (CE features only) |

| User Story | Primary Component | Supporting Component |
|------------|-------------------|---------------------|
| US-01: analyze-api job | analyze-api job | -- |
| US-02: Caller example | Documentation | Caller template |
| US-03: Options + gate | analyze-api job | -- |
| US-04: Caller template | Caller template | -- |
| US-05: Setup docs | Documentation | -- |
| US-06: Branch selection | analyze-api job | -- |
| US-07: Category filter | analyze-api job | -- |
| US-08: Timeout | analyze-api job | Caller template |
| US-09: Concurrency | Documentation | Caller template |
