# Evolution: Multi-Repository Coupling Analysis

**Date**: 2026-03-27
**Feature**: `multi-repository-analysis`
**Status**: Delivered

---

## What Was Built

A new `coupling` subcommand that analyzes cross-repository dependencies across three dimensions: temporal, team, and dependency. The tool scans a root directory for git repositories and produces pairwise coupling scores, ranked by a weighted combined score.

### Capabilities Delivered

| Release | Capability | Status |
|---------|-----------|--------|
| R1 | Repo discovery + snapshot collection + temporal coupling + CLI output | ✓ |
| R2 | Team coupling + dependency coupling + combined scoring + JSON output | ✓ |
| R3 | HTML coupling matrix/graph visualization + dimension filtering | ✓ |

---

## Key Design Decisions

### Three-Dimension Coupling Score

Temporal (50%), team (25%), dependency (25%) weighted combination. Temporal gets the highest weight because commit co-change patterns are the most objective signal — team and dependency data can be noisy or sparse.

### Temporal Coupling Algorithm

Replaced the naive O(n² × m) pairwise approach with a merged-timeline binary search: all commits across all repos are sorted into a single timeline, then per commit we binary-search for neighbors within the coupling window. This is O(M log M + M × W_avg) where M = total commits and W_avg = average commits per window — critical for 50-repo analysis.

### Directed Co-Change Counts

For pair (A, B), both directions are counted independently: "commits in A with a B neighbor" and "commits in B with an A neighbor". The final co-change count takes the maximum of both directions, which is more conservative than summing (avoids double-counting) while still capturing asymmetric coupling patterns.

### Confidence Thresholds

- HIGH: 30+ co-changes
- MEDIUM: 10–29 co-changes
- LOW: 3–9 co-changes
- < 3: suppressed (insufficient signal)

### Author Normalization

Team coupling matches authors by case-insensitive display name rather than email, handling the common case of developers with different email addresses across repos.

### Dependency Parsing

Manifest parsers for Cargo.toml (TOML), package.json (JSON), go.mod (line-by-line, skip `indirect` and stdlib), and requirements.txt. Blast radius computed from dependency graph (repos depending on each shared library/repo).

---

## Architecture

```
coupling <root-dir>
    └── RepoDiscovery         — scans first-level subdirs for .git
    └── SnapshotCollector     — reuses existing collector pipeline (rayon)
    └── CouplingEngine
        ├── temporal.rs       — merged-timeline algorithm
        ├── team.rs           — author intersection / union
        ├── dependency.rs     — manifest parsing + blast radius
        └── scorer.rs         — weighted combination
    └── CouplingReport        — CouplingPair[] + DependencyAnalysis
    └── Renderers
        ├── CLI table          — ranked pairs, confidence badges
        ├── JSON              — schema_version: 1
        └── HTML              — matrix + graph + pairs + teams + deps tabs
```

New source modules: `src/coupling/` (6 files), `src/cli/coupling_args.rs`, `src/renderer/coupling_{cli,json,html}.rs`.

---

## Test Coverage

- **Unit**: coupling module functions isolated (temporal algorithm, team score, dependency parsing, combined scoring)
- **Integration** (`tests/coupling_milestone_1.rs`): 9 scenarios covering temporal filtering, team detection, dependency parsing, combined scoring, JSON output, HTML output
- **Walking skeleton** (`tests/coupling_walking_skeleton.rs`): end-to-end `barad-dur coupling` invocation on real temp git repos
- **Mutation gate**: 83.6% kill rate (112/134) — above 80% threshold

---

## What Was Intentionally Left Out

- GitLab/GitHub group scanning as input source (future enhancement)
- Cross-repo file-level coupling (N² file pairs is prohibitive)
- Coupling trend history (requires new storage model)
- Recursive directory scanning (first-level is sufficient for standard layouts)
- `.coupling-mailmap` author override config

---

## Files Added

### Source
- `src/coupling/mod.rs`
- `src/coupling/temporal.rs`
- `src/coupling/team.rs`
- `src/coupling/dependency.rs`
- `src/coupling/scorer.rs`
- `src/coupling/types.rs`
- `src/cli/coupling_args.rs`
- `src/renderer/coupling_cli.rs`
- `src/renderer/coupling_json.rs`
- `src/renderer/coupling_html.rs`

### Tests
- `tests/coupling_milestone_1.rs`
- `tests/coupling_walking_skeleton.rs`
