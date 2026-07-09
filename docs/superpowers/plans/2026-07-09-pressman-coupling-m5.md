# Pressman Coupling M5 — History-Corroborated Findings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mark each Pressman coupling finding whose file participates in a real cross-boundary co-change ("change-coupling smell") as **corroborated**, annotate it in the report, and weight it more heavily in its metric's score.

**Architecture:** A pure join in `src/metrics/coupling/mod.rs`. The co-change smell predicate is extracted from `change_coupling_smells` into one shared helper; a `corroboration_degree` map (file → distinct qualifying-partner count) is computed once in `compute_coupling` and threaded into each `pressman_metric`, where corroborated findings are annotated and counted at `corroboration_weight` (default 2.0) toward the count fed to the unchanged `score_pressman` band function. No collector, cache, or renderer changes.

**Tech Stack:** Rust; `tree-sitter` findings and git co-change pairs already live in `RepoSnapshot`; `serde_json` (test-only) + `tempfile` + `git` CLI for the integration fixture.

**Spec:** `docs/superpowers/specs/2026-07-09-pressman-coupling-m5-design.md`

## Global Constraints

- **Report language:** the word is **"corroborated"**, never "confirmed". Copy verbatim: `corroborated (co-changes with N file(s))` in evidence strings; `(N corroborated by change history)` in the metric description.
- **Band SSOT untouched:** `score_pressman(kind, count)` is the maintainer-authored severity-band source of truth. M5 changes the *count* passed in, never the bands.
- **`corroboration_weight = 1.0` must reproduce M1 scores exactly** (regression invariant).
- **Displayed count stays truthful:** the metric description reports the real finding count; only the scored count is weighted.
- **Purity:** all M5 code is a pure function of `&RepoSnapshot` + `&CouplingThresholds`. No I/O, no collector changes, no snapshot-cache-version bump.
- **CI runs warnings-as-errors:** every commit must pass `RUSTFLAGS="-D warnings" cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt -- --check`.
- **Scope excluded (per spec):** no `corroborated_*` history fields (M2), no gate-ratchet change (M3 diffs counts, not scores), no `init.rs` template change (coupling thresholds are serde-defaulted, never templated — consistent with M1–M3).

---

### Task 1: Extract the shared co-change smell predicate

Pure refactor. Pull the smell filter out of `change_coupling_smells` into a reusable iterator so `corroboration_degree` (Task 2) and the smell metric share one definition of a qualifying pair. Behavior must not change — guarded by the existing `change_coupling_*` tests plus a new parity test.

**Files:**
- Modify: `src/metrics/coupling/mod.rs` (`change_coupling_smells`, ~lines 171–203)
- Test: `src/metrics/coupling/tests.rs`

**Interfaces:**
- Produces: `fn qualifying_smell_pairs<'a>(snapshot: &'a RepoSnapshot, thresholds: &'a CouplingThresholds) -> impl Iterator<Item = (&'a PathBuf, &'a PathBuf)> + 'a` — cross-component co-change pairs whose `co_changes / min_commits ≥ change_coupling_min_ratio` with `min_commits > 0`.

- [ ] **Step 1: Write the failing parity test**

Add to the end of `src/metrics/coupling/tests.rs` (before the final `}` if the file is wrapped in a module; it is a flat `mod tests`, so append at end of file):

```rust
#[test]
fn qualifying_smell_pairs_matches_change_coupling_count() {
    // Same fixture the scoring-band test uses: 4 cross-boundary smells.
    let snapshot = make_cross_boundary_snapshot(4);
    let via_helper = qualifying_smell_pairs(&snapshot, &default_thresholds()).count();
    // change_coupling_smells(4) scores 50 == the ">5? no, 3..=5" band for 4 smells.
    assert_eq!(via_helper, 4, "helper must yield exactly the qualifying pairs");
    assert_eq!(
        change_coupling_smells(&snapshot, &default_thresholds()).score,
        Some(50),
        "refactored smell metric must keep its score"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib qualifying_smell_pairs_matches_change_coupling_count`
Expected: FAIL — `cannot find function qualifying_smell_pairs in this scope`.

- [ ] **Step 3: Add the helper and refactor `change_coupling_smells`**

In `src/metrics/coupling/mod.rs`, add this function immediately above `fn change_coupling_smells`:

```rust
/// Cross-boundary co-change pairs that qualify as change-coupling smells:
/// different components, both files have commit history, and their
/// co-change ratio meets the configured threshold. Single source of truth
/// for "a meaningful co-change" — consumed by both the smell metric and
/// corroboration (M5).
fn qualifying_smell_pairs<'a>(
    snapshot: &'a RepoSnapshot,
    thresholds: &'a CouplingThresholds,
) -> impl Iterator<Item = (&'a PathBuf, &'a PathBuf)> + 'a {
    snapshot
        .file_change_pairs
        .iter()
        .filter_map(move |(path_a, path_b, co_changes)| {
            let comp_a = extract_component(path_a, thresholds.component_depth);
            let comp_b = extract_component(path_b, thresholds.component_depth);
            if comp_a == comp_b {
                return None;
            }
            let commits_a = snapshot.commits_by_file.get(path_a).map_or(0, |v| v.len());
            let commits_b = snapshot.commits_by_file.get(path_b).map_or(0, |v| v.len());
            let min_commits = commits_a.min(commits_b);
            if min_commits == 0 {
                return None;
            }
            ((*co_changes as f64 / min_commits as f64) >= thresholds.change_coupling_min_ratio)
                .then_some((path_a, path_b))
        })
}
```

Then replace the body of `change_coupling_smells` up to the `let score` line so it reads:

```rust
fn change_coupling_smells(snapshot: &RepoSnapshot, thresholds: &CouplingThresholds) -> MetricValue {
    let smell_count = qualifying_smell_pairs(snapshot, thresholds).count();

    let score = score_count_bands(smell_count);

    MetricValue {
        name: "Change coupling smells".to_string(),
        description: format!(
            "{} cross-boundary co-change pair(s) above {:.0}% ratio threshold",
            smell_count,
            thresholds.change_coupling_min_ratio * 100.0
        ),
        raw_value: RawValue::Count(smell_count),
        score: Some(score),
    }
}
```

- [ ] **Step 4: Run the smell + parity tests**

Run: `cargo test --lib metrics::coupling::tests::change_coupling`
Run: `cargo test --lib qualifying_smell_pairs_matches_change_coupling_count`
Expected: PASS (all existing `change_coupling_*` tests unchanged, new parity test passes).

- [ ] **Step 5: Full sweep + commit**

Run: `RUSTFLAGS="-D warnings" cargo test --lib metrics::coupling && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check`
Expected: PASS.

```bash
git add src/metrics/coupling/mod.rs src/metrics/coupling/tests.rs
git commit -m "refactor(coupling): extract shared qualifying_smell_pairs predicate"
```

---

### Task 2: `corroboration_degree` join

Build the file → qualifying-partner-count map that drives corroboration.

**Files:**
- Modify: `src/metrics/coupling/mod.rs`
- Test: `src/metrics/coupling/tests.rs`

**Interfaces:**
- Consumes: `qualifying_smell_pairs` (Task 1).
- Produces: `fn corroboration_degree(snapshot: &RepoSnapshot, thresholds: &CouplingThresholds) -> HashMap<PathBuf, usize>` — each file that appears in ≥1 qualifying pair mapped to the number of *distinct* partner files. Files not in any qualifying pair are absent (so `map.contains_key(path)` == "corroborated").

- [ ] **Step 1: Write the failing tests**

Append to `src/metrics/coupling/tests.rs`:

```rust
#[test]
fn corroboration_degree_counts_distinct_partners() {
    let mut snapshot = make_snapshot();
    // src/a.rs co-changes cross-boundary with tests/b.rs and tests/c.rs.
    for partner in ["tests/b.rs", "tests/c.rs"] {
        snapshot
            .file_change_pairs
            .push((PathBuf::from("src/a.rs"), PathBuf::from(partner), 5));
        snapshot
            .commits_by_file
            .insert(PathBuf::from(partner), (0u32..10).map(CommitId).collect());
    }
    snapshot
        .commits_by_file
        .insert(PathBuf::from("src/a.rs"), (0u32..10).map(CommitId).collect());

    let deg = corroboration_degree(&snapshot, &default_thresholds());
    assert_eq!(deg.get(&PathBuf::from("src/a.rs")), Some(&2));
    assert_eq!(deg.get(&PathBuf::from("tests/b.rs")), Some(&1));
    assert_eq!(deg.get(&PathBuf::from("tests/c.rs")), Some(&1));
}

#[test]
fn corroboration_degree_excludes_below_threshold_and_same_component() {
    let mut snapshot = make_snapshot();
    // Below ratio (2/10 < 0.30): excluded.
    snapshot
        .file_change_pairs
        .push((PathBuf::from("src/a.rs"), PathBuf::from("tests/b.rs"), 2));
    // Same component (both src/*, depth 2 differs -> actually different;
    // use src/x/ to force same depth-2 component "src/x").
    snapshot
        .file_change_pairs
        .push((PathBuf::from("src/x/a.rs"), PathBuf::from("src/x/b.rs"), 9));
    for f in ["src/a.rs", "tests/b.rs", "src/x/a.rs", "src/x/b.rs"] {
        snapshot
            .commits_by_file
            .insert(PathBuf::from(f), (0u32..10).map(CommitId).collect());
    }
    let deg = corroboration_degree(&snapshot, &default_thresholds());
    assert!(deg.is_empty(), "no pair qualifies: {deg:?}");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib corroboration_degree`
Expected: FAIL — `cannot find function corroboration_degree`.

- [ ] **Step 3: Implement**

In `src/metrics/coupling/mod.rs`, add below `qualifying_smell_pairs`:

```rust
/// Files that participate in a qualifying cross-boundary co-change, mapped
/// to their count of *distinct* qualifying partners. Presence of a finding's
/// path in this map is what marks the finding "corroborated" (M5).
fn corroboration_degree(
    snapshot: &RepoSnapshot,
    thresholds: &CouplingThresholds,
) -> HashMap<PathBuf, usize> {
    let mut partners: HashMap<PathBuf, HashSet<PathBuf>> = HashMap::new();
    for (a, b) in qualifying_smell_pairs(snapshot, thresholds) {
        partners.entry(a.clone()).or_default().insert(b.clone());
        partners.entry(b.clone()).or_default().insert(a.clone());
    }
    partners.into_iter().map(|(k, v)| (k, v.len())).collect()
}
```

(`HashMap` and `HashSet` are already imported at the top of the file.)

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib corroboration_degree`
Expected: PASS (both tests).

- [ ] **Step 5: Sweep + commit**

Run: `RUSTFLAGS="-D warnings" cargo test --lib metrics::coupling && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check`
Expected: PASS.

```bash
git add src/metrics/coupling/mod.rs src/metrics/coupling/tests.rs
git commit -m "feat(coupling): corroboration_degree join over qualifying co-change pairs"
```

---

### Task 3: `corroboration_weight` config + weighted scoring & annotation

Add the config knob, thread the corroboration map and weight through `compute_coupling` into `pressman_metric`, and apply the weighted effective count + evidence/description annotation.

**Files:**
- Modify: `src/config/thresholds.rs` (`CouplingThresholds`, defaults, `impl Default`)
- Modify: `src/metrics/coupling/mod.rs` (`compute_coupling`, `pressman_metric`)
- Test: `src/metrics/coupling/tests.rs`

**Interfaces:**
- Consumes: `corroboration_degree` (Task 2).
- Produces: `CouplingThresholds.corroboration_weight: f64` (serde default 2.0); new `pressman_metric` signature `fn pressman_metric(snapshot: &RepoSnapshot, kind: CouplingKind, extra: Vec<CouplingFinding>, corr: &HashMap<PathBuf, usize>, weight: f64) -> MetricValue`.

- [ ] **Step 1: Add the config field**

In `src/config/thresholds.rs`, add to `struct CouplingThresholds` as the **last field** (after the existing `hotspot_multiplier` field added by M4, before the closing `}`):

```rust
    /// How much a corroborated finding (its file co-changes cross-boundary)
    /// weighs versus a dormant one when scoring a Pressman metric. 1.0 = no
    /// nudge (reproduces pre-M5 scores); 2.0 = a corroborated finding counts
    /// double toward the severity band.
    #[serde(default = "default_corroboration_weight")]
    pub corroboration_weight: f64,
```

Add the default fn beside the others (after `default_hotspot_multiplier`):

```rust
fn default_corroboration_weight() -> f64 {
    2.0
}
```

Add `corroboration_weight: default_corroboration_weight(),` as the last field in `impl Default for CouplingThresholds` (which post-M4 already sets `component_depth`, `change_coupling_min_ratio`, `content_barrel_rule`, and `hotspot_multiplier`).

- [ ] **Step 2: Write the failing scoring tests**

Append to `src/metrics/coupling/tests.rs`. These rely on a helper that attaches a corroborating co-change pair to a finding's file — add it near `snapshot_with_findings`:

```rust
/// `snapshot_with_findings`, plus a qualifying cross-boundary co-change pair
/// for each given finding-file path so those findings corroborate.
fn snapshot_with_corroborated(findings: Vec<CouplingFinding>) -> RepoSnapshot {
    let mut s = snapshot_with_findings(findings.clone());
    for (i, f) in findings.iter().enumerate() {
        let partner = PathBuf::from(format!("tests/partner{i}.rs"));
        s.file_change_pairs.push((f.path.clone(), partner.clone(), 5));
        s.commits_by_file
            .insert(f.path.clone(), (0u32..10).map(CommitId).collect());
        s.commits_by_file
            .insert(partner, (0u32..10).map(CommitId).collect());
    }
    s
}

#[test]
fn corroborated_common_finding_scores_one_band_worse() {
    // 1 dormant Common finding -> count 1 -> 60.
    let dormant = snapshot_with_findings(vec![make_finding(CouplingKind::Common)]);
    let d = compute_coupling(&dormant, &CouplingThresholds::default());
    let d_common = d.metrics.iter().find(|m| m.name == "Common coupling").unwrap();
    assert_eq!(d_common.score, Some(60));

    // 1 corroborated Common finding -> effective 2 (weight 2.0) -> 40.
    let corr = snapshot_with_corroborated(vec![make_finding(CouplingKind::Common)]);
    let c = compute_coupling(&corr, &CouplingThresholds::default());
    let c_common = c.metrics.iter().find(|m| m.name == "Common coupling").unwrap();
    assert_eq!(c_common.score, Some(40));
}

#[test]
fn weight_one_reproduces_dormant_scores() {
    let corr = snapshot_with_corroborated(vec![make_finding(CouplingKind::Common)]);
    let thresholds = CouplingThresholds {
        corroboration_weight: 1.0,
        ..CouplingThresholds::default()
    };
    let c = compute_coupling(&corr, &thresholds);
    let common = c.metrics.iter().find(|m| m.name == "Common coupling").unwrap();
    assert_eq!(common.score, Some(60), "weight 1.0 must equal the dormant score");
}

#[test]
fn corroboration_can_trip_the_severity_cap() {
    // 2 corroborated Common findings on distinct files -> effective 4 -> 25,
    // which is <= the Common cap trigger (25) -> category capped.
    let corr = snapshot_with_corroborated(vec![
        CouplingFinding { path: PathBuf::from("src/a.rs"), line: Some(1), kind: CouplingKind::Common, evidence: "static mut A".into() },
        CouplingFinding { path: PathBuf::from("src/b.rs"), line: Some(1), kind: CouplingKind::Common, evidence: "static mut B".into() },
    ]);
    let c = compute_coupling(&corr, &CouplingThresholds::default());
    let common = c.metrics.iter().find(|m| m.name == "Common coupling").unwrap();
    assert_eq!(common.score, Some(25));
    assert!(c.score <= crate::scorer::SCORE_GOOD_MIN - 1, "category must be capped");
}

#[test]
fn corroborated_finding_is_annotated_in_evidence_and_description() {
    let corr = snapshot_with_corroborated(vec![make_finding(CouplingKind::Common)]);
    let c = compute_coupling(&corr, &CouplingThresholds::default());
    let common = c.metrics.iter().find(|m| m.name == "Common coupling").unwrap();
    assert!(
        common.description.contains("1 corroborated by change history"),
        "description: {}",
        common.description
    );
    match &common.raw_value {
        RawValue::List(items) => assert!(
            items.iter().any(|s| s.contains("corroborated (co-changes with 1 file(s))")),
            "evidence: {items:?}"
        ),
        other => panic!("expected List, got {other:?}"),
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test --lib corroborated_common_finding_scores_one_band_worse`
Expected: FAIL to compile — `pressman_metric` arity / `corroboration_weight` field mismatch (the tests exercise the new behavior through `compute_coupling`).

- [ ] **Step 4: Thread corroboration into `compute_coupling` and `pressman_metric`**

In `src/metrics/coupling/mod.rs`, update `compute_coupling`'s metric vector:

```rust
    let corr = corroboration_degree(snapshot, thresholds);
    let weight = thresholds.corroboration_weight;
    let metrics = vec![
        afferent_coupling(snapshot),
        efferent_coupling(snapshot),
        circular_dependencies(snapshot),
        change_coupling_smells(snapshot, thresholds),
        pressman_metric(snapshot, CouplingKind::Content, barrel, &corr, weight),
        pressman_metric(snapshot, CouplingKind::Common, Vec::new(), &corr, weight),
        pressman_metric(snapshot, CouplingKind::Control, Vec::new(), &corr, weight),
    ];
```

Replace `pressman_metric` (from `let count = findings.len();` onward) so the whole function becomes:

```rust
fn pressman_metric(
    snapshot: &RepoSnapshot,
    kind: CouplingKind,
    extra: Vec<CouplingFinding>,
    corr: &HashMap<PathBuf, usize>,
    weight: f64,
) -> MetricValue {
    let (name, rung) = match kind {
        CouplingKind::Content => (
            "Content coupling",
            "worst rung: another module's internals reached",
        ),
        CouplingKind::Common => ("Common coupling", "shared mutable global state"),
        CouplingKind::Control => ("Control coupling", "flag parameters steering callee logic"),
    };
    if !detection_ran(snapshot) {
        return MetricValue {
            name: name.to_string(),
            description: "Coupling detection did not run (no parsed files)".to_string(),
            raw_value: RawValue::Count(0),
            score: None,
        };
    }
    if !has_detectable_files(snapshot) {
        return MetricValue {
            name: name.to_string(),
            description: "No files in detectable languages (Rust, TS/JS)".to_string(),
            raw_value: RawValue::Count(0),
            score: None,
        };
    }
    let findings: Vec<CouplingFinding> = snapshot
        .coupling_findings
        .iter()
        .filter(|f| f.kind == kind)
        .cloned()
        .chain(extra)
        .collect();
    let count = findings.len();

    // Weighted effective count: corroborated findings (their file co-changes
    // cross-boundary) weigh `weight`, dormant ones weigh 1. Only the *scored*
    // count is weighted — the displayed count stays truthful. weight == 1.0
    // reproduces pre-M5 scores exactly.
    let corroborated_count = findings.iter().filter(|f| corr.contains_key(&f.path)).count();
    let dormant_count = count - corroborated_count;
    let effective = (dormant_count as f64 + corroborated_count as f64 * weight).round() as usize;

    let list: Vec<String> = findings
        .iter()
        .take(10)
        .map(|f| {
            let base = match f.line {
                Some(l) => format!("{}:{} — {}", f.path.display(), l, f.evidence),
                None => format!("{} — {}", f.path.display(), f.evidence),
            };
            match corr.get(&f.path) {
                Some(n) => format!("{base} — corroborated (co-changes with {n} file(s))"),
                None => base,
            }
        })
        .collect();

    let corr_note = if corroborated_count > 0 {
        format!(" ({corroborated_count} corroborated by change history)")
    } else {
        String::new()
    };

    MetricValue {
        name: name.to_string(),
        description: format!("{count} finding(s){corr_note} — {rung}"),
        raw_value: RawValue::List(list),
        score: Some(score_pressman(kind, effective)),
    }
}
```

- [ ] **Step 5: Run the new tests**

Run: `cargo test --lib metrics::coupling::tests`
Expected: PASS — the four new scoring/annotation tests plus all pre-existing coupling tests (M1's `score_pressman_bands_are_exact`, `one_content_finding_scores_at_most_50`, etc. are untouched because dormant snapshots have no co-change pairs → `effective == count`).

- [ ] **Step 6: Full workspace sweep + commit**

Run: `RUSTFLAGS="-D warnings" cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check`
Expected: PASS (confirms no other call site of `pressman_metric` or literal `CouplingThresholds { … }` construction broke; if the compiler flags a literal construction missing `corroboration_weight`, add `..CouplingThresholds::default()` or the field there).

```bash
git add src/config/thresholds.rs src/metrics/coupling/mod.rs src/metrics/coupling/tests.rs
git commit -m "feat(coupling): weight corroborated findings and annotate them (Pressman M5)"
```

---

### Task 4: M5 end-to-end integration test

Prove corroboration surfaces through the real binary: a fixture repo with a `static mut` (Common finding) whose file co-changes cross-component across several commits → the `analyze --json` report annotates the finding "corroborated" and scores the Common metric one band below the dormant baseline (40 < 60).

**Files:**
- Create: `tests/pressman_coupling_milestone_5.rs`
- Modify: `Cargo.toml` (`[dev-dependencies]`: add `serde_json = "1"`)

**Interfaces:**
- Consumes: the built binary `env!("CARGO_BIN_EXE_barad-dur")`; JSON shape `categories[] { name, metrics[] { name, score, raw_value } }`.

- [ ] **Step 1: Add the dev-dependency**

In `Cargo.toml` under `[dev-dependencies]`, add:

```toml
serde_json = "1"
```

- [ ] **Step 2: Write the integration test**

Create `tests/pressman_coupling_milestone_5.rs`:

```rust
//! M5 milestone E2E: a `static mut` common-coupling finding whose file
//! co-changes cross-component across several commits is reported as
//! "corroborated" and scored one severity band below the dormant baseline.
//!
//! `compute_coupling` is crate-internal, so this drives the installed binary
//! (`CARGO_BIN_EXE_barad-dur`) against a throwaway fixture repo and parses
//! the JSON report.

use std::process::{Command, Output};

/// Build a fixture repo whose `src/config.rs` holds a `static mut` (Common
/// finding) and co-changes with `lib/helper.rs` (a different depth-2
/// component) across 4 of 4 commits — ratio 1.0, well above the 0.30
/// threshold — so the finding corroborates.
fn fixture_repo() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let git = |args: &[&str]| -> Output {
        let out = Command::new("git")
            .current_dir(dir.path())
            .args(args)
            .output()
            .expect("git must spawn");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        out
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "fixture@example.com"]);
    git(&["config", "user.name", "Fixture"]);

    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::create_dir_all(dir.path().join("lib")).unwrap();
    let config_rs = dir.path().join("src/config.rs");
    let helper_rs = dir.path().join("lib/helper.rs");

    for i in 0..4 {
        std::fs::write(
            &config_rs,
            format!("static mut FLAG: bool = false;\npub fn v() -> u32 {{ {i} }}\n"),
        )
        .unwrap();
        std::fs::write(&helper_rs, format!("pub fn h() -> u32 {{ {i} }}\n")).unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", &format!("change {i}")]);
    }
    dir
}

fn common_metric_score(report: &serde_json::Value) -> (i64, Vec<String>) {
    let cats = report["categories"].as_array().expect("categories array");
    let coupling = cats
        .iter()
        .find(|c| c["name"] == "Coupling")
        .expect("Coupling category");
    let metric = coupling["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "Common coupling")
        .expect("Common coupling metric");
    let score = metric["score"].as_i64().expect("score");
    let list = metric["raw_value"]["List"]
        .as_array()
        .expect("List raw_value")
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect();
    (score, list)
}

#[test]
fn corroborated_common_finding_is_annotated_and_downscored() {
    let dir = fixture_repo();
    let out = Command::new(env!("CARGO_BIN_EXE_barad-dur"))
        .arg("analyze")
        .arg(dir.path())
        .args(["--json", "--no-cache"])
        .output()
        .expect("binary must run");
    assert!(out.status.success(), "analyze failed: {}", String::from_utf8_lossy(&out.stderr));

    let report: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be JSON");
    let (score, evidence) = common_metric_score(&report);

    // Dormant baseline for 1 Common finding is 60; corroborated (weight 2.0)
    // scores the count-2 band, 40.
    assert_eq!(score, 40, "corroborated Common finding must score one band worse");
    assert!(
        evidence.iter().any(|s| s.contains("corroborated (co-changes with")),
        "evidence must be annotated: {evidence:?}"
    );
}
```

- [ ] **Step 3: Run the integration test**

Run: `cargo test --test pressman_coupling_milestone_5`
Expected: PASS. (If it fails on the score, print `report["categories"]` to confirm the fixture produced exactly one Common finding and a qualifying pair; a stray Content/Control finding does not affect the Common metric.)

- [ ] **Step 4: Full sweep + commit**

Run: `RUSTFLAGS="-D warnings" cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check`
Expected: PASS.

```bash
git add Cargo.toml Cargo.lock tests/pressman_coupling_milestone_5.rs
git commit -m "test(coupling): M5 milestone — corroboration surfaced end-to-end"
```

---

### Task 5: Documentation — mark the M5 checkpoint resolved

Point the parent design's M5 section at the resolved design and record the shipped decision. No user-facing config docs change (coupling thresholds are serde-defaulted and never templated, consistent with M1–M3).

**Files:**
- Modify: `docs/superpowers/specs/2026-07-02-pressman-coupling-design.md` (M5 section + resolved-questions item 6)

- [ ] **Step 1: Update the M5 section status line**

In `docs/superpowers/specs/2026-07-02-pressman-coupling-design.md`, change the M5 heading's status line

```
**Status: concept approved, detailed design deferred.** ...
```

to:

```
**Status: resolved and shipped.** The checkpoint was revisited against real
finding data (barad-dur: 0 content/0 common/6 control) on 2026-07-09; the
detailed design lives in `2026-07-09-pressman-coupling-m5-design.md`.
Decisions: corroboration covers all three kinds; the criterion reuses the
change-coupling smell rule; corroborated findings weigh `corroboration_weight`
(default 2.0) toward the severity band; report language is "corroborated".
```

- [ ] **Step 2: Update resolved-question 6**

Change resolved-question item 6 ("M5 is a checkpoint, not a commitment.") to append:

```
Resolved 2026-07-09 (see 2026-07-09-pressman-coupling-m5-design.md): all
three kinds, smell-rule criterion, weighted-count score nudge (default 2.0×),
"corroborated" never "confirmed".
```

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/specs/2026-07-02-pressman-coupling-design.md
git commit -m "docs(coupling): mark Pressman M5 checkpoint resolved and shipped"
```

---

## Self-Review

**Spec coverage** (each spec section → task):
- Architecture / pure join, no collector change → Tasks 2–3.
- Shared "qualifying pair" predicate → Task 1.
- Scope: all three kinds → Task 3 (`compute_coupling` calls all three; corroboration join is kind-agnostic).
- Scoring: weighted effective count, `corroboration_weight` default 2.0, truthful displayed count, floor/cap monotonicity, weight-1.0 regression → Task 3.
- Surfacing: annotated `RawValue::List` + description, no renderer change → Task 3 (+ Task 4 E2E).
- Interactions untouched (gate/M2/backfill) → Global Constraints; no task needed (nothing changes).
- Configuration: `corroboration_weight`, defaulted, no migration → Task 3.
- Language rule "corroborated" → Global Constraints + Tasks 3/4 assertions.
- Testing: predicate parity (T1), join units (T2), scoring/weight-1.0/cap/annotation (T3), integration fixture (T4) → covered.
- Deferred future work (history fields, M6 ordering) → intentionally out of scope, noted in Global Constraints.

**Placeholder scan:** none — every code and command step is concrete.

**Type consistency:** `qualifying_smell_pairs` (T1) → `corroboration_degree` (T2) → `pressman_metric(.., corr: &HashMap<PathBuf, usize>, weight: f64)` (T3). `CouplingThresholds.corroboration_weight: f64` used consistently. Evidence/description strings identical across T3 unit tests and T4 E2E assertions (`corroborated (co-changes with N file(s))`, `N corroborated by change history`).
