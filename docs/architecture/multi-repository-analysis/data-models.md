# Data Models -- Cross-Repository Coupling Detection

## Date
2026-03-26

---

## 1. Overview

The coupling feature introduces new data types in `coupling/types.rs`. These types are consumed by the coupling engine and renderers. They do NOT modify existing types (`RepoSnapshot`, `AnalysisReport`, etc.).

---

## 2. Core Types

### 2.1 CouplingConfig

Configuration for coupling analysis, derived from CLI args.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `root_dir` | `PathBuf` | (required) | Root directory to scan for repos |
| `time_window` | `TimeWindow` | 6 months | How far back in git history |
| `coupling_window` | `Duration` | 24h | Max gap between commits for co-change |
| `min_coupling` | `f64` | 30.0 | Minimum coupling % to display |
| `min_co_changes` | `usize` | 3 | Minimum co-changes to report a pair |
| `show_all` | `bool` | false | Show pairs below threshold |
| `output_format` | `OutputFormat` | Cli | Cli, Json, Html |
| `output_path` | `Option<PathBuf>` | None | Write to file instead of stdout |
| `pretty` | `bool` | false | Pretty-print JSON |
| `open` | `bool` | false | Open HTML in browser |
| `dimension_weights` | `DimensionWeights` | (50/25/25) | Weights for combined score |

### 2.2 OutputFormat

```
Cli | Json | Html
```

### 2.3 DimensionWeights

| Field | Type | Default |
|-------|------|---------|
| `temporal` | `f64` | 0.50 |
| `team` | `f64` | 0.25 |
| `dependency` | `f64` | 0.25 |

---

## 3. Discovery Types

### 3.1 DiscoveredRepo

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Directory name |
| `path` | `PathBuf` | Absolute path to repo |

### 3.2 SkippedRepo

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Directory name |
| `reason` | `SkipReason` | Why this directory was skipped |

### 3.3 SkipReason

```
NotAGitRepo | NoCommits | PermissionDenied(String) | CollectionFailed(String)
```

### 3.4 DiscoveryResult

| Field | Type | Description |
|-------|------|-------------|
| `repos` | `Vec<DiscoveredRepo>` | Valid repos found |
| `skipped` | `Vec<SkippedRepo>` | Directories skipped with reasons |

---

## 4. Coupling Result Types

### 4.1 Confidence

```
High | Medium | Low
```

Derived from co-change count: High (30+), Medium (10-29), Low (3-9).

### 4.2 TemporalCoupling

| Field | Type | Description |
|-------|------|-------------|
| `score` | `f64` | Percentage (0-100) |
| `co_changes` | `usize` | Number of co-changes detected |
| `confidence` | `Confidence` | Signal strength |

### 4.3 TeamCoupling (R2)

| Field | Type | Description |
|-------|------|-------------|
| `score` | `f64` | shared_authors / total_unique * 100 |
| `shared_authors` | `Vec<String>` | Names of shared authors |
| `is_single_bridge` | `bool` | True if exactly 1 shared author |
| `bridge_author` | `Option<String>` | Name of the single bridge author |

### 4.4 DependencyCoupling (R2)

| Field | Type | Description |
|-------|------|-------------|
| `shared_deps` | `Vec<String>` | Shared dependency names |
| `shared_count` | `usize` | Number of shared deps |
| `direct_dependency` | `Option<DependencyDirection>` | A depends on B or B depends on A |

### 4.5 DependencyDirection (R2)

| Field | Type | Description |
|-------|------|-------------|
| `from` | `String` | Dependent repo name |
| `to` | `String` | Dependency repo name |

### 4.6 CouplingPairResult

| Field | Type | Description |
|-------|------|-------------|
| `repo_a` | `String` | First repo name (alphabetically first) |
| `repo_b` | `String` | Second repo name |
| `temporal` | `Option<TemporalCoupling>` | Temporal coupling data |
| `team` | `Option<TeamCoupling>` | Team coupling data (R2) |
| `dependency` | `Option<DependencyCoupling>` | Dependency coupling data (R2) |
| `combined_score` | `Option<f64>` | Weighted combined score (R2, when all dims available) |

### 4.7 BlastRadiusEntry (R2)

| Field | Type | Description |
|-------|------|-------------|
| `dependency_name` | `String` | Name of the hub dependency |
| `consumer_repos` | `Vec<String>` | Repos that depend on it |
| `consumer_count` | `usize` | Number of consumers |

---

## 5. Report Types

### 5.1 CouplingReport

Top-level result of the coupling pipeline.

| Field | Type | Description |
|-------|------|-------------|
| `repos_scanned` | `usize` | Number of valid repos analyzed |
| `pairs_analyzed` | `usize` | Total pairs (N*(N-1)/2) |
| `time_window` | `TimeWindow` | Analysis time window |
| `coupling_window` | `Duration` | Coupling window used |
| `generated_at` | `DateTime<Utc>` | Timestamp of report generation |
| `pairs` | `Vec<CouplingPairResult>` | All coupling pairs above min_co_changes |
| `skipped_repos` | `Vec<SkippedRepo>` | Repos that were skipped |
| `blast_radius` | `Vec<BlastRadiusEntry>` | Hub dependencies with consumer counts (R2) |

### 5.2 Sorting and Filtering

Pairs are sorted by the primary display score descending:
- R1: `temporal.score`
- R2+: `combined_score` (when available), falling back to `temporal.score`

Pairs below `min_coupling` threshold are retained in the report but marked for renderer filtering (renderer decides whether to show based on `--all` flag).

---

## 6. JSON Schema (R2)

The JSON output mirrors `CouplingReport` under a `coupling` top-level key:

```
{
  "coupling": {
    "schema_version": 1,
    "repos_scanned": 22,
    "pairs_analyzed": 231,
    "time_window": { "since": "...", "until": "..." },
    "coupling_window": "24h",
    "generated_at": "2026-03-26T10:00:00Z",
    "pairs": [
      {
        "repo_a": "payment-gateway",
        "repo_b": "billing-service",
        "temporal": 78.0,
        "co_changes": 42,
        "confidence": "HIGH",
        "team": 43.0,
        "dependency": { "shared_count": 3, "shared_deps": [...] },
        "combined": 62.0
      }
    ],
    "blast_radius": [
      { "dependency": "shared-libs", "consumers": [...], "count": 5 }
    ]
  }
}
```

---

## 7. Relationship to Existing Types

| Existing Type | Usage in Coupling | Modified? |
|--------------|-------------------|-----------|
| `RepoSnapshot` | Consumed (commits, authors, path, name) | NO |
| `TimeWindow` | Reused for time window config | NO |
| `Author` | Name field used for team coupling matching | NO |
| `Commit` | Timestamp used for temporal coupling | NO |
| `AnalysisReport` | Not used by coupling pipeline | NO |
| `CouplingPair` (in scorer.rs) | Intra-repo only; coupling pipeline uses its own `CouplingPairResult` | NO |
