<!-- markdownlint-disable MD024 -->
# User Stories: Cross-Repository Coupling Detection

---

## US-01: Coupling Subcommand Discovers Repos in Root Directory

### Problem
Adriana Kowalski is a VP of Engineering who suspects 5-6 repo pairs are too coupled, but she has no data. To investigate, she would need to manually check CI failure patterns across 24 repos -- a process that takes days. She needs a single command that scans a directory of repos and prepares them for coupling analysis.

### Who
- Engineering leaders and architects investigating cross-repo coupling
- Running from terminal, pointing at a local workspace directory
- Motivation: discover all repos automatically without listing them individually

### Solution
A new `coupling` subcommand that accepts a root directory, scans first-level subdirectories for git repositories, validates them, and reports the discovered scope (repo count and pair count).

### Domain Examples

#### 1: Adriana scans her services directory
Adriana runs `barad-dur coupling /home/adriana/work/services/`. The tool finds 24 subdirectories, determines 22 are valid git repos, and reports "Found 22 repositories (2 skipped). Analysis scope: 231 repo pairs."

#### 2: Yuki scans a smaller workspace
Yuki runs `barad-dur coupling /home/yuki/work/`. The tool finds 5 git repos and reports "Found 5 repositories. Analysis scope: 10 repo pairs."

#### 3: Root directory has no git repos
Adriana accidentally runs `barad-dur coupling /tmp/downloads/`. The tool finds 0 git repos and prints "Error: No git repositories found" with a usage example.

### UAT Scenarios (BDD)

#### Scenario: Discover repos in root directory
Given "/home/adriana/work/services/" contains 24 subdirectories
And 22 are valid git repositories with at least one commit
And "legacy-monolith" has no .git directory
And "temp-experiment" has zero commits
When Adriana runs "barad-dur coupling /home/adriana/work/services/"
Then the tool reports "Found 22 repositories (2 skipped)"
And reports "Analysis scope: 231 repo pairs"
And lists skipped repos with reasons

#### Scenario: Single repo suggests analyze
Given "/home/adriana/work/services/payment-gateway" contains exactly one git repo
When Adriana runs "barad-dur coupling /home/adriana/work/services/payment-gateway"
Then the output says "Found 1 repository. Cross-repo coupling requires 2+ repositories."
And suggests "barad-dur analyze" for intra-repo coupling
And exits with code 1

#### Scenario: No repos found
Given "/tmp/empty/" contains no git repositories
When Adriana runs "barad-dur coupling /tmp/empty/"
Then the output says "Error: No git repositories found"
And exits with code 1

#### Scenario: Non-existent directory
When Adriana runs "barad-dur coupling /path/does/not/exist"
Then the output says "Error: directory not found"
And exits with code 1

### Acceptance Criteria
- [ ] `barad-dur coupling` subcommand exists and accepts a positional directory argument
- [ ] Scans first-level subdirectories for `.git` directories with at least one commit
- [ ] Reports discovered count, skipped count, and pair scope
- [ ] Requires 2+ valid repos; exits code 1 with helpful message for 0 or 1
- [ ] Skipped repos show specific reason (not a repo, no commits, permission denied)

### Outcome KPIs
- **Who**: Users investigating cross-repo coupling
- **Does what**: Discover all repos in a workspace from one command instead of listing them individually
- **By how much**: Discovery time from manual enumeration (5-10 minutes) to automatic scan (2 seconds)
- **Measured by**: Integration test confirming subdirectory scanning finds expected repos
- **Baseline**: No multi-repo coupling detection exists

### Technical Notes
- New `CouplingArgs` struct in `cli.rs` following existing pattern (AnalyzeArgs, GateArgs)
- First-level subdirectory scan only (no recursion) per BR-01
- Reuses existing git repo validation logic
- Traces to: JS-01

---

## US-02: Snapshot Collection with Progress and Skip-on-Error

### Problem
Adriana's 22 repos need git metadata (commit timestamps, authors) collected before coupling analysis can begin. Without progress feedback, she stares at a frozen terminal. Without error handling, one corrupt repo kills the entire batch.

### Who
- Users running coupling analysis on 5+ repositories
- Motivation: see collection progress and know the tool is working; one bad repo should not stop the batch

### Solution
The tool collects RepoSnapshot per repo using the existing Collector pipeline, with a progress bar showing current repo and completed count. Failed repos are skipped with a warning.

### Domain Examples

#### 1: Adriana watches collection progress
The progress bar shows "[======>                    ] 10/22 notification-svc". Cached repos complete in 0.2s, uncached in 30-60s. ETA shows remaining time.

#### 2: One repo has a corrupt pack file
"search-indexer" fails during collection. The tool prints "WARNING: search-indexer: git error (corrupt pack file). Skipping." and continues with the remaining 21 repos.

#### 3: Cached repos complete quickly
8 of 22 repos have fresh cache. They complete in 0.2-0.3 seconds each, and the progress bar jumps from 0/22 to 8/22 in under 3 seconds.

### UAT Scenarios (BDD)

#### Scenario: Collection with progress bar
Given 22 valid repos to collect
When the tool starts snapshot collection
Then a progress bar shows completed/total count
And the current repo name is shown
And estimated time remaining is displayed after the first uncached repo

#### Scenario: Cached repos collected quickly
Given 10 repos all with fresh cache
When the tool collects snapshots
Then each completes in under 1 second
And no git blame operations are performed

#### Scenario: Collection failure continues
Given 22 repos to collect and "search-indexer" has a corrupt pack file
When collection reaches "search-indexer"
Then a warning appears on stderr
And the tool continues to the next repo
And final count shows "21/22 collected (1 failed)"

### Acceptance Criteria
- [ ] Progress bar displayed when collecting 3+ repos
- [ ] Current repo name and completed/total count shown
- [ ] ETA displayed after first uncached repo completes
- [ ] Cache used per-repo (stale check via HEAD commit SHA)
- [ ] Failed repos logged as warning; collection continues
- [ ] Final summary shows collected count, failed count, total time

### Outcome KPIs
- **Who**: Users running coupling analysis on 10+ repos
- **Does what**: Stay informed about collection progress instead of facing a frozen terminal
- **By how much**: Perceived wait time reduced (progress visibility)
- **Measured by**: Progress bar appears in integration test output
- **Baseline**: No multi-repo collection exists

### Technical Notes
- Reuses existing Collector -> Snapshot pipeline per repo
- Uses `indicatif` (already a dependency) for progress bar
- Cache check: existing `cache::load()` + `cache::is_stale()` per repo
- Consider parallel collection via rayon for uncached repos
- Traces to: JS-01

---

## US-03: Temporal Coupling Analysis Across Repo Pairs

### Problem
Adriana suspects that payment-gateway and billing-service are temporally coupled -- commits in one frequently coincide with commits in the other. She currently detects this manually by correlating CI failure timestamps. She needs an algorithm that quantifies this across all repo pairs automatically.

### Who
- Engineering leaders and architects investigating hidden coupling
- Motivation: quantify temporal coupling between repos with confidence scores, not gut feelings

### Solution
For each repo pair, count commits in repo A that occur within a configurable coupling window (default 24h) of a commit in repo B. Compute temporal coupling score as co-changes / min(commits_A, commits_B) * 100. Rank all pairs by score.

### Domain Examples

#### 1: payment-gateway and billing-service are highly coupled
payment-gateway had 54 commits in the past 6 months. billing-service had 67. 42 of payment-gateway's commits occurred within 24 hours of a billing-service commit. Score: 42/54 = 78%. Confidence: HIGH (42 co-changes). This confirms Adriana's suspicion.

#### 2: catalog-service and search-indexer show moderate coupling
catalog-service had 43 commits, search-indexer had 38. 28 co-changes within 24h. Score: 28/38 = 74%. Confidence: HIGH.

#### 3: Two repos with coincidental timing
api-gateway had 31 commits, logging-svc had 45. 4 co-changes within 24h. Score: 4/31 = 13%. Below the 30% threshold, not shown in default output.

### UAT Scenarios (BDD)

#### Scenario: High temporal coupling detected
Given payment-gateway has 54 commits in the time window
And billing-service has 67 commits
And 42 commits in payment-gateway occur within 24h of a billing-service commit
When the tool analyzes temporal coupling
Then the pair "payment-gateway <> billing-service" has score 78%
And co-change count is 42
And confidence is "HIGH"

#### Scenario: Pairs ranked by coupling score
Given 22 repos analyzed for temporal coupling
And 8 pairs have coupling above 30%
When the analysis completes
Then pairs are ranked by score descending
And the highest-coupled pair appears first

#### Scenario: No significant coupling found
Given 22 repos with independent commit histories
And no pair exceeds 30% temporal coupling
When the analysis completes
Then the result reports "No significant coupling detected above 30%"
And suggests lowering the threshold

#### Scenario: Custom coupling window
Given Adriana runs with "--coupling-window 48h"
When commits within 48 hours of each other are compared
Then co-change counts reflect the wider window
And the output header shows "Coupling window: 48h"

#### Scenario: Minimum co-change threshold
Given a pair has only 2 co-changes
When the tool computes coupling
Then the pair is excluded (below 3 co-change minimum)

### Acceptance Criteria
- [ ] Temporal coupling computed for all valid repo pairs
- [ ] Score = co_changes / min(commits_A, commits_B) * 100
- [ ] Coupling window configurable via `--coupling-window` (default 24h)
- [ ] Confidence levels: HIGH (30+), MEDIUM (10-29), LOW (3-9)
- [ ] Pairs with fewer than 3 co-changes excluded
- [ ] Pairs ranked by score descending
- [ ] Progress shown during pairwise analysis

### Outcome KPIs
- **Who**: Adriana and other engineering leaders
- **Does what**: Identify temporally coupled repo pairs with quantified scores
- **By how much**: Time to identify coupling drops from days (manual CI log correlation) to minutes
- **Measured by**: Integration test confirming coupling score computation for known pairs
- **Baseline**: No cross-repo temporal coupling detection exists

### Technical Notes
- Algorithm: for each pair (A, B), iterate A's commits, for each commit find B commits within coupling_window using binary search on sorted timestamps
- Complexity: O(P * N * log N) where P = pairs, N = avg commits per repo
- Parallel pair analysis via rayon for large repo sets
- Uses existing RepoSnapshot.commits (already sorted by date)
- Traces to: JS-01

---

## US-04: CLI Output with Ranked Coupling Pairs

### Problem
Adriana has coupling scores for 231 repo pairs. Without formatted output, she would need to scan raw data to find the worst offenders. She needs a ranked CLI display that immediately surfaces the most coupled pairs with dimension breakdown.

### Who
- Users reviewing coupling analysis results in the terminal
- Motivation: see the most coupled repo pairs at a glance, worst-first

### Solution
CLI output showing: header with repo count and analysis parameters, ranked table of coupling pairs above threshold (sorted by score descending), summary with high/medium/low counts, and skipped repos at the bottom.

### Domain Examples

#### 1: Adriana reads the ranked output
The table shows 8 pairs above 30%. Rank 1 is "payment-gateway <> billing-service" at 78% with 42 co-changes. She immediately confirms her suspicion.

#### 2: Yuki sees the summary
Summary shows "High coupling (>= 60%): 2 pairs | Medium (30-59%): 6 pairs". She now knows the scale of the coupling problem.

#### 3: No coupling above threshold
Output says "No significant temporal coupling detected above 30%. Try --min-coupling 15 or --coupling-window 48h."

### UAT Scenarios (BDD)

#### Scenario: Ranked table with coupling pairs
Given 8 coupling pairs above 30% threshold
When the CLI renders results
Then the table shows 8 rows sorted by score descending
And each row has: rank, repo A, repo B, score, co-changes, confidence
And a summary shows counts by severity

#### Scenario: Pairs below threshold hidden
Given 231 total pairs and 223 below 30%
When the CLI renders results
Then 223 pairs are not shown
And a note says "223 pairs below 30% not shown. Use --all to display."

#### Scenario: Skipped repos listed
Given 2 repos were skipped during discovery
When the CLI renders results
Then a footer section lists skipped repos with reasons

#### Scenario: No coupling detected
Given no pairs above 30%
When the CLI renders results
Then the output says "No significant coupling detected"
And suggests lowering threshold or widening coupling window

### Acceptance Criteria
- [ ] Header shows repo count, time window, coupling window, timestamp
- [ ] Ranked table sorted by coupling score descending
- [ ] Each row shows rank, repo A, repo B, score, co-changes, confidence
- [ ] Summary shows high/medium/low counts
- [ ] Pairs below threshold hidden with count and --all hint
- [ ] Skipped repos listed at bottom with reasons
- [ ] Output readable in 120-column terminal; repo names truncated at 20 chars

### Outcome KPIs
- **Who**: Users reviewing coupling analysis
- **Does what**: Identify the most coupled repo pair in seconds instead of scanning raw data
- **By how much**: Time to identify top coupling pair drops from minutes to 5 seconds (read first row)
- **Measured by**: Integration test confirming ranked output with correct sort order
- **Baseline**: No cross-repo coupling output exists

### Technical Notes
- Output formatting uses existing CLI renderer patterns
- Sort order: coupling score descending (most coupled first)
- Truncation: repo names > 20 chars get "..." suffix
- Traces to: JS-01

---

## US-05: Team Coupling Detection (Shared Authors and Bridges)

### Problem
Yuki Tanaka keeps getting pulled from search-indexer into catalog-service because she is the only person who has committed to both. Nobody realized she was a single-author bridge until she got sick and both repos stalled. The team needs visibility into cross-repo author overlap.

### Who
- Team leads managing developer assignments across repos
- New joiners who unknowingly become knowledge bridges
- Motivation: identify shared authors and single-person bus factor risks across repo boundaries

### Solution
For each repo pair, compute the author overlap percentage and list shared authors. Flag single-author bridges as bus factor risks with commit counts per repo.

### Domain Examples

#### 1: Yuki is a single-author bridge
search-indexer has 5 authors, catalog-service has 4 authors, 8 unique total. Only Yuki Tanaka appears in both. Team coupling: 1/8 = 12.5%. But she is flagged as a "single-author bridge" because she is the ONLY shared author.

#### 2: payment-gateway and billing-service share 3 authors
7 unique authors across both repos, 3 shared (Adriana, Carlos Mendez, Lisa Park). Team coupling: 3/7 = 43%. No bus factor risk because multiple people bridge the repos.

#### 3: Author email normalization
Carlos Mendez uses "carlos@acme.com" in payment-gateway and "carlos.mendez@gmail.com" in billing-service. Both commits have the name "Carlos Mendez". The tool matches by name and counts him as one shared author.

### UAT Scenarios (BDD)

#### Scenario: Single-author bridge detected
Given Yuki Tanaka is the only shared author between search-indexer and catalog-service
And she has 42 commits in search-indexer and 18 in catalog-service
When the tool analyzes team coupling
Then the pair shows team coupling score
And Yuki is flagged as "single-author bridge (bus factor risk)"
And her commit counts per repo are shown

#### Scenario: Multiple shared authors (no bus factor risk)
Given payment-gateway and billing-service share 3 authors out of 7 unique
When the tool analyzes team coupling
Then the team coupling score is 43%
And all 3 shared authors are listed
And no bus factor risk is flagged

#### Scenario: Author normalization by name
Given Carlos Mendez uses different emails across repos but the same display name
When the tool computes author overlap
Then Carlos is counted as one shared author, not two separate people

### Acceptance Criteria
- [ ] Team coupling score computed as shared_authors / total_unique_authors * 100
- [ ] Shared authors listed by name for each pair
- [ ] Single-author bridges flagged as bus factor risk
- [ ] Author matching uses display name (case-insensitive), not email
- [ ] Team coupling integrated into the ranked output alongside temporal coupling

### Outcome KPIs
- **Who**: Team leads and engineering managers
- **Does what**: Identify single-author bridges before they become incidents
- **By how much**: Detection time from "after the person is unavailable" to "proactive visibility"
- **Measured by**: Integration test confirming bridge detection for known single-author pairs
- **Baseline**: No cross-repo author overlap visibility exists

### Technical Notes
- Author normalization: lowercase display name comparison
- Uses existing `RepoSnapshot.authors` and `commits_by_author`
- Future: `.coupling-mailmap` config for manual overrides
- Traces to: JS-02

---

## US-06: Dependency Coupling Detection (Manifest Scanning)

### Problem
Tomasz Wierzbicki updated shared-libs last month and 5 downstream services broke. He had no blast radius map. He manually grepped Cargo.toml files across 12 repos to find who depends on shared-libs. He needs automated dependency coupling detection.

### Who
- Platform engineers managing shared libraries
- Architects assessing dependency topology
- Motivation: know the blast radius before making changes to shared code

### Solution
Scan manifest files (Cargo.toml, package.json, go.mod, requirements.txt) across all discovered repos. Identify shared dependencies and direct dependency relationships. Report blast radius per hub dependency.

### Domain Examples

#### 1: shared-libs has 5 direct consumers
Five repos list `shared-libs = { path = "../shared-libs" }` in their Cargo.toml. The tool reports "shared-libs: blast radius = 5 repositories" and lists all consumers.

#### 2: Two repos share 3 external dependencies
payment-gateway and billing-service both depend on serde 1.0.195, tokio 1.35, and shared-libs. The tool reports 3 shared dependencies for this pair.

#### 3: Mixed ecosystems
Repos use different languages (Rust with Cargo.toml, Node with package.json). The tool parses each manifest type independently and does not conflate cross-ecosystem packages.

### UAT Scenarios (BDD)

#### Scenario: Detect shared Cargo.toml dependencies
Given payment-gateway's Cargo.toml lists shared-libs as a path dependency
And billing-service's Cargo.toml also lists shared-libs
When the tool scans manifests
Then "payment-gateway <> billing-service" shows shared-libs as a shared dependency

#### Scenario: Blast radius for hub dependency
Given shared-libs is referenced by 5 repos
When the tool computes dependency coupling
Then the output shows "shared-libs: blast radius = 5 repositories"
And lists all 5 consumer repos

#### Scenario: No manifest file in a repo
Given "legacy-scripts" has no Cargo.toml, package.json, or other manifest
When the tool scans manifests
Then "legacy-scripts" has no dependency coupling data
And this is noted (not treated as an error)

### Acceptance Criteria
- [ ] Manifest files scanned: Cargo.toml, package.json, go.mod, requirements.txt
- [ ] Shared dependencies identified per repo pair
- [ ] Dependency direction detected (A depends on B via path dependency)
- [ ] Blast radius computed per hub dependency (count of direct consumers)
- [ ] Missing manifests handled gracefully (no error, just no dependency data)
- [ ] Dependency coupling integrated into ranked output alongside temporal and team

### Outcome KPIs
- **Who**: Platform engineers managing shared libraries
- **Does what**: Know blast radius before making shared library changes
- **By how much**: From manual grep across repos (30+ minutes) to automatic scan (seconds)
- **Measured by**: Integration test confirming blast radius computation for known dependencies
- **Baseline**: Manual Cargo.toml grep across repos

### Technical Notes
- Cargo.toml parsing via `toml` crate (already a dependency)
- package.json parsing via `serde_json`
- Path dependencies indicate direct repo-to-repo coupling
- Registry dependencies (crates.io, npm) indicate shared external coupling
- Traces to: JS-03

---

## US-07: JSON Coupling Output with Versioned Schema

### Problem
Tomasz wants to run coupling analysis weekly in CI and feed results to Grafana. He needs a stable JSON output format for the coupling report, not just CLI text.

### Who
- DevOps engineers consuming coupling data programmatically
- CI/CD pipelines feeding monitoring dashboards
- Motivation: stable, versioned JSON schema for coupling data

### Solution
`barad-dur coupling <root-dir> --json` produces a JSON object with a `coupling` top-level key containing metadata, coupling pairs array (with per-dimension scores), team bridges, and dependency map. Schema includes a version field.

### Domain Examples

#### 1: Tomasz feeds coupling data to Grafana
Tomasz runs `barad-dur coupling /path/to/repos/ --json -o coupling.json` in CI weekly. His Grafana dashboard reads `coupling.pairs[].temporal` and plots coupling trends over time.

#### 2: Alert script checks for high coupling
A shell script: `jq '.coupling.pairs[] | select(.temporal > 70)' coupling.json`. If any pairs exceed 70%, it posts to Slack.

#### 3: Schema versioning
The JSON includes `"schema_version": 1`. Tomasz's scripts check this field before parsing.

### UAT Scenarios (BDD)

#### Scenario: JSON output structure
Given 22 repos analyzed for coupling
When Tomasz runs "barad-dur coupling ./repos/ --json --pretty"
Then the output is valid JSON
And contains a "coupling" top-level object
And "coupling.repos_scanned" is 22
And "coupling.pairs_analyzed" is 231
And "coupling.schema_version" is 1
And "coupling.pairs" is an array of objects

#### Scenario: Per-pair JSON structure
Given coupling analysis complete
When Tomasz reads "coupling.pairs[0]"
Then it contains "repo_a", "repo_b", "temporal" (number), "co_changes" (integer), "confidence" (string)
And optionally "team" (number), "dependency" (object), "combined" (number)

#### Scenario: Existing analyze --json unchanged
Given the coupling feature is deployed
When Tomasz runs "barad-dur analyze . --json"
Then the output does not contain a "coupling" key
And is structurally identical to pre-coupling versions

### Acceptance Criteria
- [ ] `--json` produces JSON with `coupling` top-level key
- [ ] Schema includes: repos_scanned, pairs_analyzed, time_window, coupling_window, schema_version, generated_at, pairs array
- [ ] Each pair has: repo_a, repo_b, temporal, co_changes, confidence, and optional team/dependency/combined
- [ ] schema_version is integer 1
- [ ] `--pretty` produces indented JSON
- [ ] `barad-dur analyze . --json` output unchanged

### Outcome KPIs
- **Who**: DevOps engineers with CI/CD integrations
- **Does what**: Consume coupling data from a stable JSON schema
- **By how much**: Eliminate custom parsing/scraping of CLI text output
- **Measured by**: Schema conformance tests; `jq` queries work against output
- **Baseline**: No programmatic coupling data output exists

### Technical Notes
- JSON schema treated as API contract; breaking changes require schema_version bump
- Per-pair entries include all dimensions present in the analysis (temporal always; team/dependency when R2 is active)
- `--pretty` reuses the same serde_json::to_string_pretty pattern
- Traces to: JS-01, JS-03

---

## US-08: HTML Coupling Visualization (Interactive Graph + Matrix)

### Problem
Adriana prepares a quarterly architecture review for the CTO. She needs a visual representation of cross-repo coupling -- not terminal output. The coupling landscape is complex and benefits from interactive exploration: filtering by dimension, clicking pairs for details, seeing the overall graph topology.

### Who
- Engineering leaders presenting to non-technical stakeholders
- Architects planning decoupling work
- Motivation: shareable, interactive coupling visualization without manual diagram assembly

### Solution
`barad-dur coupling <root-dir> --html` generates a self-contained HTML file with interactive tabs: Graph (force-directed, edge thickness = coupling strength), Matrix (repos x repos heatmap), Pairs (ranked list), Teams (shared authors), Dependencies (blast radius). Dimensions are toggleable.

### Domain Examples

#### 1: Adriana generates a coupling visualization
Adriana runs `barad-dur coupling /home/adriana/work/services/ --html -o q1-coupling.html`. She opens it in her browser and sees an interactive graph where payment-gateway and billing-service have a thick red edge (78% coupling).

#### 2: Matrix view reveals cluster
The matrix tab shows a block of red cells in the upper-left corner: payment-gateway, billing-service, notification-svc, and user-auth are all coupled to each other. This cluster pattern was invisible in the ranked list.

#### 3: Adriana filters to team coupling only
She unchecks "Temporal" and "Dependencies" in the filter panel. The graph now shows only team coupling edges. She sees Yuki's single-author bridge clearly.

### UAT Scenarios (BDD)

#### Scenario: HTML generation
Given coupling analysis complete for 22 repos
When Adriana runs "barad-dur coupling ./repos/ --html -o report.html"
Then "report.html" is a valid HTML file
And contains no external CSS, JS, or image references
And renders correctly in modern browsers

#### Scenario: Interactive graph with edge filtering
Given the HTML visualization is open
Then repos are displayed as nodes in a force-directed graph
And edges represent coupling (thickness = strength, color = dimension)
And checkboxes allow toggling temporal/team/dependency edges

#### Scenario: Matrix heatmap
Given the HTML visualization is open
When Adriana clicks the "Matrix" tab
Then a repos x repos grid is shown
And cells are color-coded: red >= 60%, yellow 30-59%, green < 30%
And clicking a cell shows the pair detail

#### Scenario: HTML opened with --open flag
When Adriana runs "barad-dur coupling ./repos/ --html --open"
Then the HTML file is generated and opened in the default browser

### Acceptance Criteria
- [ ] `--html` produces self-contained HTML (no external dependencies)
- [ ] Contains tabs: Graph, Matrix, Pairs, Teams, Dependencies
- [ ] Graph shows repos as nodes, coupling as edges with thickness and color
- [ ] Matrix shows repos x repos with color-coded cells
- [ ] Dimension filtering checkboxes (temporal, team, dependency)
- [ ] `--open` generates and opens in default browser
- [ ] Follows existing HTML renderer pattern from renderer/html.rs

### Outcome KPIs
- **Who**: Adriana and other leadership-facing users
- **Does what**: Generate a shareable coupling visualization without manual diagram assembly
- **By how much**: Visualization assembly from 2+ hours (Miro) to 2 minutes (single command)
- **Measured by**: HTML file generated with all required tabs and interactive elements
- **Baseline**: Manual Miro/Lucidchart diagrams drawn from memory

### Technical Notes
- New `renderer/coupling_html.rs` module following existing `renderer/html.rs` pattern
- Self-contained: inline CSS + JS in HTML template
- Force-directed graph: lightweight inline JS (no D3.js dependency, or inline D3 if needed)
- Matrix color logic: red >= 60%, yellow 30-59%, green < 30%
- Traces to: JS-04

---

## US-09: Dimension Filtering in HTML Visualization

### Problem
Adriana's coupling visualization shows all three dimensions at once. For her architecture review, she wants to isolate temporal coupling from team coupling to tell a clear story about each type of coupling independently.

### Who
- Users analyzing coupling data in the HTML visualization
- Motivation: focus on one coupling dimension at a time for clearer analysis and presentation

### Solution
The HTML visualization includes checkboxes to toggle temporal, team, and dependency coupling independently. The graph, matrix, and pairs list all update dynamically when filters change.

### Domain Examples

#### 1: Adriana isolates temporal coupling
She unchecks "Team" and "Dependencies". The graph now shows only temporal coupling edges. Two clusters are clearly visible: the payment cluster and the catalog cluster.

#### 2: Yuki isolates team coupling
She unchecks "Temporal" and "Dependencies". The graph shows team coupling edges only. Her single-author bridge between search-indexer and catalog-service stands out as the only connection between two otherwise-separate clusters.

#### 3: Tomasz isolates dependency coupling
He unchecks "Temporal" and "Team". The graph shows dependency edges only. shared-libs appears as a central hub with 5 outgoing edges.

### UAT Scenarios (BDD)

#### Scenario: Toggle temporal coupling off
Given the HTML visualization shows all dimensions
When Adriana unchecks "Temporal"
Then temporal coupling edges are hidden from the graph
And the matrix shows only team + dependency scores
And the pairs list is re-filtered

#### Scenario: Single dimension selected
Given all dimensions are unchecked except "Team"
Then only team coupling data is displayed across all tabs
And the matrix cells show team coupling scores only

#### Scenario: All dimensions re-enabled
Given some dimensions were unchecked
When Adriana checks all three
Then the visualization returns to showing combined coupling data

### Acceptance Criteria
- [ ] Three checkboxes: Temporal, Team, Dependencies (all checked by default)
- [ ] Unchecking a dimension hides its edges from the graph
- [ ] Matrix updates to show only selected dimensions
- [ ] Pairs list re-filters to show only pairs with selected dimension scores
- [ ] Changes are instant (no page reload)

### Outcome KPIs
- **Who**: Users analyzing coupling visualization
- **Does what**: Isolate specific coupling dimensions for focused analysis
- **By how much**: From "all dimensions mixed together" to "clean single-dimension view"
- **Measured by**: HTML interactive test confirming filter toggles update the display
- **Baseline**: No dimension filtering exists

### Technical Notes
- Implemented as JavaScript event handlers on checkbox elements
- Graph edge visibility toggled via CSS class or SVG attribute
- Matrix recalculation uses the same data, filtered by selected dimensions
- Traces to: JS-04
