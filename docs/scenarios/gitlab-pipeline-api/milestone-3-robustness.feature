Feature: Timeout configuration and concurrency safeguards (R3)
  As a DevOps engineer managing large repositories or company-wide barad-dur adoption
  I want predictable timeout behavior and guidance on concurrent analysis load
  So that I can operate barad-dur reliably even under demanding conditions

  # Driving ports:
  #   - analyze-api job timeout: keyword in .gitlab-ci.yml (line 660)
  #   - .barad-dur-analysis hidden job in ci/trigger-template.yml (timeout override + resource_group)
  #   - docs/pipeline-api-setup.md (Performance Tips, Concurrency sections)

  Background:
    Given barad-dur is deployed on Froggit with the analyze-api job enabled
    And a pipeline trigger token is available as "BARAD_DUR_TRIGGER_TOKEN"

  # ── US-08: Timeout configuration ─────────────────────────────────────

  @structural @implemented
  Scenario: analyze-api job has a 30-minute default timeout
    Given the analyze-api job configuration in .gitlab-ci.yml
    When the timeout setting is inspected
    Then the job declares "timeout: 30 minutes"
    And this is sufficient for repositories with up to 10,000 commits without skip-blame

  @structural @implemented
  Scenario: Caller can extend the template with a custom timeout for large repositories
    Given a team extends ".barad-dur-analysis" with "timeout: 45 minutes"
    When the GitLab CI configuration is evaluated
    Then the caller job runs with a 45-minute timeout
    And the 30-minute default from the template does not apply because the caller overrides it

  @structural @implemented
  Scenario: Job killed by timeout produces no report artifact
    Given the analyze-api job running against a repository with 500,000 commits
    And the job timeout is set to 30 minutes
    When the job runs for more than 30 minutes
    Then GitLab terminates the job with a "failed" status
    And no "barad-dur-report.html" artifact is produced because GitLab kills the process before the artifacts step
    And this is expected GitLab CI behavior requiring no special job script handling

  @structural @skip
  Scenario: Setup guide recommends skip-blame for repositories with more than 50,000 commits
    Given the Performance Tips section of docs/pipeline-api-setup.md
    When the performance recommendations are read
    Then it states that the "--skip-blame" option is recommended for repositories with more than 50,000 commits
    And it explains that skip-blame significantly reduces analysis time at the cost of some blame-based metrics

  # Implementation gap: docs/pipeline-api-setup.md mentions --skip-blame but without the 50,000-commit threshold
  # DELIVER must add the specific threshold to the Performance Tips section

  # ── US-09: Concurrency safeguards ────────────────────────────────────

  @structural @skip
  Scenario: Documentation explains the trade-off between parallel and sequential execution
    Given the Concurrency section of docs/pipeline-api-setup.md
    When the trade-off guidance is read
    Then it explains that the default behavior allows multiple analyses to run in parallel
    And it explains that adding resource_group serializes analyses to one at a time
    And it describes when each approach is appropriate (parallel for speed, sequential for runner stability)

  # Implementation gap: docs/pipeline-api-setup.md Concurrency section shows how to add resource_group
  # but does not explain the trade-offs between parallel and sequential execution

  @structural @implemented
  Scenario: Caller template includes a commented resource_group option for teams that need it
    Given the ci/trigger-template.yml file
    When the concurrency section is inspected
    Then a commented "resource_group: barad-dur-analysis" line is present
    And the comment indicates that uncommenting it enables sequential execution

  @structural @implemented
  Scenario: Each triggered analysis job is fully isolated with no shared state
    Given multiple analyze-api jobs running simultaneously for different repositories
    When each job runs in its own container
    Then each clones to its own ephemeral "/tmp/target" directory within its container
    And no cache is shared between parallel jobs
    And one job's analysis does not affect another job's results or files

  @structural @skip
  Scenario: Documentation recommends staggering pipeline schedules for more than 10 simultaneous triggers
    Given the Concurrency section of docs/pipeline-api-setup.md
    When the staggering guidance is read
    Then it includes a recommendation to stagger cron schedules when more than 10 teams trigger simultaneously
    And it explains that staggering distributes runner load without requiring resource_group

  # Implementation gap: docs/pipeline-api-setup.md Concurrency section has no staggering guidance
  # DELIVER must add a recommendation for teams planning >10 concurrent triggers
