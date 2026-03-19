Feature: Historical Trend Analysis
  As Marco Rossi, an engineering lead who runs barad-dur regularly,
  I want to see whether repository health is improving or declining over time
  So that I can back my team's quality claims with measurable evidence

  Background:
    Given a git repository at "/home/marco/projects/infra-api"
    And the .repository-analysis/ directory is gitignored

  # ---------------------------------------------------------------------------
  # Step 1: Auto-recording — first run
  # ---------------------------------------------------------------------------

  Scenario: First analysis run creates trend snapshot without extra flags
    Given no .repository-analysis/trends.json file exists
    When Marco runs "barad-dur analyze ."
    Then the usual scored CLI report is displayed with overall score and categories
    And .repository-analysis/trends.json is created
    And trends.json contains exactly 1 snapshot entry
    And the entry includes a UTC timestamp, the current branch name, the HEAD commit SHA, and scores for all 5 dimensions (overall + 4 categories)
    And the CLI output includes a line matching "Trend: first snapshot recorded"
    And no score delta is shown (there is no prior run to compare)

  Scenario: First run does not modify existing analysis output format
    Given no trends.json file exists
    And Marco has a script that parses the CLI output for the overall score line
    When he runs "barad-dur analyze ."
    Then the overall score line format is unchanged from the current version
    And the trend line appears below the category table, not inline with scores

  # ---------------------------------------------------------------------------
  # Step 2: Delta display on subsequent runs
  # ---------------------------------------------------------------------------

  Scenario: Second run shows delta versus the previous snapshot
    Given trends.json contains 1 snapshot with overall_score 68 recorded 7 days ago on branch "main"
    And the current HEAD is on branch "main"
    When Marco runs "barad-dur analyze ." and the current overall score is 74
    Then the CLI output shows the overall score as "74/100  (+6 vs last run)"
    And each category row includes its individual delta
    And a compact trend line shows the sequence "68 → 74" with an upward indicator
    And trends.json now contains 2 entries

  Scenario: Declining score shows negative delta with downward indicator
    Given trends.json contains 1 snapshot with overall_score 74 recorded 7 days ago on branch "main"
    When Marco runs "barad-dur analyze ." and the current overall score is 68
    Then the CLI output shows the overall score as "68/100  (-6 vs last run)"
    And the trend direction indicator is "↓ declining"
    And trends.json now contains 2 entries

  Scenario: Stable score shows zero delta with neutral indicator
    Given trends.json contains 1 snapshot with overall_score 74 recorded 7 days ago on branch "main"
    When Marco runs "barad-dur analyze ." and the current overall score is 74
    Then the delta shown is "(+0 vs last run)"
    And the trend direction indicator is "→ stable"

  Scenario: Branch mismatch shows warning instead of delta
    Given trends.json contains 2 snapshots recorded on branch "feature/refactor"
    And the current HEAD is on branch "main"
    When Marco runs "barad-dur analyze ."
    Then the usual scored report is displayed
    And a warning is shown: "Trend: 2 snapshots found on 'feature/refactor'; 0 on current branch 'main'."
    And no score delta is shown
    And the new snapshot is recorded as a separate entry tagged with branch "main"

  # ---------------------------------------------------------------------------
  # Step 3: Full trend table with --trend flag
  # ---------------------------------------------------------------------------

  Scenario: Full trend history table displayed with --trend flag
    Given trends.json contains 5 snapshots recorded weekly over 5 weeks on branch "main"
    And all snapshots are for the same branch
    When Marco runs "barad-dur analyze . --trend"
    Then the output contains a TREND HISTORY section
    And the table has 6 data rows (5 prior + today's run)
    And each row shows: date, overall score, Health score, Team score, Evolution score, Git Hygiene score
    And the current run's row is marked as "today"
    And the footer shows the computed velocity in points per week
    And the footer identifies the most improved category
    And no git blame or commit re-analysis runs during this command

  Scenario: Velocity computed correctly across time range
    Given trends.json contains 3 snapshots with overall scores [60, 65, 70] recorded exactly 1 week apart
    When Marco runs "barad-dur analyze . --trend" and the current score is 74
    Then the velocity shown is approximately "+2.3/wk" (14-point gain over ~6 weeks)

  Scenario: --trend flag with fewer than 2 snapshots shows informational message
    Given trends.json contains exactly 1 snapshot
    When Marco runs "barad-dur analyze . --trend"
    Then the trend history table shows 2 rows (1 prior + today)
    And the velocity shows "N/A (need at least 3 snapshots for velocity)"

  # ---------------------------------------------------------------------------
  # Step 4: JSON export with trend data
  # ---------------------------------------------------------------------------

  Scenario: --json --trend outputs trend key in JSON
    Given trends.json contains 4 snapshots on branch "main"
    When Marco runs "barad-dur analyze . --trend --json -o out.json"
    Then out.json contains a top-level "trend" key
    And trend.snapshots is an array with 5 objects (4 prior + today)
    And each snapshot object has: timestamp (ISO8601), commit (SHA string), branch (string), overall_score (integer), and a category_scores object with Health, Team, Evolution, and "Git Hygiene" keys
    And trend.direction is one of "improving", "declining", or "stable"
    And trend.delta_vs_last is the integer difference between today's and the previous snapshot's overall_score
    And trend.velocity_per_week is a float (null when fewer than 2 prior snapshots)
    And all pre-existing top-level JSON keys remain present and unchanged

  Scenario: --json without --trend omits trend key for backward compatibility
    Given trends.json contains 4 snapshots
    When Marco runs "barad-dur analyze . --json"
    Then the JSON output does not contain a "trend" key
    And the output is identical in structure to a run without any prior trend history

  Scenario: --html --trend includes trend data in HTML report
    Given trends.json contains 6 snapshots on branch "main"
    When Marco runs "barad-dur analyze . --trend --html -o report.html"
    Then report.html renders correctly in a browser
    And the report includes a "Trends" tab or section
    And the trends section contains a table of all 7 snapshots with scores

  # ---------------------------------------------------------------------------
  # Error paths
  # ---------------------------------------------------------------------------

  Scenario: Corrupt trends.json is archived and replaced
    Given .repository-analysis/trends.json contains invalid JSON
    When Marco runs "barad-dur analyze ."
    Then the usual scored report is displayed
    And a warning is shown containing "trends.json could not be read"
    And the corrupt file is renamed to trends.json.bak
    And a new trends.json is created with exactly 1 entry (today's run)
    And the command exits with code 0

  Scenario: trends.json written by a newer incompatible version is handled gracefully
    Given .repository-analysis/trends.json contains a "schema_version" field with value 99
    When Marco runs "barad-dur analyze ."
    Then a warning is shown: "Trend history was written by a newer version of barad-dur and cannot be read"
    And the existing file is not overwritten or corrupted
    And the analysis completes and exits with code 0

  Scenario: --no-cache does not prevent trend recording
    When Marco runs "barad-dur analyze . --no-cache"
    Then the snapshot cache is bypassed (full git collection)
    And today's result is still appended to trends.json
    And the CLI output includes the trend delta if prior snapshots exist

  # ---------------------------------------------------------------------------
  # Performance constraint (non-functional)
  # ---------------------------------------------------------------------------

  @property
  Scenario: Trend recording adds negligible overhead to analysis runtime
    Given a repository with an existing analysis that completes in T seconds
    When trend recording is enabled (default)
    Then the total runtime does not exceed T + 0.5 seconds
    And no additional git blame calls are made for trend recording

  @property
  Scenario: trends.json file size grows proportionally and predictably
    Given one snapshot entry is appended per run
    Then each entry is no larger than 1KB of JSON
    And after 52 weekly runs (1 year), trends.json is no larger than 52KB
