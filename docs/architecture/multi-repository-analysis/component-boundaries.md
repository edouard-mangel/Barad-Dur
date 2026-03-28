# Component Boundaries -- Cross-Repository Coupling Detection

## Date
2026-03-26

---

## 1. Module Layout

```
src/
  coupling/
    mod.rs              -- Public API: run_coupling_analysis()
    types.rs            -- All coupling data types (CouplingReport, CouplingPairResult, etc.)
    discovery.rs        -- Scan root dir, validate repos
    collector.rs        -- Parallel snapshot collection with progress
    temporal.rs         -- Temporal coupling computation
    team.rs             -- Team coupling computation (R2)
    dependency.rs       -- Manifest scanning + dependency coupling (R2)
  renderer/
    coupling_cli.rs     -- CLI table output for CouplingReport
    coupling_json.rs    -- JSON output with versioned schema (R2)
    coupling_html.rs    -- Self-contained HTML visualization (R3)
  cli.rs                -- Extended with CouplingArgs + Commands::Coupling
  lib.rs                -- Extended with `pub mod coupling`
  main.rs               -- Extended with run_coupling() dispatch
```

---

## 2. Module Responsibilities

### 2.1 `coupling/mod.rs` -- Orchestrator

- Public entry point: `run_coupling_analysis(config: &CouplingConfig) -> Result<CouplingReport>`
- Calls discovery, collection, and analysis in sequence
- Aggregates results into `CouplingReport`
- No business logic here -- delegates to specialized modules

### 2.2 `coupling/types.rs` -- Data Model

- All types used across the coupling pipeline
- Derive: `Debug, Clone, Serialize, Deserialize`
- No behavior beyond `Display` implementations and trivial constructors
- This is the "port" that all other modules depend on

### 2.3 `coupling/discovery.rs` -- Repo Discovery

- Input: root directory path
- Output: `DiscoveryResult { repos, skipped }`
- Scans first-level subdirectories only (BR-01)
- Validates: `.git` exists, at least one commit, readable permissions
- Pure function over filesystem state

### 2.4 `coupling/collector.rs` -- Snapshot Collection

- Input: `Vec<DiscoveredRepo>`, `TimeWindow`
- Output: `Vec<(String, RepoSnapshot)>` (name + snapshot pairs), plus `Vec<SkippedRepo>` for failures
- Reuses existing `Collector::open()` and `collect_snapshot_verbose()`
- Parallel collection via rayon
- Progress bar via indicatif (showing repo name, count, ETA)
- Skip-on-error: wraps each collection in `catch_unwind` + `Result` handling
- **Performance optimization**: For coupling-only analysis, skip blame and complexity phases. Use `collect_snapshot_verbose(show_progress=false, verbose=false, skip_blame=true, no_cache=false, exclude_patterns=&[], use_default_excludes=true)`. This reduces per-repo collection to commits + file tree only.

### 2.5 `coupling/temporal.rs` -- Temporal Coupling Engine

- Input: `&[(String, RepoSnapshot)]`, `CouplingConfig`
- Output: `Vec<CouplingPairResult>` (with `temporal` field populated)
- Algorithm: for each pair (A, B), sort both commit lists by timestamp, then for each commit in A, binary search B's commits for any within coupling_window
- Deduplication: a B-commit matched by multiple A-commits counts once
- Score: `co_changes / min(commits_A, commits_B) * 100`
- Parallelized over pairs via rayon
- Pure function: takes immutable slices, returns new values

### 2.6 `coupling/team.rs` -- Team Coupling Engine (R2)

- Input: `&[(String, RepoSnapshot)]`
- Output: per-pair `TeamCoupling` added to existing `CouplingPairResult`
- Author matching: lowercase display name comparison
- Bus factor detection: single shared author flagged
- Pure function

### 2.7 `coupling/dependency.rs` -- Dependency Coupling Engine (R2)

- Input: `&[(String, RepoSnapshot)]` (uses `path` field for manifest location)
- Output: per-pair `DependencyCoupling` + `Vec<BlastRadiusEntry>`
- Scans: Cargo.toml, package.json, go.mod, requirements.txt
- Uses `toml` crate (already a dependency) for Cargo.toml
- Uses `serde_json` (already a dependency) for package.json
- Simple line parsing for go.mod and requirements.txt
- Pure function over filesystem state

### 2.8 `renderer/coupling_cli.rs` -- CLI Output

- Input: `&CouplingReport`, `&CouplingConfig`
- Output: `String` (formatted for terminal)
- Header: repo count, time window, coupling window, timestamp
- Ranked table: sorted by score descending
- Summary: high/medium/low counts
- Footer: skipped repos with reasons
- Truncation: repo names > 20 chars get "..." suffix
- 120-column terminal width constraint

### 2.9 `renderer/coupling_json.rs` -- JSON Output (R2)

- Input: `&CouplingReport`
- Output: `String` (JSON)
- Top-level `coupling` key with `schema_version: 1`
- Pretty-print option via `serde_json::to_string_pretty`

### 2.10 `renderer/coupling_html.rs` -- HTML Output (R3)

- Input: `&CouplingReport`
- Output: `String` (self-contained HTML)
- Follows `renderer/html.rs` pattern: inline CSS + JS, `window.R = <json>;`
- Tabs: Graph, Matrix, Pairs, Teams, Dependencies
- Dimension filtering via JS checkboxes

---

## 3. Boundary Rules

### 3.1 Dependency Constraints

| From | May Depend On | Must NOT Depend On |
|------|--------------|-------------------|
| `coupling/mod.rs` | `coupling/*`, `collector/mod.rs`, `snapshot.rs` | `renderer/*`, `scorer.rs`, `metrics/*` |
| `coupling/temporal.rs` | `coupling/types.rs`, `snapshot.rs` | Everything else |
| `coupling/team.rs` | `coupling/types.rs`, `snapshot.rs` | Everything else |
| `coupling/dependency.rs` | `coupling/types.rs`, `snapshot.rs` | Everything else |
| `coupling/discovery.rs` | `coupling/types.rs` | `snapshot.rs`, `collector/*` |
| `coupling/collector.rs` | `coupling/types.rs`, `collector/mod.rs`, `snapshot.rs` | `renderer/*`, `metrics/*` |
| `renderer/coupling_cli.rs` | `coupling/types.rs` | `coupling/mod.rs`, `collector/*`, `metrics/*` |
| `renderer/coupling_json.rs` | `coupling/types.rs` | Same as above |
| `renderer/coupling_html.rs` | `coupling/types.rs` | Same as above |
| ALL existing modules | Nothing in `coupling/*` | `coupling/*` (isolation) |

### 3.2 Interface Contract

The coupling pipeline has one public entry point:

```
coupling::run_coupling_analysis(config: &CouplingConfig) -> Result<CouplingReport>
```

Renderers have one function each:

```
renderer::coupling_cli::render(report: &CouplingReport, config: &CouplingConfig) -> String
renderer::coupling_json::render(report: &CouplingReport, pretty: bool) -> Result<String>
renderer::coupling_html::render(report: &CouplingReport) -> Result<String>
```

### 3.3 Existing Code Modifications (Minimal)

Only three existing files are modified, all additively:

1. **`cli.rs`**: Add `Commands::Coupling(CouplingArgs)` variant and `CouplingArgs` struct
2. **`lib.rs`**: Add `pub mod coupling;`
3. **`main.rs`**: Add `Commands::Coupling(args) => run_coupling(args)?` match arm and `run_coupling` function

No existing struct, enum, trait, or function is modified.

---

## 4. Release Boundaries

### R1 Files Created
- `coupling/mod.rs`
- `coupling/types.rs`
- `coupling/discovery.rs`
- `coupling/collector.rs`
- `coupling/temporal.rs`
- `renderer/coupling_cli.rs`

### R2 Files Created
- `coupling/team.rs`
- `coupling/dependency.rs`
- `renderer/coupling_json.rs`

### R3 Files Created
- `renderer/coupling_html.rs`

### Files Modified (R1, once)
- `cli.rs` (add CouplingArgs)
- `lib.rs` (add pub mod coupling)
- `main.rs` (add coupling dispatch)
