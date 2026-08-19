# Hotspot Naming & Vocabulary Heuristics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the three Group E gaps from the crime-scene-book analysis — Ch.5 name-based hotspot triage, Ch.11 friction-vocabulary hygiene signal, and Appendix 1 method-grouping refactor suggestions — as small, additive, TDD'd pure functions.

**Architecture:** Three independent annotations layered onto existing metrics/actions, all built from data already collected: a new `name_smell` module feeds a name-based reason into `god_reason`; a new hygiene sub-metric mirrors `firefighting_ratio`'s exact shape with a different keyword list; and a new `generate_refactoring_actions` action generator (built on an extracted `god_object_files` selector plus a new `group_methods_by_prefix` helper) attaches method-grouping suggestions to god-object findings.

**Tech Stack:** Rust, existing barad-dur `metrics`/`scorer` pipeline.

**Spec:** `docs/superpowers/specs/2026-08-18-hotspot-naming-and-vocabulary-heuristics-design.md`

## Global Constraints

- All three items are **advisory annotations only** — no new score dimension for name-smell or method-grouping. The friction-vocabulary metric *is* a real, scored 5th Git Hygiene metric (this shifts the category's average score — expected, not a bug, per the spec's Interactions section).
- All word/prefix lists are **hardcoded constants**, matching the existing `FIREFIGHTING_KEYWORDS` convention — no new `Thresholds` struct fields, no config migration.
- **No new collector/AST work** — method names (`FileComplexity.functions`), commit messages, and file paths are all already collected today.
- TDD throughout: write the failing test, watch it fail, implement the minimal code, watch it pass, commit.
- Every existing test in a touched file must still pass unchanged after each task (regression guard).
- Final verification must pass `cargo fmt -- --check`, `cargo clippy --all-targets -- -D warnings`, and `RUSTFLAGS=-D warnings cargo test` (CI parity, per root `CLAUDE.md`).

---

### Task 1: Name-smell detection module

**Files:**
- Create: `src/metrics/name_smell.rs`
- Modify: `src/metrics/file_role.rs:70` (bump `stem_lower` visibility to `pub(crate)`)
- Modify: `src/metrics/mod.rs:7` (register the new module)

**Interfaces:**
- Consumes: `crate::metrics::file_role::stem_lower(path: &Path) -> String` (existing, visibility bumped).
- Produces: `pub(crate) fn has_smelly_name(path: &Path) -> bool` — consumed by Task 2.

- [ ] **Step 1: Bump `stem_lower` to `pub(crate)`**

In `src/metrics/file_role.rs`, change:

```rust
fn stem_lower(path: &Path) -> String {
```

to:

```rust
pub(crate) fn stem_lower(path: &Path) -> String {
```

No other change in this file. Run `cargo test file_role` — all existing tests must still pass (pure visibility change, no behavior change).

- [ ] **Step 2: Write the failing tests**

Create `src/metrics/name_smell.rs`:

```rust
//! Name-based hotspot triage: a generic, responsibility-agnostic file name
//! (`Manager`, `Helper`, `Util`) is itself weak evidence a flagged file has
//! no single responsibility — a cheap annotation on hotspots you already
//! found, not a new source of hotspots (see `god_objects.rs::god_reason`).

use std::path::Path;

use crate::metrics::file_role::stem_lower;

const SMELLY_NAME_STEMS: &[&str] = &[
    "manager", "helper", "util", "utils", "handler", "processor", "service", "common", "base",
    "misc",
];

pub(crate) fn has_smelly_name(path: &Path) -> bool {
    let stem = stem_lower(path);
    SMELLY_NAME_STEMS.iter().any(|s| stem.contains(s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn smelly_stems_are_detected() {
        assert!(has_smelly_name(&PathBuf::from("src/UserManager.rs")));
        assert!(has_smelly_name(&PathBuf::from("src/user_service.py")));
        assert!(has_smelly_name(&PathBuf::from("src/Helper.ts")));
        assert!(has_smelly_name(&PathBuf::from("src/common.rs")));
    }

    #[test]
    fn non_smelly_names_are_not_flagged() {
        assert!(!has_smelly_name(&PathBuf::from("src/main.rs")));
        assert!(!has_smelly_name(&PathBuf::from("src/engine.rs")));
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert!(has_smelly_name(&PathBuf::from("src/DATA_MANAGER.rs")));
    }
}
```

- [ ] **Step 3: Register the module**

In `src/metrics/mod.rs`, insert alphabetically between `hygiene` and `team`:

```rust
pub mod hygiene;
pub mod name_smell;
pub mod team;
```

- [ ] **Step 4: Run the tests**

Run: `cargo test name_smell`
Expected: PASS (3/3 tests) — this module has no external dependents yet, so writing the implementation alongside the tests in Step 2 is correct TDD (red would require a separate empty-stub commit for a 15-line module; the spec's own review found that ceremony not worth it here — verify red by temporarily checking out the module do this instead: run `cargo test name_smell` **before** Step 2's file exists → `error[E0433]: failed to resolve` confirms red, then apply Step 2 → green).

- [ ] **Step 5: Commit**

```bash
git add src/metrics/name_smell.rs src/metrics/file_role.rs src/metrics/mod.rs
git commit -m "feat(metrics): add name-based hotspot smell detection"
```

---

### Task 2: Wire name-smell into `god_reason`

**Files:**
- Modify: `src/metrics/health/god_objects.rs:22-52` (`god_reason`), `:83` (call site)

**Interfaces:**
- Consumes: `crate::metrics::name_smell::has_smelly_name(path: &Path) -> bool` (Task 1).
- Produces: `god_reason` gains a `path: &std::path::Path` parameter (first position) — Task 4's extraction depends on this new signature.

- [ ] **Step 1: Write the failing tests**

Add to `src/metrics/health/god_objects.rs`'s `#[cfg(test)] mod tests`:

```rust
#[test]
fn god_objects_notes_generic_name_on_flagged_file() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    snapshot.file_metrics.insert(
        PathBuf::from("UserManager.rs"),
        FileComplexity {
            total_lines: 600,
            loc: 520,
            cyclomatic_complexity: 10,
            public_methods: 5,
            properties: 2,
            ..Default::default()
        },
    );
    add_normal_files(&mut snapshot, 99);
    let result = god_objects(&snapshot, &HealthThresholds::default());
    match &result.raw_value {
        RawValue::List(v) => {
            assert_eq!(v.len(), 1);
            assert_eq!(
                v[0],
                "UserManager.rs — 520 loc; generic name suggests broad responsibility",
                "smelly-named flagged file must get the name-based reason appended"
            );
        }
        _ => panic!("Expected List"),
    }
}

#[test]
fn god_objects_smelly_name_alone_does_not_trigger_flag() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    // Small, well-scoped file with a "smelly" stem — name alone must never flag it.
    snapshot.file_metrics.insert(
        PathBuf::from("common.rs"),
        FileComplexity {
            total_lines: 100,
            loc: 80,
            cyclomatic_complexity: 3,
            public_methods: 2,
            properties: 1,
            ..Default::default()
        },
    );
    let result = god_objects(&snapshot, &HealthThresholds::default());
    assert_eq!(result.score, Some(100));
    match &result.raw_value {
        RawValue::List(v) => assert!(v.is_empty(), "name-smell alone must not create a flag"),
        _ => panic!("Expected List"),
    }
}
```

Note: the existing `god_objects_detects_large_files` test (asserts `v[0] == "fat.rs — 520 loc"` with no name note, since "fat" matches no smelly stem) is the negative-case regression guard — it must keep passing unchanged.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p barad-dur god_objects_notes_generic_name`
Expected: FAIL — actual reason string is `"UserManager.rs — 520 loc"` (no name note yet).

- [ ] **Step 3: Implement**

In `src/metrics/health/god_objects.rs`, change `god_reason`'s signature and body:

```rust
fn god_reason(
    path: &std::path::Path,
    m: &crate::snapshot::FileComplexity,
    degree: usize,
    median_degree: f64,
    thresholds: &HealthThresholds,
) -> Option<String> {
    let mut reasons = Vec::new();
    if m.cyclomatic_complexity > 0 {
        if m.loc > 500 {
            reasons.push(format!("{} loc", m.loc));
        } else if m.loc > 300 && m.public_methods > 15 {
            reasons.push(format!(
                "{} loc, {} public methods",
                m.loc, m.public_methods
            ));
        }
    }
    if is_structural_hub(degree, median_degree, thresholds) {
        let ratio = if median_degree > 0.0 {
            format!("{:.1}x median", degree as f64 / median_degree)
        } else {
            "median 0".to_string()
        };
        reasons.push(format!("structural hub — {degree} connections ({ratio})"));
    }
    if !reasons.is_empty() && crate::metrics::name_smell::has_smelly_name(path) {
        reasons.push("generic name suggests broad responsibility".to_string());
    }
    (!reasons.is_empty()).then(|| reasons.join("; "))
}
```

And update the one call site inside `god_objects()`:

```rust
        .filter_map(|(p, m)| {
            let degree = degrees.get(p.as_path()).copied().unwrap_or(0);
            god_reason(p, m, degree, median_degree, thresholds)
                .map(|reason| format!("{} — {reason}", p.display()))
        })
```

(`p` is already `&PathBuf` in scope here; it deref-coerces to `&Path`.)

- [ ] **Step 4: Run the tests**

Run: `cargo test -p barad-dur god_objects`
Expected: PASS — all pre-existing `god_objects` tests plus the 2 new ones.

- [ ] **Step 5: Commit**

```bash
git add src/metrics/health/god_objects.rs
git commit -m "feat(metrics): annotate god-object findings with name-smell reason"
```

---

### Task 3: Friction-vocabulary hygiene metric

**Files:**
- Modify: `src/metrics/hygiene.rs:4-21` (`compute_hygiene`), after `:318` (new function)

**Interfaces:**
- Produces: `friction_language_ratio(snapshot: &RepoSnapshot, thresholds: &HygieneThresholds) -> MetricValue`, registered as a 5th metric in `compute_hygiene`.

- [ ] **Step 1: Write the failing tests**

Add to `src/metrics/hygiene.rs`'s `#[cfg(test)] mod tests` (mirrors the existing `firefighting_ratio_*` suite exactly):

```rust
#[test]
fn friction_language_ratio_detects_friction_commits() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );

    let now = Utc::now();
    let messages = [
        "feat: add login page",           // normal
        "hack: quick fix for the demo",   // friction
        "fix: typo in README",            // normal
        "workaround for flaky CI",        // friction
        "refactor: clean up modules",     // normal
    ];

    for (i, msg) in messages.iter().enumerate() {
        snapshot.commits.push(Commit {
            id: CommitId(i as u32),
            author: 0,
            timestamp: now - Duration::days(i as i64 + 1),
            message: msg.to_string(),
            files_changed: vec![],
            is_merge: false,
            parent_count: 1,
        });
    }

    let result =
        friction_language_ratio(&snapshot, &crate::config::HygieneThresholds::default());
    match result.raw_value {
        RawValue::Percentage(p) => assert!((p - 40.0).abs() < 1.0, "Expected 40%, got {}", p),
        _ => panic!("Expected Percentage"),
    }
    assert!(
        result.score.unwrap() <= 35,
        "40% friction language should score ≤35, got {:?}",
        result.score
    );
}

#[test]
fn friction_language_ratio_ignores_merge_commits() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );

    let now = Utc::now();
    snapshot.commits = vec![
        Commit {
            id: CommitId(0),
            author: 0,
            timestamp: now - Duration::days(1),
            message: "Merge branch main".into(),
            files_changed: vec![],
            is_merge: true,
            parent_count: 2,
        },
        Commit {
            id: CommitId(1),
            author: 0,
            timestamp: now - Duration::days(2),
            message: "hack: temporary fix".into(),
            files_changed: vec![],
            is_merge: false,
            parent_count: 1,
        },
        Commit {
            id: CommitId(2),
            author: 0,
            timestamp: now - Duration::days(3),
            message: "feat: new feature".into(),
            files_changed: vec![],
            is_merge: false,
            parent_count: 1,
        },
    ];

    let result =
        friction_language_ratio(&snapshot, &crate::config::HygieneThresholds::default());
    match result.raw_value {
        RawValue::Percentage(p) => assert!((p - 50.0).abs() < 1.0, "Expected 50%, got {}", p),
        _ => panic!("Expected Percentage"),
    }
}

#[test]
fn friction_language_ratio_all_keywords_detected() {
    let now = Utc::now();
    for (msg, label) in &[
        ("hack: quick patch", "hack"),
        ("workaround for the bug", "workaround"),
        ("kludge to unblock release", "kludge"),
        ("temporary disable of the check", "temporary"),
        ("fixme: revisit this later", "fixme"),
        ("sorry, this is ugly", "sorry"),
    ] {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.commits = vec![
            Commit {
                id: CommitId(0),
                author: 0,
                timestamp: now - Duration::days(1),
                message: msg.to_string(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
            Commit {
                id: CommitId(1),
                author: 0,
                timestamp: now - Duration::days(2),
                message: "feat: normal commit".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            },
        ];
        let result =
            friction_language_ratio(&snapshot, &crate::config::HygieneThresholds::default());
        match result.raw_value {
            RawValue::Percentage(p) => assert!(
                (p - 50.0).abs() < 1.0,
                "keyword '{}' should yield 50%, got {}",
                label,
                p
            ),
            _ => panic!("Expected Percentage for keyword '{}'", label),
        }
    }
}

#[test]
fn friction_language_ratio_zero_percent_scores_highest() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    let now = Utc::now();
    snapshot.commits = vec![
        Commit {
            id: CommitId(0),
            author: 0,
            timestamp: now - Duration::days(1),
            message: "feat: add login".into(),
            files_changed: vec![],
            is_merge: false,
            parent_count: 1,
        },
        Commit {
            id: CommitId(1),
            author: 0,
            timestamp: now - Duration::days(2),
            message: "refactor: extract module".into(),
            files_changed: vec![],
            is_merge: false,
            parent_count: 1,
        },
    ];
    let result =
        friction_language_ratio(&snapshot, &crate::config::HygieneThresholds::default());
    assert_eq!(result.score, Some(90), "0% friction language should score 90");
}

#[test]
fn friction_language_ratio_returns_na_when_no_commits_in_window() {
    let snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    let result =
        friction_language_ratio(&snapshot, &crate::config::HygieneThresholds::default());
    match result.raw_value {
        RawValue::Text(ref s) => assert_eq!(s, "N/A"),
        _ => panic!("Expected Text(N/A) for empty commit list"),
    }
    assert_eq!(result.score, None);
}

#[test]
fn friction_language_ratio_is_case_insensitive() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    let now = Utc::now();
    snapshot.commits = vec![Commit {
        id: CommitId(0),
        author: 0,
        timestamp: now - Duration::days(1),
        message: "HACK: SHIP IT ANYWAY".into(),
        files_changed: vec![],
        is_merge: false,
        parent_count: 1,
    }];
    let result =
        friction_language_ratio(&snapshot, &crate::config::HygieneThresholds::default());
    match result.raw_value {
        RawValue::Percentage(p) => assert!((p - 100.0).abs() < 1.0),
        _ => panic!("Expected Percentage"),
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p barad-dur friction_language_ratio`
Expected: FAIL with "cannot find function `friction_language_ratio`".

- [ ] **Step 3: Implement**

Add to `src/metrics/hygiene.rs`, right after `firefighting_ratio` (after line 318):

```rust
const FRICTION_KEYWORDS: &[&str] = &["hack", "workaround", "kludge", "temporary", "fixme", "sorry"];

/// Percentage of commits whose message admits technical-debt friction
/// (hacks, workarounds, temporary fixes) — a different social signal than
/// `firefighting_ratio`'s reactive-incident-response keywords: this one
/// signals debt knowingly shipped, not something that broke.
fn friction_language_ratio(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::HygieneThresholds,
) -> MetricValue {
    let window_commits: Vec<_> = snapshot
        .commits
        .iter()
        .filter(|c| !c.is_merge && snapshot.time_window.contains(&c.timestamp))
        .collect();

    if window_commits.is_empty() {
        return MetricValue {
            name: "Friction language ratio".to_string(),
            description: "No commits in window".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
    }

    let friction = window_commits
        .iter()
        .filter(|c| {
            let msg = c.message.to_lowercase();
            FRICTION_KEYWORDS.iter().any(|kw| msg.contains(kw))
        })
        .count();

    let total = window_commits.len();
    let pct = (friction as f64 / total as f64) * 100.0;

    let score = if pct < 2.0 {
        90
    } else if pct < 5.0 {
        75
    } else if pct < 10.0 {
        55
    } else if pct < 20.0 {
        35
    } else {
        20
    };

    MetricValue {
        name: "Friction language ratio".to_string(),
        description: format!(
            "{friction} commit(s) admitting technical-debt friction ({pct:.1}% of {total} non-merge commits)"
        ),
        raw_value: RawValue::Percentage(pct),
        score: Some(score),
    }
}
```

Register it in `compute_hygiene` (line 4-13):

```rust
pub fn compute_hygiene(
    snapshot: &RepoSnapshot,
    thresholds: &crate::config::HygieneThresholds,
) -> CategoryResult {
    let metrics = vec![
        commit_message_quality(snapshot, thresholds),
        history_cleanliness(snapshot, thresholds),
        gitignore_coverage(snapshot, thresholds),
        firefighting_ratio(snapshot, thresholds),
        friction_language_ratio(snapshot, thresholds),
    ];

    CategoryResult {
        name: "Git Hygiene".to_string(),
        score: 0,
        metrics,
    }
    .compute_score()
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p barad-dur hygiene`
Expected: PASS — all pre-existing hygiene tests plus the 6 new ones.

- [ ] **Step 5: Commit**

```bash
git add src/metrics/hygiene.rs
git commit -m "feat(metrics): add friction-language-ratio hygiene metric"
```

---

### Task 4: Extract `god_object_files` reusable selector

**Files:**
- Modify: `src/metrics/health/god_objects.rs:56-116` (`god_objects`, new `god_object_files`)
- Modify: `src/metrics/health/mod.rs` (re-export)

**Interfaces:**
- Consumes: `god_reason(path, m, degree, median_degree, thresholds)` (Task 2's signature).
- Produces: `pub(crate) fn god_object_files(snapshot: &RepoSnapshot, thresholds: &HealthThresholds) -> Vec<(PathBuf, String)>`, re-exported as `crate::metrics::health::god_object_files` — consumed by Task 5.

- [ ] **Step 1: Write the failing parity test**

Add to `src/metrics/health/god_objects.rs`'s test module:

```rust
#[test]
fn god_object_files_matches_god_objects_flagged_set() {
    // Regression guard for the extraction below: `god_objects()`'s display
    // list and `god_object_files()`'s structured list must always agree on
    // which files qualify (same "one definition, not two" rule as the M5
    // corroboration-predicate extraction).
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    snapshot.file_metrics.insert(
        PathBuf::from("fat.rs"),
        FileComplexity {
            total_lines: 600,
            loc: 520,
            cyclomatic_complexity: 10,
            public_methods: 5,
            properties: 2,
            ..Default::default()
        },
    );
    add_normal_files(&mut snapshot, 99);
    let thresholds = HealthThresholds::default();
    let result = god_objects(&snapshot, &thresholds);
    let files = god_object_files(&snapshot, &thresholds);
    let expected: Vec<String> = files
        .iter()
        .map(|(p, reason)| format!("{} — {reason}", p.display()))
        .collect();
    match &result.raw_value {
        RawValue::List(v) => assert_eq!(v, &expected),
        _ => panic!("Expected List"),
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p barad-dur god_object_files_matches`
Expected: FAIL with "cannot find function `god_object_files`".

- [ ] **Step 3: Implement the extraction**

In `src/metrics/health/god_objects.rs`, replace the body of `god_objects` (lines 56-116) with the extraction plus a thin wrapper:

```rust
/// Files flagged as god objects, with their reason string — the single
/// definition `god_objects()`'s display list and any downstream action
/// generator (`generate_refactoring_actions`) share, so they never diverge
/// on which files qualify.
pub(crate) fn god_object_files(
    snapshot: &RepoSnapshot,
    thresholds: &HealthThresholds,
) -> Vec<(std::path::PathBuf, String)> {
    let incoming = crate::metrics::incoming_import_counts(&snapshot.import_graph);

    let degrees: std::collections::HashMap<&std::path::Path, usize> = snapshot
        .file_metrics
        .keys()
        .filter(|p| is_source_file(p))
        .map(|p| {
            let outgoing = crate::metrics::outgoing_degree(&snapshot.import_graph, p);
            let inc = incoming.get(p.as_path()).copied().unwrap_or(0);
            (p.as_path(), outgoing + inc)
        })
        .collect();

    let degree_values: Vec<usize> = degrees.values().copied().collect();
    let median_degree = median(&degree_values);

    let mut flagged: Vec<(std::path::PathBuf, String)> = snapshot
        .file_metrics
        .iter()
        .filter(|(p, _)| is_source_file(p))
        .filter_map(|(p, m)| {
            let degree = degrees.get(p.as_path()).copied().unwrap_or(0);
            god_reason(p, m, degree, median_degree, thresholds).map(|reason| (p.clone(), reason))
        })
        .collect();
    // snapshot.file_metrics is a HashMap — sort for deterministic report output.
    flagged.sort_by(|a, b| a.0.cmp(&b.0));
    flagged
}

/// Files that have grown too large to maintain (god objects / bloaters), or
/// that dominate the import graph as a structural hub.
pub(super) fn god_objects(snapshot: &RepoSnapshot, thresholds: &HealthThresholds) -> MetricValue {
    let source_total = snapshot
        .file_metrics
        .keys()
        .filter(|p| is_source_file(p))
        .count();

    let gods: Vec<String> = god_object_files(snapshot, thresholds)
        .into_iter()
        .map(|(p, reason)| format!("{} — {reason}", p.display()))
        .collect();

    let count = gods.len();
    let pct = if source_total > 0 {
        count as f64 / source_total as f64 * 100.0
    } else {
        0.0
    };

    let score = if count == 0 {
        100
    } else if pct <= 2.0 {
        75
    } else if pct <= 8.0 {
        50
    } else {
        25
    };

    MetricValue {
        name: "God objects".to_string(),
        description: format!(
            "{}/{} source files oversized or structurally overconnected ({:.1}%)",
            count, source_total, pct
        ),
        raw_value: RawValue::List(gods),
        score: Some(score),
    }
}
```

In `src/metrics/health/mod.rs`, add a re-export right after `mod god_objects;`:

```rust
mod god_objects;
pub(crate) use god_objects::god_object_files;
```

- [ ] **Step 4: Run the full health test suite**

Run: `cargo test -p barad-dur god_objects`
Expected: PASS — every pre-existing `god_objects` test (all 15+ of them, unchanged assertions) plus the new parity test, proving the refactor is behavior-identical.

- [ ] **Step 5: Commit**

```bash
git add src/metrics/health/god_objects.rs src/metrics/health/mod.rs
git commit -m "refactor(metrics): extract god_object_files as a reusable selector"
```

---

### Task 5: Method-grouping refactor suggestions, wired into the report

**Files:**
- Modify: `src/scorer/actions.rs` (new `group_methods_by_prefix`, new `generate_refactoring_actions`)
- Modify: `src/scorer.rs:9,59-108` (`build_report` signature + wiring)
- Modify: `src/cmd/analyze.rs:93-99`, `src/cmd/gate.rs:51-57`, `src/backfill/mod.rs:66-72` (call sites)
- Modify: `src/scorer.rs` test module (8 call sites), `tests/pressman_coupling_milestone_2.rs` (2 call sites), `tests/pressman_coupling_milestone_4.rs` (2 call sites)

**Interfaces:**
- Consumes: `crate::metrics::health::god_object_files(snapshot, thresholds) -> Vec<(PathBuf, String)>` (Task 4), `crate::snapshot::FunctionMetrics { name, loc, cyclomatic_complexity, max_nesting_depth }` (existing).
- Produces: `generate_refactoring_actions(snapshot: &RepoSnapshot, thresholds: &HealthThresholds) -> Vec<ActionItem>`; `build_report` gains a 6th parameter `health: &crate::config::HealthThresholds` (last position).

- [ ] **Step 1: Write the failing unit tests for `group_methods_by_prefix`**

Add to `src/scorer/actions.rs`'s existing `#[cfg(test)] mod tests` (starts at line 265):

```rust
fn fm(name: &str) -> crate::snapshot::FunctionMetrics {
    crate::snapshot::FunctionMetrics {
        name: name.to_string(),
        loc: 10,
        cyclomatic_complexity: 2,
        max_nesting_depth: 1,
    }
}

#[test]
fn group_methods_by_prefix_groups_shared_verbs() {
    let functions = vec![
        fm("handle_a"),
        fm("handle_b"),
        fm("handle_c"),
        fm("validate_x"),
        fm("validate_y"),
        fm("parse_one"),
    ];
    let groups = group_methods_by_prefix(&functions);
    assert_eq!(
        groups,
        vec![
            ("handle_", vec!["handle_a", "handle_b", "handle_c"]),
            ("validate_", vec!["validate_x", "validate_y"]),
        ]
    );
}

#[test]
fn group_methods_by_prefix_excludes_singleton_groups() {
    let functions = vec![fm("parse_only_one"), fm("main")];
    assert!(group_methods_by_prefix(&functions).is_empty());
}

#[test]
fn group_methods_by_prefix_returns_empty_for_no_matches() {
    let functions = vec![fm("run")];
    assert!(group_methods_by_prefix(&functions).is_empty());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p barad-dur group_methods_by_prefix`
Expected: FAIL with "cannot find function `group_methods_by_prefix`".

- [ ] **Step 3: Implement `group_methods_by_prefix`**

Add to `src/scorer/actions.rs`, after `generate_coupling_actions` (after its closing brace, currently ending around line 153):

```rust
const GROUPING_PREFIXES: &[&str] = &[
    "get_", "set_", "handle_", "validate_", "build_", "compute_", "parse_", "render_", "is_",
    "has_",
];

/// Group a file's function names by a known verb prefix — a cheap split-
/// boundary suggestion for a god-object file (Appendix 1). Only groups with
/// ≥2 members are returned; a lone `handle_x` isn't a split boundary.
fn group_methods_by_prefix(
    functions: &[crate::snapshot::FunctionMetrics],
) -> Vec<(&'static str, Vec<&str>)> {
    let mut groups: HashMap<&'static str, Vec<&str>> = HashMap::new();
    for f in functions {
        if let Some(prefix) = GROUPING_PREFIXES.iter().find(|p| f.name.starts_with(**p)) {
            groups.entry(prefix).or_default().push(f.name.as_str());
        }
    }
    let mut result: Vec<(&'static str, Vec<&str>)> = groups
        .into_iter()
        .filter(|(_, names)| names.len() >= 2)
        .collect();
    for (_, names) in result.iter_mut() {
        names.sort();
    }
    result.sort_by_key(|(prefix, _)| *prefix);
    result
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p barad-dur group_methods_by_prefix`
Expected: PASS (3/3).

- [ ] **Step 5: Write the failing tests for `generate_refactoring_actions`**

Add to the same test module:

```rust
#[test]
fn generate_refactoring_actions_emits_action_for_clustering_god_object() {
    let mut snapshot = crate::snapshot::RepoSnapshot::new(
        std::path::PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        crate::snapshot::TimeWindow::default(),
    );
    snapshot.file_metrics.insert(
        std::path::PathBuf::from("god.rs"),
        crate::snapshot::FileComplexity {
            total_lines: 600,
            loc: 520,
            cyclomatic_complexity: 10,
            public_methods: 5,
            properties: 2,
            functions: vec![fm("handle_a"), fm("handle_b"), fm("main")],
            ..Default::default()
        },
    );
    let thresholds = crate::config::HealthThresholds::default();
    let actions = generate_refactoring_actions(&snapshot, &thresholds);
    assert_eq!(actions.len(), 1);
    assert!(actions[0].text.contains("god.rs"));
    assert!(actions[0].text.contains("handle_* (2)"));
    assert_eq!(actions[0].target_tab, Some("hotspots".to_string()));
}

#[test]
fn generate_refactoring_actions_skips_god_object_with_no_clustering() {
    let mut snapshot = crate::snapshot::RepoSnapshot::new(
        std::path::PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        crate::snapshot::TimeWindow::default(),
    );
    snapshot.file_metrics.insert(
        std::path::PathBuf::from("god.rs"),
        crate::snapshot::FileComplexity {
            total_lines: 600,
            loc: 520,
            cyclomatic_complexity: 10,
            public_methods: 5,
            properties: 2,
            functions: vec![fm("run")],
            ..Default::default()
        },
    );
    let thresholds = crate::config::HealthThresholds::default();
    assert!(generate_refactoring_actions(&snapshot, &thresholds).is_empty());
}

#[test]
fn generate_refactoring_actions_skips_non_god_object_with_clustering_names() {
    let mut snapshot = crate::snapshot::RepoSnapshot::new(
        std::path::PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        crate::snapshot::TimeWindow::default(),
    );
    // Small file (not flagged as a god object) with clustering method names —
    // proves the shared-selection-function gate (decision 6 in the spec).
    snapshot.file_metrics.insert(
        std::path::PathBuf::from("small.rs"),
        crate::snapshot::FileComplexity {
            total_lines: 50,
            loc: 40,
            cyclomatic_complexity: 3,
            public_methods: 2,
            properties: 1,
            functions: vec![fm("handle_a"), fm("handle_b")],
            ..Default::default()
        },
    );
    let thresholds = crate::config::HealthThresholds::default();
    assert!(generate_refactoring_actions(&snapshot, &thresholds).is_empty());
}
```

- [ ] **Step 6: Run to verify failure**

Run: `cargo test -p barad-dur generate_refactoring_actions`
Expected: FAIL with "cannot find function `generate_refactoring_actions`".

- [ ] **Step 7: Implement `generate_refactoring_actions`**

Add to `src/scorer/actions.rs`, after `group_methods_by_prefix`:

```rust
/// Per-file method-grouping refactor suggestions for god-object files
/// (Appendix 1) — groups function names by shared verb prefix to hint at a
/// split boundary that already exists in the code. Advisory only: files
/// with no qualifying group get no action.
pub(super) fn generate_refactoring_actions(
    snapshot: &crate::snapshot::RepoSnapshot,
    thresholds: &crate::config::HealthThresholds,
) -> Vec<ActionItem> {
    crate::metrics::health::god_object_files(snapshot, thresholds)
        .into_iter()
        .filter_map(|(path, _reason)| {
            let functions = &snapshot.file_metrics.get(&path)?.functions;
            let groups = group_methods_by_prefix(functions);
            if groups.is_empty() {
                return None;
            }
            let groups_text = groups
                .iter()
                .map(|(prefix, names)| format!("{prefix}* ({})", names.len()))
                .collect::<Vec<_>>()
                .join(", ");
            Some(ActionItem {
                text: format!(
                    "[Health] {} — consider splitting by responsibility: {}",
                    path.display(),
                    groups_text
                ),
                target_tab: Some("hotspots".to_string()),
                sort_by: Some("complexity".to_string()),
            })
        })
        .collect()
}
```

- [ ] **Step 8: Run the tests**

Run: `cargo test -p barad-dur generate_refactoring_actions`
Expected: PASS (3/3).

- [ ] **Step 9: Wire into `build_report`**

In `src/scorer.rs:9`, extend the import:

```rust
use actions::{generate_coupling_actions, generate_refactoring_actions, generate_top_actions};
```

In `src/scorer.rs`, change `build_report`'s signature (line 59-65) to add a 6th parameter:

```rust
pub fn build_report(
    snapshot: &RepoSnapshot,
    categories: Vec<CategoryResult>,
    remote_meta: Option<RemoteMeta>,
    weights: &[(&str, f64)],
    coupling: &crate::config::CouplingThresholds,
    health: &crate::config::HealthThresholds,
) -> AnalysisReport {
```

And change the `top_actions` line (line 67) to extend with refactoring actions:

```rust
    let mut top_actions = generate_top_actions(&categories);
    top_actions.extend(generate_refactoring_actions(snapshot, health));
```

- [ ] **Step 10: Update the 3 production call sites**

In `src/cmd/analyze.rs` (around line 93-99), add `&cfg.thresholds.health,` after `&cfg.thresholds.coupling,`:

```rust
    let mut report = scorer::build_report(
        &snapshot,
        categories,
        remote_meta,
        &weight_pairs,
        &cfg.thresholds.coupling,
        &cfg.thresholds.health,
    );
```

In `src/cmd/gate.rs` (around line 51-57), same pattern:

```rust
    let report = scorer::build_report(
        &snapshot,
        categories,
        None,
        &weight_pairs,
        &cfg.thresholds.coupling,
        &cfg.thresholds.health,
    );
```

In `src/backfill/mod.rs` (around line 66-72), same pattern:

```rust
        let report = scorer::build_report(
            &snapshot,
            categories,
            None,
            &weight_pairs,
            &cfg.thresholds.coupling,
            &cfg.thresholds.health,
        );
```

- [ ] **Step 11: Update all test call sites**

In `src/scorer.rs`'s test module, every `build_report(...)` call ends with a line reading either `&crate::config::CouplingThresholds::default(),` (8 occurrences: lines ~148, 176, 331, 357, 389, 411, 436, 452). After each such line, insert:

```rust
            &crate::config::HealthThresholds::default(),
```

Verify with `grep -c "CouplingThresholds::default()" src/scorer.rs` before and `grep -c "HealthThresholds::default()" src/scorer.rs` after — both counts must match (8).

In `tests/pressman_coupling_milestone_2.rs`, both `build_report(...)` calls end with `&default_cfg.thresholds.coupling,` (lines ~31, 88). After each, insert:

```rust
        &default_cfg.thresholds.health,
```

In `tests/pressman_coupling_milestone_4.rs`, both `build_report(...)` calls end with `&cfg.thresholds.coupling,` (lines ~89, 134). After each, insert:

```rust
        &cfg.thresholds.health,
```

- [ ] **Step 12: Run the full test suite**

Run: `RUSTFLAGS="-D warnings" cargo test`
Expected: PASS — every existing test across the workspace, plus every test added in Tasks 1-5.

- [ ] **Step 13: Full CI-parity verification**

```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
RUSTFLAGS="-D warnings" cargo test
```

Expected: all three clean.

- [ ] **Step 14: Dogfood sanity pass**

```bash
cargo run -- analyze . -v
```

Confirm visually: the Git Hygiene category now shows 5 metrics including "Friction language ratio"; any god-object findings with clustering method names show a `[Health] ... consider splitting by responsibility: ...` action in "Top Actions"; any smelly-named god-object finding's reason string ends with "generic name suggests broad responsibility". No assertion — a sanity pass, same as the M5 design's dogfood step.

- [ ] **Step 15: Commit**

```bash
git add src/scorer/actions.rs src/scorer.rs src/cmd/analyze.rs src/cmd/gate.rs src/backfill/mod.rs \
        tests/pressman_coupling_milestone_2.rs tests/pressman_coupling_milestone_4.rs
git commit -m "feat(scorer): add method-grouping refactor actions for god objects"
```
