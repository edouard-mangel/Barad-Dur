# Wave Decisions: gitlab-pipeline-api -- DESIGN

## Decision Log

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| D-01 | Architecture approach | GitLab CI Pipeline Trigger API | Zero code changes, zero new infrastructure, best time-to-market. See ADR-007. |
| D-02 | New CI stage name | `api` | Semantically distinct from existing stages; contains only trigger-activated jobs |
| D-03 | Job activation rule | `$CI_PIPELINE_SOURCE == "trigger" && $REPO_URL` | Ensures analyze-api runs only on triggers with a target; never on push/MR/schedule |
| D-04 | Artifact retention | 1 month (`expire_in: 1 month`) | Matches existing `self-analysis` job retention; balances storage quota with usefulness |
| D-05 | Artifact availability | `when: always` | Report must be downloadable even when gate fails (FR-04 requirement) |
| D-06 | Caller template location | `ci/trigger-template.yml` | Conventional GitLab CI template path; includable via `include:project:` |
| D-07 | Shell injection mitigation | Controlled quoting, no eval | ANALYSIS_OPTIONS is user-supplied; must never be evaluated as shell code |
| D-08 | Default timeout | 30 minutes | Covers repos up to ~50K commits; overridable via template extension |
| D-09 | Polling interval | 15 seconds | Respectful of API rate limits while providing reasonable responsiveness |
| D-10 | Concurrency default | Parallel (no resource_group) | Simpler default; resource_group documented as opt-in for constrained environments |
| D-11 | Development paradigm | No change needed | Functional paradigm (Rust) already set in CLAUDE.md; this feature has no Rust code |
| D-12 | C4 depth | L1 + L2 only | Feature has 3 components (job, template, docs); no subsystem warrants L3 |

## Architecture Artifacts Produced

| Artifact | Location |
|----------|----------|
| Architecture design | `docs/feature/gitlab-pipeline-api/design/architecture-design.md` |
| Technology stack | `docs/feature/gitlab-pipeline-api/design/technology-stack.md` |
| Component boundaries | `docs/feature/gitlab-pipeline-api/design/component-boundaries.md` |
| Data models | `docs/feature/gitlab-pipeline-api/design/data-models.md` |
| ADR-007 | `docs/adrs/ADR-007-ci-trigger-over-http-server.md` |
| Wave decisions | This file |

## Quality Gates Checklist

- [x] Requirements traced to components (component-boundaries.md traceability table)
- [x] Component boundaries with clear responsibilities (3 components defined)
- [x] Technology choices in ADR with alternatives (ADR-007: 4 alternatives evaluated)
- [x] Quality attributes addressed (architecture-design.md quality attribute strategies)
- [x] Dependency-inversion compliance (N/A -- no application code; CI jobs are naturally isolated)
- [x] C4 diagrams L1+L2 in Mermaid (architecture-design.md)
- [x] Integration patterns specified (trigger -> poll -> download; architecture-design.md)
- [x] OSS preference validated (all GitLab CE features; no proprietary)
- [x] AC behavioral, not implementation-coupled (inherited from DISCUSS wave)
- [x] External integrations annotated with contract test recommendation (architecture-design.md)
- [x] Architectural enforcement tooling recommended (technology-stack.md)
- [x] Peer review: Deferred -- skills not loaded (nw-sa-critique-dimensions SKILL.md not found)

## Changed Assumptions

- **Output format**: DISCUSS assumed JSON artifact (`barad-dur-report.json`). Changed to **HTML report** (`barad-dur-report.html`) per user feedback. The HTML report is a self-contained interactive single-file report (D3 visualizations, 5 tabs, dark theme, works offline). This is the primary deliverable — users get a rich visual report, not raw JSON.
- **Target audience**: DISCUSS assumed internal DevOps teams on Froggit. Broadened to **anyone with a public git repository** — the pipeline API is a public service where external users can trigger analysis on their public repos and download the HTML report.

## Handoff Notes for Acceptance Designer (DISTILL Wave)

1. **No Rust code** -- all acceptance tests should target CI pipeline behavior (trigger, artifact presence, exit codes)
2. **Key testable boundaries**: trigger API response (201), HTML artifact download (valid HTML file), error messages in job log, gate pass/fail
3. **External integrations**: GitLab Trigger API, Artifacts API, Pipeline Status API -- smoke test recommended
4. **Three releases**: R1 (US-01 + US-02), R2 (US-03 through US-07), R3 (US-08 + US-09)
5. **Security-sensitive**: ANALYSIS_OPTIONS shell injection prevention must be verified
6. **Public repo focus**: Only public HTTPS repos are supported (no SSH, no auth tokens for private repos)
