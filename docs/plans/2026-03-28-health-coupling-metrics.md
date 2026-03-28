# Health & Coupling Metrics Redesign — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace 5 weak health metrics with 3 sharper health metrics and a new Coupling category (3 metrics), backed by AST data already in the snapshot.

**Architecture:** Health keeps individual-file signals (bus_factor, god_objects, complex_hotspots); a new `src/metrics/coupling.rs` holds inter-file signals (temporal_coupling moved, fan_out_coupling, demeter_violations). Config gains a `coupling` weight field; `main.rs` wires the new category in.

**Tech Stack:** Rust, tree-sitter (existing), `src/snapshot::FileComplexity`, `src/metrics/`, `src/config.rs`

---

### Task 1: Add `demeter_violations` field to `FileComplexity`

**Files:**
- Modify: `src/snapshot.rs:61-67`

**Step 1: Add the field**

In `src/snapshot.rs`, extend the `FileComplexity` struct:

```rust
pub struct FileComplexity {
    pub total_lines: usize,
    pub loc: usize,
    pub cyclomatic_complexity: u32,
    pub public_methods: u32,
    pub properties: u32,
    pub demeter_violations: u32,   // ← add this
}
```

**Step 2: Fix the construction site in `treesitter.rs`**

In `src/metrics/complexity/treesitter.rs:23-29`, add `demeter_violations: 0` to the `Some(FileComplexity { ... })` literal so it compiles (we'll populate it properly in Task 3):

```rust
Some(FileComplexity {
    total_lines,
    loc,
    cyclomatic_complexity,
    public_methods,
    properties,
    demeter_violations: 0,
})
```

**Step 3: Build to confirm it compiles**

```bash
rtk cargo check 2>&1
```
Expected: no errors.

**Step 4: Commit**

```bash
rtk git add src/snapshot.rs src/metrics/complexity/treesitter.rs
rtk git commit -m "feat(snapshot): add demeter_violations field to FileComplexity"
```

---

### Task 2: Add Demeter tree-sitter queries

**Files:**
- Modify: `src/metrics/complexity/queries.rs`

**Step 1: Add queries for each language**

Append to `src/metrics/complexity/queries.rs`. Each query matches a method chain of depth ≥ 3 (two nested calls before the outermost one):

```rust
// ── Demeter (method chains depth ≥ 3) ───────────────────────────────

pub const RUST_DEMETER: &str = r#"
(call_expression
  function: (field_expression
    value: (call_expression
      function: (field_expression
        value: (call_expression)
      )
    )
  )
) @demeter"#;

pub const JS_DEMETER: &str = r#"
(call_expression
  function: (member_expression
    object: (call_expression
      function: (member_expression
        object: (call_expression)
      )
    )
  )
) @demeter"#;

// TypeScript shares the same query as JavaScript.
pub const TS_DEMETER: &str = JS_DEMETER;

pub const PYTHON_DEMETER: &str = r#"
(attribute
  object: (attribute
    object: (attribute)
  )
) @demeter"#;

pub const GO_DEMETER: &str = r#"
(call_expression
  function: (selector_expression
    operand: (call_expression
      function: (selector_expression
        operand: (call_expression)
      )
    )
  )
) @demeter"#;

pub const JAVA_DEMETER: &str = r#"
(method_invocation
  object: (method_invocation
    object: (method_invocation)
  )
) @demeter"#;

pub const CSHARP_DEMETER: &str = r#"
(invocation_expression
  function: (member_access_expression
    expression: (invocation_expression
      function: (member_access_expression
        expression: (invocation_expression)
      )
    )
  )
) @demeter"#;
```

**Step 2: Commit**

```bash
rtk git add src/metrics/complexity/queries.rs
rtk git commit -m "feat(queries): add Demeter chain queries for all supported languages"
```

---

### Task 3: Implement `count_demeter_violations` in treesitter.rs

**Files:**
- Modify: `src/metrics/complexity/treesitter.rs`

**Step 1: Write the failing test**

Add to the `#[cfg(test)]` block at the bottom of `treesitter.rs`:

```rust
#[test]
fn demeter_violation_rust_depth3() {
    // a.foo().bar().baz() — depth-3 chain, should count as 1 violation
    let src = r#"fn f() { let _ = a.foo().bar().baz(); }"#;
    let result = analyse(src, Language::Rust, "rs").unwrap();
    assert_eq!(result.demeter_violations, 1);
}

#[test]
fn demeter_no_violation_rust_depth2() {
    // a.foo().bar() — depth-2 chain, below threshold, 0 violations
    let src = r#"fn f() { let _ = a.foo().bar(); }"#;
    let result = analyse(src, Language::Rust, "rs").unwrap();
    assert_eq!(result.demeter_violations, 0);
}
```

**Step 2: Run to confirm they fail**

```bash
rtk cargo test demeter 2>&1
```
Expected: FAIL (field exists but is always 0).

**Step 3: Implement `count_demeter_violations`**

Add before the `analyse` function:

```rust
fn count_demeter_violations(
    tree: &tree_sitter::Tree,
    source: &[u8],
    grammar: &tree_sitter::Language,
    lang: Language,
    ext: &str,
) -> u32 {
    let query_src = match lang {
        Language::Rust => queries::RUST_DEMETER,
        Language::JsTs => match ext {
            "ts" | "tsx" => queries::TS_DEMETER,
            _ => queries::JS_DEMETER,
        },
        Language::Python => queries::PYTHON_DEMETER,
        Language::Go => queries::GO_DEMETER,
        Language::Java => queries::JAVA_DEMETER,
        Language::CSharp => queries::CSHARP_DEMETER,
        Language::Kotlin | Language::Generic => return 0,
    };
    run_query(tree, source, query_src, grammar)
}
```

**Step 4: Wire it into `analyse`**

In the `analyse` function, after `let properties = ...`:

```rust
let demeter_violations =
    count_demeter_violations(&tree, content.as_bytes(), &grammar, lang, ext);

Some(FileComplexity {
    total_lines,
    loc,
    cyclomatic_complexity,
    public_methods,
    properties,
    demeter_violations,
})
```

**Step 5: Run tests to confirm they pass**

```bash
rtk cargo test demeter 2>&1
```
Expected: PASS.

**Step 6: Run full suite**

```bash
rtk cargo test 2>&1
```
Expected: all tests pass.

**Step 7: Commit**

```bash
rtk git add src/metrics/complexity/treesitter.rs
rtk git commit -m "feat(treesitter): implement count_demeter_violations"
```

---

### Task 4: Fix `bus_factor` in health.rs

The current implementation takes the *minimum* bus factor across all files, which means one trivially-owned file tanks the score. Replace with: *ratio of files where a single author owns >50% of lines*.

**Files:**
- Modify: `src/metrics/health.rs`

**Step 1: Update the existing test to expect new behavior**

In `src/metrics/health.rs`, the existing `bus_factor_detects_single_author_dominance` test has one file where Alice owns 80%. With the new logic, 100% of files are dominated → score = 25.

Replace the assertion:
```rust
// was: assert_eq!(result.score, 20);
assert_eq!(result.score, 25);
match result.raw_value {
    RawValue::Percentage(p) => assert!((p - 100.0).abs() < 1.0),
    _ => panic!("Expected Percentage"),
}
```

**Step 2: Run to confirm test fails**

```bash
rtk cargo test bus_factor 2>&1
```
Expected: FAIL.

**Step 3: Rewrite `bus_factor` function**

Replace the entire `bus_factor` function (lines 26–95 in `health.rs`):

```rust
fn bus_factor(snapshot: &RepoSnapshot, _thresholds: &HealthThresholds) -> MetricValue {
    if snapshot.blame_map.is_empty() {
        return MetricValue {
            name: "Bus factor".to_string(),
            description: "No blame data available".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
        };
    }

    let total_files = snapshot.blame_map.len();
    let dominated = snapshot
        .blame_map
        .values()
        .filter(|lines| {
            if lines.is_empty() {
                return false;
            }
            let mut author_lines: HashMap<usize, usize> = HashMap::new();
            for line in lines.iter() {
                *author_lines.entry(line.author_id).or_insert(0) += 1;
            }
            let total: usize = author_lines.values().sum();
            let max: usize = author_lines.values().copied().max().unwrap_or(0);
            max * 2 > total
        })
        .count();

    let pct = (dominated as f64 / total_files as f64) * 100.0;

    let score = if pct < 10.0 {
        100
    } else if pct < 25.0 {
        75
    } else if pct < 50.0 {
        50
    } else {
        25
    };

    MetricValue {
        name: "Bus factor".to_string(),
        description: format!("{:.0}% of files single-author dominated", pct),
        raw_value: RawValue::Percentage(pct),
        score,
    }
}
```

**Step 4: Run tests**

```bash
rtk cargo test bus_factor 2>&1
```
Expected: PASS.

**Step 5: Commit**

```bash
rtk git add src/metrics/health.rs
rtk git commit -m "fix(health): rebase bus_factor on dominated-file ratio instead of minimum"
```

---

### Task 5: Add `god_objects` metric to health.rs

**Files:**
- Modify: `src/metrics/health.rs`

**Step 1: Write the failing test**

Add to the `#[cfg(test)]` block:

```rust
#[test]
fn god_objects_detects_large_files() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    use crate::snapshot::FileComplexity;

    // One god object: >500 LOC
    snapshot.file_metrics.insert(
        PathBuf::from("fat.rs"),
        FileComplexity {
            total_lines: 600,
            loc: 520,
            cyclomatic_complexity: 10,
            public_methods: 5,
            properties: 2,
            demeter_violations: 0,
        },
    );
    // One normal file
    snapshot.file_metrics.insert(
        PathBuf::from("small.rs"),
        FileComplexity {
            total_lines: 100,
            loc: 80,
            cyclomatic_complexity: 3,
            public_methods: 2,
            properties: 1,
            demeter_violations: 0,
        },
    );

    let result = god_objects(&snapshot);
    assert_eq!(result.score, 75); // 1 god object → score 75
    match &result.raw_value {
        RawValue::List(v) => assert_eq!(v.len(), 1),
        _ => panic!("Expected List"),
    }
}

#[test]
fn god_objects_detects_method_bloat() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    use crate::snapshot::FileComplexity;

    // LOC=310, methods=16 → both thresholds exceeded
    snapshot.file_metrics.insert(
        PathBuf::from("bloated.rs"),
        FileComplexity {
            total_lines: 350,
            loc: 310,
            cyclomatic_complexity: 5,
            public_methods: 16,
            properties: 3,
            demeter_violations: 0,
        },
    );

    let result = god_objects(&snapshot);
    assert_eq!(result.score, 75);
}
```

**Step 2: Run to confirm failure**

```bash
rtk cargo test god_objects 2>&1
```
Expected: FAIL (function not found).

**Step 3: Implement `god_objects`**

Add after the `bus_factor` function:

```rust
/// Files that have grown too large to maintain (god objects / bloaters).
fn god_objects(snapshot: &RepoSnapshot) -> MetricValue {
    let gods: Vec<String> = snapshot
        .file_metrics
        .iter()
        .filter(|(_, m)| m.loc > 500 || (m.loc > 300 && m.public_methods > 15))
        .map(|(p, _)| p.display().to_string())
        .collect();

    let count = gods.len();
    let score = match count {
        0 => 100,
        1..=2 => 75,
        3..=5 => 50,
        _ => 25,
    };

    MetricValue {
        name: "God objects".to_string(),
        description: format!("{} oversized files detected", count),
        raw_value: RawValue::List(gods),
        score,
    }
}
```

**Step 4: Run tests**

```bash
rtk cargo test god_objects 2>&1
```
Expected: PASS.

**Step 5: Commit**

```bash
rtk git add src/metrics/health.rs
rtk git commit -m "feat(health): add god_objects bloater metric"
```

---

### Task 6: Add `complex_hotspots` metric to health.rs

**Files:**
- Modify: `src/metrics/health.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn complex_hotspots_finds_high_cc_high_churn_files() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    use crate::snapshot::FileComplexity;

    // 4 files: only "bad.rs" is in top quartile of both CC and churn
    let files = [
        ("bad.rs",  20u32, 20usize), // high CC, high churn
        ("ok1.rs",   2,     1),
        ("ok2.rs",   3,     2),
        ("ok3.rs",   4,     3),
    ];
    for (name, cc, churn) in &files {
        snapshot.file_metrics.insert(
            PathBuf::from(name),
            FileComplexity {
                total_lines: 100,
                loc: 80,
                cyclomatic_complexity: *cc,
                public_methods: 2,
                properties: 1,
                demeter_violations: 0,
            },
        );
        snapshot.commits_by_file.insert(
            PathBuf::from(name),
            (0..*churn).map(|i| format!("c{}", i)).collect(),
        );
    }

    let result = complex_hotspots(&snapshot);
    assert_eq!(result.score, 75); // 1 hotspot
    match &result.raw_value {
        RawValue::List(v) => assert_eq!(v.len(), 1),
        _ => panic!("Expected List"),
    }
}
```

**Step 2: Run to confirm failure**

```bash
rtk cargo test complex_hotspots 2>&1
```
Expected: FAIL.

**Step 3: Implement `complex_hotspots`**

```rust
/// Files in the top quartile of both cyclomatic complexity and churn —
/// the Tornhill composite hotspot signal.
fn complex_hotspots(snapshot: &RepoSnapshot) -> MetricValue {
    if snapshot.file_metrics.is_empty() {
        return MetricValue {
            name: "Complex hotspots".to_string(),
            description: "No AST data available".to_string(),
            raw_value: RawValue::Count(0),
            score: 100,
        };
    }

    let mut cc_values: Vec<u32> = snapshot
        .file_metrics
        .values()
        .map(|m| m.cyclomatic_complexity)
        .collect();
    cc_values.sort_unstable();
    let cc_p75 = cc_values
        .get(cc_values.len().saturating_sub(1) * 3 / 4)
        .copied()
        .unwrap_or(0);

    let mut churn_values: Vec<usize> = snapshot
        .commits_by_file
        .values()
        .map(|c| c.len())
        .collect();
    churn_values.sort_unstable();
    let churn_p75 = churn_values
        .get(churn_values.len().saturating_sub(1) * 3 / 4)
        .copied()
        .unwrap_or(0);

    let hotspots: Vec<String> = snapshot
        .file_metrics
        .iter()
        .filter(|(path, m)| {
            let churn = snapshot
                .commits_by_file
                .get(*path)
                .map(|c| c.len())
                .unwrap_or(0);
            m.cyclomatic_complexity >= cc_p75 && churn >= churn_p75
        })
        .map(|(p, _)| p.display().to_string())
        .collect();

    let count = hotspots.len();
    let score = match count {
        0 => 100,
        1..=2 => 75,
        3..=5 => 50,
        _ => 25,
    };

    MetricValue {
        name: "Complex hotspots".to_string(),
        description: format!("{} files with high complexity and high churn", count),
        raw_value: RawValue::List(hotspots),
        score,
    }
}
```

**Step 4: Run tests**

```bash
rtk cargo test complex_hotspots 2>&1
```
Expected: PASS.

**Step 5: Commit**

```bash
rtk git add src/metrics/health.rs
rtk git commit -m "feat(health): add complex_hotspots Tornhill composite metric"
```

---

### Task 7: Remove old health metrics and rewire `compute_health`

**Files:**
- Modify: `src/metrics/health.rs`

**Step 1: Delete old functions and tests**

Remove the following functions entirely from `health.rs`:
- `churn_hotspots` (lines ~97–152)
- `temporal_coupling` (lines ~154–193)
- `stale_code` (lines ~195–249)
- `file_complexity` (lines ~251–280)

Remove the corresponding tests:
- `churn_hotspots_detects_concentration`
- `temporal_coupling_detects_pairs`
- `stale_code_detects_untouched_files`
- `file_complexity_flags_large_and_deep`

**Step 2: Rewire `compute_health`**

Replace the function body:

```rust
pub fn compute_health(snapshot: &RepoSnapshot, _thresholds: &HealthThresholds) -> CategoryResult {
    let metrics = vec![
        bus_factor(snapshot, _thresholds),
        god_objects(snapshot),
        complex_hotspots(snapshot),
    ];

    CategoryResult {
        name: "Health".to_string(),
        score: 0,
        metrics,
    }
    .compute_score()
}
```

**Step 3: Build and test**

```bash
rtk cargo test 2>&1
```
Expected: all remaining tests pass, no unused import warnings.

**Step 4: Commit**

```bash
rtk git add src/metrics/health.rs
rtk git commit -m "refactor(health): remove stale/churn/temporal/complexity, wire new metrics"
```

---

### Task 8: Create `src/metrics/coupling.rs`

**Files:**
- Create: `src/metrics/coupling.rs`

**Step 1: Write tests first**

Create the file with tests only initially:

```rust
use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::HealthThresholds;
use crate::metrics::{CategoryResult, MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

pub fn compute_coupling(snapshot: &RepoSnapshot) -> CategoryResult {
    todo!()
}

fn temporal_coupling(snapshot: &RepoSnapshot) -> MetricValue {
    todo!()
}

fn fan_out_coupling(snapshot: &RepoSnapshot) -> MetricValue {
    todo!()
}

fn demeter_violations(snapshot: &RepoSnapshot) -> MetricValue {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::*;
    use chrono::Utc;

    #[test]
    fn temporal_coupling_detects_pairs() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.file_change_pairs = vec![(PathBuf::from("a.rs"), PathBuf::from("b.rs"), 9)];
        snapshot.commits_by_file.insert(
            PathBuf::from("a.rs"),
            (0..10).map(|i| format!("c{}", i)).collect(),
        );
        snapshot.commits_by_file.insert(
            PathBuf::from("b.rs"),
            (0..10).map(|i| format!("c{}", i)).collect(),
        );
        let result = temporal_coupling(&snapshot);
        match result.raw_value {
            RawValue::Count(c) => assert_eq!(c, 1),
            _ => panic!("Expected Count"),
        }
    }

    #[test]
    fn fan_out_coupling_detects_high_fanout_files() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let hub = PathBuf::from("hub.rs");
        // hub.rs co-changes with 6 distinct partners
        for i in 0..6 {
            let partner = PathBuf::from(format!("p{}.rs", i));
            snapshot
                .file_change_pairs
                .push((hub.clone(), partner, 3));
        }
        let result = fan_out_coupling(&snapshot);
        assert_eq!(result.score, 75); // 1 high-fanout file
    }

    #[test]
    fn demeter_violations_sums_file_metrics() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        use crate::snapshot::FileComplexity;
        snapshot.file_metrics.insert(
            PathBuf::from("a.rs"),
            FileComplexity {
                total_lines: 50,
                loc: 40,
                cyclomatic_complexity: 2,
                public_methods: 1,
                properties: 0,
                demeter_violations: 3,
            },
        );
        snapshot.file_metrics.insert(
            PathBuf::from("b.rs"),
            FileComplexity {
                total_lines: 50,
                loc: 40,
                cyclomatic_complexity: 2,
                public_methods: 1,
                properties: 0,
                demeter_violations: 2,
            },
        );
        let result = demeter_violations(&snapshot);
        match result.raw_value {
            RawValue::Count(c) => assert_eq!(c, 5),
            _ => panic!("Expected Count"),
        }
        assert_eq!(result.score, 75); // 5 violations → score 75
    }
}
```

**Step 2: Run to confirm failures**

```bash
rtk cargo test -p barad-dur coupling 2>&1
```
Expected: compile errors / FAIL on todo!().

**Step 3: Implement all three functions**

Replace the `todo!()` stubs:

```rust
pub fn compute_coupling(snapshot: &RepoSnapshot) -> CategoryResult {
    let metrics = vec![
        temporal_coupling(snapshot),
        fan_out_coupling(snapshot),
        demeter_violations(snapshot),
    ];
    CategoryResult {
        name: "Coupling".to_string(),
        score: 0,
        metrics,
    }
    .compute_score()
}

/// File pairs that change together suspiciously often (>70% co-change ratio).
fn temporal_coupling(snapshot: &RepoSnapshot) -> MetricValue {
    let suspicious: Vec<String> = snapshot
        .file_change_pairs
        .iter()
        .filter(|(a, b, count)| {
            let a_ch = snapshot.commits_by_file.get(a).map(|c| c.len()).unwrap_or(0);
            let b_ch = snapshot.commits_by_file.get(b).map(|c| c.len()).unwrap_or(0);
            let min_ch = a_ch.min(b_ch);
            min_ch > 0 && (*count as f64 / min_ch as f64) > 0.7
        })
        .map(|(a, b, count)| {
            format!("{} <> {} ({} co-changes)", a.display(), b.display(), count)
        })
        .collect();

    let count = suspicious.len();
    let score = match count {
        0 => 100,
        1..=3 => 75,
        4..=8 => 50,
        _ => 25,
    };

    MetricValue {
        name: "Temporal coupling".to_string(),
        description: format!("{} suspicious file pairs detected", count),
        raw_value: RawValue::Count(count),
        score,
    }
}

/// Files that co-change with many distinct partners (high fan-out).
fn fan_out_coupling(snapshot: &RepoSnapshot) -> MetricValue {
    let mut partners: HashMap<&PathBuf, std::collections::HashSet<&PathBuf>> = HashMap::new();
    for (a, b, _) in &snapshot.file_change_pairs {
        partners.entry(a).or_default().insert(b);
        partners.entry(b).or_default().insert(a);
    }

    let high_fanout: Vec<String> = partners
        .iter()
        .filter(|(_, ps)| ps.len() > 5)
        .map(|(p, ps)| format!("{} ({} partners)", p.display(), ps.len()))
        .collect();

    let count = high_fanout.len();
    let score = match count {
        0 => 100,
        1..=2 => 75,
        3..=5 => 50,
        _ => 25,
    };

    MetricValue {
        name: "Fan-out coupling".to_string(),
        description: format!("{} files with high fan-out (>5 co-change partners)", count),
        raw_value: RawValue::List(high_fanout),
        score,
    }
}

/// Method chains of depth ≥ 3 summed across all files with AST data.
fn demeter_violations(snapshot: &RepoSnapshot) -> MetricValue {
    let total: u32 = snapshot
        .file_metrics
        .values()
        .map(|m| m.demeter_violations)
        .sum();

    let score = match total {
        0 => 100,
        1..=5 => 75,
        6..=15 => 50,
        _ => 25,
    };

    MetricValue {
        name: "Demeter violations".to_string(),
        description: format!("{} method chain violations detected", total),
        raw_value: RawValue::Count(total as usize),
        score,
    }
}
```

**Step 4: Run tests**

```bash
rtk cargo test coupling 2>&1
```
Expected: PASS.

**Step 5: Commit**

```bash
rtk git add src/metrics/coupling.rs
rtk git commit -m "feat(coupling): add coupling category with temporal, fan-out, and demeter metrics"
```

---

### Task 9: Register coupling module and add weight to config

**Files:**
- Modify: `src/metrics/mod.rs`
- Modify: `src/config.rs`

**Step 1: Register in `mod.rs`**

In `src/metrics/mod.rs`, add:

```rust
pub mod coupling;
```

**Step 2: Add `coupling` field to `CategoryWeights` in `config.rs`**

```rust
pub struct CategoryWeights {
    #[serde(default = "default_health_weight")]
    pub health: u32,
    #[serde(default = "default_team_weight")]
    pub team: u32,
    #[serde(default = "default_evolution_weight")]
    pub evolution: u32,
    #[serde(default = "default_hygiene_weight")]
    pub hygiene: u32,
    #[serde(default = "default_coupling_weight")]
    pub coupling: u32,
}
```

Update the default functions (must sum to 100):

```rust
fn default_health_weight() -> u32 { 25 }
fn default_team_weight() -> u32 { 10 }
fn default_evolution_weight() -> u32 { 25 }
fn default_hygiene_weight() -> u32 { 20 }
fn default_coupling_weight() -> u32 { 20 }
```

Update `Default`:

```rust
impl Default for CategoryWeights {
    fn default() -> Self {
        Self { health: 25, team: 10, evolution: 25, hygiene: 20, coupling: 20 }
    }
}
```

Update `sum()`:

```rust
pub fn sum(&self) -> u32 {
    self.health + self.team + self.evolution + self.hygiene + self.coupling
}
```

Update `as_weight_pairs()`:

```rust
pub fn as_weight_pairs(&self) -> Vec<(&'static str, f64)> {
    let s = self.sum() as f64;
    vec![
        ("Health",    self.health   as f64 / s),
        ("Team",      self.team     as f64 / s),
        ("Evolution", self.evolution as f64 / s),
        ("Git Hygiene", self.hygiene as f64 / s),
        ("Coupling",  self.coupling  as f64 / s),
    ]
}
```

Update the validation error message in `config.rs` to include `coupling={}`.

Update any tests in `config.rs` that assert `weights.sum() == 100` — change the default construction to match new defaults (they already sum to 100, no change needed).

**Step 3: Fix the `#[cfg(test)]` WEIGHTS in `scorer.rs`**

In `src/scorer.rs`, update the test-only constant:

```rust
#[cfg(test)]
const WEIGHTS: &[(&str, f64)] = &[
    ("Health",      0.25),
    ("Team",        0.10),
    ("Evolution",   0.25),
    ("Git Hygiene", 0.20),
    ("Coupling",    0.20),
];
```

**Step 4: Build and test**

```bash
rtk cargo test 2>&1
```
Expected: PASS.

**Step 5: Commit**

```bash
rtk git add src/metrics/mod.rs src/config.rs src/scorer.rs
rtk git commit -m "feat(config): add Coupling weight field, rebalance defaults to 25/10/25/20/20"
```

---

### Task 10: Wire `compute_coupling` into `main.rs`

**Files:**
- Modify: `src/main.rs`

**Step 1: Add import**

At the top of `src/main.rs`, alongside the other metrics imports, add:

```rust
use crate::metrics::coupling;
```

**Step 2: Add to `compute_selected_metrics`**

In the `compute_selected_metrics` function (around line 618), after the hygiene push:

```rust
categories.push(coupling::compute_coupling(snapshot));
```

Also add it to the direct call site around line 277–284 (the non-selected-metrics path):

```rust
let categories = vec![
    health::compute_health(&snapshot, &cfg.thresholds.health),
    team::compute_team(&snapshot, &cfg.thresholds.team),
    coupling::compute_coupling(&snapshot),
];
```

**Step 3: Build and test**

```bash
rtk cargo test 2>&1
```
Expected: PASS.

**Step 4: Commit**

```bash
rtk git add src/main.rs
rtk git commit -m "feat(main): wire compute_coupling into analysis pipeline"
```

---

### Task 11: Update action hints in `scorer.rs`

**Files:**
- Modify: `src/scorer.rs`

**Step 1: Update `target_tab_for_metric`**

Remove old entries (`"Churn hotspots"`, `"Stale code"`, `"File complexity"`). Add new ones:

```rust
"God objects" => (Some("hotspots"), Some("complexity")),
"Complex hotspots" => (Some("hotspots"), Some("complexity")),
"Fan-out coupling" => (Some("coupling"), None),
"Demeter violations" => (Some("coupling"), None),
```

**Step 2: Update `suggest_action`**

Remove old entries (`"Churn hotspots"`, `"Stale code"`, `"File complexity"`). Add new ones:

```rust
"God objects" => "Break down large files by extracting responsibilities into smaller modules",
"Complex hotspots" => "Prioritize refactoring files with both high complexity and high churn",
"Fan-out coupling" => "Reduce dependencies by extracting shared interfaces or facades",
"Demeter violations" => "Apply the Law of Demeter: only call methods on direct collaborators",
```

**Step 3: Build and full test**

```bash
rtk cargo test 2>&1
```
Expected: all tests pass with no warnings.

**Step 4: Commit**

```bash
rtk git add src/scorer.rs
rtk git commit -m "fix(scorer): update action hints and tab targets for new metric names"
```

---

### Task 12: Final verification

**Step 1: Run the full test suite with warnings as errors**

```bash
RUSTFLAGS="-D warnings" rtk cargo test 2>&1
```
Expected: all tests pass, zero warnings.

**Step 2: Self-analysis smoke test**

```bash
rtk cargo build 2>&1 && ./target/debug/barad-dur analyze . --pretty 2>&1 | head -40
```
Expected: report includes both `Health` and `Coupling` categories.

**Step 3: Run mutation testing**

```bash
rtk cargo install cargo-mutants --locked 2>/dev/null; cargo mutants --package barad-dur --file src/metrics/health.rs --file src/metrics/coupling.rs 2>&1 | tail -20
```
Expected: kill rate ≥ 80%.
