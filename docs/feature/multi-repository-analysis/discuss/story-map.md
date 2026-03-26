# Story Map: Cross-Repository Coupling Detection

## User: Adriana Kowalski (VP Eng) + Tomasz Wierzbicki (Platform Lead) + Yuki Tanaka (New Joiner)
## Goal: Detect temporal, team, and dependency coupling between repositories to identify which repos are too tightly bound

---

## Backbone

| Discover Repos | Collect Snapshots | Analyze Coupling | Render Results | Share Findings |
|----------------|-------------------|------------------|----------------|----------------|
| Scan root dir for git repos | Build RepoSnapshot per repo | Correlate commit timestamps (temporal) | Rank coupling pairs by score | Output to stdout, file, or browser |
| Skip non-repo dirs with reason | Use cache when available | Compute author overlap (team) | Show per-dimension breakdown | JSON format for CI integration |
| Report repo count and pair scope | Show progress during collection | Scan manifests (dependency) | Highlight bus factor risks | HTML interactive visualization |
| Validate minimum 2 repos | Handle collection failures | Compute combined score | Filter by threshold | |

---

## Walking Skeleton

Minimum end-to-end slice that connects all five activities:

1. **Discover**: `barad-dur coupling /path/to/repos/` scans subdirectories, finds 2+ git repos
2. **Collect**: Build RepoSnapshot per repo (reuse existing Collector pipeline), collect commit timestamps
3. **Analyze**: For each repo pair, count commits in repo A that occur within 24h of a commit in repo B. Compute temporal coupling percentage.
4. **Render**: CLI output shows ranked list of coupling pairs above 30% threshold
5. **Share**: Print to stdout (default)

This skeleton is verifiable: Adriana points at a directory with 5 repos and sees which pairs have correlated commit activity. No team coupling, no dependency scanning, no HTML, no JSON -- just temporal coupling pairs in the terminal.

---

## Release 1: Temporal Coupling Detection + CLI Output

**Target outcome**: Adriana can detect which repo pairs have temporally correlated commits by running one command.
**Estimated effort**: 5-7 days

Stories:
- US-01: Coupling subcommand discovers repos in root directory
- US-02: Snapshot collection with progress and skip-on-error
- US-03: Temporal coupling analysis across repo pairs
- US-04: CLI output with ranked coupling pairs

---

## Release 2: Team Coupling + Dependency Coupling + JSON Output

**Target outcome**: Tomasz can see shared authors, dependency blast radius, and export coupling data as JSON. Yuki can see that she is a single-author bridge.
**Estimated effort**: 5-7 days

Stories:
- US-05: Team coupling detection (shared authors, single-author bridges)
- US-06: Dependency coupling detection (manifest scanning, blast radius)
- US-07: JSON coupling output with versioned schema

---

## Release 3: HTML Visualization + GitLab Group Scan (future)

**Target outcome**: Adriana can generate an interactive coupling visualization for her architecture review. GitLab group scanning is on the roadmap as a future input source.
**Estimated effort**: 4-6 days

Stories:
- US-08: HTML coupling visualization (interactive graph + matrix)
- US-09: Dimension filtering in HTML (toggle temporal/team/dependency)

Note: GitLab group scan is tracked as a future enhancement, not a story in this feature.

---

## Scope Assessment: PASS -- 9 stories, 2 bounded contexts, estimated 14-20 days

**Bounded contexts**: (1) Cross-repo data collection and coupling analysis (discover, collect, analyze), (2) Coupling result rendering (CLI, JSON, HTML)

**Note**: This is within right-sizing limits. The 3-release slicing keeps each release under 7 days. Release 1 alone delivers verifiable value (temporal coupling detection). Releases 2 and 3 add dimensions and presentation quality.

**Brownfield evaluation**: The existing codebase already has:
- `Collector -> Snapshot` pipeline for git data collection (reusable per-repo)
- `file_change_pairs` in `RepoSnapshot` for intra-repo temporal coupling (conceptual pattern to extend)
- `temporal_coupling()` in `health.rs` (algorithm reference, not reusable directly)
- `renderer/html.rs` for self-contained HTML generation (pattern to follow for coupling HTML)
- `rayon` already in `Cargo.toml` for parallel processing

The primary new code is: repo discovery from root directory, cross-repo temporal correlation algorithm, author normalization for team coupling, manifest scanning for dependency coupling, and three renderers (CLI, JSON, HTML coupling visualization).
