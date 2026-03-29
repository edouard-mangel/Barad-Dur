# Wave Decisions — release-pipeline

## Decision Summary

### D1 — Deployment Target
**Decision**: N/A — distribution only.
**Rationale**: barad-dur is a CLI tool. There is no server to deploy to. The pipeline's job is to build, validate, and distribute artifacts to GitLab Package Registry and Container Registry.

### D2 — Container Orchestration
**Decision**: Docker image build/push only. No orchestration.
**Rationale**: The Docker image is a distribution artifact for users who prefer containers. There is no runtime infrastructure to orchestrate. Docker-in-Docker via `docker:27` image is sufficient.

### D3 — CI/CD Platform
**Decision**: GitLab CI on self-hosted instance at `lab.frogg.it`.
**Rationale**: Repository is on self-hosted GitLab. Migrating to another CI platform adds operational cost with no benefit. All required features (Package Registry, Container Registry, Pages, SAST, Secret Detection) are available in the existing GitLab instance.

### D4 — Existing Infrastructure Reuse
**Decision**: Extend existing `.gitlab-ci.yml`. No new CI infrastructure.
**Alternatives rejected**:
- GitHub Actions: Repository is on GitLab — migration unjustified.
- External CI (CircleCI, Buildkite): Additional cost and maintenance for a self-hosted GitLab project.

### D5 — Observability
**Decision**: GitLab CI built-in observability only.
**Components**: Cobertura coverage widget, JUnit test reports (proposed), pipeline job duration tracking, email failure notifications, self-analysis HTML report on GitLab Pages.
**Rationale**: No server = no runtime observability needed. Build pipeline health is the only operational concern. All required signals are available without external tooling.

### D6 — Deployment Strategy
**Decision**: N/A — not applicable.
**Rationale**: Binary distribution has no "deployment" in the traditional sense. Users download and run the binary. Docker image is tagged immutably. There is no canary/blue-green/rolling concept.

### D7 — Continuous Learning / Feature Flags
**Decision**: N/A.
**Rationale**: CLI tool with no telemetry, no server-side feature flags. User adoption is invisible by design.

### D8 — Branching Strategy
**Decision**: Trunk-Based Development. Single `main` branch. Short-lived feature branches merged via MR. Tags drive releases.
**Alternatives rejected**:
- GitFlow: Over-engineered for a single-maintainer project. No parallel release lines needed.
- GitHub Flow with release branches: Release branches add coordination overhead not justified for this project size.

### D9 — Mutation Testing Strategy
**Decision**: Hybrid — per-feature on push/MR (diff-scoped, blocking), full-codebase nightly (scheduled, non-blocking until baseline established).
**Rationale**: Per-feature gives fast feedback (5–15 min) scoped to changed code, catching regressions before merge. Nightly full-codebase run catches cumulative gaps not visible in individual diffs — tests that individually pass the 80% gate but collectively leave core logic uncovered.
**Kill rate gate**: ≥ 80% for both modes.
**Tool**: `cargo-mutants` with `--in-diff` for per-feature, no filter for nightly.

## Pipeline Gaps Identified and Recommended Fixes

| Gap | Severity | Recommended Fix |
|---|---|---|
| No JUnit XML test reports | Medium | Add cargo-nextest with CI profile |
| Binary not tested after build (release) | High | Add `binary-smoke-test` job |
| Docker image not scanned for CVEs | Medium | Add `trivy-scan` job |
| `audit`/`deny`/`semver-check` non-blocking on tag pipelines | High | Set `allow_failure: false` for tag `rules:` |
| Mutation gate is `allow_failure: true` and schedule-only | High | Add per-feature job; fix nightly scope |
| Coverage not enforced (no `--fail-under`) | Medium | Add `--fail-under 80` to tarpaulin |
| `release-linux` uses `rust:1.85`, not `rust:1.94` | Low | Align to current toolchain version |
| No `--jobs` parallelism in nightly mutation | Medium | Add `--jobs 4` for nightly full-codebase run |

## Quality Gate Status

| Gate | Status |
|---|---|
| Lint (fmt + clippy) | Passing, blocking |
| Build | Passing, blocking |
| Unit + integration tests | Passing, blocking |
| SAST | Active (GitLab template) |
| Secret detection | Active (GitLab template) |
| CVE audit | Active, non-blocking |
| License/supply chain (deny) | Active, non-blocking |
| Coverage enforcement | Partial — reports but does not block |
| Per-feature mutation gate | Not yet implemented |
| Binary smoke test | Not yet implemented |
| Docker CVE scan | Not yet implemented |
| Semver compatibility | Active, non-blocking |
