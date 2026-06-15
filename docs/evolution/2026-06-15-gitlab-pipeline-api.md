# Evolution: GitLab CI Pipeline Trigger API

**Date**: 2026-06-15
**Feature ID**: gitlab-pipeline-api
**Status**: Delivered

---

## Feature Summary

Exposes barad-dur as a CI-triggered analysis service. Any public git repository can be
analyzed by triggering a pipeline with `REPO_URL` as a variable — no local tooling required.

The mechanism is a new `api` stage in `.gitlab-ci.yml` containing an `analyze-api` job that:
1. Clones the target repository from `REPO_URL`
2. Runs `barad-dur analyze` with configurable options
3. Produces a self-contained `barad-dur-report.html` artifact (interactive, offline-capable)
4. Enforces a quality gate (`MIN_SCORE`, default 70)

Callers integrate via a reusable `ci/trigger-template.yml` template that wraps the GitLab
Pipeline Trigger API (`POST /api/v4/projects/:id/trigger/pipeline`), polls for completion,
and downloads the report artifact.

---

## Business Context

DevOps engineers embedding barad-dur into nightly pipelines previously needed the binary
installed on their runners. This feature removes that friction: a single `include:` reference
to `ci/trigger-template.yml` is sufficient. External users with public repositories can
trigger analysis on demand and receive an interactive HTML health report without any local
setup.

Target persona: Fatima, a DevOps engineer maintaining 12 microservices, who wants automated
code health reports in her team's nightly pipeline without managing a barad-dur installation.

---

## Waves Completed

| Wave | Scope | Artifacts |
|------|-------|-----------|
| DISCUSS | User stories, acceptance criteria, emotional journey, story map | 9 user stories across 3 releases (R1–R3); 44 acceptance criteria; journey-pipeline-trigger.yaml; outcome-kpis.md |
| DESIGN | Architecture (C4 L1+L2), component boundaries, technology stack, data models, ADR-007 | architecture-design.md; component-boundaries.md; technology-stack.md; data-models.md; ADR-007 |
| DISTILL | 44 acceptance scenarios in 3 milestone files; walking skeleton confirmation; test coverage map | walking-skeleton.feature (12); milestone-2-enhanced-api.feature (24); milestone-3-robustness.feature (8); walking-skeleton.md; test-scenarios.md |
| DELIVER | 3 gap fixes: ANALYSIS_OPTIONS pass-through, caller template forwarding, documentation | .gitlab-ci.yml; ci/trigger-template.yml; docs/pipeline-api-setup.md |

---

## Key Decisions

### ADR-007: GitLab CI Trigger API over HTTP Server

The primary architecture decision. Four alternatives were evaluated:

| Alternative | Rejected reason |
|-------------|-----------------|
| Standalone HTTP server | New infrastructure, new security surface, new deployment concern |
| GitLab webhook receiver | Requires inbound network access; complex auth |
| Scheduled CI job polling | No on-demand capability; batch-only |
| **GitLab CI Pipeline Trigger API** | Selected: zero new infrastructure, zero code changes to barad-dur binary, native GitLab auth, no server to maintain |

ADR-007 is at `docs/adrs/ADR-007-ci-trigger-over-http-server.md`.

### Output format: HTML over JSON

DISCUSS wave acceptance criteria (AC-01.4, AC-02.4) referenced `barad-dur-report.json`.
DESIGN wave changed this to `barad-dur-report.html` — the self-contained interactive single-file
report (D3 visualizations, 5 tabs, dark theme, works offline). All downstream artifacts
(DISTILL scenarios, DELIVER implementation) use HTML. The JSON reference is superseded.

### ANALYSIS_OPTIONS injection protection

`ANALYSIS_OPTIONS` is user-supplied via pipeline trigger variables. The implementation uses
validate-then-expand: a shell guard rejects any value containing metacharacters
(`; & | \` $ ( ) < > \`) before the variable reaches the `barad-dur analyze` command.
Shell quoting with `${ANALYSIS_OPTIONS:-}` (not `eval`) ensures the value is always treated
as a data argument, never as a shell command.

### Concurrency default: parallel (no resource_group)

The `analyze-api` job does not declare `resource_group`, making parallel execution the
default. Sequential execution via `resource_group` is documented as an opt-in for
constrained runner environments. Staggering cron schedules (offset by 5–10 minutes per team)
is recommended for company-wide rollouts with more than 10 concurrent triggers.

---

## Steps Completed (DELIVER Wave)

All three steps were completed on 2026-06-15 as phase 01 "Pipeline API Gap Fixes".

### 01-01: ANALYSIS_OPTIONS pass-through + injection guard in .gitlab-ci.yml

- **Completed**: 2026-06-15T18:29:41Z
- **Files modified**: `.gitlab-ci.yml`
- **What**: Added `${ANALYSIS_OPTIONS:-}` to the `barad-dur analyze` command in the
  `analyze-api` job. Added a pre-validate block that rejects values containing shell
  metacharacters with a clear error message. Enabled scenario S-03.2 (`@security`).
- **TDD**: RED (acceptance + unit) skipped — CI YAML has no BDD or unit test runtime;
  structural YAML validation serves as the test.

### 01-02: ANALYSIS_OPTIONS forwarding in ci/trigger-template.yml

- **Completed**: 2026-06-15T18:31:20Z
- **Files modified**: `ci/trigger-template.yml`
- **What**: Added conditional forwarding of `ANALYSIS_OPTIONS` to the trigger body,
  following the established pattern for `REPO_BRANCH`, `MIN_SCORE`, and `CATEGORIES`.
  Enabled the caller-side scenario in milestone-2-enhanced-api.feature.
- **TDD**: RED skipped — CI YAML, no BDD runtime.

### 01-03: Documentation gaps in docs/pipeline-api-setup.md

- **Completed**: 2026-06-15T18:31:50Z
- **Files modified**: `docs/pipeline-api-setup.md`
- **What**: Added three sections:
  1. Performance Tips: `--skip-blame` recommendation for repositories with more than 50,000 commits
  2. Concurrency: parallel vs `resource_group` trade-off explanation with decision guidance
  3. Concurrency: staggering schedule recommendation for more than 10 concurrent triggers
- **TDD**: RED skipped — documentation, no BDD runtime.

---

## Upstream Issues Resolved

| ID | Issue | Resolution |
|----|-------|------------|
| UI-01 | DISCUSS referenced JSON artifact (`barad-dur-report.json`); DESIGN changed to HTML | All artifacts corrected to `barad-dur-report.html`. DISCUSS acceptance criteria AC-01.4/AC-02.4 are superseded. |
| UI-02 | Walking skeleton (US-01 + US-02) was fully implemented before DISTILL ran | DISTILL confirmed implementation at `.gitlab-ci.yml` lines 596–661 and `ci/trigger-template.yml`. DELIVER scoped to 3 gaps only. |

---

## ADR-007 Status

`docs/adrs/ADR-007-ci-trigger-over-http-server.md` exists (verified during finalization).

---

## Migrated Artifacts

Architecture documents, acceptance scenarios, and UX artifacts were migrated from the
feature workspace to permanent locations:

| Source | Destination |
|--------|-------------|
| `docs/feature/gitlab-pipeline-api/design/architecture-design.md` | `docs/architecture/gitlab-pipeline-api/architecture-design.md` |
| `docs/feature/gitlab-pipeline-api/design/component-boundaries.md` | `docs/architecture/gitlab-pipeline-api/component-boundaries.md` |
| `docs/feature/gitlab-pipeline-api/design/technology-stack.md` | `docs/architecture/gitlab-pipeline-api/technology-stack.md` |
| `docs/feature/gitlab-pipeline-api/design/data-models.md` | `docs/architecture/gitlab-pipeline-api/data-models.md` |
| `docs/feature/gitlab-pipeline-api/distill/test-scenarios.md` | `docs/scenarios/gitlab-pipeline-api/test-scenarios.md` |
| `docs/feature/gitlab-pipeline-api/distill/walking-skeleton.md` | `docs/scenarios/gitlab-pipeline-api/walking-skeleton.md` |
| `docs/feature/gitlab-pipeline-api/discuss/journey-pipeline-trigger.yaml` | `docs/ux/gitlab-pipeline-api/journey-pipeline-trigger.yaml` |
| `docs/feature/gitlab-pipeline-api/discuss/journey-pipeline-trigger-visual.md` | `docs/ux/gitlab-pipeline-api/journey-pipeline-trigger-visual.md` |
