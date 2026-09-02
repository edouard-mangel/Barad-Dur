# Review Field-Test Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the field-test harness that runs Barad-dûr across 11 pinned real repositories, diffs the recommendations it produces against committed baselines, proves the analysis is deterministic, and emits an audit worksheet — then write the process documents that surround it.

**Architecture:** A `field_test` module of pure functions in the lib crate (corpus loading, decision-surface extraction, surface diffing, audit sampling), plus thin I/O shells (git worktree guard, analysis runner). Driven by a `field-test` binary that is feature-gated so it never ships with `cargo install`. The harness reads the report as `serde_json::Value` rather than deserializing into `AnalysisReport`, because that type is `Serialize`-only and `#[non_exhaustive]` — and because reading the JSON as a downstream consumer would is precisely what the harness is validating.

**Tech Stack:** Rust 2021, `serde` / `serde_json` (already deps), `toml` 0.8 (already a dep), `tempfile` 3 (already a dep), `anyhow` (already a dep). Git worktrees via the `git` CLI. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-01-review-process-design.md`

## Global Constraints

- Functional style per `CLAUDE.md`: pure functions, iterator chains over mutable loops, `?` propagation over explicit `match`, immutable bindings unless `mut` is required.
- `RUSTFLAGS=-D warnings cargo test` must pass — this is how CI runs.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt -- --check` must pass. The pre-push hook enforces both plus `cargo install --path .`.
- TDD is mandatory. Write the failing test, run it, watch it fail for the right reason, then implement. No exceptions — this project has a recorded correction for skipping it.
- **The harness must never mutate a corpus repository's working tree.** `barad-dur analyze` creates `.repository-analysis/` in its target and appends to that target's `.gitignore`. All analysis therefore happens inside a throwaway `git worktree`.
- Score band thresholds come from `scorer/types.rs`; never hardcode 71/41.
- New integration test files follow the existing naming: `tests/field_test_walking_skeleton.rs`, then `tests/field_test_milestone_N.rs`.

---

## File Structure

| File | Responsibility |
|---|---|
| `src/field_test/mod.rs` | Module root; re-exports the public surface |
| `src/field_test/corpus.rs` | `CorpusEntry`, parsing `field-test/corpus.toml`, resolving repo paths |
| `src/field_test/surface.rs` | `DecisionSurface` + extraction from a report `Value` |
| `src/field_test/diff.rs` | Comparing two `DecisionSurface`s into a readable report |
| `src/field_test/baseline.rs` | Reading/writing `field-test/baselines/<name>.surface.json` |
| `src/field_test/worktree.rs` | RAII guard: `git worktree add --detach` / `remove --force` |
| `src/field_test/runner.rs` | Orchestration: worktree → analyze → extract, single and double pass |
| `src/field_test/audit.rs` | Audit worksheet generation and rotation state |
| `src/bin/field-test.rs` | CLI entry point (`run`, `accept`, `audit`) |
| `field-test/corpus.toml` | The committed corpus manifest |
| `field-test/baselines/*.surface.json` | Committed baselines |
| `docs/review-process.md` | P0/P1, evidence contract, minors policy, escape accounting |

---

### Task 1: Corpus manifest

**Files:**
- Create: `src/field_test/mod.rs`, `src/field_test/corpus.rs`
- Create: `field-test/corpus.toml`
- Modify: `src/lib.rs` (add `pub mod field_test;`)
- Test: unit tests inside `src/field_test/corpus.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub struct CorpusEntry { pub name: String, pub path: String, pub pin: String, pub lang: String }`, `pub fn parse_corpus(toml_src: &str) -> anyhow::Result<Vec<CorpusEntry>>`, `pub fn resolve_path(entry: &CorpusEntry, root: &Path) -> PathBuf`.

- [ ] **Step 1: Write the failing test**

In `src/field_test/corpus.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parses_entries_in_declaration_order() {
        let src = r#"
[[repo]]
name = "ripgrep"
path = "ripgrep"
pin  = "3fce3b5b"
lang = "Rust"

[[repo]]
name = "mautic"
path = "mautic"
pin  = "181701cd"
lang = "PHP"
"#;
        let entries = parse_corpus(src).expect("parses");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "ripgrep");
        assert_eq!(entries[0].pin, "3fce3b5b");
        assert_eq!(entries[1].lang, "PHP");
    }

    #[test]
    fn resolves_path_against_corpus_root() {
        let entry = CorpusEntry {
            name: "ripgrep".into(),
            path: "ripgrep".into(),
            pin: "3fce3b5b".into(),
            lang: "Rust".into(),
        };
        assert_eq!(
            resolve_path(&entry, Path::new("/home/edouard/WS")),
            Path::new("/home/edouard/WS/ripgrep")
        );
    }

    #[test]
    fn rejects_a_manifest_with_no_repos() {
        assert!(parse_corpus("").is_err());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib field_test::corpus -- --nocapture`
Expected: FAIL to compile — `parse_corpus`, `CorpusEntry`, `resolve_path` not found.

- [ ] **Step 3: Write minimal implementation**

`src/field_test/mod.rs`:

```rust
//! Field-test harness: runs Barad-dûr across a pinned corpus of real
//! repositories and diffs the recommendations it produces.
//!
//! Not part of the shipped product; the `field-test` binary that drives
//! this module is gated behind the `field-test` cargo feature.

pub mod corpus;
```

`src/field_test/corpus.rs`:

```rust
use anyhow::{bail, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// One pinned repository in the field-test corpus.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CorpusEntry {
    pub name: String,
    /// Path relative to the corpus root.
    pub path: String,
    /// Commit the analysis is pinned to. Unpinned repos drift and every
    /// diff becomes noise.
    pub pin: String,
    pub lang: String,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(default)]
    repo: Vec<CorpusEntry>,
}

/// Parse `field-test/corpus.toml`.
pub fn parse_corpus(toml_src: &str) -> Result<Vec<CorpusEntry>> {
    let manifest: Manifest = toml::from_str(toml_src)?;
    if manifest.repo.is_empty() {
        bail!("corpus manifest declares no [[repo]] entries");
    }
    Ok(manifest.repo)
}

/// Absolute path to a corpus entry's repository.
pub fn resolve_path(entry: &CorpusEntry, root: &Path) -> PathBuf {
    root.join(&entry.path)
}
```

Add to `src/lib.rs`, keeping the list alphabetical:

```rust
pub mod field_test;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib field_test::corpus`
Expected: 3 passed.

- [ ] **Step 5: Write the corpus manifest**

`field-test/corpus.toml` — every pin below was measured on 2026-09-01:

```toml
# Field-test corpus. Paths are relative to $BARAD_DUR_CORPUS_ROOT
# (default: $HOME/WS). Pins are mandatory: unpinned repos drift and
# every diff becomes noise.

[[repo]]
name = "barad-dur"
path = "barad-dur"
pin  = "73ebdf3e"
lang = "Rust"

[[repo]]
name = "ripgrep"
path = "ripgrep"
pin  = "3fce3b5b"
lang = "Rust"

[[repo]]
name = "helix"
path = "helix"
pin  = "079a789e"
lang = "Rust"

[[repo]]
name = "starship"
path = "starship"
pin  = "e939a19a"
lang = "Rust"

[[repo]]
name = "dotnet-starter-kit"
path = "dotnet-starter-kit"
pin  = "b21bdd93"
lang = "CSharp"

[[repo]]
name = "evolutionary-architecture-by-example"
path = "evolutionary-architecture-by-example"
pin  = "536af586"
lang = "CSharp"

[[repo]]
name = "eShopModernizing"
path = "eShopModernizing"
pin  = "63bc9ec4"
lang = "CSharp"

[[repo]]
name = "App-Serveat"
path = "App-Serveat"
pin  = "6fcfa756"
lang = "CSharp"

[[repo]]
name = "payp-app-front"
path = "payp-app-front"
pin  = "2260b980"
lang = "TypeScript"

[[repo]]
name = "kairis-crm"
path = "kairis-crm"
pin  = "663493ef"
lang = "TypeScript"

[[repo]]
name = "mautic"
path = "mautic"
pin  = "181701cd"
lang = "PHP"
```

- [ ] **Step 6: Add a test that the real manifest parses**

```rust
    #[test]
    fn the_committed_manifest_parses_and_pins_every_repo() {
        let src = include_str!("../../field-test/corpus.toml");
        let entries = parse_corpus(src).expect("committed manifest parses");
        assert_eq!(entries.len(), 11);
        assert!(
            entries.iter().all(|e| e.pin.len() >= 7),
            "every corpus entry must carry a pin"
        );
    }
```

- [ ] **Step 7: Run the full suite and commit**

Run: `RUSTFLAGS=-D warnings cargo test --lib field_test`
Expected: 4 passed.

```bash
git add src/lib.rs src/field_test/mod.rs src/field_test/corpus.rs field-test/corpus.toml
git commit -m "feat(field-test): corpus manifest with pinned repositories"
```

---

### Task 2: Decision surface extraction

**Files:**
- Create: `src/field_test/surface.rs`
- Modify: `src/field_test/mod.rs` (add `pub mod surface;`)
- Test: unit tests inside `src/field_test/surface.rs`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: `DecisionSurface`, `CategorySurface`, `MetricSurface`, `ActionSurface`, and `pub fn extract_surface(report: &serde_json::Value) -> DecisionSurface`.

**Why these fields:** measured from a real report. `history` is excluded because it carries a per-run `timestamp` and would make every sweep diff. `raw_value` and `description` are excluded because they restate the score in prose and would diff on wording. `score: null` means *unscored* and must survive — reporting a fabricated `100` instead of unscored is the exact regression class this catches.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_report() -> serde_json::Value {
        json!({
            "overall_score": 55,
            "total_files": 203,
            "total_commits": 2287,
            "total_authors": 27,
            "score_thresholds": { "good_min": 71, "warn_min": 41 },
            "coupling_finding_counts": {
                "common": 4, "content": 2, "control": 5, "inheritance": 0
            },
            "categories": [
                { "name": "Health", "score": 40, "metrics": [
                    { "name": "Bus factor", "score": 25,
                      "description": "prose", "raw_value": { "Count": 1 } },
                    { "name": "Churn-ownership risk", "score": null,
                      "description": "prose", "raw_value": null }
                ]}
            ],
            "top_actions": [
                { "target_tab": "ownership", "text": "[Team] Knowledge distribution" }
            ],
            "coupling_actions": [
                { "target_tab": "coupling", "text": "[Coupling] mod.rs — 2 finding(s)" }
            ],
            "file_hotspots": [
                { "path": "crates/ignore/src/walk.rs", "hotspot_score": 65.1 },
                { "path": "crates/core/main.rs", "hotspot_score": 40.0 }
            ],
            "history": [ { "timestamp": "2026-09-01T12:00:00Z", "head": "abc" } ]
        })
    }

    #[test]
    fn captures_scores_counts_and_thresholds() {
        let s = extract_surface(&sample_report());
        assert_eq!(s.overall_score, 55);
        assert_eq!(s.total_commits, 2287);
        assert_eq!(s.score_thresholds.get("good_min"), Some(&71));
        assert_eq!(s.coupling_finding_counts.get("control"), Some(&5));
    }

    #[test]
    fn preserves_unscored_metrics_as_none() {
        let s = extract_surface(&sample_report());
        let health = &s.categories[0];
        assert_eq!(health.metrics[0].score, Some(25));
        assert_eq!(
            health.metrics[1].score, None,
            "an unscored metric must stay unscored, never become a number"
        );
    }

    #[test]
    fn merges_both_action_lists_in_deterministic_order() {
        let s = extract_surface(&sample_report());
        assert_eq!(s.actions.len(), 2);
        assert_eq!(s.actions[0].target_tab, "coupling");
        assert_eq!(s.actions[1].target_tab, "ownership");
    }

    #[test]
    fn keeps_hotspot_rank_order_and_caps_at_twenty() {
        let s = extract_surface(&sample_report());
        assert_eq!(s.top_hotspots, vec![
            "crates/ignore/src/walk.rs".to_string(),
            "crates/core/main.rs".to_string(),
        ]);
    }

    #[test]
    fn excludes_the_volatile_history_field() {
        let s = extract_surface(&sample_report());
        let encoded = serde_json::to_string(&s).expect("serializes");
        assert!(
            !encoded.contains("timestamp"),
            "history carries a per-run timestamp and must not reach the baseline"
        );
    }

    #[test]
    fn serializes_deterministically() {
        let a = serde_json::to_string(&extract_surface(&sample_report())).unwrap();
        let b = serde_json::to_string(&extract_surface(&sample_report())).unwrap();
        assert_eq!(a, b);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib field_test::surface`
Expected: FAIL to compile — `extract_surface` not found.

- [ ] **Step 3: Write minimal implementation**

`src/field_test/surface.rs`:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// How many hotspots enter the baseline. Beyond the top of the ranking the
/// ordering is noise rather than signal.
const HOTSPOT_LIMIT: usize = 20;

/// The part of a report that represents a decision the tool is making.
/// Deliberately smaller than the full report: a baseline that diffs on
/// every run is a baseline everybody learns to ignore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionSurface {
    pub overall_score: i64,
    pub total_files: i64,
    pub total_commits: i64,
    pub total_authors: i64,
    pub score_thresholds: BTreeMap<String, i64>,
    pub coupling_finding_counts: BTreeMap<String, i64>,
    pub categories: Vec<CategorySurface>,
    pub actions: Vec<ActionSurface>,
    pub top_hotspots: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategorySurface {
    pub name: String,
    pub score: Option<i64>,
    pub metrics: Vec<MetricSurface>,
}

/// `score: None` means *unscored* — a metric the analysis could not
/// compute. Collapsing it to a number is the regression this guards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricSurface {
    pub name: String,
    pub score: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ActionSurface {
    pub target_tab: String,
    pub text: String,
}

fn int_at(report: &Value, key: &str) -> i64 {
    report.get(key).and_then(Value::as_i64).unwrap_or_default()
}

fn int_map(report: &Value, key: &str) -> BTreeMap<String, i64> {
    report
        .get(key)
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| v.as_i64().map(|n| (k.clone(), n)))
                .collect()
        })
        .unwrap_or_default()
}

fn actions_from(report: &Value, key: &str) -> Vec<ActionSurface> {
    report
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|a| ActionSurface {
                    target_tab: a
                        .get("target_tab")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    text: a
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Reduce a full report to the decisions it expresses.
pub fn extract_surface(report: &Value) -> DecisionSurface {
    let categories = report
        .get("categories")
        .and_then(Value::as_array)
        .map(|cats| {
            cats.iter()
                .map(|c| CategorySurface {
                    name: c
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    score: c.get("score").and_then(Value::as_i64),
                    metrics: c
                        .get("metrics")
                        .and_then(Value::as_array)
                        .map(|ms| {
                            ms.iter()
                                .map(|m| MetricSurface {
                                    name: m
                                        .get("name")
                                        .and_then(Value::as_str)
                                        .unwrap_or_default()
                                        .to_string(),
                                    score: m.get("score").and_then(Value::as_i64),
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();

    // Both action lists are recommendations to a human; sorting makes the
    // merge order independent of how the renderer happened to emit them.
    let actions = {
        let mut merged = actions_from(report, "top_actions");
        merged.extend(actions_from(report, "coupling_actions"));
        merged.sort();
        merged
    };

    let top_hotspots = report
        .get("file_hotspots")
        .and_then(Value::as_array)
        .map(|hs| {
            hs.iter()
                .take(HOTSPOT_LIMIT)
                .filter_map(|h| h.get("path").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    DecisionSurface {
        overall_score: int_at(report, "overall_score"),
        total_files: int_at(report, "total_files"),
        total_commits: int_at(report, "total_commits"),
        total_authors: int_at(report, "total_authors"),
        score_thresholds: int_map(report, "score_thresholds"),
        coupling_finding_counts: int_map(report, "coupling_finding_counts"),
        categories,
        actions,
        top_hotspots,
    }
}
```

Add `pub mod surface;` to `src/field_test/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `RUSTFLAGS=-D warnings cargo test --lib field_test::surface`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add src/field_test/mod.rs src/field_test/surface.rs
git commit -m "feat(field-test): extract the decision surface from a report"
```

---

### Task 3: Surface diff

**Files:**
- Create: `src/field_test/diff.rs`
- Modify: `src/field_test/mod.rs` (add `pub mod diff;`)
- Test: unit tests inside `src/field_test/diff.rs`

**Interfaces:**
- Consumes: `DecisionSurface`, `CategorySurface`, `MetricSurface`, `ActionSurface` from Task 2.
- Produces: `pub struct SurfaceDiff { pub changes: Vec<String> }`, `impl SurfaceDiff { pub fn is_empty(&self) -> bool; pub fn render(&self) -> String }`, `pub fn diff_surfaces(baseline: &DecisionSurface, current: &DecisionSurface) -> SurfaceDiff`.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::field_test::surface::{
        ActionSurface, CategorySurface, DecisionSurface, MetricSurface,
    };
    use std::collections::BTreeMap;

    fn surface(overall: i64, metric_score: Option<i64>, action: &str) -> DecisionSurface {
        DecisionSurface {
            overall_score: overall,
            total_files: 10,
            total_commits: 100,
            total_authors: 3,
            score_thresholds: BTreeMap::new(),
            coupling_finding_counts: BTreeMap::new(),
            categories: vec![CategorySurface {
                name: "Health".into(),
                score: Some(40),
                metrics: vec![MetricSurface {
                    name: "Bus factor".into(),
                    score: metric_score,
                }],
            }],
            actions: vec![ActionSurface {
                target_tab: "ownership".into(),
                text: action.into(),
            }],
            top_hotspots: vec!["a.rs".into()],
        }
    }

    #[test]
    fn identical_surfaces_produce_no_changes() {
        let a = surface(55, Some(25), "do the thing");
        assert!(diff_surfaces(&a, &a).is_empty());
    }

    #[test]
    fn reports_a_changed_overall_score() {
        let d = diff_surfaces(&surface(55, Some(25), "x"), &surface(60, Some(25), "x"));
        assert!(!d.is_empty());
        assert!(d.render().contains("overall_score"), "got: {}", d.render());
        assert!(d.render().contains("55"));
        assert!(d.render().contains("60"));
    }

    #[test]
    fn reports_a_metric_that_stopped_being_unscored() {
        let d = diff_surfaces(&surface(55, None, "x"), &surface(55, Some(100), "x"));
        let text = d.render();
        assert!(text.contains("Bus factor"), "got: {text}");
        assert!(text.contains("unscored"), "got: {text}");
    }

    #[test]
    fn reports_added_and_removed_recommendations() {
        let d = diff_surfaces(&surface(55, Some(25), "old advice"), &surface(55, Some(25), "new advice"));
        let text = d.render();
        assert!(text.contains("- old advice"), "got: {text}");
        assert!(text.contains("+ new advice"), "got: {text}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib field_test::diff`
Expected: FAIL to compile — `diff_surfaces` not found.

- [ ] **Step 3: Write minimal implementation**

`src/field_test/diff.rs`:

```rust
use crate::field_test::surface::DecisionSurface;
use std::collections::BTreeSet;

/// A human-readable account of how two decision surfaces differ.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SurfaceDiff {
    pub changes: Vec<String>,
}

impl SurfaceDiff {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn render(&self) -> String {
        self.changes.join("\n")
    }
}

fn show(score: Option<i64>) -> String {
    score.map_or_else(|| "unscored".to_string(), |n| n.to_string())
}

fn scalar(label: &str, before: i64, after: i64) -> Option<String> {
    (before != after).then(|| format!("  {label}: {before} -> {after}"))
}

/// Compare a committed baseline against a freshly measured surface.
pub fn diff_surfaces(baseline: &DecisionSurface, current: &DecisionSurface) -> SurfaceDiff {
    let scalars = [
        scalar("overall_score", baseline.overall_score, current.overall_score),
        scalar("total_files", baseline.total_files, current.total_files),
        scalar("total_commits", baseline.total_commits, current.total_commits),
        scalar("total_authors", baseline.total_authors, current.total_authors),
    ];

    let metric_changes = baseline.categories.iter().flat_map(|b| {
        let matching = current.categories.iter().find(|c| c.name == b.name);
        b.metrics.iter().filter_map(move |bm| {
            let cm = matching?.metrics.iter().find(|m| m.name == bm.name)?;
            (cm.score != bm.score).then(|| {
                format!(
                    "  metric {}/{}: {} -> {}",
                    matching?.name,
                    bm.name,
                    show(bm.score),
                    show(cm.score)
                )
            })
        })
    });

    let before: BTreeSet<_> = baseline.actions.iter().collect();
    let after: BTreeSet<_> = current.actions.iter().collect();
    let removed = before
        .difference(&after)
        .map(|a| format!("  - {} [{}]", a.text, a.target_tab));
    let added = after
        .difference(&before)
        .map(|a| format!("  + {} [{}]", a.text, a.target_tab));

    let hotspots = (baseline.top_hotspots != current.top_hotspots)
        .then(|| "  top_hotspots ranking changed".to_string());

    let changes = scalars
        .into_iter()
        .flatten()
        .chain(metric_changes)
        .chain(removed)
        .chain(added)
        .chain(hotspots)
        .collect();

    SurfaceDiff { changes }
}
```

Note: the `matching?` inside the closure requires the closure to return
`Option<String>`; if the borrow checker objects to the nested `?`, bind
`let cat_name = &b.name;` before the inner closure and use it instead.

Add `pub mod diff;` to `src/field_test/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `RUSTFLAGS=-D warnings cargo test --lib field_test::diff`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add src/field_test/mod.rs src/field_test/diff.rs
git commit -m "feat(field-test): diff two decision surfaces"
```

---

### Task 4: Baseline store

**Files:**
- Create: `src/field_test/baseline.rs`
- Modify: `src/field_test/mod.rs` (add `pub mod baseline;`)
- Test: unit tests inside `src/field_test/baseline.rs`

**Interfaces:**
- Consumes: `DecisionSurface` from Task 2.
- Produces: `pub fn baseline_path(dir: &Path, name: &str) -> PathBuf`, `pub fn write_baseline(dir: &Path, name: &str, s: &DecisionSurface) -> Result<()>`, `pub fn read_baseline(dir: &Path, name: &str) -> Result<Option<DecisionSurface>>`.

`read_baseline` returns `Ok(None)` when no baseline exists yet, so a newly
added corpus entry seeds rather than fails.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::field_test::surface::DecisionSurface;
    use std::collections::BTreeMap;

    fn empty_surface() -> DecisionSurface {
        DecisionSurface {
            overall_score: 55,
            total_files: 1,
            total_commits: 2,
            total_authors: 3,
            score_thresholds: BTreeMap::new(),
            coupling_finding_counts: BTreeMap::new(),
            categories: vec![],
            actions: vec![],
            top_hotspots: vec![],
        }
    }

    #[test]
    fn round_trips_a_surface() {
        let dir = tempfile::tempdir().expect("tempdir");
        let s = empty_surface();
        write_baseline(dir.path(), "ripgrep", &s).expect("writes");
        let back = read_baseline(dir.path(), "ripgrep").expect("reads");
        assert_eq!(back, Some(s));
    }

    #[test]
    fn missing_baseline_reads_as_none_so_new_entries_seed() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(read_baseline(dir.path(), "brand-new").expect("reads"), None);
    }

    #[test]
    fn writes_pretty_json_so_diffs_are_reviewable_in_git() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_baseline(dir.path(), "ripgrep", &empty_surface()).expect("writes");
        let raw = std::fs::read_to_string(baseline_path(dir.path(), "ripgrep")).expect("read");
        assert!(raw.contains('\n'), "baseline must be multi-line to diff well");
        assert!(raw.ends_with('\n'), "baseline must end with a newline");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib field_test::baseline`
Expected: FAIL to compile — `write_baseline` not found.

- [ ] **Step 3: Write minimal implementation**

```rust
use crate::field_test::surface::DecisionSurface;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Where a repository's committed baseline lives.
pub fn baseline_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.surface.json"))
}

/// Write a baseline as pretty JSON with a trailing newline — these are
/// reviewed as git diffs, so readability is the point.
pub fn write_baseline(dir: &Path, name: &str, surface: &DecisionSurface) -> Result<()> {
    std::fs::create_dir_all(dir)
        .with_context(|| format!("creating baseline dir {}", dir.display()))?;
    let path = baseline_path(dir, name);
    let body = format!("{}\n", serde_json::to_string_pretty(surface)?);
    std::fs::write(&path, body).with_context(|| format!("writing {}", path.display()))
}

/// Read a baseline. `Ok(None)` means this repository has never been
/// baselined, so the caller should seed instead of failing.
pub fn read_baseline(dir: &Path, name: &str) -> Result<Option<DecisionSurface>> {
    let path = baseline_path(dir, name);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    Ok(Some(serde_json::from_str(&raw)?))
}
```

Add `pub mod baseline;` to `src/field_test/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `RUSTFLAGS=-D warnings cargo test --lib field_test::baseline`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add src/field_test/mod.rs src/field_test/baseline.rs
git commit -m "feat(field-test): read and write committed baselines"
```

---

### Task 5: Worktree guard

**Files:**
- Create: `src/field_test/worktree.rs`
- Modify: `src/field_test/mod.rs` (add `pub mod worktree;`)
- Test: `tests/field_test_walking_skeleton.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub struct Worktree { .. }` with `pub fn add(repo: &Path, pin: &str, dir: &Path) -> Result<Worktree>` and `pub fn path(&self) -> &Path`; removal happens in `Drop`.

**Why this exists:** `barad-dur analyze` creates `.repository-analysis/` in its target and appends to that target's `.gitignore`. Analysing a corpus repo directly dirties it. The worktree is what makes the harness incapable of that.

- [ ] **Step 1: Write the failing test**

`tests/field_test_walking_skeleton.rs`:

```rust
use barad_dur::field_test::worktree::Worktree;
use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git runs");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Build a throwaway repo with two commits and return (dir, first_sha).
fn fixture_repo() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    git(p, &["config", "user.email", "t@example.com"]);
    git(p, &["config", "user.name", "Test"]);
    std::fs::write(p.join("a.txt"), "one").expect("write");
    git(p, &["add", "."]);
    git(p, &["commit", "-qm", "first"]);
    let first = git(p, &["rev-parse", "HEAD"]);
    std::fs::write(p.join("a.txt"), "two").expect("write");
    git(p, &["commit", "-qam", "second"]);
    (dir, first)
}

#[test]
fn worktree_checks_out_the_pin_and_cleans_up_after_itself() {
    let (repo, first) = fixture_repo();
    let scratch = tempfile::tempdir().expect("tempdir");
    let wt_dir = scratch.path().join("wt");

    {
        let wt = Worktree::add(repo.path(), &first, &wt_dir).expect("worktree added");
        assert_eq!(
            std::fs::read_to_string(wt.path().join("a.txt")).expect("read"),
            "one",
            "worktree must be checked out at the pinned commit, not HEAD"
        );
    }

    assert!(!wt_dir.exists(), "worktree directory removed on drop");
    assert_eq!(
        git(repo.path(), &["status", "--short"]),
        "",
        "the source repository must be left untouched"
    );
}

#[test]
fn worktree_removal_survives_a_dirtied_working_tree() {
    let (repo, first) = fixture_repo();
    let scratch = tempfile::tempdir().expect("tempdir");
    let wt_dir = scratch.path().join("wt");

    {
        let wt = Worktree::add(repo.path(), &first, &wt_dir).expect("worktree added");
        // barad-dur analyze does exactly this to its target.
        std::fs::write(wt.path().join(".gitignore"), ".repository-analysis/\n").expect("write");
        std::fs::create_dir_all(wt.path().join(".repository-analysis")).expect("mkdir");
    }

    assert!(!wt_dir.exists(), "a dirtied worktree must still be removed");
    assert_eq!(git(repo.path(), &["status", "--short"]), "");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test field_test_walking_skeleton`
Expected: FAIL to compile — `Worktree` not found.

- [ ] **Step 3: Write minimal implementation**

```rust
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// A throwaway `git worktree` checked out at a pinned commit.
///
/// `barad-dur analyze` writes into the repository it analyses — it creates
/// `.repository-analysis/` and appends to `.gitignore`. Running it against a
/// real repository therefore dirties that repository. Every analysis in the
/// harness runs inside one of these instead, and it is removed on drop.
#[derive(Debug)]
pub struct Worktree {
    repo: PathBuf,
    dir: PathBuf,
}

fn run_git(repo: &Path, args: &[&str]) -> Result<()> {
    let out = Command::new("git").current_dir(repo).args(args).output()?;
    if !out.status.success() {
        bail!(
            "git {} failed in {}: {}",
            args.join(" "),
            repo.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

impl Worktree {
    /// Check `pin` out into `dir` as a detached worktree of `repo`.
    pub fn add(repo: &Path, pin: &str, dir: &Path) -> Result<Self> {
        let dir_s = dir.to_string_lossy().to_string();
        run_git(repo, &["worktree", "add", "--detach", "--quiet", &dir_s, pin])?;
        Ok(Self {
            repo: repo.to_path_buf(),
            dir: dir.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Worktree {
    fn drop(&mut self) {
        // --force because the analysis dirties the worktree by design.
        let dir_s = self.dir.to_string_lossy().to_string();
        let _ = run_git(&self.repo, &["worktree", "remove", "--force", &dir_s]);
        let _ = run_git(&self.repo, &["worktree", "prune"]);
    }
}
```

Add `pub mod worktree;` to `src/field_test/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `RUSTFLAGS=-D warnings cargo test --test field_test_walking_skeleton`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add src/field_test/mod.rs src/field_test/worktree.rs tests/field_test_walking_skeleton.rs
git commit -m "feat(field-test): throwaway worktree isolates analysis from corpus repos"
```

---

### Task 6: Analysis runner with determinism double-run

**Files:**
- Create: `src/field_test/runner.rs`
- Modify: `src/field_test/mod.rs` (add `pub mod runner;`)
- Test: unit tests in `src/field_test/runner.rs`

**Interfaces:**
- Consumes: `CorpusEntry`/`resolve_path` (Task 1), `extract_surface` (Task 2), `diff_surfaces` (Task 3), `Worktree` (Task 5).
- Produces: `pub struct RepoOutcome { pub name: String, pub surface: DecisionSurface, pub nondeterminism: Option<SurfaceDiff> }` and `pub fn analyze_pinned(binary: &Path, repo: &Path, pin: &str, scratch: &Path, passes: u8) -> Result<RepoOutcome>`.

`passes == 2` runs the analysis twice and records any difference between the
two runs as `nondeterminism`. Ordering bugs have already cost this project an
Important; the check is mechanical and needs no judgement.

- [ ] **Step 1: Write the failing test**

Determinism logic is pure and testable without running an analysis:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::field_test::surface::DecisionSurface;
    use std::collections::BTreeMap;

    fn surface(score: i64) -> DecisionSurface {
        DecisionSurface {
            overall_score: score,
            total_files: 1,
            total_commits: 1,
            total_authors: 1,
            score_thresholds: BTreeMap::new(),
            coupling_finding_counts: BTreeMap::new(),
            categories: vec![],
            actions: vec![],
            top_hotspots: vec![],
        }
    }

    #[test]
    fn two_identical_passes_report_no_nondeterminism() {
        let outcome = outcome_from_passes("ripgrep", vec![surface(55), surface(55)]);
        assert!(outcome.nondeterminism.is_none());
        assert_eq!(outcome.surface.overall_score, 55);
    }

    #[test]
    fn differing_passes_are_flagged_as_nondeterminism() {
        let outcome = outcome_from_passes("ripgrep", vec![surface(55), surface(56)]);
        let nd = outcome.nondeterminism.expect("nondeterminism detected");
        assert!(nd.render().contains("overall_score"));
    }

    #[test]
    fn a_single_pass_cannot_report_nondeterminism() {
        let outcome = outcome_from_passes("ripgrep", vec![surface(55)]);
        assert!(outcome.nondeterminism.is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib field_test::runner`
Expected: FAIL to compile — `outcome_from_passes` not found.

- [ ] **Step 3: Write minimal implementation**

```rust
use crate::field_test::diff::{diff_surfaces, SurfaceDiff};
use crate::field_test::surface::{extract_surface, DecisionSurface};
use crate::field_test::worktree::Worktree;
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

/// The result of analysing one corpus repository.
#[derive(Debug, Clone)]
pub struct RepoOutcome {
    pub name: String,
    pub surface: DecisionSurface,
    /// `Some` when two passes over identical input disagreed.
    pub nondeterminism: Option<SurfaceDiff>,
}

/// Fold N measured passes into an outcome. Pure, so determinism handling is
/// testable without running an analysis.
pub fn outcome_from_passes(name: &str, passes: Vec<DecisionSurface>) -> RepoOutcome {
    let nondeterminism = passes
        .first()
        .zip(passes.get(1))
        .map(|(a, b)| diff_surfaces(a, b))
        .filter(|d| !d.is_empty());

    RepoOutcome {
        name: name.to_string(),
        surface: passes.into_iter().next().unwrap_or_else(|| {
            unreachable!("callers always supply at least one pass")
        }),
        nondeterminism,
    }
}

fn analyze_once(binary: &Path, target: &Path, out: &Path) -> Result<DecisionSurface> {
    let status = Command::new(binary)
        .arg("analyze")
        .arg(target)
        .arg("--json")
        .arg("--no-cache")
        .arg("-o")
        .arg(out)
        .status()
        .with_context(|| format!("running {}", binary.display()))?;
    if !status.success() {
        bail!("analysis of {} failed", target.display());
    }
    let raw = std::fs::read_to_string(out)
        .with_context(|| format!("reading report {}", out.display()))?;
    Ok(extract_surface(&serde_json::from_str(&raw)?))
}

/// Analyse `repo` at `pin` inside a throwaway worktree, `passes` times.
pub fn analyze_pinned(
    binary: &Path,
    name: &str,
    repo: &Path,
    pin: &str,
    scratch: &Path,
    passes: u8,
) -> Result<RepoOutcome> {
    let measured = (0..passes.max(1))
        .map(|i| {
            let wt_dir = scratch.join(format!("{name}-{i}"));
            let wt = Worktree::add(repo, pin, &wt_dir)?;
            analyze_once(binary, wt.path(), &scratch.join(format!("{name}-{i}.json")))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(outcome_from_passes(name, measured))
}
```

Add `pub mod runner;` to `src/field_test/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `RUSTFLAGS=-D warnings cargo test --lib field_test::runner`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add src/field_test/mod.rs src/field_test/runner.rs
git commit -m "feat(field-test): pinned analysis runner with determinism double-run"
```

---

### Task 7: Audit worksheet

**Files:**
- Create: `src/field_test/audit.rs`
- Modify: `src/field_test/mod.rs` (add `pub mod audit;`)
- Test: unit tests in `src/field_test/audit.rs`

**Interfaces:**
- Consumes: `ActionSurface`, `DecisionSurface` (Task 2).
- Produces: `pub fn select_for_audit(baseline: &DecisionSurface, current: &DecisionSurface, already_seen: &BTreeSet<String>, rotation: usize) -> Vec<ActionSurface>` and `pub fn render_worksheet(repo: &str, items: &[ActionSurface]) -> String`.

**Why this is not a diff:** BD-001 was found on a first-ever run of a repo
with no baseline. A pure regression gate would have written that advice into
the baseline and passed clean forever. Audit mode samples *all* new or changed
recommendations plus up to `rotation` previously-unseen ones, so pre-existing
bad advice eventually surfaces.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::field_test::surface::{ActionSurface, DecisionSurface};
    use std::collections::{BTreeMap, BTreeSet};

    fn with_actions(texts: &[&str]) -> DecisionSurface {
        DecisionSurface {
            overall_score: 1,
            total_files: 1,
            total_commits: 1,
            total_authors: 1,
            score_thresholds: BTreeMap::new(),
            coupling_finding_counts: BTreeMap::new(),
            categories: vec![],
            actions: texts
                .iter()
                .map(|t| ActionSurface { target_tab: "x".into(), text: (*t).into() })
                .collect(),
            top_hotspots: vec![],
        }
    }

    #[test]
    fn always_includes_every_new_recommendation() {
        let picked = select_for_audit(
            &with_actions(&["old"]),
            &with_actions(&["old", "brand new"]),
            &BTreeSet::new(),
            0,
        );
        assert!(picked.iter().any(|a| a.text == "brand new"));
    }

    #[test]
    fn rotates_through_unseen_pre_existing_recommendations() {
        let seen: BTreeSet<String> = ["a".to_string()].into_iter().collect();
        let picked = select_for_audit(
            &with_actions(&["a", "b", "c"]),
            &with_actions(&["a", "b", "c"]),
            &seen,
            2,
        );
        let texts: Vec<_> = picked.iter().map(|a| a.text.as_str()).collect();
        assert!(!texts.contains(&"a"), "already audited");
        assert_eq!(texts.len(), 2, "rotation slice is bounded");
    }

    #[test]
    fn rotation_slice_is_bounded_even_when_much_is_unseen() {
        let picked = select_for_audit(
            &with_actions(&["a", "b", "c", "d", "e", "f"]),
            &with_actions(&["a", "b", "c", "d", "e", "f"]),
            &BTreeSet::new(),
            5,
        );
        assert_eq!(picked.len(), 5);
    }

    #[test]
    fn worksheet_carries_the_true_safe_actionable_rubric() {
        let sheet = render_worksheet("ripgrep", &with_actions(&["do a thing"]).actions);
        assert!(sheet.contains("do a thing"));
        assert!(sheet.contains("True"));
        assert!(sheet.contains("Safe"));
        assert!(sheet.contains("Actionable"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib field_test::audit`
Expected: FAIL to compile — `select_for_audit` not found.

- [ ] **Step 3: Write minimal implementation**

```rust
use crate::field_test::surface::{ActionSurface, DecisionSurface};
use std::collections::BTreeSet;

/// Pick the recommendations a reviewer must read this merge: every one that
/// is new or changed, plus a bounded rotating slice of ones never audited.
pub fn select_for_audit(
    baseline: &DecisionSurface,
    current: &DecisionSurface,
    already_seen: &BTreeSet<String>,
    rotation: usize,
) -> Vec<ActionSurface> {
    let before: BTreeSet<&ActionSurface> = baseline.actions.iter().collect();

    let fresh: Vec<ActionSurface> = current
        .actions
        .iter()
        .filter(|a| !before.contains(*a))
        .cloned()
        .collect();

    let fresh_texts: BTreeSet<&str> = fresh.iter().map(|a| a.text.as_str()).collect();

    let rotated = current
        .actions
        .iter()
        .filter(|a| !fresh_texts.contains(a.text.as_str()))
        .filter(|a| !already_seen.contains(&a.text))
        .take(rotation)
        .cloned();

    fresh.iter().cloned().chain(rotated).collect()
}

/// Render the worksheet a reviewer fills in. The rubric is not decoration:
/// BD-001 passed "True" and failed only "Safe".
pub fn render_worksheet(repo: &str, items: &[ActionSurface]) -> String {
    let header = format!(
        "## {repo}\n\n\
         For each recommendation below, answer all three. \
         Any **Safe** failure blocks the merge; \
         **True** and **Actionable** failures become tickets.\n\n\
         | # | Recommendation | True? | Safe? | Actionable? | Notes |\n\
         |---|---|---|---|---|---|\n"
    );
    let rows = items.iter().enumerate().map(|(i, a)| {
        let text = a.text.replace('|', "\\|");
        format!("| {} | {} | | | | |\n", i + 1, text)
    });
    header.chars().chain(rows.collect::<String>().chars()).collect()
}
```

Add `pub mod audit;` to `src/field_test/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `RUSTFLAGS=-D warnings cargo test --lib field_test::audit`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add src/field_test/mod.rs src/field_test/audit.rs
git commit -m "feat(field-test): audit sampling and True/Safe/Actionable worksheet"
```

---

### Task 8: The `field-test` binary

**Files:**
- Create: `src/bin/field-test.rs`
- Modify: `Cargo.toml` (add `[features]` and `[[bin]]`)
- Modify: `.gitignore` (ignore `field-test/archive/`)
- Test: `tests/field_test_milestone_1.rs`

**Interfaces:**
- Consumes: everything from Tasks 1–7.
- Produces: a binary with three subcommands — `run`, `accept`, `audit`. Exit codes: `0` clean, `1` differences found, `2` harness error.

**Why feature-gated:** `cargo install --path .` runs in the pre-push hook and
installs every declared binary. `required-features` keeps dev tooling out of
what users install.

- [ ] **Step 1: Add the feature and bin declaration**

In `Cargo.toml`, after `[dependencies]`:

```toml
[features]
field-test = []

[[bin]]
name = "field-test"
path = "src/bin/field-test.rs"
required-features = ["field-test"]
```

**Probed 2026-09-01, do not "fix" this.** An explicit `[[bin]]` section does
**not** disable cargo's auto-discovery of `src/main.rs`, so the default
`barad-dur` binary must *not* be declared here. Verified on a scratch crate
with this exact layout:

```
$ cargo build                      # default binary still discovered
  targets built: target/debug/binprobe
$ cargo build --features field-test
  targets built: target/debug/binprobe target/debug/field-test
```

`required-features` gating behaves as intended: without the feature the
harness binary is not built, so `cargo install --path .` in the pre-push hook
will not install it.

- [ ] **Step 2: Write the failing test**

`tests/field_test_milestone_1.rs`:

```rust
use barad_dur::field_test::corpus::parse_corpus;

#[test]
fn the_committed_corpus_covers_every_language_the_spec_requires() {
    let entries = parse_corpus(include_str!("../field-test/corpus.toml")).expect("parses");
    let langs: std::collections::BTreeSet<_> =
        entries.iter().map(|e| e.lang.as_str()).collect();
    for required in ["Rust", "CSharp", "TypeScript", "PHP"] {
        assert!(langs.contains(required), "corpus is missing {required}");
    }
}

#[test]
fn rust_is_represented_by_more_than_our_own_repository() {
    let entries = parse_corpus(include_str!("../field-test/corpus.toml")).expect("parses");
    let foreign_rust = entries
        .iter()
        .filter(|e| e.lang == "Rust" && e.name != "barad-dur")
        .count();
    assert!(
        foreign_rust >= 2,
        "self-dogfooding on a tidy repo is what this corpus exists to fix"
    );
}
```

- [ ] **Step 3: Run test to verify it fails, then passes**

Run: `cargo test --test field_test_milestone_1`
Expected: PASS if Task 1's manifest is complete. If it fails, the manifest is
missing a language — fix `field-test/corpus.toml`.

- [ ] **Step 4: Write the binary**

`src/bin/field-test.rs`:

```rust
//! Field-test harness driver. Not shipped: gated behind the `field-test`
//! cargo feature so `cargo install` never picks it up.

use anyhow::{Context, Result};
use barad_dur::field_test::{
    audit::{render_worksheet, select_for_audit},
    baseline::{read_baseline, write_baseline},
    corpus::{parse_corpus, resolve_path},
    diff::diff_surfaces,
    runner::analyze_pinned,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const ROTATION: usize = 5;

fn corpus_root() -> PathBuf {
    std::env::var_os("BARAD_DUR_CORPUS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var("HOME").expect("HOME is set")).join("WS")
        })
}

fn main() -> Result<()> {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "run".to_string());
    let root = corpus_root();
    let baselines = Path::new("field-test/baselines");
    let archive = Path::new("field-test/archive");
    std::fs::create_dir_all(archive)?;

    let entries = parse_corpus(
        &std::fs::read_to_string("field-test/corpus.toml")
            .context("reading field-test/corpus.toml")?,
    )?;
    let binary = PathBuf::from("target/release/barad-dur");
    let passes = if mode == "run" { 2 } else { 1 };

    let mut failures = 0usize;
    let mut worksheet = String::new();

    for entry in &entries {
        let repo = resolve_path(entry, &root);
        let outcome = analyze_pinned(&binary, &entry.name, &repo, &entry.pin, archive, passes)?;

        if let Some(nd) = &outcome.nondeterminism {
            println!("NONDETERMINISM {}:\n{}", entry.name, nd.render());
            failures += 1;
        }

        match (mode.as_str(), read_baseline(baselines, &entry.name)?) {
            ("accept", _) => write_baseline(baselines, &entry.name, &outcome.surface)?,
            (_, None) => {
                println!("SEEDED {} (no baseline existed)", entry.name);
                write_baseline(baselines, &entry.name, &outcome.surface)?;
            }
            ("audit", Some(base)) => {
                let items = select_for_audit(&base, &outcome.surface, &BTreeSet::new(), ROTATION);
                if !items.is_empty() {
                    worksheet.push_str(&render_worksheet(&entry.name, &items));
                    worksheet.push('\n');
                }
            }
            (_, Some(base)) => {
                let d = diff_surfaces(&base, &outcome.surface);
                if !d.is_empty() {
                    println!("CHANGED {}:\n{}", entry.name, d.render());
                    failures += 1;
                }
            }
        }
    }

    if mode == "audit" {
        print!("{worksheet}");
        return Ok(());
    }

    if failures > 0 {
        eprintln!("\n{failures} repository/repositories differ from baseline.");
        eprintln!("Explain each change in the review, or run `make field-test-accept`.");
        std::process::exit(1);
    }
    println!("field test clean across {} repositories", entries.len());
    Ok(())
}
```

- [ ] **Step 5: Ignore the archive**

Append to `.gitignore`:

```
field-test/archive/
```

- [ ] **Step 6: Verify it builds and the suite is green**

Run: `cargo build --features field-test --bin field-test`
Expected: builds clean.

Run: `RUSTFLAGS=-D warnings cargo test && cargo clippy --all-targets --features field-test -- -D warnings && cargo fmt -- --check`
Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml src/bin/field-test.rs tests/field_test_milestone_1.rs .gitignore
git commit -m "feat(field-test): driver binary behind the field-test feature"
```

---

### Task 9: Make targets

**Files:**
- Modify: `Makefile`

- [ ] **Step 1: Add the targets**

Add `field-test field-test-accept field-audit` to the `.PHONY` line, then append:

```makefile
## Field test — run the corpus twice, diff recommendations vs baselines
field-test: build
	cargo run --release --features field-test --bin field-test -- run

## Accept new baselines — MUST be its own reviewed commit showing the diff
field-test-accept: build
	cargo run --release --features field-test --bin field-test -- accept

## Emit the audit worksheet for this merge
field-audit: build
	cargo run --release --features field-test --bin field-test -- audit
```

- [ ] **Step 2: Verify the targets resolve**

Run: `make -n field-test`
Expected: prints the build and run commands without executing.

- [ ] **Step 3: Commit**

```bash
git add Makefile
git commit -m "build: make targets for the field test harness"
```

---

### Task 10: Seed the baselines

**Files:**
- Create: `field-test/baselines/*.surface.json` (11 files)

This task produces data, not code. It is separate because the baselines are
the harness's first real output and deserve their own reviewable commit.

- [ ] **Step 1: Confirm every corpus repo is present and clean**

```bash
for r in barad-dur ripgrep helix starship dotnet-starter-kit \
         evolutionary-architecture-by-example eShopModernizing App-Serveat \
         payp-app-front kairis-crm mautic; do
  printf "%-40s %s\n" "$r" "$(git -C ~/WS/$r rev-parse --short=8 HEAD 2>&1)"
done
```

Expected: each prints the pin recorded in `field-test/corpus.toml`. A
mismatch means the repo moved; update the pin deliberately, in its own commit.

- [ ] **Step 2: Seed**

```bash
make build
make field-test
```

Expected: `SEEDED <name>` for all 11, then a clean exit. Runtime roughly
6 minutes — two passes over ~3.1 minutes of analysis.

- [ ] **Step 3: Verify the seeded baselines look sane**

```bash
ls field-test/baselines/ | wc -l          # expect 11
grep -L '"overall_score"' field-test/baselines/*.json   # expect no output
grep -c 'null' field-test/baselines/ripgrep.surface.json  # expect > 0: unscored metrics exist
git status --short ~/WS/ripgrep           # expect empty: worktree isolation held
```

- [ ] **Step 4: Re-run to prove the gate is stable**

```bash
make field-test
```

Expected: `field test clean across 11 repositories`, exit 0. Any
`NONDETERMINISM` output here is a real ordering bug — fix it before committing.

- [ ] **Step 5: Commit**

```bash
git add field-test/baselines
git commit -m "chore(field-test): seed baselines for the 11-repo corpus"
```

---

### Task 11: Process documents

**Files:**
- Create: `docs/review-process.md`
- Modify: `CLAUDE.md` (add a "Review gates" section pointing at it)

- [ ] **Step 1: Write `docs/review-process.md`**

The document has five sections, each stating what the gate is, when it runs,
and what evidence it must emit. Content comes from the spec — do not invent:

1. **P0 — Plan-claim verification.** Runs at plan freeze. Every claim about a
   grammar, crate API, or type shape is probed and the probe's *output* is
   recorded. Unprobeable claims are marked `unverified` in the plan. Plans
   carry a required "invariants this feature introduces" section.
2. **P1 — Invariant sweep.** Runs at final review. Emits a table of
   rule → call sites found → verdict. Seeded from the plan's invariants
   section. Its completeness ceiling is stated, not hidden.
3. **P2 — Corpus sweep.** `make field-test` (regression + determinism) and
   `make field-audit` (True/Safe/Actionable worksheet, every merge). A `Safe`
   failure blocks the merge. Completed worksheets are committed under
   `field-test/audit/`.
4. **Evidence contract.** Reports state evidence, not verdicts: call sites
   enumerated, corpus samples inspected, RED→GREEN transcript, full-suite
   output. Full-suite, never `--lib` alone.
5. **Minors policy and escape accounting.** Every minor gets fix /
   corpus-test / retire-with-rationale; a minor recurring in a later
   milestone auto-escalates. Every escape found is logged with **which gate
   caught it, or that none did**.

- [ ] **Step 2: Reference it from `CLAUDE.md`**

Add after the "Gotchas" section:

```markdown
## Review gates

Full definitions in `docs/review-process.md`. In short:

- **P0** at plan freeze — probe every claim about a grammar or API, record the output
- **P1** at final review — sweep each invariant across *all* its call sites
- **P2** at final review — `make field-test` (regression + determinism) and
  `make field-audit` (True/Safe/Actionable). A `Safe` failure blocks the merge
- Reports state **evidence**, not verdicts. Full suite, never `--lib` alone
- Minors are fixed, corpus-tested, or retired with a rationale — never silently deferred
```

- [ ] **Step 3: Commit**

```bash
git add docs/review-process.md CLAUDE.md
git commit -m "docs: review gates P0-P2, evidence contract, minors policy"
```

---

## Self-Review

**Spec coverage:** P0 → Task 11. P1 → Task 11. P2a regression → Tasks 3, 4, 8.
P2b determinism → Task 6. P2c audit → Tasks 7, 8. Evidence contract → Task 11.
Minors policy → Task 11. Escape accounting → Task 11. Corpus manifest with
pins → Task 1. Worktree isolation → Task 5. Decision surface incl.
scored/unscored and `history` exclusion → Task 2. Archive gitignored, not
committed → Task 8. `make` targets → Task 9. Baselines → Task 10.

**Known deviation:** the spec mentions `field-test/audit/rotation.json` as
committed rotation state. Task 8 passes an empty `already_seen` set, so the
first implementation audits the first `ROTATION` recommendations each time
rather than genuinely rotating. Persisting rotation state is deliberately
deferred to a follow-up — the sampling function already takes `already_seen`,
so wiring it up is additive and needs no redesign. Record this in the SDD
ledger as an open follow-up rather than letting it disappear.

**Type consistency:** `DecisionSurface` field names are identical across
Tasks 2, 3, 4, 6, 7. `ActionSurface { target_tab, text }` is used
consistently. `SurfaceDiff::render()` is used in Tasks 6 and 8 as defined in
Task 3. `analyze_pinned` gains a `name` parameter in Task 6's implementation
versus its interface sketch — the implementation signature is authoritative.
