Feature: Pipeline analysis triggered from any CI pipeline (Walking Skeleton — R1)
  As a DevOps engineer managing multiple repositories on Froggit
  I want to trigger barad-dur analysis from my own CI pipeline
  So that I can get an interactive HTML health report without installing barad-dur locally

  # Driving ports:
  #   - GitLab Trigger API POST /api/v4/projects/:id/trigger/pipeline
  #   - analyze-api job in .gitlab-ci.yml (lines 596-661)
  #   - .barad-dur-analysis hidden job in ci/trigger-template.yml
  #
  # Artifact: barad-dur-report.html (self-contained interactive HTML)
  # Note: DISCUSS referenced barad-dur-report.json; DESIGN changed this to HTML (UI-01)

  Background:
    Given barad-dur is deployed on Froggit with the analyze-api job enabled
    And a pipeline trigger token is stored as a masked CI variable "BARAD_DUR_TRIGGER_TOKEN"
    And the barad-dur Docker image is available in the Froggit container registry

  # ── Structural Verification (no live pipeline needed) ──────────────────

  @structural @implemented
  Scenario: analyze-api job structure activates only on pipeline triggers
    Given the .gitlab-ci.yml file in the barad-dur project
    When the analyze-api job rules are inspected
    Then the job has the rule "CI_PIPELINE_SOURCE == trigger AND REPO_URL is set"
    And the job is not triggered by push, merge request, or scheduled pipelines
    And the job uses the container image from the Froggit project registry

  @structural @implemented
  Scenario: Artifact is retained even when quality gate fails
    Given the analyze-api job configuration
    When the artifacts section is inspected
    Then the artifact path is "barad-dur-report.html"
    And the artifact retention policy is set to "always" regardless of whether the job succeeds or fails
    And the artifact expires after 1 week

  @structural @implemented
  Scenario: Caller template exists and defines a reusable hidden job
    Given the ci/trigger-template.yml file in the barad-dur project
    When the file structure is inspected
    Then a hidden job named ".barad-dur-analysis" is defined
    And the job validates BARAD_DUR_TRIGGER_TOKEN, BARAD_DUR_PROJECT_ID, and REPO_URL before triggering
    And the job downloads "barad-dur-report.html" into the caller's workspace on success

  # ── Happy Path — US-01 ────────────────────────────────────────────────

  @walking_skeleton @implemented
  Scenario: DevOps engineer triggers analysis and receives an HTML health report
    Given Fatima has stored her trigger token and the barad-dur project ID as CI variables
    And she has added "analyze-my-repo" job that extends the barad-dur caller template
    When her nightly pipeline triggers barad-dur with her repository URL "https://froggit.example.com/fintech/payment-gateway.git"
    Then a new barad-dur analysis pipeline starts
    And the analyze-api job clones the payment-gateway repository
    And the job produces a "barad-dur-report.html" artifact with the full health report
    And Fatima's pipeline downloads the report and makes it available as a pipeline artifact

  @implemented
  Scenario: Repository with no recent activity still produces a report
    Given the target repository "fintech/legacy-auth" has no commits in the past 6 months
    When a pipeline trigger is sent with the repository URL
    Then the analyze-api job completes successfully
    And a "barad-dur-report.html" artifact is produced
    And the report includes a notice about the empty analysis window

  # ── Error Paths — US-01 ───────────────────────────────────────────────

  @implemented
  Scenario: Trigger without a repository URL fails immediately with a clear error
    Given a pipeline trigger is sent without the REPO_URL variable
    When the analyze-api job starts
    Then the job fails immediately before attempting any repository operation
    And the job log shows "ERROR: REPO_URL is required"
    And no report artifact is produced

  @implemented
  Scenario: Non-HTTPS repository URL is rejected before cloning
    Given a pipeline trigger is sent with REPO_URL "ftp://example.com/repo.git"
    When the analyze-api job validates the repository URL
    Then the job fails immediately with a clear message that only secure HTTPS URLs are accepted
    And no clone is attempted
    And no report artifact is produced

  @implemented
  Scenario: Repository that does not exist causes a clear clone failure
    Given a pipeline trigger is sent with REPO_URL "https://froggit.example.com/nonexistent/repo.git"
    When the analyze-api job attempts to clone the repository
    Then the clone fails with a repository not found error
    And the job log shows a clear message that the repository could not be cloned
    And the job fails without producing a report artifact

  # ── Happy Path — US-02 ────────────────────────────────────────────────

  @implemented
  Scenario: Caller pipeline polls for completion and downloads the report
    Given Fatima's pipeline has triggered barad-dur with her repository URL
    And the barad-dur analysis is running
    When the analysis completes successfully
    Then Fatima's polling job detects the "success" status
    And downloads "barad-dur-report.html" from the completed analyze-api job
    And prints a summary confirming the report is available

  # ── Error Paths — US-02 ───────────────────────────────────────────────

  @implemented
  Scenario: Caller handles downstream analysis failure with actionable error message
    Given Fatima's pipeline has triggered barad-dur with an invalid repository URL
    When the analyze-api job fails
    Then Fatima's polling loop detects the "failed" pipeline status
    And the caller log shows the failed pipeline URL for investigation
    And the caller job exits with a non-zero exit code

  @implemented
  Scenario: Caller handles trigger authentication failure with a clear message
    Given BARAD_DUR_TRIGGER_TOKEN contains an expired token
    When Fatima's pipeline attempts to trigger a new barad-dur analysis
    Then the trigger is rejected because the token is not recognized
    And the caller log shows a clear authentication failure message with guidance to check the token
    And the caller job fails without attempting to poll for a pipeline that was never created

  @implemented
  Scenario: Trigger token is never exposed in caller job logs
    Given Fatima's trigger job is running with BARAD_DUR_TRIGGER_TOKEN set
    When the trigger, poll, and download steps run
    Then the raw token value never appears in any log line
    And the token is only referenced via the masked CI variable name
