# Acceptance Criteria Summary -- Cross-Repository Coupling Detection

## Purpose

This file consolidates all acceptance criteria across user stories for quick reference during DESIGN and DELIVER waves.

---

## US-01: Coupling Subcommand Discovers Repos

| # | Criterion | Source Scenario |
|---|-----------|----------------|
| AC-01.1 | `barad-dur coupling` subcommand exists and accepts a positional directory argument | Happy path |
| AC-01.2 | Scans first-level subdirectories for `.git` directories with at least one commit | Discovery |
| AC-01.3 | Reports discovered count, skipped count, and pair scope | Happy path |
| AC-01.4 | Requires 2+ valid repos; exits code 1 with helpful message for 0 or 1 | Single repo, no repos |
| AC-01.5 | Skipped repos show specific reason (not a repo, no commits, permission denied) | Error paths |

## US-02: Snapshot Collection with Progress

| # | Criterion | Source Scenario |
|---|-----------|----------------|
| AC-02.1 | Progress bar displayed when collecting 3+ repos | Happy path |
| AC-02.2 | Current repo name and completed/total count shown | Progress display |
| AC-02.3 | ETA displayed after first uncached repo completes | ETA display |
| AC-02.4 | Cache used per-repo (stale check via HEAD commit SHA) | Cached repos |
| AC-02.5 | Failed repos logged as warning; collection continues | Collection failure |
| AC-02.6 | Final summary shows collected count, failed count, total time | Summary |

## US-03: Temporal Coupling Analysis

| # | Criterion | Source Scenario |
|---|-----------|----------------|
| AC-03.1 | Temporal coupling computed for all valid repo pairs | Happy path |
| AC-03.2 | Score = co_changes / min(commits_A, commits_B) * 100 | Score computation |
| AC-03.3 | Coupling window configurable via `--coupling-window` (default 24h) | Custom window |
| AC-03.4 | Confidence levels: HIGH (30+), MEDIUM (10-29), LOW (3-9) | Confidence |
| AC-03.5 | Pairs with fewer than 3 co-changes excluded | Minimum threshold |
| AC-03.6 | Pairs ranked by score descending | Ranking |
| AC-03.7 | Progress shown during pairwise analysis | Progress |

## US-04: CLI Output with Ranked Pairs

| # | Criterion | Source Scenario |
|---|-----------|----------------|
| AC-04.1 | Header shows repo count, time window, coupling window, timestamp | Header |
| AC-04.2 | Ranked table sorted by coupling score descending | Ranking |
| AC-04.3 | Each row shows rank, repo A, repo B, score, co-changes, confidence | Row content |
| AC-04.4 | Summary shows high/medium/low counts | Summary |
| AC-04.5 | Pairs below threshold hidden with count and --all hint | Threshold filtering |
| AC-04.6 | Skipped repos listed at bottom with reasons | Skipped |
| AC-04.7 | Output readable in 120-column terminal; names truncated at 20 chars | Readability |

## US-05: Team Coupling (Shared Authors)

| # | Criterion | Source Scenario |
|---|-----------|----------------|
| AC-05.1 | Team coupling score = shared_authors / total_unique_authors * 100 | Score computation |
| AC-05.2 | Shared authors listed by name for each pair | Author listing |
| AC-05.3 | Single-author bridges flagged as bus factor risk | Bridge detection |
| AC-05.4 | Author matching uses display name (case-insensitive) | Normalization |
| AC-05.5 | Team coupling integrated into ranked output alongside temporal | Integration |

## US-06: Dependency Coupling (Manifest Scanning)

| # | Criterion | Source Scenario |
|---|-----------|----------------|
| AC-06.1 | Manifest files scanned: Cargo.toml, package.json, go.mod, requirements.txt | Manifest types |
| AC-06.2 | Shared dependencies identified per repo pair | Shared deps |
| AC-06.3 | Dependency direction detected (A depends on B) | Direction |
| AC-06.4 | Blast radius computed per hub dependency | Blast radius |
| AC-06.5 | Missing manifests handled gracefully | Missing manifest |
| AC-06.6 | Dependency coupling integrated into ranked output | Integration |

## US-07: JSON Coupling Output

| # | Criterion | Source Scenario |
|---|-----------|----------------|
| AC-07.1 | `--json` produces JSON with `coupling` top-level key | JSON output |
| AC-07.2 | Schema includes repos_scanned, pairs_analyzed, time_window, coupling_window, schema_version, generated_at, pairs | Schema |
| AC-07.3 | Each pair has repo_a, repo_b, temporal, co_changes, confidence, optional team/dependency/combined | Per-pair |
| AC-07.4 | schema_version is integer 1 | Version |
| AC-07.5 | `--pretty` produces indented JSON | Pretty print |
| AC-07.6 | `barad-dur analyze . --json` output unchanged | Backward compat |

## US-08: HTML Coupling Visualization

| # | Criterion | Source Scenario |
|---|-----------|----------------|
| AC-08.1 | `--html` produces self-contained HTML (no external deps) | Self-containment |
| AC-08.2 | Contains tabs: Graph, Matrix, Pairs, Teams, Dependencies | Tab structure |
| AC-08.3 | Graph shows repos as nodes, coupling as edges with thickness/color | Graph |
| AC-08.4 | Matrix shows repos x repos with color-coded cells | Matrix |
| AC-08.5 | Dimension filtering checkboxes | Filtering |
| AC-08.6 | `--open` generates and opens in default browser | Auto-open |
| AC-08.7 | Follows existing HTML renderer pattern | Consistency |

## US-09: Dimension Filtering in HTML

| # | Criterion | Source Scenario |
|---|-----------|----------------|
| AC-09.1 | Three checkboxes: Temporal, Team, Dependencies (all checked by default) | Default state |
| AC-09.2 | Unchecking a dimension hides its edges from graph | Graph filtering |
| AC-09.3 | Matrix updates to show only selected dimensions | Matrix filtering |
| AC-09.4 | Pairs list re-filters to selected dimensions | List filtering |
| AC-09.5 | Changes are instant (no page reload) | Performance |

---

## Cross-Cutting Acceptance Criteria

| # | Criterion | Applies To |
|---|-----------|------------|
| CC-01 | Ctrl-C exits cleanly; no partial files or orphan temp dirs | US-02, US-03 |
| CC-02 | `barad-dur analyze` single-repo behavior completely unchanged | All stories |
| CC-03 | Memory usage under 500 MB for 50-repo analysis | US-03 |
| CC-04 | JSON schema_version field present and set to 1 | US-07 |
| CC-05 | Existing RepoSnapshot struct not modified | US-02 |
| CC-06 | HTML visualization works offline (self-contained) | US-08 |
