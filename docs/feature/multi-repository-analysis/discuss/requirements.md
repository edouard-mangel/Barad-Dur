# Requirements -- Cross-Repository Coupling Detection

## Functional Requirements

### FR-01: Coupling subcommand
The tool provides a `coupling` subcommand that accepts a root directory path, scans its subdirectories for git repositories, and analyzes coupling between them.

### FR-02: Repository discovery
The tool automatically discovers git repositories by scanning first-level subdirectories of the provided root directory for `.git` directories with at least one commit. Non-repo directories are skipped with reason.

### FR-03: Minimum repo count
Cross-repo coupling requires at least 2 valid repositories. A single repo directs the user to `barad-dur analyze` for intra-repo coupling.

### FR-04: Temporal coupling analysis
For each repo pair, the tool counts how many commits in repo A occur within a configurable time window (default: 24 hours) of a commit in repo B. The temporal coupling score is the co-change count divided by the minimum of the two repos' total commits, expressed as a percentage.

### FR-05: Team coupling analysis (R2)
For each repo pair, the tool computes the overlap in commit authors. Team coupling score is the number of shared authors divided by the total unique authors across both repos. Single-author bridges (one person is the only shared author) are flagged as bus factor risks.

### FR-06: Dependency coupling analysis (R2)
The tool scans manifest files (Cargo.toml, package.json, go.mod, requirements.txt) to identify shared dependencies between repo pairs. It reports shared dependency count, dependency direction (A depends on B), and blast radius (number of repos depending on a given library/repo).

### FR-07: Ranked CLI output
The default CLI output shows coupling pairs ranked by score (highest first) with columns for each coupling dimension. Pairs below the minimum threshold (default 30%) are hidden unless `--all` is specified.

### FR-08: Coupling summary
The output includes a summary section showing: total repos scanned, total pairs analyzed, count of high/medium/low coupling pairs, and skipped repos with reasons.

### FR-09: Confidence indicator
Each coupling pair shows a confidence level based on sample size: HIGH (30+ co-changes), MEDIUM (10-29), LOW (3-9). Pairs with fewer than 3 co-changes are not reported.

### FR-10: Configurable parameters
Users can configure: time window for git history (`--window`, default 6 months), coupling window for co-change detection (`--coupling-window`, default 24h), minimum coupling threshold for display (`--min-coupling`, default 30%).

### FR-11: JSON coupling output (R2)
`--json` produces a JSON object with a `coupling` top-level key containing metadata, coupling pairs array (with per-dimension scores), team bridges, and dependency map. Schema includes a version field.

### FR-12: HTML coupling visualization (R3)
`--html` produces a self-contained HTML file with interactive tabs: Graph (force-directed repo graph with coupling edges), Matrix (repos x repos coupling heatmap), Pairs (ranked list), Teams (shared authors and bridges), Dependencies (blast radius view). Filterable by coupling dimension.

### FR-13: Skip-and-continue on failure
If a repository fails during discovery or collection, it is skipped with a warning. A single failure does not stop the batch. Only if ALL repos are invalid does the tool exit with code 1.

---

## Non-Functional Requirements

### NFR-01: Performance
- Cached repo snapshots load in under 1 second each
- Temporal coupling analysis for 50 repos (1225 pairs) completes in under 60 seconds
- Memory usage under 500 MB for 50-repo analysis
- Parallel snapshot collection via rayon (already a dependency)

### NFR-02: CLI readability
- Output fits 120-column terminal without wrapping
- Repository names truncated to 20 characters with "..." for longer names
- Table alignment maintained regardless of pair count

### NFR-03: Backward compatibility
- `barad-dur analyze` command behavior is completely unchanged
- No modifications to existing `RepoSnapshot`, `AnalysisReport`, or scorer output
- The coupling subcommand is additive; existing features are untouched

### NFR-04: JSON schema stability (R2)
- Coupling JSON output includes `schema_version: 1`
- Schema changes require version bump
- Additive changes (new optional fields) allowed within same version

### NFR-05: Error resilience
- Ctrl-C exits cleanly (no partial files, no orphan temp dirs)
- Corrupt repos do not crash the tool
- Permission errors are caught and reported, not propagated as panics

### NFR-06: HTML self-containment (R3)
- HTML visualization has no external CSS, JS, or image dependencies
- Works offline after generation
- Follows existing HTML report pattern from `renderer/html.rs`

---

## Business Rules

### BR-01: Root directory scanning
The coupling subcommand scans first-level subdirectories only. It does not recurse into nested directories. A directory like `/repos/team-a/service-x/` is found if `coupling /repos/team-a/` is invoked, but not if `coupling /repos/` is invoked (service-x is two levels deep).

### BR-02: Coupling score calculation
Temporal coupling score = co_changes / min(commits_A, commits_B) * 100. Using the minimum avoids penalizing repos with very different commit frequencies.

### BR-03: Confidence levels
- HIGH: 30+ co-changes (strong signal)
- MEDIUM: 10-29 co-changes (moderate signal)
- LOW: 3-9 co-changes (weak signal, may be coincidental)
- Below 3 co-changes: not reported (insufficient data)

### BR-04: Author normalization (R2)
Authors are matched by display name (case-insensitive), not by email. This handles the common case of developers using different email addresses across repos. A future `.coupling-mailmap` config can override matches.

### BR-05: Coupling window vs time window
The TIME window (`--window`) controls how far back in git history to look (default: 6 months). The COUPLING window (`--coupling-window`) controls the maximum gap between two commits to count as a co-change (default: 24 hours). These are independent parameters.

---

## Out of Scope

| Item | Rationale |
|------|-----------|
| GitLab group scan as input source | Future enhancement; local directory scan covers the immediate use case |
| Portfolio health dashboard | Replaced by coupling-focused feature per user clarification |
| API contract detection (proto, OpenAPI) | Requires deep parsing; defer until manifest scanning proves value |
| Coupling trend history | Requires new storage model for coupling snapshots over time |
| Automated refactoring suggestions | Requires architectural knowledge beyond raw coupling data |
| Recursive directory scanning | Adds complexity; first-level scan is sufficient for standard layouts |
| Cross-repo file-level coupling | N^2 file pairs across repos is computationally prohibitive; repo-level coupling first |

---

## Dependencies

| Dependency | Status | Impact |
|------------|--------|--------|
| Existing Collector -> Snapshot pipeline | Stable | Snapshot collection per repo (FR-02) reuses existing code |
| RepoSnapshot struct (commits, authors) | Stable | Temporal and team coupling consume existing indexed data |
| rayon (parallelism) | Available | Already in Cargo.toml; used for parallel snapshot collection |
| clap subcommands | Available | Coupling subcommand follows existing pattern (Analyze, Init, Gate) |
| renderer/html.rs pattern | Stable | HTML visualization follows existing self-contained HTML pattern |
| toml crate | Available | Needed for Cargo.toml parsing in dependency coupling (R2) |

---

## Glossary

| Term | Definition |
|------|-----------|
| **Temporal coupling** | Two repos whose commits cluster together in time (within the coupling window) |
| **Team coupling** | Two repos that share the same commit authors |
| **Dependency coupling** | Two repos that share the same library dependencies or have direct dependency relationships |
| **Coupling window** | Maximum time gap (default 24h) between commits in different repos to count as a co-change |
| **Time window** | How far back in git history to look (default 6 months) |
| **Co-change** | A commit in repo A that occurs within the coupling window of a commit in repo B |
| **Blast radius** | Number of repos that depend on a given repo or library |
| **Single-author bridge** | One person who is the only shared committer between two repos (bus factor risk) |
| **Confidence** | Signal strength based on co-change sample size (HIGH/MEDIUM/LOW) |
