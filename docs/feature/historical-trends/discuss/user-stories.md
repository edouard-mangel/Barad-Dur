<!-- markdownlint-disable MD024 -->
# User Stories: historical-trends

---

## US-01: Auto-record Trend Snapshot

### Problem
Marco Rossi is an engineering lead who runs `barad-dur analyze .` monthly. He finds it impossible to answer "are we improving?" because each run produces an isolated score with no connection to prior runs. He currently keeps a manual spreadsheet with dates and scores.

### Who
- Engineering leads and developers who run barad-dur repeatedly
- Running in a terminal or CI/CD pipeline
- Motivation: accumulate history passively, without any extra effort

### Solution
Every successful `barad-dur analyze` run automatically appends a snapshot entry to `.repository-analysis/trends.json`. No flag required. On the first run, the file is created silently. On subsequent runs, the entry is appended.

### Domain Examples

#### 1: Marco's first run on a fresh repo
Marco installs barad-dur and runs `barad-dur analyze .` for the first time. The analysis completes and the usual report appears. Below the category table, a new line reads: `Trend: first snapshot recorded`. The file `.repository-analysis/trends.json` now exists with one entry: today's timestamp, the HEAD commit SHA, branch "main", overall score 68, and all four category scores.

#### 2: Priya's scheduled CI run
Priya has `barad-dur analyze . --json -o report.json` in a weekly CI job. After merging this feature, each CI run silently appends to `trends.json` before writing `report.json`. After 4 weeks, trends.json has 4 entries. No CI configuration changes were needed.

#### 3: Corrupt trends.json after disk error
Marco's laptop had a disk write failure mid-run. The next time he runs the tool, `trends.json` contains partial JSON. The tool detects this, renames the file to `trends.json.bak`, prints a warning, creates a fresh `trends.json` with today's run as entry 1, and exits with code 0.

### UAT Scenarios (BDD)

#### Scenario: First run creates trend store
Given no `.repository-analysis/trends.json` file exists
When Marco runs `barad-dur analyze .` and the analysis succeeds
Then `trends.json` is created with exactly 1 entry
And the entry contains: UTC timestamp, HEAD commit SHA, branch name, overall_score, and all 4 category scores
And the CLI output contains the line "Trend: first snapshot recorded"
And the command exits with code 0

#### Scenario: Second run appends to trend store
Given `trends.json` exists with 1 entry
When Marco runs `barad-dur analyze .` again
Then `trends.json` contains exactly 2 entries
And the entries are ordered by timestamp ascending
And the CLI output shows a delta vs the first entry

#### Scenario: Corrupt file is archived and replaced
Given `trends.json` contains invalid JSON
When Marco runs `barad-dur analyze .`
Then a warning is shown containing the phrase "trends.json could not be read"
And `trends.json.bak` exists and contains the corrupt content
And `trends.json` exists with exactly 1 entry (today's run)
And the command exits with code 0

#### Scenario: --no-cache still records trend
Given `trends.json` exists with 1 entry
When Marco runs `barad-dur analyze . --no-cache`
Then the snapshot cache is bypassed
And `trends.json` now contains 2 entries

### Acceptance Criteria
- [ ] Every successful `analyze` run appends to `.repository-analysis/trends.json`
- [ ] Entry contains: ISO8601 UTC timestamp, commit SHA, branch, overall_score, and scores for Health / Team / Evolution / Git Hygiene
- [ ] First run creates the file and outputs "Trend: first snapshot recorded"
- [ ] Corrupt or unreadable trends.json is archived to trends.json.bak and a fresh file is created
- [ ] `--no-cache` still records a trend entry
- [ ] Trend recording adds at most 0.5 seconds to total runtime (no new git calls)

### Outcome KPIs
- **Who**: Users who run barad-dur more than once on the same repo
- **Does what**: Accumulate trend history without any manual steps or config changes
- **By how much**: 100% of repeated runs produce a growing trends.json (measurable in integration tests)
- **Measured by**: Integration test coverage + manual verification in CI logs
- **Baseline**: Currently 0% — no trend data is stored

### Technical Notes
- `trends.json` lives in `.repository-analysis/` which is already gitignored
- Append-only write; no existing entries are modified
- Schema must include a `schema_version` field (value: 1) for future compatibility
- Deduplication: if the same commit SHA is already the last entry, skip the append (idempotent)
- Traces to: JS-01, JS-03, JS-04

---

## US-02: Inline Delta Display in CLI Output

### Problem
Marco gets a score of 74 this month. Without context, he cannot tell his team whether this is progress or regression. The number alone does not answer the only question that matters: "is this getting better or worse?"

### Who
- Engineering leads reading CLI output after a regular analysis run
- No extra flags needed — delta appears automatically once trend history exists
- Motivation: instant directional answer without leaving the terminal

### Solution
When 2+ trend entries exist (prior run + today), the CLI output adds an inline delta next to the overall score and small deltas per category. A compact trend sparkline with a direction indicator appears below the category table.

### Domain Examples

#### 1: Marco's monthly check after a productive sprint
Marco runs `barad-dur analyze .`. Overall score: 79. trends.json has the previous entry: 74. The CLI shows: `Overall Score: 79/100  (+5 vs last run · +11 vs 6 weeks ago)`. Team row shows `+7`. Marco copies this to his status update.

#### 2: Score declined after a noisy merge week
Score is 68, down from 74. CLI shows: `68/100  (-6 vs last run)`. Direction indicator: `↓ declining`. Marco sees it immediately and looks at which category dropped most.

#### 3: Score unchanged
Score is 74, same as last week. CLI shows: `74/100  (+0 vs last run)`. Direction indicator: `→ stable`. No alarm — the plateau is visible.

### UAT Scenarios (BDD)

#### Scenario: Positive delta shown inline
Given `trends.json` has 1 prior entry with overall_score 68 on branch "main"
When Marco runs `barad-dur analyze .` and the current overall score is 74
Then the output contains "74/100  (+6 vs last run)"
And each category row has its numeric delta
And a trend line shows "68 → 74  ↑ improving"

#### Scenario: Negative delta shown with declining indicator
Given `trends.json` has 1 prior entry with overall_score 74 on branch "main"
When Marco runs `barad-dur analyze .` and the current overall score is 68
Then the output contains "68/100  (-6 vs last run)"
And the trend direction indicator is "↓ declining"

#### Scenario: Branch mismatch suppresses delta with warning
Given `trends.json` has 2 entries recorded on branch "feature/refactor"
And the current HEAD is on branch "main"
When Marco runs `barad-dur analyze .`
Then no delta is shown next to the overall score
And a warning message mentions "feature/refactor" and "main"
And the new snapshot is appended with branch "main"

#### Scenario: No prior same-branch snapshots — first-run message
Given `trends.json` does not exist
When Marco runs `barad-dur analyze .`
Then no delta is shown
And the output contains "Trend: first snapshot recorded"

### Acceptance Criteria
- [ ] Delta appears inline with overall score when same-branch prior snapshot exists
- [ ] Per-category deltas shown on each category row
- [ ] Compact trend sparkline with direction indicator shown below category table
- [ ] Branch mismatch shows warning and suppresses delta (no wrong numbers)
- [ ] Output remains parseable by existing scripts that extract the score number (no breaking format change to the score line itself)

### Outcome KPIs
- **Who**: Marco and users who run barad-dur more than once
- **Does what**: Read directional trend data without opening a separate report
- **By how much**: Time to answer "is it improving?" drops from minutes (manual lookup) to seconds (inline delta)
- **Measured by**: User feedback; CLI output verification in integration tests
- **Baseline**: Currently requires manual spreadsheet comparison

### Technical Notes
- Delta is computed at render time from trends.json (not stored as a field)
- Trend sparkline max 8 entries; older entries omitted with "..." if history is longer
- No color-only encoding: direction symbol (↑ ↓ →) always accompanies color
- Traces to: JS-01

---

## US-03: Full Trend History Table (`--trend` flag)

### Problem
Priya just completed a 2-week refactoring sprint. She wants to present before/after data at the sprint review. The inline delta shows the last run only. She needs the full history table to tell the whole story: where scores were 6 weeks ago, the progression, and the final delta.

### Who
- Senior developers and leads preparing to present or justify quality work
- Intentional use: user explicitly wants the full picture
- Motivation: produce a shareable artifact with complete evidence

### Solution
`barad-dur analyze . --trend` runs the usual analysis, records the snapshot, and appends a TREND HISTORY section showing all recorded snapshots in a table with dates and all 5 score columns, plus a footer with velocity and category insights.

### Domain Examples

#### 1: Priya's sprint review preparation
Priya runs `barad-dur analyze . --trend`. The TREND HISTORY table shows 7 rows spanning 7 weeks. She can read: "Team went from 58 to 78 in 7 weeks. Velocity: +2.9/week. Most improved: Team." She pastes this into her retrospective slide.

#### 2: Marco checking for slow-burn decay
Marco runs with `--trend` after noticing the Evolution score looking low in the delta. The table shows: Evolution has been declining 1 point per week for 6 weeks. That's the signal he was watching for.

#### 3: Only 2 runs ever recorded
Priya joins a new team that only has 2 snapshots. She runs `--trend`. The table shows 2 rows. The footer shows: "Velocity: N/A (need at least 3 snapshots)" so she is not misled by a two-point trend.

### UAT Scenarios (BDD)

#### Scenario: Full trend table shows all snapshots
Given `trends.json` has 5 entries on branch "main"
When Marco runs `barad-dur analyze . --trend`
Then the output contains a TREND HISTORY section
And the table has 6 rows (5 prior + today's run, latest marked with * or "today")
And each row has: date, overall score, Health, Team, Evolution, Git Hygiene
And the footer shows computed velocity in points per week
And no git blame or git log operations run during this command

#### Scenario: Most improved and watch categories identified
Given trend history shows Team score growing by 20 points over 6 weeks and Evolution growing by only 4
When Marco runs `barad-dur analyze . --trend`
Then the footer shows "Most improved: Team (+20)"
And shows "Watch: Evolution (slowest improvement)"

#### Scenario: Fewer than 3 snapshots — velocity shown as N/A
Given `trends.json` has exactly 1 entry
When Marco runs `barad-dur analyze . --trend`
Then the table shows 2 rows
And the velocity shows "N/A (need at least 3 snapshots)"
And no other error or warning is shown

### Acceptance Criteria
- [ ] `--trend` shows complete history table with all recorded snapshots
- [ ] Table columns: date, Overall, Health, Team, Evolution, Git Hygiene
- [ ] Footer shows velocity (points/week) when 3+ snapshots exist
- [ ] Footer identifies most improved and watch (slowest) categories
- [ ] Velocity shown as N/A when fewer than 3 snapshots
- [ ] No additional git operations run during `--trend` output
- [ ] Table readable in 80-column terminal without truncation

### Outcome KPIs
- **Who**: Priya and other developers presenting quality work
- **Does what**: Share structured trend evidence in sprint reviews without manual data assembly
- **By how much**: Before/after comparison available in < 30 seconds (one command)
- **Measured by**: User feedback; manual verification of table accuracy
- **Baseline**: Currently requires manual comparison of saved report files

### Technical Notes
- `--trend` reads exclusively from trends.json; no re-analysis
- 80-column terminal constraint: use 10-char date (YYYY-MM-DD), 3-digit scores, compact column widths
- Velocity formula: `(last_overall - first_overall) / weeks_between(first.timestamp, last.timestamp)`
- Traces to: JS-02, JS-03

---

## US-04: JSON Trend Schema (`--json --trend`)

### Problem
Priya has a CI pipeline that runs barad-dur weekly and stores `report.json`. She has started writing a Grafana dashboard that reads these files. With historical trends, she wants to read all historical scores from a single JSON output rather than diffing multiple report files.

### Who
- Developers and DevOps engineers integrating barad-dur into CI/CD
- Consuming `--json` output programmatically (scripts, dashboards, alerting)
- Motivation: stable, versioned JSON schema for trend data that does not break pipelines

### Solution
When both `--json` and `--trend` are specified, the JSON output includes a top-level `trend` key with a versioned schema containing the full snapshot array and computed fields. When `--trend` is absent, the JSON output is structurally identical to the current version.

### Domain Examples

#### 1: Priya's Grafana dashboard
Priya runs `barad-dur analyze . --trend --json -o trend-report.json` in CI. Her Grafana plugin reads `trend.snapshots` and plots each `overall_score` over time. After adding the `--trend` flag to her CI step, the dashboard starts showing a trend line automatically.

#### 2: Script checking for declining score
A DevOps engineer writes a shell script that reads `trend.direction` from the JSON output. If the value is "declining", it posts a Slack alert. The script is 5 lines and works on the first try because the schema is documented and stable.

#### 3: CI job without --trend (backward compatibility)
An existing CI job runs `barad-dur analyze . --json -o report.json` without `--trend`. After the feature ships, the output is identical to before — no `trend` key, no schema changes. The existing JSON parser continues to work.

### UAT Scenarios (BDD)

#### Scenario: --json --trend outputs trend key with snapshots
Given `trends.json` has 4 entries on branch "main"
When Priya runs `barad-dur analyze . --trend --json`
Then the JSON output contains a top-level `trend` object
And `trend.snapshots` is an array with 5 objects
And each object has: `timestamp` (ISO8601), `commit` (SHA string), `branch` (string), `overall_score` (integer), `category_scores` object with keys "Health", "Team", "Evolution", "Git Hygiene"
And `trend.direction` is one of "improving", "declining", "stable"
And `trend.delta_vs_last` is the integer difference between today's and the previous overall_score
And `trend.velocity_per_week` is a float (null when < 2 prior snapshots)
And `trend.schema_version` is the integer 1

#### Scenario: --json without --trend produces unchanged output
Given `trends.json` has 4 entries
When Priya runs `barad-dur analyze . --json`
Then the JSON output does not contain a `trend` key
And all existing top-level fields are present and unchanged

#### Scenario: direction field reflects actual trajectory
Given trends.json has entries with overall scores [60, 65, 68, 72]
When Priya runs `barad-dur analyze . --trend --json` and current score is 75
Then `trend.direction` is "improving"
And `trend.delta_vs_last` is 3

### Acceptance Criteria
- [ ] `--json --trend` includes `trend` key with snapshots array, direction, delta_vs_last, delta_vs_oldest, velocity_per_week, schema_version
- [ ] `--json` without `--trend` produces output structurally identical to current version
- [ ] Each snapshot entry has: timestamp, commit, branch, overall_score, category_scores (all 4 keys)
- [ ] schema_version is integer 1
- [ ] velocity_per_week is null (not absent) when fewer than 2 prior snapshots
- [ ] direction is exactly one of "improving", "declining", "stable"

### Outcome KPIs
- **Who**: DevOps engineers and developers with CI/CD integrations
- **Does what**: Parse trend data from JSON without writing custom diffing scripts
- **By how much**: CI dashboard integration time < 1 hour with published schema
- **Measured by**: Schema conformance tests; downstream consumer feedback
- **Baseline**: Currently requires manual diffing of multiple report.json files

### Technical Notes
- JSON schema treated as API contract once published
- `category_scores` key names must exactly match the existing `categories[*].name` values in AnalysisReport to ensure consistency
- velocity_per_week: round to 2 decimal places
- Traces to: JS-04
