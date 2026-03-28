Feature: Cross-Repository Coupling Detection
  As an engineering leader managing multiple repositories
  I want to detect temporal, team, and dependency coupling between repos
  So that I can identify which repositories are too tightly coupled and plan decoupling work

  Background:
    Given Adriana has 24 microservice repositories under "/home/adriana/work/services/"
    And each repository has git history spanning at least 6 months
    And barad-dur is installed with the coupling subcommand available

  # =========================================================================
  # Step 1: Discover Repositories
  # =========================================================================

  @discover @happy-path
  Scenario: Scan root directory for git repos
    When Adriana runs "barad-dur coupling /home/adriana/work/services/"
    Then the tool scans subdirectories for git repositories
    And reports "Found 24 repositories"
    And reports "Analysis scope: 276 repo pairs"

  @discover @happy-path
  Scenario: Scan with custom time window
    When Adriana runs "barad-dur coupling /home/adriana/work/services/ --window 3months"
    Then the tool reports "Time window: 3 months"
    And only commits within the last 3 months are considered

  @discover @error
  Scenario: No git repos found in directory
    When Adriana runs "barad-dur coupling /tmp/empty/"
    Then the tool prints "Error: No git repositories found"
    And suggests "barad-dur coupling /path/to/repos/"
    And exits with code 1

  @discover @error
  Scenario: Single repo found suggests analyze
    When Adriana runs "barad-dur coupling /home/adriana/work/services/payment-gateway"
    Then the tool prints "Found 1 repository"
    And suggests "barad-dur analyze" for single-repo coupling analysis
    And exits with code 1

  @discover @error
  Scenario: Non-existent directory
    When Adriana runs "barad-dur coupling /path/that/does/not/exist"
    Then the tool prints "Error: directory not found"
    And exits with code 1

  # =========================================================================
  # Step 2: Collect Git Snapshots
  # =========================================================================

  @collect @happy-path
  Scenario: Collect snapshots with progress
    Given 24 subdirectories found, 22 are valid git repos
    When the tool collects git history per repo
    Then a progress bar shows "10/22" incrementally
    And cached repos complete in under 1 second each
    And all 22 repos are collected successfully

  @collect @error
  Scenario: Non-git directories are skipped
    Given "legacy-monolith" is not a git repository
    And "temp-experiment" has no commits
    When the tool scans and collects
    Then "legacy-monolith" is skipped with reason "not a git repository"
    And "temp-experiment" is skipped with reason "no commits"
    And the tool proceeds with 22 valid repos

  @collect @error
  Scenario: Permission denied on a repo
    Given "/opt/restricted-repo" is not readable
    When the tool attempts to collect its history
    Then it is skipped with reason "permission denied"
    And the tool continues with remaining repos

  @collect @error
  Scenario: All repos invalid exits with error
    Given all subdirectories are not git repos
    When the tool attempts collection
    Then the tool prints "Error: No valid repositories to analyze"
    And exits with code 1

  # =========================================================================
  # Step 3: Analyze Temporal Coupling (Release 1)
  # =========================================================================

  @temporal @happy-path
  Scenario: Detect temporally coupled repo pairs
    Given 22 repos collected with commit histories
    And payment-gateway had 54 commits in the window
    And billing-service had 67 commits in the window
    And 42 of payment-gateway's commits occurred within 24 hours of a billing-service commit
    When the tool analyzes temporal coupling
    Then the pair "payment-gateway <> billing-service" has a temporal coupling score of 78%
    And the co-change count is 42
    And the confidence is "HIGH"

  @temporal @happy-path
  Scenario: Ranked output sorted by coupling score
    Given 22 repos analyzed for temporal coupling
    And 8 pairs have coupling above 30%
    When the CLI renders coupling results
    Then 8 pairs are shown sorted by coupling score descending
    And each row shows: rank, repo A, repo B, score, co-changes, confidence
    And a summary shows "High coupling: 2 | Medium: 6 | Total significant: 8 of 231"

  @temporal @happy-path
  Scenario: Low-coupling pairs hidden by default
    Given 231 total repo pairs analyzed
    And 223 pairs have coupling below 30%
    When the CLI renders coupling results
    Then only 8 pairs above the threshold are shown
    And a note says "223 pairs below 30% not shown. Use --all to display."

  @temporal @edge-case
  Scenario: No significant coupling detected
    Given 22 repos with independent commit histories
    And no pair exceeds 30% temporal coupling
    When the tool analyzes temporal coupling
    Then the output says "No significant temporal coupling detected above 30%"
    And suggests "Try lowering the threshold: --min-coupling 15"

  @temporal @edge-case
  Scenario: Custom coupling window
    When Adriana runs "barad-dur coupling ./repos/ --coupling-window 48h"
    Then commits within 48 hours of each other are counted as co-changes
    And the output header shows "Coupling window: 48h"

  # =========================================================================
  # Step 4: Analyze Team Coupling (Release 2)
  # =========================================================================

  @team @happy-path
  Scenario: Detect shared authors between repos
    Given "search-indexer" has 5 unique authors
    And "catalog-service" has 4 unique authors
    And Yuki Tanaka committed to both repos
    When the tool analyzes team coupling
    Then the pair "search-indexer <> catalog-service" shows team coupling at 80%
    And lists "Yuki Tanaka" as a shared author

  @team @happy-path
  Scenario: Single-author bridge flagged as bus factor risk
    Given Yuki Tanaka is the ONLY shared author between search-indexer and catalog-service
    When the tool analyzes team coupling
    Then the output flags "Yuki Tanaka: single-author bridge (bus factor risk)"
    And the team coupling section highlights this pair

  @team @edge-case
  Scenario: Author with different emails across repos
    Given Carlos Mendez uses "carlos@acme.com" in payment-gateway
    And Carlos Mendez uses "carlos.mendez@gmail.com" in billing-service
    And both commits use the name "Carlos Mendez"
    When the tool analyzes team coupling
    Then Carlos is matched as the same author (name-based normalization)
    And appears as a shared author between payment-gateway and billing-service

  # =========================================================================
  # Step 5: Analyze Dependency Coupling (Release 2)
  # =========================================================================

  @dependency @happy-path
  Scenario: Detect shared Cargo.toml dependencies
    Given payment-gateway's Cargo.toml lists "shared-libs = { path = '../shared-libs' }"
    And billing-service's Cargo.toml lists "shared-libs = { path = '../shared-libs' }"
    When the tool analyzes dependency coupling
    Then the pair "payment-gateway <> billing-service" shows "shared-libs" as a shared dependency
    And "shared-libs" is reported with 5 direct consumers

  @dependency @happy-path
  Scenario: Blast radius report for hub dependency
    Given shared-libs is referenced by 5 repos' Cargo.toml files
    When the tool analyzes dependency coupling
    Then the output shows "shared-libs: blast radius = 5 repositories"
    And lists all 5 consumer repos

  @dependency @edge-case
  Scenario: Mixed manifest types across repos
    Given payment-gateway has Cargo.toml (Rust)
    And frontend-app has package.json (Node)
    When the tool analyzes dependency coupling
    Then each repo's manifest is parsed according to its type
    And shared external dependencies (e.g., "serde" in Cargo.toml and "@serde/wasm" in package.json) are NOT conflated

  # =========================================================================
  # Step 6: Output Formats
  # =========================================================================

  @output @happy-path
  Scenario: JSON output with coupling data
    Given coupling analysis complete for 22 repos
    When Adriana runs "barad-dur coupling ./repos/ --json -o coupling.json"
    Then "coupling.json" contains valid JSON
    And the JSON has a "coupling" top-level object with:
      | field            | type    |
      | repos_scanned    | integer |
      | pairs_analyzed   | integer |
      | time_window      | string  |
      | coupling_window  | string  |
      | generated_at     | string  |
    And "coupling.pairs" is an array of objects with "repo_a", "repo_b", "temporal", "team", "dependency", "combined"

  @output @happy-path
  Scenario: HTML visualization opens in browser
    Given coupling analysis complete for 22 repos
    When Adriana runs "barad-dur coupling ./repos/ --html --open"
    Then an HTML file is generated
    And it is self-contained (no external CSS or JS)
    And it contains tabs: Graph, Matrix, Pairs, Teams, Dependencies
    And it opens in the default browser

  @output @happy-path
  Scenario: Output to file
    Given coupling analysis complete
    When Adriana runs "barad-dur coupling ./repos/ -o coupling-report.txt"
    Then the CLI output is written to "coupling-report.txt"
    And nothing is printed to stdout

  # =========================================================================
  # Non-Functional Requirements
  # =========================================================================

  @property @performance
  Scenario: 50-repo analysis completes in reasonable time
    Given 50 repositories with cached snapshots
    When the tool analyzes temporal coupling (1225 pairs)
    Then total execution time is under 60 seconds
    And memory usage stays under 500 MB

  @property @usability
  Scenario: CLI output fits terminal width
    Given 30 repo pairs above threshold
    When the CLI renders coupling results
    Then no line exceeds 120 characters
    And repository names longer than 20 characters are truncated with "..."

  @property @backward-compat
  Scenario: Existing analyze command unchanged
    When Tomasz runs "barad-dur analyze . --json"
    Then the JSON output is structurally identical to pre-coupling versions
    And no "coupling" keys appear in the output

  @property @resilience
  Scenario: Ctrl-C during analysis exits cleanly
    Given coupling analysis is running
    When the user presses Ctrl-C
    Then the tool exits immediately with code 130
    And no partial results are written
