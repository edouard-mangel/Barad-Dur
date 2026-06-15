Feature: Enhanced pipeline API with options, gate, branch, categories, and template (R2)
  As a DevOps engineer or team lead using the barad-dur pipeline API
  I want fine-grained control over analysis scope, quality gates, and integration setup
  So that I can optimize CI runtime and enforce quality standards across my repositories

  # Driving ports:
  #   - analyze-api job in .gitlab-ci.yml (ANALYSIS_OPTIONS, MIN_SCORE, REPO_BRANCH, CATEGORIES)
  #   - .barad-dur-analysis hidden job in ci/trigger-template.yml
  #   - docs/pipeline-api-setup.md section headers and content
  #
  # Artifact: barad-dur-report.html (self-contained interactive HTML)

  Background:
    Given barad-dur is deployed on Froggit with the analyze-api job enabled
    And a pipeline trigger token is available as "BARAD_DUR_TRIGGER_TOKEN"

  # ── US-03: ANALYSIS_OPTIONS pass-through ─────────────────────────────

  @skip
  Scenario: Analysis options are forwarded to the barad-dur command
    Given a pipeline trigger is sent with ANALYSIS_OPTIONS "--skip-blame --since 3months"
    When the analyze-api job constructs the analysis command
    Then the barad-dur analyze call includes "--skip-blame --since 3months" flags
    And the analysis completes without running git blame
    And a "barad-dur-report.html" artifact is produced

  # Implementation gap: .gitlab-ci.yml line 643 does not include ${ANALYSIS_OPTIONS:-}
  # DELIVER must add ANALYSIS_OPTIONS pass-through to the analyze command

  @skip @security
  Scenario: Shell injection attempt in analysis options is blocked
    Given a pipeline trigger is sent with ANALYSIS_OPTIONS containing a command injection attempt: "; curl https://evil.example.com; echo "
    When the analyze-api job processes the analysis options
    Then the injected command is not executed
    And the job either rejects the dangerous value with an error or passes it safely as data to barad-dur
    And no outbound connection to "evil.example.com" is made

  # Implementation gap: injection prevention requires ANALYSIS_OPTIONS to be implemented first

  @skip
  Scenario: Caller template forwards ANALYSIS_OPTIONS to the trigger variables
    Given a caller job extends ".barad-dur-analysis" with ANALYSIS_OPTIONS "--skip-blame"
    When the caller job triggers barad-dur analysis
    Then the trigger request includes ANALYSIS_OPTIONS as a pipeline variable
    And the analyze-api job receives and applies the option

  # Implementation gap: ci/trigger-template.yml trigger body does not include ANALYSIS_OPTIONS

  # ── US-03: MIN_SCORE gate ─────────────────────────────────────────────

  @implemented
  Scenario: Quality gate passes when repository score meets the threshold
    Given a pipeline trigger is sent with MIN_SCORE "70"
    And the target repository health score is 78
    When the analysis completes and the gate check runs
    Then the job log shows the score and that it meets the threshold
    And the job completes successfully
    And the "barad-dur-report.html" artifact is produced

  @implemented
  Scenario: Quality gate passes when score exactly equals the threshold
    Given a pipeline trigger is sent with MIN_SCORE "70"
    And the target repository health score is exactly 70
    When the gate check runs
    Then the gate passes because the comparison accepts scores equal to or above the threshold
    And the job completes successfully

  @implemented
  Scenario: Quality gate fails but report is still available for diagnosis
    Given a pipeline trigger is sent with MIN_SCORE "70"
    And the target repository health score is 58
    When the gate check runs
    Then the job log shows the score and that it falls below the threshold
    And the job fails the pipeline
    But the "barad-dur-report.html" artifact is still available for download

  @implemented
  Scenario: Non-integer MIN_SCORE is rejected before analysis runs
    Given a pipeline trigger is sent with MIN_SCORE "not-a-number"
    When the analyze-api job validates the gate threshold
    Then the job fails immediately with a clear message that the threshold must be a whole number
    And no analysis is performed
    And no report artifact is produced

  # ── US-04: Reusable caller template ──────────────────────────────────

  @structural @implemented
  Scenario: Team integrates barad-dur analysis with minimal CI configuration
    Given a project includes the barad-dur template from "devops/barad-dur" via the include directive
    And the project has BARAD_DUR_TRIGGER_TOKEN and BARAD_DUR_PROJECT_ID set as CI variables
    When the team adds a job that extends ".barad-dur-analysis" with their repository URL
    Then the job triggers barad-dur analysis
    And downloads the "barad-dur-report.html" report into the team's pipeline workspace
    And the entire integration requires fewer than 10 lines of CI configuration

  @structural @implemented
  Scenario: Missing required CI variables are caught before any API call is made
    Given a project includes the barad-dur template
    And BARAD_DUR_TRIGGER_TOKEN has not been set as a CI variable
    When the caller job starts
    Then the job fails immediately with a message listing the missing variable
    And no trigger API call is attempted

  @structural @implemented
  Scenario: Caller template allows overriding timeout for large repositories
    Given a team extends ".barad-dur-analysis" in their CI configuration
    When they add "timeout: 45 minutes" to their extending job
    Then the job runs with the 45-minute timeout instead of any default
    And the template does not override the caller-specified timeout

  @structural @implemented
  Scenario: Template is includable via GitLab CI include directive without modification
    Given the ci/trigger-template.yml file in the barad-dur repository
    When the file is validated for CI configuration syntax correctness
    Then the file is syntactically valid and can be parsed by the CI system
    And it contains no project-specific secrets or hardcoded URLs that would break inclusion

  @structural @implemented
  Scenario: Commented resource_group is present for teams that need sequential execution
    Given the ci/trigger-template.yml file
    When the concurrency options are inspected
    Then a commented "resource_group" line is present with an example group name
    And a comment explains that uncommenting it enables sequential execution

  # ── US-05: Setup documentation ────────────────────────────────────────

  @structural @implemented
  Scenario: Setup guide exists and covers all required sections for a new adopter
    Given the docs/pipeline-api-setup.md file in the barad-dur repository
    When the document sections are listed
    Then the guide includes all of: prerequisites, trigger token creation, CI variable storage, caller pipeline configuration, verification steps, and troubleshooting
    And each section provides concrete steps or commands a DevOps engineer can follow

  @structural @implemented
  Scenario: Troubleshooting section covers the top five errors adopters encounter
    Given the troubleshooting section of docs/pipeline-api-setup.md
    When the troubleshooting entries are listed
    Then it covers authentication failure (invalid or expired token) with cause and fix
    And it covers project not found (wrong project ID) with cause and fix
    And it covers clone failure (private or invalid repository URL) with cause and fix
    And it covers timeout (repository too large for default duration) with cause and fix
    And it covers empty report (analysis error) with cause and fix

  @structural @implemented
  Scenario: Setup guide specifies the Maintainer permission required for token creation
    Given the prerequisites section of docs/pipeline-api-setup.md
    When the prerequisites are read
    Then it states that Maintainer access to the barad-dur project is required to create trigger tokens
    And it suggests contacting the DevOps lead for teams without Maintainer access

  @structural @implemented
  Scenario: Setup guide references the caller template as the recommended integration approach
    Given the docs/pipeline-api-setup.md guide
    When the CI/CD integration section is read
    Then it references ci/trigger-template.yml and the .barad-dur-analysis job
    And it presents the template as the recommended approach over writing manual trigger scripts

  # ── US-06: Branch selection ───────────────────────────────────────────

  @structural @implemented
  Scenario: REPO_BRANCH defaults to "main" when not specified
    Given the analyze-api job variable declarations in .gitlab-ci.yml
    When the REPO_BRANCH default is inspected
    Then the default value is "main"

  @implemented
  Scenario: Analysis runs on the specified feature branch
    Given a pipeline trigger is sent with REPO_BRANCH "feature/new-payment-flow"
    When the analyze-api job clones the repository
    Then it checks out the "feature/new-payment-flow" branch
    And the analysis reflects the state of that branch
    And the report artifact is produced for that branch

  @implemented
  Scenario: Nonexistent branch causes a clear checkout failure
    Given a pipeline trigger is sent with REPO_BRANCH "feature/typo-brannch"
    When the analyze-api job attempts to clone and check out that branch
    Then the clone fails because the branch does not exist
    And the job log shows a branch not found error
    And the job fails without producing a report artifact

  @implemented
  Scenario: Omitting REPO_BRANCH triggers analysis on the main branch
    Given a pipeline trigger is sent with only REPO_URL and no REPO_BRANCH
    When the analyze-api job clones the repository
    Then it uses the default branch "main"
    And the analysis completes successfully on main

  # ── US-07: Category filter ────────────────────────────────────────────

  @structural @implemented
  Scenario: CATEGORIES variable maps valid names to CLI flags in a case-insensitive way
    Given the CATEGORIES variable processing logic in the analyze-api job script
    When the case matching is inspected
    Then "health", "team", "evolution", and "hygiene" map to their respective CLI flags
    And the matching is case-insensitive (uppercase input is lowercased before matching)

  @implemented
  Scenario: Specifying two categories reduces analysis scope and runtime
    Given a pipeline trigger is sent with CATEGORIES "health,hygiene"
    When the analyze-api job constructs the barad-dur command
    Then it includes "--health --hygiene" flags
    And Team and Evolution category analysis is not performed
    And the "barad-dur-report.html" artifact reflects only the requested categories

  @implemented
  Scenario: Unknown category name produces a warning but does not abort analysis
    Given a pipeline trigger is sent with CATEGORIES "health,unknown-category"
    When the analyze-api job processes the category list
    Then the job logs a warning about the unknown category name
    And the job continues running Health analysis
    And the job does not exit with an error due to the unknown category

  @implemented
  Scenario: Omitting CATEGORIES runs all four analysis categories
    Given a pipeline trigger is sent without the CATEGORIES variable
    When the analyze-api job constructs the analysis command
    Then no category filter flags are added to the barad-dur command
    And the analysis produces a full report covering all four categories
