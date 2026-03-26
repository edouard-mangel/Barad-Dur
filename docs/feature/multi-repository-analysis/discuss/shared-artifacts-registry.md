# Shared Artifacts Registry -- Cross-Repository Coupling Detection

## Purpose

Every `${variable}` referenced in TUI mockups, Gherkin scenarios, and YAML schemas must have a single documented source. This registry prevents integration failures caused by mismatched data expectations.

---

## Artifacts

### ${root_dir}

| Property | Value |
|----------|-------|
| **Description** | Root directory to scan for git repositories |
| **Source** | CLI positional arg: `barad-dur coupling <root-dir>` |
| **Type** | `PathBuf` |
| **Consumers** | Repo discovery step |
| **Lifecycle** | Created at invocation, immutable during run |
| **Integration risk** | LOW -- single path, validated at invocation |

---

### ${discovered_repos}

| Property | Value |
|----------|-------|
| **Description** | Git repositories found in subdirectories of root_dir |
| **Source** | Directory scan: iterate subdirs, check for `.git` directory and at least 1 commit |
| **Type** | `Vec<DiscoveredRepo>` where `DiscoveredRepo { path: PathBuf, name: String }` |
| **Consumers** | Snapshot collection, pair count computation |
| **Integration risk** | LOW -- deterministic directory scan |

---

### ${skipped_repos}

| Property | Value |
|----------|-------|
| **Description** | Subdirectories that failed validation or collection, with reason |
| **Source** | Discovery step + Collection step (merged) |
| **Type** | `Vec<SkippedRepo>` where `SkippedRepo { path: String, name: String, reason: String }` |
| **Consumers** | CLI output footer, JSON output |
| **Integration risk** | MEDIUM -- skipped repos come from two phases (discovery + collection); must merge without duplicates |

---

### ${repo_snapshots}

| Property | Value |
|----------|-------|
| **Description** | Per-repo git metadata: commits with timestamps, authors, file changes |
| **Source** | Existing Collector -> Snapshot pipeline per repo |
| **Type** | `HashMap<String, RepoSnapshot>` -- repo name to snapshot |
| **Consumers** | Temporal coupling analyzer, team coupling analyzer |
| **Integration risk** | LOW -- reuses existing RepoSnapshot struct. Key fields: `commits` (with `committed_date`), `authors`, `commits_by_author` |
| **Dependency** | Existing `src/snapshot.rs` and `src/collector/mod.rs` |

---

### ${time_window}

| Property | Value |
|----------|-------|
| **Description** | How far back in git history to look for commits |
| **Source** | CLI `--window` flag or default (6 months) |
| **Type** | `Duration` or time window spec (e.g., "6months", "3months", "1year") |
| **Consumers** | Snapshot collection (filters commits), display in output header |
| **Integration risk** | LOW -- reuses existing TimeWindow from the analyze command |

---

### ${coupling_window}

| Property | Value |
|----------|-------|
| **Description** | Maximum time gap between commits in two repos to count as a "co-change" |
| **Source** | CLI `--coupling-window` flag or default (24 hours) |
| **Type** | `Duration` (e.g., 24h, 48h, 7d) |
| **Consumers** | Temporal coupling analyzer |
| **Integration risk** | LOW -- new parameter, no existing code depends on it |

---

### ${min_coupling}

| Property | Value |
|----------|-------|
| **Description** | Minimum coupling score to include in output |
| **Source** | CLI `--min-coupling` flag or default (30%) |
| **Type** | `f64` (0.0 to 100.0) |
| **Consumers** | Output filtering (CLI, JSON, HTML) |
| **Integration risk** | LOW -- display-only threshold |

---

### ${coupling_pairs}

| Property | Value |
|----------|-------|
| **Description** | Ranked list of repo pairs with coupling scores per dimension |
| **Source** | Pairwise coupling analysis across all valid repo combinations |
| **Type** | `Vec<CouplingPairResult>` where each contains: repo_a, repo_b, temporal_score, team_score (R2), dependency_score (R2), combined_score, co_change_count, confidence |
| **Consumers** | CLI renderer, JSON renderer, HTML renderer |
| **Integration risk** | MEDIUM -- central artifact consumed by all renderers; struct must be stable across releases. R1 has only temporal fields; R2 adds team and dependency. Design for extensibility. |

---

### ${team_bridges}

| Property | Value |
|----------|-------|
| **Description** | Authors who are the sole bridge between two repos (bus factor risk) |
| **Source** | Team coupling analyzer (R2) |
| **Type** | `Vec<TeamBridge>` where `TeamBridge { author: String, repo_a: String, repo_b: String, commits_a: usize, commits_b: usize }` |
| **Consumers** | CLI output (team coupling risks section), HTML teams tab |
| **Integration risk** | LOW -- computed from author overlap; only exists in R2+ |
| **Dependency** | Author normalization logic |

---

### ${dependency_map}

| Property | Value |
|----------|-------|
| **Description** | Directed dependency graph between repos (from manifest scanning) |
| **Source** | Manifest file parser (Cargo.toml, package.json, etc.) in R2 |
| **Type** | `HashMap<String, Vec<String>>` -- repo name to list of repos it depends on |
| **Consumers** | CLI output (dependency section), HTML dependencies tab |
| **Integration risk** | MEDIUM -- new code for manifest parsing; different ecosystems have different formats |
| **Dependency** | Manifest parser (new module) |

---

### ${output_format}

| Property | Value |
|----------|-------|
| **Description** | Desired output format for coupling report |
| **Source** | CLI flags: `--json`, `--html`, default=CLI |
| **Type** | `enum { Cli, Json, Html }` |
| **Consumers** | Renderer dispatch |
| **Integration risk** | LOW |

---

## Integration Checkpoint Matrix

| Producer Step | Artifact | Consumer Step | Validation |
|---------------|----------|---------------|------------|
| Discover | `${root_dir}` | Discover | Directory exists and is readable |
| Discover | `${discovered_repos}` | Collect | Non-empty list; each path has .git directory |
| Discover | `${skipped_repos}` (partial) | Render | Merged with collection failures later |
| Collect | `${repo_snapshots}` | Analyze | Each snapshot has commits with timestamps and authors |
| Collect | `${skipped_repos}` (complete) | Render | Merged: discovery skips + collection failures |
| Analyze | `${coupling_pairs}` | Render | Each pair has valid scores (0-100%) and co-change count |
| Analyze | `${team_bridges}` (R2) | Render | Each bridge has valid author name and commit counts |
| Analyze | `${dependency_map}` (R2) | Render | Each dependency is a valid repo name in discovered_repos |

---

## Data Flow Diagram

```
  CLI: barad-dur coupling <root-dir> [--window] [--coupling-window] [--min-coupling]
        |
        v
  [Discover: scan subdirs for git repos]
        |
        +---> skipped_repos (non-repo dirs)
        |
        v
  [Collect: build RepoSnapshot per repo]  <--- existing Collector pipeline
        |
        +---> skipped_repos (collection failures, merged)
        |
        v
  [Analyze: pairwise coupling computation]
        |
        +---> coupling_pairs (temporal: R1; + team, dependency: R2)
        +---> team_bridges (R2)
        +---> dependency_map (R2)
        |
        v
  [Render: CLI / JSON / HTML coupling report]
        |
        v
  [Export: stdout / file / browser]
```
