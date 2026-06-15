Feature: Pipeline Trigger API for barad-dur analysis
  As a DevOps engineer managing multiple repositories on Froggit
  I want to trigger barad-dur analysis from any CI pipeline
  So that I can integrate repository health checks into my team's workflows
  without installing barad-dur locally or managing its dependencies

  Background:
    Given barad-dur is deployed on Froggit project "devops/barad-dur" with ID 4217
    And a pipeline trigger token "BARAD_DUR_TRIGGER_TOKEN" exists for that project
    And the barad-dur Docker image is published to the Froggit container registry

  # ── Happy Path ──────────────────────────────────────────────

  Scenario: Trigger analysis on a public repository
    Given Fatima Benali has stored the trigger token as a masked CI variable
    And her project "team/my-service" has a nightly pipeline job "trigger-analysis"
    When the job triggers the barad-dur pipeline with variables:
      | variable         | value                                              |
      | REPO_URL         | https://froggit.example.com/team/my-service.git    |
      | REPO_BRANCH      | main                                               |
    Then a new pipeline starts on "devops/barad-dur"
    And the "analyze-api" job clones the target repository
    And the job produces a "barad-dur-report.json" artifact
    And the artifact contains a valid JSON report with "overall_score" field

  Scenario: Retrieve report from triggered pipeline
    Given the barad-dur pipeline #58432 has completed successfully
    And the "analyze-api" job produced "barad-dur-report.json"
    When the calling pipeline downloads the artifact via GitLab API
    Then the caller receives a JSON file with the full analysis report
    And the report contains scores for Health, Team, Evolution, and Git Hygiene

  Scenario: Trigger analysis with custom options
    When the job triggers the barad-dur pipeline with variables:
      | variable           | value                                              |
      | REPO_URL           | https://froggit.example.com/team/my-service.git    |
      | REPO_BRANCH        | develop                                            |
      | ANALYSIS_OPTIONS   | --skip-blame --since 3months                       |
    Then the analysis runs with blame skipped and a 3-month time window
    And the report is produced faster due to skip-blame

  Scenario: Trigger analysis with score gate
    When the job triggers the barad-dur pipeline with variables:
      | variable         | value                                              |
      | REPO_URL         | https://froggit.example.com/team/my-service.git    |
      | MIN_SCORE        | 70                                                 |
    And the analysis produces an overall score of 74
    Then the "analyze-api" job succeeds (exit code 0)
    And the artifact contains the report with overall_score 74

  # ── Error Paths ─────────────────────────────────────────────

  Scenario: Trigger with invalid token
    When a pipeline triggers barad-dur with an invalid trigger token
    Then GitLab responds with HTTP 401 Unauthorized
    And no pipeline is created on the barad-dur project

  Scenario: Trigger with nonexistent repository URL
    When the job triggers the barad-dur pipeline with variables:
      | variable         | value                                              |
      | REPO_URL         | https://froggit.example.com/nonexistent/repo.git   |
    Then the "analyze-api" job starts and attempts to clone
    And the clone fails with "repository not found"
    And the job exits with a non-zero exit code
    And no artifact is produced

  Scenario: Analysis score below gate threshold
    When the job triggers the barad-dur pipeline with variables:
      | variable         | value                                              |
      | REPO_URL         | https://froggit.example.com/team/legacy-app.git    |
      | MIN_SCORE        | 80                                                 |
    And the analysis produces an overall score of 52
    Then the "analyze-api" job fails (exit code 1)
    But the report artifact is still produced (for debugging)

  Scenario: Analysis on a very large repository exceeds timeout
    When the job triggers the barad-dur pipeline with variables:
      | variable         | value                                              |
      | REPO_URL         | https://froggit.example.com/org/monorepo.git       |
    And the repository has 500,000 commits and 50,000 files
    Then the "analyze-api" job exceeds the configured timeout
    And GitLab terminates the job with status "failed"
    And no artifact is produced

  # ── Category Filter ─────────────────────────────────────────

  Scenario: Trigger analysis for specific categories only
    When the job triggers the barad-dur pipeline with variables:
      | variable         | value                                              |
      | REPO_URL         | https://froggit.example.com/team/my-service.git    |
      | CATEGORIES       | health,hygiene                                     |
    Then the report contains scores only for Health and Git Hygiene
    And Team and Evolution categories are not computed
