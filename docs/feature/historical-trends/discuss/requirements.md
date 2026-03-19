# Requirements: historical-trends

## Business Context

barad-dur today produces a single-point-in-time analysis. Users — engineering leads and senior developers — have no way to determine whether their scores are improving or declining. This blocks two critical use cases: tracking the impact of quality investments, and detecting slow-burn decay before it causes incidents.

Historical trend analysis adds append-only snapshot recording to every analysis run and surfaces trend data in the CLI, JSON, and HTML outputs. It requires zero new user behavior for the core loop (auto-recording), and adds an opt-in `--trend` flag for explicit trend exploration.

---

## Domain Glossary

| Term | Definition |
|------|-----------|
| Snapshot | A single analysis result (scores + metadata) recorded at one point in time |
| Trend entry | Same as snapshot when stored in trends.json (persisted form) |
| Trend history | The ordered sequence of all recorded snapshots for a branch |
| Delta | Numeric difference between current score and the previous snapshot's score |
| Velocity | Points-per-week rate of change computed from the full trend history |
| Trend store | `.repository-analysis/trends.json` — the append-only file holding all snapshots |
| Direction | Qualitative trajectory label: "improving", "declining", or "stable" |
| Branch mismatch | State where trend history exists for a different branch than the current run |

---

## Personas

### Marco Rossi — Engineering Lead
- Runs barad-dur monthly (sometimes weekly after large merges)
- Reads CLI output; occasionally opens HTML report to share with stakeholders
- Primary pain: scores exist in isolation; cannot communicate direction
- Primary job: JS-01 (track direction), JS-03 (detect decay)

### Priya Nair — Senior Developer
- Runs barad-dur after every major refactoring sprint
- Shares analysis results in sprint reviews and with managers
- Primary pain: no shareable before/after artifact
- Primary job: JS-02 (validate refactoring impact), JS-04 (CI integration)

---

## Functional Requirements

### FR-01: Automatic Trend Recording
Every successful `barad-dur analyze` run appends a snapshot entry to `.repository-analysis/trends.json`. No flag required. The entry contains: UTC timestamp, HEAD commit SHA, branch name, overall score, and all 4 category scores.

### FR-02: Delta Display in CLI Output
When trend history exists (2+ entries including today), the CLI output shows:
- Inline delta on the overall score line: `(+N vs last run)`
- Per-category deltas on each category row
- A compact trend sparkline below the category table
- A direction indicator: ↑ improving / ↓ declining / → stable

### FR-03: First-Run Informational Message
When no prior trend data exists (first run ever, or first run on current branch), the CLI output includes a single line: `Trend: first snapshot recorded`. No delta or trend line is shown.

### FR-04: Full History Table (`--trend` flag)
`barad-dur analyze . --trend` shows a TREND HISTORY section with all recorded snapshots in a tabular format (date, overall, all 4 categories), plus a footer with velocity and category insights.

### FR-05: JSON Trend Output (`--json --trend`)
When both `--json` and `--trend` are specified, the JSON output includes a top-level `trend` key containing the complete snapshot array plus computed fields (direction, delta_vs_last, delta_vs_oldest, velocity_per_week). The existing JSON structure is unchanged when `--trend` is absent.

### FR-06: HTML Trend Tab (`--html --trend`)
When both `--html` and `--trend` are specified, the HTML report includes a "Trends" tab containing the snapshot history as a table. Sparkline charts are desirable but not required for Release 2.

### FR-07: Branch Mismatch Warning
When the current branch differs from the branch recorded in existing trend entries, a warning is shown explaining the mismatch. No delta is shown. The new snapshot is tagged with the current branch and appended (not merged with the other branch's history).

### FR-08: Corrupt Trend Store Recovery
If `trends.json` cannot be parsed (corrupt JSON or incompatible schema version), the file is renamed to `trends.json.bak`, a warning message is shown, and a fresh trends.json is created. The analysis exits with code 0.

---

## Non-Functional Requirements

### NFR-01: Performance
Trend recording adds at most 0.5 seconds to a completed analysis run. No additional git blame or git log calls are made for trend recording. Trend recording reads/writes only from `.repository-analysis/trends.json`.

### NFR-02: File Size
Each trend entry is at most 1KB of JSON. After 52 weekly runs (1 year), trends.json is at most 52KB.

### NFR-03: Backward Compatibility
`--json` without `--trend` produces output structurally identical to the current version. No existing fields are removed or renamed. The `trend` key only appears when `--trend` is explicitly specified.

### NFR-04: Schema Stability
The JSON trend schema includes a `schema_version` field. Minor additions (new fields) are backward-compatible. Field removals or renames require a major version increment and a deprecation notice.

### NFR-05: Gitignore Safety
`trends.json` is stored in `.repository-analysis/` which is already gitignored by the `ensure_gitignore` mechanism. No additional gitignore action is required.

---

## Business Rules

### BR-01: No re-analysis of past commits
Historical trend data is built exclusively from forward-running analyses. Past commits are never re-analyzed. Trend depth grows naturally over calendar time.

### BR-02: Branch isolation
Trend entries are tagged by branch. Delta computation only uses entries from the same branch as the current run. Mixing branches requires explicit user consent (Release 3: `--trend-branch` flag).

### BR-03: Deduplication
If the same commit SHA appears in consecutive entries (e.g., user runs analyze twice without any new commits), the second entry is either skipped or updated in place. Duplicate entries with the same commit must not skew velocity calculations.

### BR-04: Stable direction thresholds
"Improving" = delta > 0. "Declining" = delta < 0. "Stable" = delta == 0. Thresholds are not user-configurable in Release 1.

---

## Out of Scope

- Re-analyzing past commits on demand
- Comparing repositories against each other
- Team-level or project-level aggregation across repos
- Trend data stored remotely (all storage is local, in `.repository-analysis/`)
- Automated alerting or threshold-based warnings (deferred to Release 3+)
- `barad-dur trend list` convenience subcommand (Release 3 candidate)

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| `AnalysisReport` struct (`scorer.rs`) | Exists | Trend recording reads from this after each run |
| `CACHE_DIR` constant (`cache/storage.rs`) | Exists | trends.json stored in same directory |
| `ensure_gitignore` (`cache/storage.rs`) | Exists | No change needed — directory already gitignored |
| `--json` flag + renderer | Exists | Trend key added only when `--trend` also specified |
| `--html` flag + renderer | Exists | Trend tab added when `--trend` specified |
| New: `src/trend.rs` (suggested) | New | Trend storage, loading, delta computation, schema |
