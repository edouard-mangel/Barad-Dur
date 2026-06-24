# CI/CD Pipeline Design — release-pipeline

## Stage Breakdown

```
lint → build → test → analysis → secret-detection → deploy → release → docker → api
```

### lint
| Job | Responsibility | Trigger |
|---|---|---|
| `fmt-check` | `cargo fmt --check` — formatting gate | all pipelines |
| `clippy` | `cargo clippy -- -D warnings` — static analysis gate | all pipelines |

Both jobs must pass before `build` proceeds. No `needs:` overrides — sequential by stage.

### build
| Job | Responsibility | Trigger |
|---|---|---|
| `build` | `cargo build --release` (glibc, CI validation binary) | all pipelines |

Cache policy: `pull-push`. All downstream jobs use `pull` only. This is the sole cache writer, preventing the 85K-file upload bottleneck on parallel jobs.

Artifact: `target/release/barad-dur`, expires 1 week.

### test
| Job | Responsibility | Trigger |
|---|---|---|
| `test` | lib + collector + integration tests | all pipelines |
| `coverage` | tarpaulin LLVM, Cobertura XML → GitLab coverage widget | all pipelines |
| `sast` | GitLab SAST template | all pipelines |

**Gap — JUnit XML missing.** `cargo test` does not emit JUnit XML by default. Add `cargo-junit` or `cargo nextest` with `--profile ci` to produce `junit.xml`, then surface via `artifacts.reports.junit`. This enables per-test pass/fail in the MR diff view.

### analysis
| Job | Responsibility | Trigger |
|---|---|---|
| `audit` | `cargo audit` — CVE scan | all pipelines |
| `deny` | `cargo deny check` — license + supply chain | all pipelines |
| `semver-check` | API compatibility check | all pipelines |
| `mutation` (per-feature) | diff-scoped mutation, ≥80% kill rate gate | MR push + push to main |
| `mutation-nightly` | full-codebase mutation, ≥80% kill rate gate | schedule only |
| `self-analysis` | run barad-dur on itself, produce JSON+HTML | all pipelines (needs: build) |

**Gap — `audit`, `deny`, `semver-check` are `allow_failure: true` on all pipelines.** On tag pipelines this is a release quality risk. Recommendation: add `rules:` to these jobs to set `allow_failure: false` when `$CI_COMMIT_TAG` matches `v*.*.*`.

### secret-detection
| Job | Responsibility | Trigger |
|---|---|---|
| `secret_detection` | GitLab Secret Detection template | all pipelines |

### deploy
| Job | Responsibility | Trigger |
|---|---|---|
| `pages` | Publish self-analysis HTML to GitLab Pages | `main` branch only |

### release
| Job | Responsibility | Trigger |
|---|---|---|
| `release-linux` | Build musl static binary | `v*.*.*` tag |
| `release-windows` | Cross-compile Windows GNU binary | `v*.*.*` tag |
| `binary-smoke-test` (new) | Verify Linux binary runs and exits 0 | `v*.*.*` tag, needs: release-linux |
| `release-publish` | Upload to Package Registry + create GitLab Release | `v*.*.*` tag, needs: release-linux + release-windows + binary-smoke-test |

`release-linux` and `release-windows` run in parallel (no `needs:` between them). `binary-smoke-test` unblocks `release-publish` only after the Linux binary is validated.

### docker
| Job | Responsibility | Trigger |
|---|---|---|
| `docker` | Build image, push to Container Registry | `main` push + `v*.*.*` tag |
| `trivy-scan` (new) | Scan built image for CVEs, fail on CRITICAL | `main` push + `v*.*.*` tag, needs: docker |

### api
| Job | Responsibility | Trigger |
|---|---|---|
| `analyze-api` | Clone external repo, run analysis, produce report | `trigger` source with `$REPO_URL` |

## Trigger Rules Summary

| Event | Stages active |
|---|---|
| Push to `main` | lint, build, test, analysis (no mutation-nightly), secret-detection, deploy, docker |
| MR push | lint, build, test, analysis (with per-feature mutation), secret-detection |
| Tag `v*.*.*` | lint, build, test, analysis, secret-detection, release, docker |
| Schedule (daily) | mutation-nightly only |
| Pipeline trigger (`$REPO_URL`) | api only |

## Cache Strategy

| Job | Policy | Key |
|---|---|---|
| `build` | pull-push | `rust-${CI_COMMIT_REF_SLUG}` |
| `fmt-check`, `clippy`, `test`, `coverage`, `audit`, `deny`, `semver-check`, `self-analysis`, `mutation` | pull | `rust-${CI_COMMIT_REF_SLUG}` |
| `release-linux` | pull-push | `release-linux-${CI_COMMIT_TAG}` |
| `release-windows` | pull-push | `release-windows-${CI_COMMIT_TAG}` |
| `self-analysis` | pull (analysis cache) | `barad-dur-analysis` (blame_cache + trends) |

The `barad-dur-analysis` key is unique — it persists blame cache and trends across pipeline runs to avoid re-computation.

## Artifact Retention Policy

| Artifact | Retention | Rationale |
|---|---|---|
| `target/release/barad-dur` (build) | 1 week | CI validation only, not for distribution |
| Coverage XML (cobertura) | 1 week | GitLab coverage widget |
| `mutants.out/` | 1 week | Review missed mutants, not long-term |
| `barad-dur-report.json/html` (self-analysis) | 1 month | Historical trend review |
| `dist/barad-dur-linux-x86_64` | never | Release artifact — permanent |
| `dist/barad-dur-windows-x86_64.exe` | never | Release artifact — permanent |
| `barad-dur-report.html` (api) | 1 week | External analysis ephemeral |

## Parallelism and Dependency Graph

```
fmt-check ─┐
           ├─► build ─► test (parallel: test, coverage, sast)
clippy ────┘         └─► analysis (parallel: audit, deny, semver-check,
                                   mutation, self-analysis)
                              └─► deploy (pages)
                              └─► release (parallel: release-linux, release-windows)
                                    └─► binary-smoke-test
                                          └─► release-publish
                              └─► docker
                                    └─► trivy-scan
```

`test` and `coverage` are independent and can run in parallel within the test stage (no `needs:` between them, separated only by stage ordering unless explicit `needs:` is added to enable cross-stage parallelism).

## New Jobs — Design Spec

### binary-smoke-test
```yaml
binary-smoke-test:
  stage: release
  image: alpine:3.21
  needs:
    - job: release-linux
      artifacts: true
  rules:
    - if: $CI_COMMIT_TAG =~ /^v\d+\.\d+\.\d+$/
  script:
    - chmod +x dist/barad-dur-linux-x86_64
    - dist/barad-dur-linux-x86_64 --version
    - dist/barad-dur-linux-x86_64 --help
    - dist/barad-dur-linux-x86_64 analyze . 2>&1 | grep -q "Score"
  # No artifacts — validation only
```

Uses `alpine` (not full Rust image) to validate the static musl binary runs on a minimal libc-less environment — exactly the distribution target.

### trivy-scan
```yaml
trivy-scan:
  stage: docker
  image: aquasec/trivy:latest
  needs:
    - docker
  rules:
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
    - if: $CI_COMMIT_TAG =~ /^v\d+\.\d+\.\d+$/
  script:
    - trivy image --exit-code 1 --severity CRITICAL ${CI_REGISTRY_IMAGE}:${CI_COMMIT_REF_SLUG}
  allow_failure: true  # Remove once baseline CVE count is known
```
