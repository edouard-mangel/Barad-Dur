# Change Coupling Smells — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a "Change Coupling Smells" metric to the Coupling category that detects files in different architectural components that co-change frequently, surfacing architecture leakage.

**Architecture:** New `change_coupling_smells` function in `src/metrics/coupling.rs`; new `CouplingThresholds` struct in `src/config.rs`; `cross_boundary` flag added to `CouplingPair` in `src/scorer/types.rs`; "Cross-boundary" column added to HTML temporal coupling table.

**Tech Stack:** Rust, `std::path::Path`, existing `RepoSnapshot` fields (`file_change_pairs`, `commits_by_file`), serde/toml for config.

---

## File Map

| File | Change |
|------|--------|
| `src/config.rs` | Add `CouplingThresholds` + `coupling` field on `Thresholds`; update `validate` |
| `src/metrics/coupling.rs` | Add `extract_component` (pub(crate)), `change_coupling_smells`; update `compute_coupling` signature |
| `src/main.rs` | Update 2 `compute_coupling` call sites (lines ~282, ~637) to pass `&cfg.thresholds.coupling` |
| `src/scorer/types.rs` | Add `cross_boundary: bool` to `CouplingPair` |
| `src/scorer/builders.rs` | Update `build_coupling_pairs` to accept `component_depth: usize` and compute `cross_boundary` |
| `src/scorer.rs` | Update `build_report` to accept `component_depth: usize`; pass to `build_coupling_pairs` |

---

## Context

Tornhill's *Your Code as a Crime Scene* identifies **cross-boundary temporal coupling** as a key architecture smell: when files in different architectural components change together repeatedly, it signals hidden coupling that bypasses component boundaries. barad-dur already computes `file_change_pairs` (co-change pairs with ≥3 co-changes) and has a Coupling category with three metrics (afferent, efferent, circular). This design adds a fourth metric — **Change Coupling Smells** — that surfaces cross-boundary co-change pairs and scores the file accordingly.

## Architecture

New function `change_coupling_smells` in `src/metrics/coupling.rs`. Called from `compute_coupling` alongside the existing three metrics. Returns a `MetricResult` with score (100→25) and detail string. The HTML coupling tab gains a "Cross-boundary" column in the temporal coupling table.

No new files. Changes touch `src/metrics/coupling.rs`, `src/config.rs`, and the JS coupling renderer.

## Components

### 1. Component extraction helper

```rust
fn extract_component(path: &Path, depth: usize) -> String {
    path.components()
        .take(depth)
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
```

Falls back gracefully when path has fewer components than `depth` (returns all available components joined). Root-level files get their filename as the component.

**Reuses** the single-component pattern from `src/metrics/team.rs:287-291`.

### 2. `change_coupling_smells` metric function

Signature:
```rust
fn change_coupling_smells(
    snapshot: &RepoSnapshot,
    thresholds: &CouplingThresholds,
) -> MetricResult
```

Algorithm:
1. For each `(path_a, path_b, co_changes)` in `snapshot.file_change_pairs`:
   - Extract components for both paths at `thresholds.component_depth`
   - If same component → skip
   - Compute `commits_a = snapshot.commits_by_file.get(path_a).map_or(0, |v| v.len())`
   - Compute `commits_b = snapshot.commits_by_file.get(path_b).map_or(0, |v| v.len())`
   - If `min(commits_a, commits_b) == 0` → skip (no ratio possible)
   - If `co_changes as f64 / min(commits_a, commits_b) as f64 >= thresholds.change_coupling_min_ratio` → count as smell
2. Score by smell count: 0 → 100, 1–2 → 75, 3–5 → 50, >5 → 25

### 3. Config additions

In `src/config.rs`, add to `Thresholds`:
```rust
pub coupling: CouplingThresholds,
```

New struct:
```rust
#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
pub struct CouplingThresholds {
    pub component_depth: usize,           // default: 2
    pub change_coupling_min_ratio: f64,   // default: 0.30
}
```

`#[non_exhaustive]` required — same pattern as `HealthThresholds` — to prevent semver-check failures when future fields are added.

Validation: `component_depth == 0` → error; `change_coupling_min_ratio` outside `[0.0, 1.0]` → error.

### 4. HTML coupling tab

In the JS coupling renderer (temporal coupling table), add a "Cross-boundary" boolean column: `true` if the two files belong to different components at configured depth, `false` otherwise. Computed from the same `extract_component` logic. Allows users to visually filter for architecture leakage.

## Error Handling

- Missing `commits_by_file` entry → treat as 0 commits → pair excluded (no panic)
- Path with fewer components than `depth` → use all available components as the name
- `component_depth == 0` → rejected at config parse time with descriptive error
- `change_coupling_min_ratio` outside `[0.0, 1.0]` → rejected at config parse time

## Testing

Unit tests in `src/metrics/coupling.rs`:

| Test | Input | Expected |
|------|-------|----------|
| Same-component excluded | `src/a.rs` + `src/b.rs`, 5 co-changes, depth=2 | smell count = 0 |
| Cross-component included | `src/a.rs` + `tests/b.rs`, 5 co-changes, each in 10 commits | ratio = 0.5 ≥ 0.30 → count = 1 |
| Ratio below threshold | same cross-component pair, ratio = 0.20 | count = 0 |
| Missing commits entry | one file absent from `commits_by_file` | count = 0, no panic |
| Scoring: 0 smells | — | score = 100 |
| Scoring: 2 smells | — | score = 75 |
| Scoring: 4 smells | — | score = 50 |
| Scoring: 6 smells | — | score = 25 |
| depth=1 | `src/a.rs` + `src/b.rs` | both in `"src"` → excluded |
| depth=3 | `a/b/c/f.rs` + `a/b/d/f.rs` | components differ → included |

---

## Task 1: Add `CouplingThresholds` to `src/config.rs`

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block at the bottom of `src/config.rs`:

```rust
#[test]
fn load_coupling_thresholds_defaults() {
    let dir = TempDir::new().unwrap();
    let cfg = load(dir.path()).unwrap();
    assert_eq!(cfg.thresholds.coupling.component_depth, 2);
    assert!((cfg.thresholds.coupling.change_coupling_min_ratio - 0.30).abs() < f64::EPSILON);
}

#[test]
fn load_coupling_thresholds_from_toml() {
    let dir = TempDir::new().unwrap();
    let cache_dir = dir.path().join(".repository-analysis");
    fs::create_dir_all(&cache_dir).unwrap();
    fs::write(
        cache_dir.join("barad-dur.toml"),
        "[thresholds.coupling]\ncomponent_depth = 3\nchange_coupling_min_ratio = 0.50\n",
    )
    .unwrap();
    let cfg = load(dir.path()).unwrap();
    assert_eq!(cfg.thresholds.coupling.component_depth, 3);
    assert!((cfg.thresholds.coupling.change_coupling_min_ratio - 0.50).abs() < f64::EPSILON);
}

#[test]
fn validate_coupling_depth_zero_errors() {
    let mut cfg = RepoConfig::default();
    cfg.thresholds.coupling.component_depth = 0;
    let err = validate(&cfg).unwrap_err();
    assert!(err.to_string().contains("component_depth"));
}

#[test]
fn validate_coupling_ratio_out_of_range_errors() {
    let mut cfg = RepoConfig::default();
    cfg.thresholds.coupling.change_coupling_min_ratio = 1.5;
    let err = validate(&cfg).unwrap_err();
    assert!(err.to_string().contains("change_coupling_min_ratio"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p barad-dur load_coupling_thresholds 2>&1 | tail -5
cargo test -p barad-dur validate_coupling 2>&1 | tail -5
```
Expected: compile error — `CouplingThresholds` does not exist yet.

- [ ] **Step 3: Implement `CouplingThresholds`**

In `src/config.rs`, after the `HygieneThresholds` block (around line 205), add:

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
pub struct CouplingThresholds {
    #[serde(default = "default_component_depth")]
    pub component_depth: usize,
    #[serde(default = "default_change_coupling_min_ratio")]
    pub change_coupling_min_ratio: f64,
}

fn default_component_depth() -> usize {
    2
}
fn default_change_coupling_min_ratio() -> f64 {
    0.30
}

impl Default for CouplingThresholds {
    fn default() -> Self {
        Self {
            component_depth: default_component_depth(),
            change_coupling_min_ratio: default_change_coupling_min_ratio(),
        }
    }
}
```

- [ ] **Step 4: Add `coupling` field to `Thresholds`**

In `src/config.rs`, update the `Thresholds` struct (currently ends at `hygiene: HygieneThresholds`):

```rust
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Thresholds {
    #[serde(default)]
    pub health: HealthThresholds,
    #[serde(default)]
    pub team: TeamThresholds,
    #[serde(default)]
    pub evolution: EvolutionThresholds,
    #[serde(default)]
    pub hygiene: HygieneThresholds,
    #[serde(default)]
    pub coupling: CouplingThresholds,
}
```

- [ ] **Step 5: Update `validate()` to check coupling thresholds**

In `src/config.rs`, update the `validate` function:

```rust
pub fn validate(config: &RepoConfig) -> Result<()> {
    let sum = config.weights.sum();
    if sum != 100 {
        bail!(
            "Category weights must sum to 100, got {} (health={}, team={}, evolution={}, hygiene={}, coupling={})",
            sum,
            config.weights.health,
            config.weights.team,
            config.weights.evolution,
            config.weights.hygiene,
            config.weights.coupling,
        );
    }
    if config.thresholds.coupling.component_depth == 0 {
        bail!("thresholds.coupling.component_depth must be >= 1, got 0");
    }
    let ratio = config.thresholds.coupling.change_coupling_min_ratio;
    if !(0.0..=1.0).contains(&ratio) {
        bail!(
            "thresholds.coupling.change_coupling_min_ratio must be in [0.0, 1.0], got {}",
            ratio
        );
    }
    Ok(())
}
```

- [ ] **Step 6: Run tests to verify they pass**

```bash
cargo test -p barad-dur config 2>&1 | tail -10
```
Expected: all config tests pass including the 4 new ones.

- [ ] **Step 7: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): add CouplingThresholds with component_depth and change_coupling_min_ratio"
```

---

## Task 2: Add `change_coupling_smells` metric to `src/metrics/coupling.rs`

**Files:**
- Modify: `src/metrics/coupling.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `src/metrics/coupling.rs`:

```rust
use crate::config::CouplingThresholds;
use crate::snapshot::{CommitId, TimeWindow};

fn default_thresholds() -> CouplingThresholds {
    CouplingThresholds::default()
}

fn thresholds_with_depth(depth: usize) -> CouplingThresholds {
    CouplingThresholds {
        component_depth: depth,
        ..CouplingThresholds::default()
    }
}

fn thresholds_with_ratio(ratio: f64) -> CouplingThresholds {
    CouplingThresholds {
        change_coupling_min_ratio: ratio,
        ..CouplingThresholds::default()
    }
}

#[test]
fn extract_component_depth2() {
    let path = std::path::Path::new("src/metrics/coupling.rs");
    assert_eq!(extract_component(path, 2), "src/metrics");
}

#[test]
fn extract_component_depth1() {
    let path = std::path::Path::new("src/metrics/coupling.rs");
    assert_eq!(extract_component(path, 1), "src");
}

#[test]
fn extract_component_shallow_path() {
    // path has fewer components than depth — return all available
    let path = std::path::Path::new("main.rs");
    assert_eq!(extract_component(path, 2), "main.rs");
}

#[test]
fn change_coupling_same_component_excluded() {
    let mut snapshot = empty_snapshot();
    // Both files in src/ — same component at depth=2
    snapshot.file_change_pairs.push((
        PathBuf::from("src/a.rs"),
        PathBuf::from("src/b.rs"),
        5,
    ));
    snapshot.commits_by_file.insert(PathBuf::from("src/a.rs"), vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    snapshot.commits_by_file.insert(PathBuf::from("src/b.rs"), vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    let result = change_coupling_smells(&snapshot, &default_thresholds());
    assert_eq!(result.score, 100); // 0 smells
}

#[test]
fn change_coupling_cross_component_above_threshold_counted() {
    let mut snapshot = empty_snapshot();
    // src/a.rs and tests/b.rs — different components
    snapshot.file_change_pairs.push((
        PathBuf::from("src/a.rs"),
        PathBuf::from("tests/b.rs"),
        5, // co_changes
    ));
    // 10 commits each → ratio = 5/10 = 0.50 ≥ 0.30 → smell
    snapshot.commits_by_file.insert(PathBuf::from("src/a.rs"), vec![0,1,2,3,4,5,6,7,8,9]);
    snapshot.commits_by_file.insert(PathBuf::from("tests/b.rs"), vec![0,1,2,3,4,5,6,7,8,9]);
    let result = change_coupling_smells(&snapshot, &default_thresholds());
    assert_eq!(result.score, 75); // 1 smell → 75
}

#[test]
fn change_coupling_ratio_below_threshold_excluded() {
    let mut snapshot = empty_snapshot();
    snapshot.file_change_pairs.push((
        PathBuf::from("src/a.rs"),
        PathBuf::from("tests/b.rs"),
        2, // co_changes
    ));
    // 10 commits each → ratio = 2/10 = 0.20 < 0.30 → no smell
    snapshot.commits_by_file.insert(PathBuf::from("src/a.rs"), vec![0,1,2,3,4,5,6,7,8,9]);
    snapshot.commits_by_file.insert(PathBuf::from("tests/b.rs"), vec![0,1,2,3,4,5,6,7,8,9]);
    let result = change_coupling_smells(&snapshot, &default_thresholds());
    assert_eq!(result.score, 100); // 0 smells
}

#[test]
fn change_coupling_missing_commits_entry_excluded() {
    let mut snapshot = empty_snapshot();
    snapshot.file_change_pairs.push((
        PathBuf::from("src/a.rs"),
        PathBuf::from("tests/b.rs"),
        5,
    ));
    // No commits_by_file entries → min(0,0) == 0 → skip
    let result = change_coupling_smells(&snapshot, &default_thresholds());
    assert_eq!(result.score, 100);
}

#[test]
fn change_coupling_scoring_bands() {
    // 0 smells → 100
    let snap0 = empty_snapshot();
    assert_eq!(change_coupling_smells(&snap0, &default_thresholds()).score, 100);

    // 2 smells → 75
    let snap2 = make_cross_boundary_snapshot(2);
    assert_eq!(change_coupling_smells(&snap2, &default_thresholds()).score, 75);

    // 4 smells → 50
    let snap4 = make_cross_boundary_snapshot(4);
    assert_eq!(change_coupling_smells(&snap4, &default_thresholds()).score, 50);

    // 6 smells → 25
    let snap6 = make_cross_boundary_snapshot(6);
    assert_eq!(change_coupling_smells(&snap6, &default_thresholds()).score, 25);
}

// Helper: create snapshot with N cross-boundary pairs all above ratio threshold
fn make_cross_boundary_snapshot(n: usize) -> RepoSnapshot {
    let mut snapshot = empty_snapshot();
    for i in 0..n {
        let a = PathBuf::from(format!("src/f{}.rs", i));
        let b = PathBuf::from(format!("tests/f{}.rs", i));
        snapshot.file_change_pairs.push((a.clone(), b.clone(), 5));
        snapshot.commits_by_file.insert(a, vec![0,1,2,3,4,5,6,7,8,9]);
        snapshot.commits_by_file.insert(b, vec![0,1,2,3,4,5,6,7,8,9]);
    }
    snapshot
}

#[test]
fn change_coupling_depth1_same_component() {
    let mut snapshot = empty_snapshot();
    // depth=1: both in "src" → same component
    snapshot.file_change_pairs.push((
        PathBuf::from("src/a.rs"),
        PathBuf::from("src/b.rs"),
        5,
    ));
    snapshot.commits_by_file.insert(PathBuf::from("src/a.rs"), vec![0,1,2,3,4,5,6,7,8,9]);
    snapshot.commits_by_file.insert(PathBuf::from("src/b.rs"), vec![0,1,2,3,4,5,6,7,8,9]);
    let result = change_coupling_smells(&snapshot, &thresholds_with_depth(1));
    assert_eq!(result.score, 100);
}

#[test]
fn change_coupling_depth3_different_component() {
    let mut snapshot = empty_snapshot();
    // depth=3: a/b/c vs a/b/d → different
    snapshot.file_change_pairs.push((
        PathBuf::from("a/b/c/file.rs"),
        PathBuf::from("a/b/d/file.rs"),
        5,
    ));
    snapshot.commits_by_file.insert(PathBuf::from("a/b/c/file.rs"), vec![0,1,2,3,4,5,6,7,8,9]);
    snapshot.commits_by_file.insert(PathBuf::from("a/b/d/file.rs"), vec![0,1,2,3,4,5,6,7,8,9]);
    let result = change_coupling_smells(&snapshot, &thresholds_with_depth(3));
    assert_eq!(result.score, 75); // 1 smell
}

#[test]
fn compute_coupling_returns_four_metrics() {
    let snapshot = empty_snapshot();
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    assert_eq!(result.metrics.len(), 4);
    assert_eq!(result.name, "Coupling");
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p barad-dur change_coupling 2>&1 | tail -5
cargo test -p barad-dur extract_component 2>&1 | tail -5
```
Expected: compile error — functions not defined yet.

- [ ] **Step 3: Add imports to `src/metrics/coupling.rs`**

At the top of the file, add:
```rust
use std::path::Path;

use crate::config::CouplingThresholds;
```

(The existing `use std::collections::{HashMap, HashSet};` and `use std::path::PathBuf;` stay.)

- [ ] **Step 4: Add `extract_component` helper**

After the `median` function in `src/metrics/coupling.rs`, add:

```rust
/// Extract the first `depth` path components as a single string.
/// Falls back to fewer components if the path is shallower than `depth`.
pub(crate) fn extract_component(path: &Path, depth: usize) -> String {
    path.components()
        .take(depth)
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}
```

- [ ] **Step 5: Add `change_coupling_smells` function**

After `efferent_coupling` and before `circular_dependencies`, add:

```rust
/// Change coupling smells: files in different architectural components that
/// co-change frequently (ratio ≥ threshold).
///
/// Scored on the count of cross-boundary smell pairs:
///   0 → 100, 1–2 → 75, 3–5 → 50, >5 → 25
fn change_coupling_smells(
    snapshot: &RepoSnapshot,
    thresholds: &CouplingThresholds,
) -> MetricValue {
    let smell_count = snapshot
        .file_change_pairs
        .iter()
        .filter(|(path_a, path_b, co_changes)| {
            let comp_a = extract_component(path_a, thresholds.component_depth);
            let comp_b = extract_component(path_b, thresholds.component_depth);
            if comp_a == comp_b {
                return false;
            }
            let commits_a = snapshot
                .commits_by_file
                .get(path_a)
                .map_or(0, |v| v.len());
            let commits_b = snapshot
                .commits_by_file
                .get(path_b)
                .map_or(0, |v| v.len());
            let min_commits = commits_a.min(commits_b);
            if min_commits == 0 {
                return false;
            }
            (*co_changes as f64 / min_commits as f64) >= thresholds.change_coupling_min_ratio
        })
        .count();

    let score = match smell_count {
        0 => 100,
        1..=2 => 75,
        3..=5 => 50,
        _ => 25,
    };

    MetricValue {
        name: "Change coupling smells".to_string(),
        description: format!(
            "{} cross-boundary co-change pair(s) above {:.0}% ratio threshold",
            smell_count,
            thresholds.change_coupling_min_ratio * 100.0
        ),
        raw_value: RawValue::Count(smell_count),
        score,
    }
}
```

- [ ] **Step 6: Update `compute_coupling` signature**

Replace the current `compute_coupling` function in `src/metrics/coupling.rs`:

```rust
pub fn compute_coupling(snapshot: &RepoSnapshot, thresholds: &CouplingThresholds) -> CategoryResult {
    let metrics = vec![
        afferent_coupling(snapshot),
        efferent_coupling(snapshot),
        circular_dependencies(snapshot),
        change_coupling_smells(snapshot, thresholds),
    ];
    CategoryResult {
        name: "Coupling".to_string(),
        score: 0,
        metrics,
    }
    .compute_score()
}
```

- [ ] **Step 7: Run tests to verify they pass**

```bash
cargo test -p barad-dur coupling 2>&1 | tail -15
```
Expected: all coupling tests pass (including the 4 `compute_coupling_returns_four_metrics` test).

Note: the build will have compile errors in `src/main.rs` because the `compute_coupling` call sites don't pass thresholds yet. That's expected — fix those next.

- [ ] **Step 8: Commit**

```bash
git add src/metrics/coupling.rs
git commit -m "feat(coupling): add change_coupling_smells metric and extract_component helper"
```

---

## Task 3: Update call sites in `src/main.rs`

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Fix the `analyze` call site (~line 282)**

Find the line:
```rust
coupling::compute_coupling(&snapshot),
```
Replace with:
```rust
coupling::compute_coupling(&snapshot, &cfg.thresholds.coupling),
```

- [ ] **Step 2: Fix the `backfill` helper call site (~line 637)**

Find the line:
```rust
categories.push(coupling::compute_coupling(snapshot));
```
Replace with:
```rust
categories.push(coupling::compute_coupling(snapshot, &cfg.thresholds.coupling));
```

- [ ] **Step 3: Verify it compiles and tests pass**

```bash
cargo test 2>&1 | tail -10
```
Expected: all tests pass, no compile errors.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "fix: pass CouplingThresholds to compute_coupling call sites"
```

---

## Task 4: Add `cross_boundary` to coupling pair data

**Files:**
- Modify: `src/scorer/types.rs`
- Modify: `src/scorer/builders.rs`
- Modify: `src/scorer.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add `cross_boundary` field to `CouplingPair`**

In `src/scorer/types.rs`, update `CouplingPair`:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct CouplingPair {
    pub file_a: String,
    pub file_b: String,
    pub co_changes: usize,
    pub coupling_pct: f64,
    pub cross_boundary: bool,
}
```

- [ ] **Step 2: Update `build_coupling_pairs` to accept `component_depth` and compute `cross_boundary`**

In `src/scorer/builders.rs`, add import at the top:

```rust
use crate::metrics::coupling::extract_component;
```

Update the `build_coupling_pairs` signature and body:

```rust
pub(super) fn build_coupling_pairs(snapshot: &RepoSnapshot, component_depth: usize) -> Vec<CouplingPair> {
    snapshot
        .file_change_pairs
        .iter()
        .map(|(a, b, co)| {
            let a_changes = snapshot
                .commits_by_file
                .get(a)
                .map(|v| v.len())
                .unwrap_or(0);
            let b_changes = snapshot
                .commits_by_file
                .get(b)
                .map(|v| v.len())
                .unwrap_or(0);
            let min_changes = a_changes.min(b_changes).max(1);
            let coupling_pct = (*co as f64 / min_changes as f64 * 100.0).min(100.0);
            let cross_boundary =
                extract_component(a, component_depth) != extract_component(b, component_depth);
            CouplingPair {
                file_a: a.to_string_lossy().to_string(),
                file_b: b.to_string_lossy().to_string(),
                co_changes: *co,
                coupling_pct,
                cross_boundary,
            }
        })
        .collect()
}
```

- [ ] **Step 3: Update `build_report` to accept `component_depth`**

In `src/scorer.rs`, update the `build_report` function signature and the call to `build_coupling_pairs`:

```rust
pub fn build_report(
    snapshot: &RepoSnapshot,
    categories: Vec<CategoryResult>,
    remote_meta: Option<RemoteMeta>,
    weights: &[(&str, f64)],
    component_depth: usize,
) -> AnalysisReport {
    let overall_score = compute_overall_score_with_weights(&categories, weights);
    let top_actions = generate_top_actions(&categories);
    let file_hotspots = build_hotspots(snapshot);
    let coupling_pairs = build_coupling_pairs(snapshot, component_depth);
    // ... rest unchanged
```

- [ ] **Step 4: Update `build_report` call sites in `src/main.rs`**

There are two `build_report` calls in `src/main.rs`:

**Line 158** (backfill path):
```rust
// Before:
let mut report = scorer::build_report(&snapshot, categories, remote_meta, &weight_pairs);
// After:
let mut report = scorer::build_report(&snapshot, categories, remote_meta, &weight_pairs, cfg.thresholds.coupling.component_depth);
```

**Line 286** (analyze path):
```rust
// Before:
let report = scorer::build_report(&snapshot, categories, None, &weight_pairs);
// After:
let report = scorer::build_report(&snapshot, categories, None, &weight_pairs, cfg.thresholds.coupling.component_depth);
```

- [ ] **Step 5: Run all tests**

```bash
cargo test 2>&1 | tail -10
```
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/scorer/types.rs src/scorer/builders.rs src/scorer.rs src/main.rs
git commit -m "feat(scorer): add cross_boundary field to CouplingPair using configurable component depth"
```

---

## Task 5: Add "Cross-boundary" column to the HTML coupling table

**Files:**
- Modify: `src/renderer/html/js_coupling.rs`

- [ ] **Step 1: Add the column header**

In `src/renderer/html/js_coupling.rs`, find:

```javascript
['File A', 'File B', 'Co-changes', 'Coupling %', '', ''].forEach(function(h) {
```

Replace with:

```javascript
['File A', 'File B', 'Co-changes', 'Coupling %', 'Cross-boundary', '', ''].forEach(function(h) {
```

- [ ] **Step 2: Add the column cell in the row builder**

In the same file, find the row construction section. After the `pctCell` block (the `pctSpan` with coupling_pct coloring), add a new cell before `barCell`:

```javascript
        var cbCell = el('td');
        if (p.cross_boundary) {
          var cbBadge = el('span', { style: { color: '#f59e0b', fontWeight: '600', fontSize: '0.75rem' } });
          cbBadge.append(txt('\u26a0 cross-boundary'));
          cbCell.append(cbBadge);
        }
```

And update the row append line from:
```javascript
        row.append(aCell, bCell, coCell, pctCell, barCell, dismissCell);
```
to:
```javascript
        row.append(aCell, bCell, coCell, pctCell, cbCell, barCell, dismissCell);
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p barad-dur renderer 2>&1 | tail -10
cargo test -p barad-dur js_coupling 2>&1 | tail -10
```
Expected: all renderer tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/renderer/html/js_coupling.rs
git commit -m "feat(html): add Cross-boundary column to temporal coupling table"
```

---

## Task 6: Final verification

- [ ] **Step 1: Run full test suite**

```bash
cargo test 2>&1 | tail -15
```
Expected: all tests pass.

- [ ] **Step 2: Run clippy**

```bash
cargo clippy --all-targets -- -D warnings 2>&1 | tail -10
```
Expected: no warnings.

- [ ] **Step 3: Run fmt check**

```bash
cargo fmt -- --check
```
Expected: no diff.

- [ ] **Step 4: Self-analysis smoke test**

```bash
cargo run -- analyze . -v 2>&1 | grep -A 20 "Coupling"
```
Expected: Coupling category shows 4 metrics including "Change coupling smells".

- [ ] **Step 5: Verify cross-boundary column in HTML output**

```bash
cargo run -- analyze . --html -o /tmp/barad-test.html 2>&1
grep -c "cross-boundary" /tmp/barad-test.html
```
Expected: count > 0 (the badge text appears in the JS source within the HTML).

- [ ] **Step 6: Save design spec to project docs**

```bash
cp /home/edouard/.claude/plans/noble-gathering-moon.md /home/edouard/WS/tool/barad-dur/docs/plans/2026-04-08-change-coupling-smells-design.md
git add docs/plans/2026-04-08-change-coupling-smells-design.md
git commit -m "docs: add Change Coupling Smells design spec"
```
