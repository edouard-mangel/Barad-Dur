<!-- markdownlint-disable MD024 -->

# User Stories: gitlab-pipeline-api

## US-01: Analyze-API Job Accepts Repo URL and Produces JSON Artifact

### Problem

Fatima Benali is a DevOps engineer at a fintech company hosting 30+ repositories
on Froggit. She finds it tedious to manually run barad-dur on each repo because
she has to clone barad-dur locally, build it, then run it per-repo. She wants to
call it as a service from her existing CI pipelines.

### Who

- DevOps engineer | Configuring nightly quality pipeline | Wants automated health reports without local tooling

### Solution

A new CI job "analyze-api" in barad-dur's `.gitlab-ci.yml` that runs only on
pipeline triggers, accepts `REPO_URL` as a variable, runs analysis inside the
existing Docker image, and saves the JSON report as a pipeline artifact.

### Domain Examples

#### 1: Happy Path -- Fatima triggers analysis of her team's main service

Fatima's nightly pipeline triggers the barad-dur project (ID 4217) with
`REPO_URL=https://froggit.example.com/fintech/payment-gateway.git`. The
analyze-api job starts, clones payment-gateway into `/tmp`, runs
`barad-dur analyze /tmp/payment-gateway --json --pretty`, and saves the output
as `barad-dur-report.json`. The job exits 0. The artifact is downloadable.

#### 2: Edge Case -- Repo with no commits in the last 6 months

Fatima triggers analysis of `https://froggit.example.com/fintech/legacy-auth.git`.
The repository has no commits in the default 6-month window. barad-dur produces
a report with a warning ("No commits found in the specified time window") and
zero-value metrics. The artifact is still produced. The job exits 0.

#### 3: Error -- Invalid repository URL

Fatima's pipeline triggers with
`REPO_URL=https://froggit.example.com/nonexistent/repo.git`. The clone fails.
The job log shows: `Error: Failed to clone: repository not found`. The job exits
with code 1. No artifact is produced.

### UAT Scenarios (BDD)

#### Scenario: Successful analysis produces JSON artifact

```gherkin
Given the barad-dur project has an "analyze-api" job configured for pipeline triggers
And the Docker image "registry.froggit.example.com/devops/barad-dur:latest" is available
When a pipeline is triggered with REPO_URL "https://froggit.example.com/fintech/payment-gateway.git"
Then the "analyze-api" job clones the repository
And runs "barad-dur analyze" with --json --pretty flags
And produces a "barad-dur-report.json" artifact
And the artifact contains valid JSON with "overall_score" as a number between 0 and 100
```

#### Scenario: Empty time window still produces artifact

```gherkin
Given the target repository "legacy-auth" has no commits in the last 6 months
When the "analyze-api" job runs against that repository
Then the job completes with exit code 0
And the artifact contains a JSON report with a warning about empty time window
```

#### Scenario: Invalid repo URL causes job failure

```gherkin
Given REPO_URL is set to "https://froggit.example.com/nonexistent/repo.git"
When the "analyze-api" job attempts to clone the repository
Then the clone fails with "repository not found"
And the job exits with a non-zero exit code
And no "barad-dur-report.json" artifact is produced
```

#### Scenario: Missing REPO_URL variable

```gherkin
Given a pipeline is triggered without the REPO_URL variable
When the "analyze-api" job starts
Then the job fails immediately with message "REPO_URL is required"
And the job exits with a non-zero exit code
```

### Acceptance Criteria

- [ ] analyze-api job exists in .gitlab-ci.yml and runs only on pipeline triggers
- [ ] Job accepts REPO_URL as a required trigger variable
- [ ] Job uses the existing Docker image from the Froggit container registry
- [ ] Job produces barad-dur-report.json as a pipeline artifact (expire_in: 1 month)
- [ ] Job fails with clear error message when REPO_URL is missing or invalid
- [ ] Job succeeds (exit 0) even when target repo has no recent commits

### Outcome KPIs

- **Who**: DevOps engineers on Froggit
- **Does what**: Trigger barad-dur analysis via pipeline API instead of local CLI
- **By how much**: 100% of triggered analyses produce a downloadable artifact (when repo is valid)
- **Measured by**: Pipeline success rate for analyze-api jobs
- **Baseline**: No pipeline trigger capability exists today

### Technical Notes

- Job rule: `rules: - if: $CI_PIPELINE_SOURCE == "trigger" && $REPO_URL`
- Image: `$CI_REGISTRY_IMAGE:latest`
- The Docker image already includes git + barad-dur + SSL certs
- `/tmp` is writable in the scratch-based image (needed for temp clone)
- GIT_DEPTH: 0 must be set for full history analysis

---

## US-02: Caller Pipeline Triggers and Downloads Report

### Problem

Fatima Benali has configured the analyze-api job (US-01), but she does not know
the exact curl commands and GitLab API calls needed to trigger it from her own
pipeline, wait for completion, and download the artifact. She needs a working
example she can copy into her `.gitlab-ci.yml`.

### Who

- DevOps engineer | Writing the calling pipeline | Wants a copy-paste example that works

### Solution

A documented, tested example of a caller pipeline job that: triggers the
barad-dur pipeline, polls for completion, downloads the artifact, and parses
the overall score.

### Domain Examples

#### 1: Happy Path -- Fatima copies the example into her nightly pipeline

Fatima adds the example job to `fintech/payment-gateway/.gitlab-ci.yml`. The job
uses curl to POST to the trigger API with her masked token. It receives pipeline
ID 58432 in the response. It polls every 15 seconds until status is "success".
It downloads `barad-dur-report.json` and extracts `overall_score: 74`. It prints
"PASS: score 74 >= threshold 60".

#### 2: Edge Case -- Triggered pipeline fails

Fatima triggers analysis of a repo with a broken URL. The triggered pipeline
fails. Her polling loop detects `status: "failed"`. The job prints
"ERROR: barad-dur analysis failed. Check pipeline #58433 logs at <URL>."
and exits with code 1.

#### 3: Error -- Trigger API returns 401

Fatima's trigger token expired. The curl POST returns HTTP 401. Her job
detects the non-201 response and prints: "ERROR: Trigger failed (HTTP 401).
Check BARAD_DUR_TRIGGER_TOKEN." and exits with code 1.

### UAT Scenarios (BDD)

#### Scenario: Caller triggers, polls, and downloads report

```gherkin
Given Fatima has a CI job "trigger-analysis" in her project
And BARAD_DUR_TRIGGER_TOKEN and BARAD_DUR_PROJECT_ID are set as CI variables
When the job triggers the barad-dur pipeline with REPO_URL for her repository
Then it receives a pipeline ID in the trigger response
And it polls the pipeline status until completion
And it downloads "barad-dur-report.json" from the completed job
And it extracts and displays the overall score
```

#### Scenario: Caller handles failed analysis

```gherkin
Given the barad-dur pipeline was triggered but failed (bad REPO_URL)
When the caller's polling detects status "failed"
Then the caller prints an error with the failed pipeline URL
And the caller job exits with code 1
```

#### Scenario: Caller handles authentication failure

```gherkin
Given BARAD_DUR_TRIGGER_TOKEN contains an expired or invalid token
When the caller POSTs to the trigger API
Then the API responds with HTTP 401
And the caller prints "Trigger failed (HTTP 401). Check BARAD_DUR_TRIGGER_TOKEN."
And the caller job exits with code 1
```

### Acceptance Criteria

- [ ] Example caller job is documented with curl commands for trigger, poll, download
- [ ] Example handles success, failure, and auth error cases
- [ ] Example uses masked CI variables (never exposes tokens in logs)
- [ ] Example extracts overall_score from the downloaded JSON
- [ ] Example is tested end-to-end on Froggit

### Outcome KPIs

- **Who**: DevOps engineers adopting the pipeline API
- **Does what**: Successfully integrate barad-dur trigger into their pipelines on first attempt
- **By how much**: First-attempt success rate >= 80%
- **Measured by**: Support requests / questions about setup
- **Baseline**: No example exists today; setup requires reading GitLab API docs

### Technical Notes

- Caller only needs curl and jq (available in most CI images)
- Polling interval: 15 seconds (respectful of API rate limits)
- Maximum poll duration: configurable, default 30 minutes
- Artifact download uses CI_JOB_TOKEN for same-instance authentication

---

## US-03: Options Pass-Through and Score Gate

### Problem

Romain Dupont is a team lead who wants to run barad-dur with `--skip-blame` on
his large monorepo (analysis takes 45 minutes with blame, 8 minutes without).
He also wants his nightly pipeline to fail if the health score drops below 70,
enforcing a quality gate.

### Who

- Team lead | Running nightly quality gate | Wants fast analysis with enforced thresholds

### Solution

The analyze-api job accepts `ANALYSIS_OPTIONS` (string of CLI flags) and
`MIN_SCORE` (integer threshold). Options are passed through to the barad-dur
command. If MIN_SCORE is set, the job runs `barad-dur gate` after analysis.

### Domain Examples

#### 1: Happy Path -- Romain uses skip-blame and passes the gate

Romain triggers with `ANALYSIS_OPTIONS=--skip-blame` and `MIN_SCORE=70`. The
analysis runs in 8 minutes. The score is 78. The gate passes. The artifact is
produced. Exit code 0.

#### 2: Edge Case -- Score exactly at threshold

Romain triggers with `MIN_SCORE=70`. The score is exactly 70. The gate passes
(>=, not >). Exit code 0.

#### 3: Error -- Score below threshold

Romain triggers with `MIN_SCORE=70`. His team merged a large untested PR. The
score drops to 58. The gate fails. The artifact is still produced (Romain needs
it to diagnose what dropped). Exit code 1. Job log shows:
"FAIL: overall score 58 < threshold 70".

### UAT Scenarios (BDD)

#### Scenario: Analysis options are passed through to CLI

```gherkin
Given the analyze-api job is triggered with ANALYSIS_OPTIONS "--skip-blame --since 3months"
When the job constructs the barad-dur command
Then it runs "barad-dur analyze $REPO_URL --json --pretty --skip-blame --since 3months"
And the analysis completes without blame computation
```

#### Scenario: Score gate passes

```gherkin
Given the analyze-api job is triggered with MIN_SCORE "70"
And the analysis produces an overall score of 78
When the gate check runs
Then the job log shows "PASS: overall score 78 >= threshold 70"
And the job exits with code 0
```

#### Scenario: Score gate fails but artifact is preserved

```gherkin
Given the analyze-api job is triggered with MIN_SCORE "70"
And the analysis produces an overall score of 58
When the gate check runs
Then the job log shows "FAIL: overall score 58 < threshold 70"
And the "barad-dur-report.json" artifact is still available
And the job exits with code 1
```

### Acceptance Criteria

- [ ] ANALYSIS_OPTIONS variable is passed through to barad-dur analyze command
- [ ] Dangerous shell characters in ANALYSIS_OPTIONS are not executed (no injection)
- [ ] MIN_SCORE triggers a gate check after analysis completes
- [ ] Gate uses >= comparison (score at threshold passes)
- [ ] Artifact is produced regardless of gate result (artifacts:when: always)
- [ ] Invalid MIN_SCORE (non-integer, negative) is rejected with clear error

### Outcome KPIs

- **Who**: Team leads running quality gates
- **Does what**: Enforce minimum health scores in CI
- **By how much**: Gate check adds < 5 seconds to total job time
- **Measured by**: Gate pass/fail rate across projects
- **Baseline**: No automated gate enforcement via trigger API

### Technical Notes

- Shell injection prevention: ANALYSIS_OPTIONS must be quoted properly
- Artifact `when: always` ensures report is saved even on gate failure
- MIN_SCORE validation: check it is a positive integer before use

---

## US-04: Reusable Caller Pipeline Template

### Problem

After Fatima set up the trigger in her pipeline, three other teams asked her for
the same configuration. She ended up copy-pasting and adjusting the curl commands
each time. She wants a reusable CI template that teams can include with minimal
configuration.

### Who

- DevOps engineer | Supporting multiple teams | Wants to reduce duplication across projects

### Solution

A `.gitlab-ci.yml` template file (or CI include snippet) in the barad-dur
repository that other projects can include. The template defines a hidden job
(`.barad-dur-analysis`) that teams extend with their specific variables.

### Domain Examples

#### 1: Happy Path -- Team includes template with 5 lines of config

The `platform/api-gateway` team adds to their `.gitlab-ci.yml`:
```yaml
include:
  - project: 'devops/barad-dur'
    file: '/ci/trigger-template.yml'

quality-check:
  extends: .barad-dur-analysis
  variables:
    REPO_URL: "${CI_PROJECT_URL}.git"
```
The job runs nightly, triggers barad-dur, and reports the score.

#### 2: Edge Case -- Team overrides timeout and threshold

The `data/etl-pipeline` team has a large repo. They extend the template with:
```yaml
quality-check:
  extends: .barad-dur-analysis
  variables:
    REPO_URL: "${CI_PROJECT_URL}.git"
    ANALYSIS_OPTIONS: "--skip-blame"
    MIN_SCORE: "65"
  timeout: 20 minutes
```

#### 3: Error -- Team forgets to set CI variables

A new team includes the template but does not create `BARAD_DUR_TRIGGER_TOKEN`
and `BARAD_DUR_PROJECT_ID` as CI variables. The job fails with:
"ERROR: BARAD_DUR_TRIGGER_TOKEN is not set. See setup guide."

### UAT Scenarios (BDD)

#### Scenario: Template include with minimal config

```gherkin
Given a project includes the barad-dur trigger template from "devops/barad-dur"
And the project has BARAD_DUR_TRIGGER_TOKEN and BARAD_DUR_PROJECT_ID as CI variables
When the team extends ".barad-dur-analysis" with their REPO_URL
Then the job triggers barad-dur analysis and retrieves the report
```

#### Scenario: Missing CI variables are caught early

```gherkin
Given a project includes the template but has not set BARAD_DUR_TRIGGER_TOKEN
When the trigger job starts
Then it fails immediately with a message listing the missing variables
And does not attempt to call the trigger API
```

### Acceptance Criteria

- [ ] Template file exists at `ci/trigger-template.yml` in the barad-dur repo
- [ ] Template defines a hidden job `.barad-dur-analysis` with sensible defaults
- [ ] Template validates required CI variables before triggering
- [ ] Template is usable via GitLab CI `include: project:` directive
- [ ] Template supports variable overrides for ANALYSIS_OPTIONS, MIN_SCORE

### Outcome KPIs

- **Who**: Teams adopting barad-dur pipeline API
- **Does what**: Integrate analysis with <= 10 lines of CI config
- **By how much**: Setup time < 15 minutes per team
- **Measured by**: Number of projects using the template include
- **Baseline**: Each team writes ~50 lines of curl-based trigger code

### Technical Notes

- GitLab CI `include: project:` works for same-instance projects
- Template must not contain secrets (token comes from caller's CI variables)

---

## US-05: Setup Documentation

### Problem

Karim Mesbah is a junior DevOps engineer who has never used GitLab pipeline
triggers. He found the barad-dur project and wants to set it up for his team,
but the GitLab trigger API documentation is generic and does not explain the
barad-dur-specific variables and workflow.

### Who

- Junior DevOps engineer | First time using pipeline triggers | Wants step-by-step guide

### Solution

A setup guide in the barad-dur repository (`docs/pipeline-api-setup.md`) with:
step-by-step instructions for creating the trigger token, storing it as a CI
variable, configuring the caller pipeline, and verifying the setup.

### Domain Examples

#### 1: Happy Path -- Karim follows the guide and sets up in 20 minutes

Karim reads the guide. Step 1: navigate to barad-dur project > Settings > CI/CD >
Pipeline triggers > Add trigger. Step 2: copy the token. Step 3: go to his
project > Settings > CI/CD > Variables > Add BARAD_DUR_TRIGGER_TOKEN (masked).
Step 4: add the template include to his .gitlab-ci.yml. Step 5: push and verify
the pipeline runs.

#### 2: Edge Case -- Karim does not have Maintainer access to barad-dur

Karim is a Developer on the barad-dur project. He cannot create trigger tokens.
The guide tells him: "You need Maintainer access to create triggers. Ask your
DevOps lead or project maintainer."

#### 3: Error -- Karim uses the wrong project ID

Karim copies project ID 4218 instead of 4217. His trigger returns 404. The guide
includes a troubleshooting section: "404 on trigger? Verify the project ID at
the top of the barad-dur project page."

### UAT Scenarios (BDD)

#### Scenario: Guide covers end-to-end setup

```gherkin
Given the setup guide exists at "docs/pipeline-api-setup.md"
Then it includes sections for: prerequisites, token creation, variable storage,
  caller configuration, verification, and troubleshooting
And each section has numbered steps with screenshots or example commands
```

#### Scenario: Troubleshooting section covers common errors

```gherkin
Given the troubleshooting section exists
Then it covers: 401 (bad token), 404 (wrong project ID), missing variables,
  clone failures, and timeout issues
And each error has a clear cause and fix
```

### Acceptance Criteria

- [ ] Setup guide exists at docs/pipeline-api-setup.md
- [ ] Guide covers: prerequisites, token creation, variable storage, caller config, verification
- [ ] Guide includes troubleshooting for top 5 error scenarios
- [ ] Guide references the CI template (US-04) as the recommended approach
- [ ] Guide specifies required GitLab permissions (Maintainer for token creation)

### Outcome KPIs

- **Who**: New adopters of the pipeline API
- **Does what**: Complete setup without asking for help
- **By how much**: Self-service setup rate >= 80%
- **Measured by**: Ratio of successful setups to support requests
- **Baseline**: No documentation exists; 100% rely on word-of-mouth

### Technical Notes

- No Froggit-specific screenshots (may change); use text descriptions with exact navigation paths

---

## US-06: Branch Selection Variable

### Problem

Amina Toure runs analysis on feature branches during pull request pipelines, not
just on main. She needs to tell barad-dur which branch to check out after
cloning, because the default (main) misses her in-progress work.

### Who

- Developer | Running analysis on feature branches | Wants branch-specific health reports

### Solution

The analyze-api job accepts a `REPO_BRANCH` trigger variable (default: "main")
and checks out that branch before running analysis.

### Domain Examples

#### 1: Happy Path -- Amina analyzes her feature branch

Amina triggers with `REPO_URL=https://froggit.example.com/fintech/payment-gateway.git`
and `REPO_BRANCH=feature/new-payment-flow`. The job clones the repo and checks
out `feature/new-payment-flow`. Analysis runs on that branch's state.

#### 2: Edge Case -- Branch does not exist

Amina triggers with `REPO_BRANCH=feature/typo-brannch`. The clone succeeds but
checkout fails. Job log: "Error: branch 'feature/typo-brannch' not found."
Exit code 1.

#### 3: Edge Case -- Default branch used when REPO_BRANCH is omitted

A caller triggers with only `REPO_URL`. `REPO_BRANCH` defaults to "main".
The analysis runs on main.

### UAT Scenarios (BDD)

#### Scenario: Analysis on a specific branch

```gherkin
Given the analyze-api job is triggered with REPO_BRANCH "feature/new-payment-flow"
When the job clones the repository
Then it checks out the "feature/new-payment-flow" branch
And the analysis runs on that branch's file state
```

#### Scenario: Nonexistent branch is rejected

```gherkin
Given the analyze-api job is triggered with REPO_BRANCH "feature/typo-brannch"
When the job attempts to check out that branch
Then the checkout fails with "branch not found"
And the job exits with a non-zero exit code
```

#### Scenario: Default branch when REPO_BRANCH is omitted

```gherkin
Given a pipeline is triggered without the REPO_BRANCH variable
When the analyze-api job clones the repository
Then it uses the default branch "main"
```

### Acceptance Criteria

- [ ] REPO_BRANCH variable is accepted with default value "main"
- [ ] Job checks out the specified branch after cloning
- [ ] Nonexistent branch produces a clear error message
- [ ] Default branch works when REPO_BRANCH is not provided

### Outcome KPIs

- **Who**: Developers running branch-level analysis
- **Does what**: Get health reports for feature branches, not just main
- **By how much**: Branch-specific analysis available for 100% of existing branches
- **Measured by**: Percentage of triggers using non-default REPO_BRANCH
- **Baseline**: Only main branch analysis possible

### Technical Notes

- `git clone --branch $REPO_BRANCH` handles this natively
- Must still use `GIT_DEPTH: 0` for full history on the branch

---

## US-07: Category Filter Variable

### Problem

Romain Dupont only cares about Health and Git Hygiene scores for his nightly
gate. Running all 4 categories wastes 3 minutes of CI time. He wants to
specify which categories to compute.

### Who

- Team lead | Optimizing CI runtime | Wants to run only relevant categories

### Solution

The analyze-api job accepts a `CATEGORIES` trigger variable (comma-separated).
If set, only those categories are computed. Mapped to CLI flags like
`--health --hygiene`.

### Domain Examples

#### 1: Happy Path -- Romain runs only Health and Hygiene

Romain triggers with `CATEGORIES=health,hygiene`. The analyze-api job runs
`barad-dur analyze $REPO_URL --json --pretty --health --hygiene`. The report
contains only Health and Git Hygiene scores. Runtime: 5 minutes instead of 8.

#### 2: Edge Case -- Invalid category name

Romain triggers with `CATEGORIES=health,typo`. barad-dur ignores "typo" and
runs only Health. The job log warns: "Unknown category 'typo', skipping."

#### 3: Edge Case -- CATEGORIES is empty

A caller triggers without CATEGORIES. All 4 categories are computed (default).

### UAT Scenarios (BDD)

#### Scenario: Selective category analysis

```gherkin
Given the analyze-api job is triggered with CATEGORIES "health,hygiene"
When the job constructs the barad-dur command
Then it includes "--health --hygiene" flags
And the report contains only Health and Git Hygiene categories
```

#### Scenario: Invalid category name is warned and skipped

```gherkin
Given CATEGORIES is set to "health,typo"
When the job maps categories to CLI flags
Then it passes "--health" to barad-dur
And logs a warning "Unknown category: typo"
```

### Acceptance Criteria

- [ ] CATEGORIES variable accepts comma-separated category names
- [ ] Valid names: health, team, evolution, hygiene
- [ ] Invalid names produce a warning but do not fail the job
- [ ] Empty or missing CATEGORIES runs all categories (default behavior)
- [ ] Category names are case-insensitive

### Outcome KPIs

- **Who**: Teams optimizing CI runtime
- **Does what**: Reduce analysis time by running fewer categories
- **By how much**: 20-40% runtime reduction for 2-category runs
- **Measured by**: Average job duration with vs without category filter
- **Baseline**: All triggers run all 4 categories

### Technical Notes

- Map comma-separated string to CLI flags in shell: `--$cat` for each valid category

---

## US-08: Configurable Job Timeout

### Problem

The `data/etl-pipeline` team has a monorepo with 200,000 commits. The default
CI timeout (1 hour) is sometimes not enough when blame analysis is enabled.
They want to configure the timeout without modifying the barad-dur project.

### Who

- DevOps engineer | Managing large repos | Wants predictable timeout behavior

### Solution

The analyze-api job has a configurable timeout via a `JOB_TIMEOUT` trigger
variable, with a sensible default (30 minutes). The CI template (US-04) allows
overriding `timeout:` in the extending job.

### Domain Examples

#### 1: Happy Path -- Team sets 45-minute timeout

The team extends the template with `timeout: 45 minutes`. Their large repo
analysis completes in 38 minutes. Job succeeds.

#### 2: Edge Case -- Analysis exceeds timeout

A monorepo analysis runs for 46 minutes against a 45-minute timeout. GitLab
kills the job. Status: "failed". No artifact produced.

#### 3: Edge Case -- Default timeout is sufficient

A team does not override timeout. The default 30 minutes is enough for their
5,000-commit repo (analysis takes 4 minutes).

### UAT Scenarios (BDD)

#### Scenario: Custom timeout allows large repo analysis

```gherkin
Given the caller extends the template with timeout "45 minutes"
And the analysis takes 38 minutes
When the job completes
Then the artifact is produced
And the job status is "success"
```

#### Scenario: Default timeout covers typical repos

```gherkin
Given the analyze-api job uses the default timeout of 30 minutes
And the target repository has fewer than 10,000 commits
When the analysis runs
Then it completes well within the timeout
```

### Acceptance Criteria

- [ ] analyze-api job has a default timeout of 30 minutes
- [ ] Timeout is overridable via CI template extension or JOB_TIMEOUT variable
- [ ] Job killed by timeout produces no artifact (GitLab native behavior)
- [ ] Documentation recommends --skip-blame for repos with >50,000 commits

### Outcome KPIs

- **Who**: Teams with large repositories
- **Does what**: Successfully analyze repos that exceed default timeout
- **By how much**: Zero timeout-related failures for repos under 100,000 commits with skip-blame
- **Measured by**: Timeout failure rate
- **Baseline**: No timeout configuration; 1-hour hard limit

### Technical Notes

- GitLab CI `timeout:` is a job-level keyword, overridable in extending jobs
- `--skip-blame` is the primary mitigation for large repos

---

## US-09: Concurrency Safeguards

### Problem

During a company-wide "quality week," 15 teams simultaneously trigger barad-dur
analysis. The CI runners are overloaded, some jobs queue for 20+ minutes, and
the Froggit instance rate-limits API calls. Fatima needs a way to manage
concurrent trigger load.

### Who

- DevOps engineer | Platform-wide adoption | Wants stable performance under load

### Solution

Document concurrency characteristics and provide guidance on resource management:
GitLab CI `resource_group` for sequential execution, runner tag configuration,
and recommendations for staggering triggers.

### Domain Examples

#### 1: Happy Path -- Sequential execution via resource group

Fatima configures the analyze-api job with `resource_group: barad-dur-analysis`.
When 5 triggers arrive simultaneously, they queue and run one at a time. Each
gets the full runner resources. Average wait: 15 minutes. All succeed.

#### 2: Edge Case -- Parallel execution without resource group

Without resource_group, all 15 triggers run in parallel. Each gets a runner
but competes for network bandwidth during clone. Some jobs are slower but all
eventually complete (within timeout).

#### 3: Edge Case -- Runner capacity exhausted

All runners are busy. Triggered pipelines sit in "pending" state. The caller's
polling loop waits. After 10 minutes, a runner becomes available.

### UAT Scenarios (BDD)

#### Scenario: Resource group ensures sequential execution

```gherkin
Given the analyze-api job has resource_group "barad-dur-analysis"
When 3 pipelines are triggered simultaneously
Then only 1 analyze-api job runs at a time
And the others queue in order
And all 3 eventually complete successfully
```

#### Scenario: Parallel triggers complete without interference

```gherkin
Given the analyze-api job has no resource_group
When 3 pipelines are triggered simultaneously for different repositories
Then all 3 jobs run in parallel on separate runners
And each produces its own independent artifact
And no data is shared between jobs
```

### Acceptance Criteria

- [ ] Documentation describes concurrency options (resource_group vs parallel)
- [ ] CI template includes commented resource_group option
- [ ] Each triggered job is fully isolated (no shared cache or state)
- [ ] Documentation recommends staggering for large-scale rollouts (>10 concurrent)

### Outcome KPIs

- **Who**: Platform team managing company-wide adoption
- **Does what**: Support 10+ concurrent analysis triggers without failures
- **By how much**: Zero concurrency-related failures at 10 concurrent triggers
- **Measured by**: Job failure rate during peak concurrent usage
- **Baseline**: Untested; unknown behavior under concurrent load

### Technical Notes

- `resource_group:` is a standard GitLab CI keyword (free tier)
- Each Docker container is isolated; no shared /tmp between jobs
- Runner scaling is a Froggit infrastructure concern, not a barad-dur concern
