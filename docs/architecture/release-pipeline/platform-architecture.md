# Platform Architecture — release-pipeline

## Overview

barad-dur is a Rust CLI distributed as pre-built binaries and a Docker image. There is no server deployment — the platform exists purely to build, validate, and distribute release artifacts.

## Distribution Targets

| Artifact | Registry | Trigger |
|---|---|---|
| `barad-dur-linux-x86_64` (musl static) | GitLab Generic Package Registry | `v*.*.*` tag |
| `barad-dur-windows-x86_64.exe` (GNU) | GitLab Generic Package Registry | `v*.*.*` tag |
| Docker image `registry.lab.frogg.it/…/barad-dur` | GitLab Container Registry | `main` push + `v*.*.*` tag |

## Existing Infrastructure

| Component | State | Notes |
|---|---|---|
| `.gitlab-ci.yml` | Exists, in production | 10 stages, 15+ jobs |
| GitLab Generic Package Registry | In use | Upload via `curl --upload-file` + `JOB-TOKEN` |
| GitLab Container Registry | In use | Docker-in-Docker with `docker:27` |
| GitLab Pages | In use | Self-analysis HTML report |
| GitLab SAST template | Included | `Security/SAST.gitlab-ci.yml` |
| GitLab Secret Detection | Included | `Security/Secret-Detection.gitlab-ci.yml` |
| cargo-audit | In use | CVE scanning, `allow_failure: true` |
| cargo-deny | In use | License/supply-chain policy, `allow_failure: true` |
| cargo-tarpaulin | In use | Cobertura coverage report |
| cargo-mutants | In use | Mutation testing, schedule-only |

## Component Map

```
┌─────────────────────────────────────────────────────────┐
│                   GitLab CI Runners                     │
│                                                         │
│  Push to main ──► lint → build → test → analysis       │
│                                        │                │
│                                        ├─► deploy       │
│                                        │   (pages)      │
│                                        │                │
│                                        └─► docker       │
│                                            (registry)   │
│                                                         │
│  Tag v*.*.*  ──► lint → build → test → release         │
│                                        │                │
│                                        ├─► docker       │
│                                        │   (tagged)     │
│                                        │                │
│                                        └─► api          │
│                                                         │
│  Schedule    ──► mutation (full-codebase nightly)       │
│                                                         │
│  MR push     ──► lint → build → test → mutation        │
│                                        (per-feature,    │
│                                         diff-scoped)    │
└─────────────────────────────────────────────────────────┘
```

## What Was Added (release-pipeline feature)

1. **Docker image scan job** — `trivy` scan of the built image for CVEs after push to registry. Commit `faf26d0`.
2. **Per-feature mutation job** — `mutation` job triggered on MR/push to main (not schedule), scoped to files changed in the current commit range, gated at ≥80% kill rate. Commit `4b72f97`.
3. **Nightly full-codebase mutation job** — the existing `mutation` job retargeted to full codebase on schedule (no diff filter). Commit `4b72f97`.

## Deferred Items

The following were identified in the audit but not implemented in this feature:

- **Binary smoke test**: Validates musl static binary executes correctly before `release-publish`. Design spec exists in ci-cd-pipeline.md.
- **JUnit XML from `cargo test`**: Structured test report for GitLab MR UI. Requires cargo-nextest migration.
- **Release checklist validation**: `semver-check` and `cargo-deny` promoted to blocking on tag pipelines.

## Rejected Alternatives

- **GitHub Actions migration**: No — existing GitLab CI is functional and the repo is on self-hosted GitLab. Migration cost is not justified.
- **Kubernetes-based runner fleet**: No — single-maintainer project, shared runners or a single dedicated runner is sufficient.
- **Multi-platform Docker builds (ARM)**: Deferred — no evidence of ARM user demand. Add when requested.
