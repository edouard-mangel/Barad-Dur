# Organizational (Conway's-Law) Coupling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a sixth Team metric, `cross_team_coupling`, that flags day-bucketed
co-change pairs whose two files have different primary (blame-dominant) authors —
Ch. 12 of *Your Code as a Crime Scene*.

**Architecture:** Pure `(snapshot) → MetricValue` computation, no collector or config
changes. Two new pure helpers in `src/metrics/team/mod.rs` (`primary_author`,
`day_bucketed_pairs`) feed a new `cross_team_coupling` metric wired into
`compute_team`, which gains a `&CouplingThresholds` parameter (three call sites).

**Tech Stack:** Rust; existing `RepoSnapshot` data only (`blame_map`, `commits`,
`authors`). Tests via `cargo test`, mutation gate via `cargo mutants --in-diff`
(≥ 80% kill rate on the MR).

**Spec:** `docs/superpowers/specs/2026-08-18-organizational-coupling-design.md`

## Global Constraints

- Functional style: pure functions, iterator chains, no mutation of inputs
  (CLAUDE.md paradigm).
- TDD mandatory: every step below writes and *watches* the failing test before
  implementation.
- Mutation-hardening assertion style (established in the call-graph MRs): exact
  expected values, both-sides boundary tests, exact evidence strings, sorted-output
  determinism tests — never bare `!is_empty()`.
- Do NOT touch `count_co_changed_pairs`, `file_change_pairs`,
  `qualifying_smell_pairs`, or anything in `src/metrics/coupling/` — the design's
  Decision 2 explicitly scopes day-bucketing to this feature only.
- Reuse `CouplingThresholds.change_coupling_min_ratio` for ratio qualification; add
  NO new config fields (design "Configuration" section).
- Commit messages: conventional-commit style, no AI attribution, written to a file
  and committed with `git commit -F <file>` (a hook injects trailers into `-m`;
  verify with `git cat-file commit HEAD`).
- Branch: `worktree-feat+org-coupling` off current `main`; one MR at the end.

---

### Task 1: `primary_author` helper

**Files:**
- Modify: `src/metrics/team/mod.rs` (add function + tests in its `#[cfg(test)]` module — team tests live at the bottom of the same file, follow that placement)

**Interfaces:**
- Consumes: `crate::metrics::author_line_counts(&[BlameLine]) -> HashMap<usize, usize>` (already imported at the top of `team/mod.rs`), `crate::snapshot::BlameLine`.
- Produces: `fn primary_author(lines: &[BlameLine]) -> Option<usize>` — the `author_id` holding a *strict* majority (> 50%) of blamed lines, `None` otherwise. Task 3 calls this.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/metrics/team/mod.rs` (it already has
`use super::*;` and `crate::metrics::testutil` available; `BlameLine::new(author_id, timestamp)`
constructs a 1-line entry — set `line_count` directly for multi-line runs):

```rust
mod primary_author_tests {
    use super::*;
    use crate::snapshot::BlameLine;
    use chrono::Utc;

    fn lines(counts: &[(usize, usize)]) -> Vec<BlameLine> {
        counts
            .iter()
            .map(|&(author_id, line_count)| {
                let mut l = BlameLine::new(author_id, Utc::now());
                l.line_count = line_count;
                l
            })
            .collect()
    }

    #[test]
    fn empty_blame_has_no_primary_author() {
        assert_eq!(primary_author(&[]), None);
    }

    #[test]
    fn exact_fifty_fifty_split_has_no_primary_author() {
        // Mirrors bus_factor.rs's strict-majority semantics (`max * 2 > total`).
        assert_eq!(primary_author(&lines(&[(0, 50), (1, 50)])), None);
    }

    #[test]
    fn fifty_one_forty_nine_yields_the_majority_author() {
        assert_eq!(primary_author(&lines(&[(0, 49), (1, 51)])), Some(1));
    }

    #[test]
    fn single_author_file_yields_that_author() {
        assert_eq!(primary_author(&lines(&[(3, 10)])), Some(3));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib metrics::team::tests::primary_author -- --nocapture 2>&1 | tail -5`
Expected: compile error `cannot find function primary_author` (a compile-fail IS
the red for a missing function in Rust).

- [ ] **Step 3: Write the implementation**

Add above the `tests` module in `src/metrics/team/mod.rs`:

```rust
/// The author holding a *strict* majority (> 50%) of a file's blamed lines
/// — the "main developer" proxy from the org-coupling design (Decision 1).
/// `None` when blame is empty or no author clears the majority (a
/// collectively-owned file has no single owner to mismatch against).
/// Same strict-majority rule as `bus_factor`'s `is_file_author_dominated`,
/// but returns *which* author instead of discarding it.
fn primary_author(lines: &[crate::snapshot::BlameLine]) -> Option<usize> {
    let counts = author_line_counts(lines);
    let total: usize = counts.values().sum();
    counts
        .into_iter()
        .find(|&(_, count)| count * 2 > total)
}
```

Note: `find` on the `(author, count)` pairs is safe because at most one author can
hold a strict majority — no tie-breaking needed. But `find` returns the pair; map it:

```rust
    counts
        .into_iter()
        .find(|&(_, count)| count * 2 > total)
        .map(|(author, _)| author)
```

(Use the second form — the first doesn't type-check; it's shown only to explain the
uniqueness argument.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib metrics::team::tests::primary_author 2>&1 | tail -3`
Expected: `4 passed; 0 failed`. Also run `cargo test --lib metrics::team 2>&1 | tail -3`
— everything else still green. A `dead_code` warning on `primary_author` is expected
until Task 3 wires it; do not commit yet if using `-D warnings` locally — Task 1 and
Task 2 land together with Task 3's wiring in one MR, but each task still commits
(the pre-push hook, not per-commit CI, enforces warnings; `#[allow(dead_code)]` is
NOT the answer — instead commit Tasks 1–3 in order and only push after Task 3).

- [ ] **Step 5: Commit**

```bash
printf 'feat(team): add primary_author blame-majority helper\n\nStrict-majority (>50%%) blamed-lines author per file, or None for\ncollectively-owned files. First half of the Ch. 12 org-coupling signal\n(design: docs/superpowers/specs/2026-08-18-organizational-coupling-design.md).\n' > /tmp/msg.txt
git add src/metrics/team/mod.rs && git commit -F /tmp/msg.txt
git cat-file commit HEAD | tail -4   # verify no injected trailers
```

---

### Task 2: `day_bucketed_pairs` helper

**Files:**
- Modify: `src/metrics/team/mod.rs`

**Interfaces:**
- Consumes: `snapshot.commits` (`Vec<Commit>` with `author: usize`, `timestamp: DateTime<Utc>`, `files_changed: Vec<FileChange>` where `FileChange.path: PathBuf`), `snapshot.files` (`Vec<FileEntry>`, `.path`).
- Produces:
  - `fn day_bucketed_pairs(snapshot: &RepoSnapshot) -> Vec<(PathBuf, PathBuf, usize)>` — pairs `(a, b, bucket_count)` with `a < b` lexicographically, sorted by `(a, b)`, counting distinct `(author, utc_day)` buckets in which the same author touched both files. Only files present in `snapshot.files` count (mirrors `count_co_changed_pairs`'s known-files filter).
  - `fn day_bucket_counts(snapshot: &RepoSnapshot) -> HashMap<PathBuf, usize>` — per file, the number of distinct `(author, utc_day)` buckets it appears in (the ratio denominator source; spec's "Note on day-bucketing").
- Both consumed by Task 3.

- [ ] **Step 1: Write the failing tests**

Test-fixture note: build snapshots with `crate::metrics::testutil::make_snapshot()`,
push `Commit` values directly. A minimal commit helper keeps the tests readable.
`CommitId` and `ChangeType` come from `crate::snapshot`. Timestamps: use
`chrono::TimeZone` — `Utc.with_ymd_and_hms(2026, 8, 19, 9, 0, 0).unwrap()` for
"same day, different commit", vary the day for the different-day case.

```rust
mod day_bucketed_pairs_tests {
    use super::*;
    use crate::metrics::testutil::{make_file, make_snapshot};
    use crate::snapshot::{ChangeType, Commit, CommitId, FileChange, RepoSnapshot};
    use chrono::{TimeZone, Utc};
    use std::path::PathBuf;

    fn commit(id: u32, author: usize, day: u32, hour: u32, paths: &[&str]) -> Commit {
        Commit {
            id: CommitId(id),
            author,
            timestamp: Utc.with_ymd_and_hms(2026, 8, day, hour, 0, 0).unwrap(),
            message: String::new(),
            files_changed: paths
                .iter()
                .map(|p| FileChange {
                    path: PathBuf::from(p),
                    additions: 1,
                    deletions: 0,
                    change_type: ChangeType::Modified,
                })
                .collect(),
            is_merge: false,
            parent_count: 1,
        }
    }

    fn snap(commits: Vec<Commit>, files: &[&str]) -> RepoSnapshot {
        let mut s = make_snapshot();
        s.files = files.iter().map(|f| make_file(f)).collect();
        s.commits = commits;
        s
    }

    #[test]
    fn same_author_same_day_separate_commits_pair() {
        // The whole point of day-bucketing: two commits, same author, same
        // UTC day — exact-commit pairing would miss this.
        let s = snap(
            vec![
                commit(0, 1, 19, 9, &["a.rs"]),
                commit(1, 1, 19, 14, &["b.rs"]),
            ],
            &["a.rs", "b.rs"],
        );
        assert_eq!(
            day_bucketed_pairs(&s),
            vec![(PathBuf::from("a.rs"), PathBuf::from("b.rs"), 1)]
        );
    }

    #[test]
    fn same_author_different_days_do_not_pair() {
        let s = snap(
            vec![
                commit(0, 1, 19, 9, &["a.rs"]),
                commit(1, 1, 20, 9, &["b.rs"]),
            ],
            &["a.rs", "b.rs"],
        );
        assert_eq!(day_bucketed_pairs(&s), vec![]);
    }

    #[test]
    fn different_authors_same_day_do_not_pair() {
        // Pairing is per-(author, day) — repo-wide same-day coincidence is
        // not a coupling signal (spec Risks section).
        let s = snap(
            vec![
                commit(0, 1, 19, 9, &["a.rs"]),
                commit(1, 2, 19, 9, &["b.rs"]),
            ],
            &["a.rs", "b.rs"],
        );
        assert_eq!(day_bucketed_pairs(&s), vec![]);
    }

    #[test]
    fn bucket_count_is_distinct_author_days_not_commit_count() {
        // Author 1 touches the pair on two days (three commits total) —
        // count is 2 buckets, not 3 commits. Kills += / counting mutants.
        let s = snap(
            vec![
                commit(0, 1, 19, 9, &["a.rs", "b.rs"]),
                commit(1, 1, 19, 15, &["a.rs", "b.rs"]),
                commit(2, 1, 20, 9, &["a.rs", "b.rs"]),
            ],
            &["a.rs", "b.rs"],
        );
        assert_eq!(
            day_bucketed_pairs(&s),
            vec![(PathBuf::from("a.rs"), PathBuf::from("b.rs"), 2)]
        );
    }

    #[test]
    fn files_outside_the_known_tree_are_ignored() {
        // Excluded files (not in snapshot.files) never form pairs —
        // mirrors count_co_changed_pairs's known-files filter.
        let s = snap(
            vec![commit(0, 1, 19, 9, &["a.rs", "vendor/x.rs"])],
            &["a.rs"],
        );
        assert_eq!(day_bucketed_pairs(&s), vec![]);
    }

    #[test]
    fn pairs_are_lexicographic_and_sorted() {
        // Input order z-before-a; output must normalize (a < z within the
        // pair) and sort across pairs — determinism for report output.
        let s = snap(
            vec![
                commit(0, 1, 19, 9, &["z.rs", "a.rs"]),
                commit(1, 1, 20, 9, &["b.rs", "a.rs"]),
            ],
            &["a.rs", "b.rs", "z.rs"],
        );
        assert_eq!(
            day_bucketed_pairs(&s),
            vec![
                (PathBuf::from("a.rs"), PathBuf::from("b.rs"), 1),
                (PathBuf::from("a.rs"), PathBuf::from("z.rs"), 1),
            ]
        );
    }

    #[test]
    fn day_bucket_counts_count_distinct_author_days_per_file() {
        // a.rs: author 1 on day 19 + day 20, author 2 on day 19 → 3 buckets.
        let s = snap(
            vec![
                commit(0, 1, 19, 9, &["a.rs"]),
                commit(1, 1, 19, 15, &["a.rs"]), // same bucket as commit 0
                commit(2, 1, 20, 9, &["a.rs"]),
                commit(3, 2, 19, 9, &["a.rs"]),
            ],
            &["a.rs"],
        );
        let counts = day_bucket_counts(&s);
        assert_eq!(counts.get(&PathBuf::from("a.rs")), Some(&3));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib metrics::team::tests::day_bucketed 2>&1 | tail -5`
Expected: compile error `cannot find function day_bucketed_pairs` / `day_bucket_counts`.

- [ ] **Step 3: Write the implementation**

Add above the `tests` module in `src/metrics/team/mod.rs`:

```rust
use chrono::Datelike;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

/// The (author, UTC calendar day) bucket key for a commit. Day-granularity
/// per the org-coupling design (Decision 2): the same author touching two
/// files in separate commits a few hours apart is still one coordination
/// context.
fn bucket_key(commit: &crate::snapshot::Commit) -> (usize, i32, u32) {
    (
        commit.author,
        commit.timestamp.year(),
        commit.timestamp.ordinal(),
    )
}

/// Known-tree files touched in each (author, day) bucket. Shared core of
/// `day_bucketed_pairs` and `day_bucket_counts` so both count the same
/// universe.
fn files_by_bucket(snapshot: &RepoSnapshot) -> BTreeMap<(usize, i32, u32), HashSet<&PathBuf>> {
    let known: HashSet<&PathBuf> = snapshot.files.iter().map(|f| &f.path).collect();
    snapshot
        .commits
        .iter()
        .fold(BTreeMap::new(), |mut buckets, commit| {
            let entry = buckets.entry(bucket_key(commit)).or_default();
            commit
                .files_changed
                .iter()
                .filter_map(|fc| known.get(&fc.path).copied())
                .for_each(|p| {
                    entry.insert(p);
                });
            buckets
        })
}

/// Co-changed file pairs grouped by (author, UTC day) instead of exact
/// commit — a *separate* data source from `snapshot.file_change_pairs`;
/// existing coupling metrics are untouched (design Decision 2). Pairs are
/// lexicographically normalized (a < b) and sorted for determinism.
fn day_bucketed_pairs(snapshot: &RepoSnapshot) -> Vec<(PathBuf, PathBuf, usize)> {
    let pair_counts: BTreeMap<(PathBuf, PathBuf), usize> = files_by_bucket(snapshot)
        .into_values()
        .flat_map(|files| {
            let mut sorted: Vec<&PathBuf> = files.into_iter().collect();
            sorted.sort();
            (0..sorted.len())
                .flat_map(move |i| {
                    let sorted = sorted.clone();
                    (i + 1..sorted.len())
                        .map(move |j| (sorted[i].clone(), sorted[j].clone()))
                })
                .collect::<Vec<_>>()
        })
        .fold(BTreeMap::new(), |mut m, pair| {
            *m.entry(pair).or_insert(0) += 1;
            m
        });
    pair_counts
        .into_iter()
        .map(|((a, b), count)| (a, b, count))
        .collect()
}

/// Per file: the number of distinct (author, day) buckets it appears in —
/// the ratio denominator for day-bucketed qualification (spec's "Note on
/// day-bucketing").
fn day_bucket_counts(snapshot: &RepoSnapshot) -> HashMap<PathBuf, usize> {
    files_by_bucket(snapshot)
        .into_values()
        .flat_map(|files| files.into_iter().cloned().collect::<Vec<_>>())
        .fold(HashMap::new(), |mut m, path| {
            *m.entry(path).or_insert(0) += 1;
            m
        })
}
```

Implementation notes for the executor:
- `BTreeMap` (not `HashMap`) for `pair_counts` makes the output ordering fall out
  of iteration — no separate sort call to mutate away. The `files_by_bucket`
  BTreeMap choice is incidental (any map works there).
- The nested-loop pair generation clones `sorted` to satisfy the borrow checker
  inside `flat_map`; if that reads poorly, an explicit
  `let mut pairs = Vec::new(); for i.. { for j.. { } }` inner block inside a
  `.map(|files| ...)` is equally acceptable — functional style at the pipeline
  level matters more than avoiding an inner loop (see `count_co_changed_pairs`,
  which uses indexed loops for exactly this).
- `ordinal()` + `year()` as the day key avoids allocating date strings.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib metrics::team 2>&1 | tail -3`
Expected: all pass (7 new + existing).

- [ ] **Step 5: Commit**

```bash
printf 'feat(team): add day-bucketed co-change pair computation\n\nPairs grouped by (author, UTC day) instead of exact commit, with\nper-file bucket counts for ratio qualification. Separate data source\nfrom file_change_pairs -- existing coupling metrics untouched\n(org-coupling design, Decision 2).\n' > /tmp/msg.txt
git add src/metrics/team/mod.rs && git commit -F /tmp/msg.txt
```

---

### Task 3: `cross_team_coupling` metric + `compute_team` wiring

**Files:**
- Modify: `src/metrics/team/mod.rs` (metric + wiring + tests)
- Modify: `src/cmd/analyze.rs:308` (call site)
- Modify: `src/cmd/gate.rs:49` (call site)
- Modify: `src/backfill/mod.rs:66` (call site)

**Interfaces:**
- Consumes: `primary_author` (Task 1), `day_bucketed_pairs` + `day_bucket_counts` (Task 2), `snapshot.blame_map: HashMap<PathBuf, Vec<BlameLine>>`, `snapshot.authors: Vec<Author>` (`.name: String`), `crate::config::CouplingThresholds.change_coupling_min_ratio: f64`, `crate::metrics::score_count_bands(usize) -> u32`.
- Produces: `pub fn compute_team(snapshot, team: &TeamThresholds, coupling: &CouplingThresholds) -> CategoryResult` — the NEW signature all three call sites use. Metric name string: `"Cross-team coupling"`.

- [ ] **Step 1: Write the failing tests**

```rust
mod cross_team_coupling_tests {
    use super::*;
    use crate::config::CouplingThresholds;
    use crate::metrics::testutil::{make_file, make_snapshot};
    use crate::snapshot::{
        Author, BlameLine, ChangeType, Commit, CommitId, FileChange, RepoSnapshot,
    };
    use chrono::{TimeZone, Utc};
    use std::path::PathBuf;

    fn commit(id: u32, author: usize, day: u32, hour: u32, paths: &[&str]) -> Commit {
        Commit {
            id: CommitId(id),
            author,
            timestamp: Utc.with_ymd_and_hms(2026, 8, day, hour, 0, 0).unwrap(),
            message: String::new(),
            files_changed: paths
                .iter()
                .map(|p| FileChange {
                    path: PathBuf::from(p),
                    additions: 1,
                    deletions: 0,
                    change_type: ChangeType::Modified,
                })
                .collect(),
            is_merge: false,
            parent_count: 1,
        }
    }

    fn author(id: usize, name: &str) -> Author {
        Author {
            id,
            name: name.into(),
            email: format!("{name}@t"),
        }
    }

    fn owned_lines(author_id: usize) -> Vec<BlameLine> {
        let mut l = BlameLine::new(author_id, Utc::now());
        l.line_count = 100;
        vec![l]
    }

    /// Alice (0) owns a.rs, Bob (1) owns b.rs; the pair co-changes in every
    /// bucket either file appears in (ratio 1.0 >= any sane threshold).
    fn cross_owned_snapshot() -> RepoSnapshot {
        let mut s = make_snapshot();
        s.files = vec![make_file("a.rs"), make_file("b.rs")];
        s.authors = vec![author(0, "alice"), author(1, "bob")];
        s.commits = vec![
            commit(0, 0, 19, 9, &["a.rs", "b.rs"]),
            commit(1, 0, 20, 9, &["a.rs", "b.rs"]),
        ];
        s.blame_map.insert("a.rs".into(), owned_lines(0));
        s.blame_map.insert("b.rs".into(), owned_lines(1));
        s
    }

    #[test]
    fn differing_primary_owners_on_qualifying_pair_is_a_finding() {
        let m = cross_team_coupling(&cross_owned_snapshot(), &CouplingThresholds::default());
        assert_eq!(m.name, "Cross-team coupling");
        assert_eq!(m.score, Some(75), "1 finding -> band 75 (score_count_bands)");
        match &m.raw_value {
            RawValue::List(v) => assert_eq!(
                v,
                &vec!["a.rs ↔ b.rs — coupled 2 day(s), primary owners: alice vs. bob".to_string()]
            ),
            other => panic!("expected List, got {other:?}"),
        }
        assert_eq!(
            m.description,
            "1 cross-team coupling pair(s) — coupled files with different primary owners"
        );
    }

    #[test]
    fn same_primary_owner_on_both_files_is_not_a_finding() {
        let mut s = cross_owned_snapshot();
        s.blame_map.insert("b.rs".into(), owned_lines(0)); // alice owns both
        let m = cross_team_coupling(&s, &CouplingThresholds::default());
        assert_eq!(m.score, Some(100));
        assert_eq!(
            m.description,
            "0 cross-team coupling pair(s) — coupled files with different primary owners"
        );
    }

    #[test]
    fn file_without_a_primary_owner_is_not_a_finding() {
        let mut s = cross_owned_snapshot();
        // b.rs collectively owned: exact 50/50 -> no primary author.
        let mut l0 = BlameLine::new(0, Utc::now());
        l0.line_count = 50;
        let mut l1 = BlameLine::new(1, Utc::now());
        l1.line_count = 50;
        s.blame_map.insert("b.rs".into(), vec![l0, l1]);
        let m = cross_team_coupling(&s, &CouplingThresholds::default());
        assert_eq!(m.score, Some(100));
    }

    #[test]
    fn pair_below_ratio_threshold_is_not_a_finding() {
        // a.rs appears in 10 buckets, pairs with b.rs in only 1 of b.rs's
        // 1 bucket... make b.rs the busy one: b.rs in 10 buckets, pair
        // count 1 -> ratio = 1 / min(1, 10)?? — construct so that
        // min(day_count(a), day_count(b)) makes the ratio fall below the
        // default 0.30: pair once, but BOTH files each active on 4 buckets
        // -> ratio 1/4 = 0.25 < 0.30.
        let mut s = make_snapshot();
        s.files = vec![make_file("a.rs"), make_file("b.rs")];
        s.authors = vec![author(0, "alice"), author(1, "bob")];
        s.commits = vec![
            commit(0, 0, 19, 9, &["a.rs", "b.rs"]), // the one co-change bucket
            commit(1, 0, 20, 9, &["a.rs"]),
            commit(2, 0, 21, 9, &["a.rs"]),
            commit(3, 0, 22, 9, &["a.rs"]),
            commit(4, 1, 20, 9, &["b.rs"]),
            commit(5, 1, 21, 9, &["b.rs"]),
            commit(6, 1, 22, 9, &["b.rs"]),
        ];
        s.blame_map.insert("a.rs".into(), owned_lines(0));
        s.blame_map.insert("b.rs".into(), owned_lines(1));
        let m = cross_team_coupling(&s, &CouplingThresholds::default());
        assert_eq!(
            m.score,
            Some(100),
            "ratio 1/4 = 0.25 < 0.30 default must not qualify"
        );
    }

    #[test]
    fn no_blame_data_is_not_applicable() {
        let mut s = cross_owned_snapshot();
        s.blame_map.clear();
        let m = cross_team_coupling(&s, &CouplingThresholds::default());
        assert_eq!(m.score, None);
        assert_eq!(m.description, "No blame data available");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib metrics::team::tests::cross_team 2>&1 | tail -5`
Expected: compile error `cannot find function cross_team_coupling`.

- [ ] **Step 3: Write the implementation**

```rust
/// Cross-team (Conway's-law) coupling: day-bucketed co-change pairs that
/// meet `change_coupling_min_ratio` and whose two files have *different*
/// primary owners — a coordination cost on top of the code coupling
/// (Crime Scene Ch. 12; design Decision 3). Files without a strict-majority
/// owner are skipped: collectively-owned code has no owner to mismatch.
fn cross_team_coupling(
    snapshot: &RepoSnapshot,
    coupling: &crate::config::CouplingThresholds,
) -> MetricValue {
    let name = "Cross-team coupling".to_string();
    if snapshot.blame_map.is_empty() {
        return MetricValue {
            name,
            description: "No blame data available".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
    }
    let bucket_counts = day_bucket_counts(snapshot);
    let author_name = |id: usize| {
        snapshot
            .authors
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.name.clone())
            .unwrap_or_else(|| format!("author #{id}"))
    };
    let findings: Vec<String> = day_bucketed_pairs(snapshot)
        .into_iter()
        .filter_map(|(a, b, co_days)| {
            let min_days = bucket_counts
                .get(&a)
                .copied()
                .unwrap_or(0)
                .min(bucket_counts.get(&b).copied().unwrap_or(0));
            if min_days == 0
                || (co_days as f64 / min_days as f64) < coupling.change_coupling_min_ratio
            {
                return None;
            }
            let owner_a = primary_author(snapshot.blame_map.get(&a)?)?;
            let owner_b = primary_author(snapshot.blame_map.get(&b)?)?;
            (owner_a != owner_b).then(|| {
                format!(
                    "{} ↔ {} — coupled {} day(s), primary owners: {} vs. {}",
                    a.display(),
                    b.display(),
                    co_days,
                    author_name(owner_a),
                    author_name(owner_b),
                )
            })
        })
        .collect();
    let count = findings.len();
    MetricValue {
        name,
        description: format!(
            "{count} cross-team coupling pair(s) — coupled files with different primary owners"
        ),
        raw_value: RawValue::List(findings),
        score: Some(crate::metrics::score_count_bands(count)),
    }
}
```

- [ ] **Step 4: Run new tests, then wire `compute_team`**

Run: `cargo test --lib metrics::team::tests::cross_team 2>&1 | tail -3` — expect the
5 new tests pass (the metric exists but isn't wired yet; a `dead_code` warning is
the cue for the next edit).

Change `compute_team`'s signature and both branches:

```rust
pub fn compute_team(
    snapshot: &RepoSnapshot,
    thresholds: &crate::config::TeamThresholds,
    coupling: &crate::config::CouplingThresholds,
) -> CategoryResult {
```

In the `MIN_TEAM_SIZE` early-return, add to the `metrics: vec![...]`:

```rust
                na("Cross-team coupling"),
```

In the main path, add as the sixth element:

```rust
        cross_team_coupling(snapshot, coupling),
```

Update the three call sites (each already has `cfg` in scope):

```rust
// src/cmd/analyze.rs:308
categories.push(team::compute_team(
    snapshot,
    &cfg.thresholds.team,
    &cfg.thresholds.coupling,
));
// src/cmd/gate.rs:49
team::compute_team(&snapshot, &cfg.thresholds.team, &cfg.thresholds.coupling),
// src/backfill/mod.rs:66
team::compute_team(&snapshot, &cfg.thresholds.team, &cfg.thresholds.coupling),
```

Any `compute_team` calls inside `team/mod.rs` tests gain
`&crate::config::CouplingThresholds::default()` as the third argument (compile
errors will list them).

- [ ] **Step 5: Run the full suite**

Run: `RUSTFLAGS="-D warnings" cargo test 2>&1 | grep "test result" | tail -3` and
`cargo clippy --all-targets -- -D warnings` and `cargo fmt`.
Expected: everything green, no warnings.

- [ ] **Step 6: Commit**

```bash
printf 'feat(team): add cross-team coupling metric (Crime Scene Ch. 12)\n\nDay-bucketed co-change pairs meeting change_coupling_min_ratio whose\nfiles have different primary (blame-majority) owners become findings on\na sixth Team metric, count-banded like its siblings. compute_team gains\nthe CouplingThresholds parameter (three call sites). N/A without blame\ndata -- consistent with the Team category under ADR-005 backfill.\n' > /tmp/msg.txt
git add -A && git commit -F /tmp/msg.txt
```

---

### Task 4: Integration test (walking skeleton)

**Files:**
- Create: `tests/cross_team_coupling_walking_skeleton.rs`

**Interfaces:**
- Consumes: the `barad-dur` binary (`env!("CARGO_BIN_EXE_barad-dur")`), `analyze --json` output shape: `report["categories"]` array of `{name, metrics: [{name, description, raw_value}]}`.
- Produces: nothing downstream — this is the E2E pin.

- [ ] **Step 1: Write the test**

Fixture: two authors, two files, each file majority-owned by a different author,
co-changed across same-day commits. Note `MIN_TEAM_SIZE = 4`: the Team category
N/A's below 4 authors, so the fixture needs **4 authors** — two extras with
trivial files. Blame comes from real `git blame`, so each owner must actually
author their file's lines (set `GIT_AUTHOR_NAME`/`GIT_AUTHOR_DATE` per commit;
`GIT_COMMITTER_*` too). Same-day coupling needs two commits by one author dated
the same UTC day touching `a.rs` and `b.rs` respectively — but each file's
*content* must stay majority-authored by its owner, so the coupling commits
should make small edits (1 line) to the other file, keeping blame dominance.

```rust
//! Cross-team coupling E2E: two files, each blame-dominated by a
//! different author, co-changing across same-day separate commits,
//! surface a Team-category finding naming both owners.

use std::path::Path;
use std::process::{Command, Output};

fn git(dir: &Path, envs: &[(&str, &str)], args: &[&str]) -> Output {
    let mut c = Command::new("git");
    c.current_dir(dir).args(args);
    for (k, v) in envs {
        c.env(k, v);
    }
    let out = c.output().expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn as_author(name: &str, date: &str) -> Vec<(&'static str, String)> {
    vec![
        ("GIT_AUTHOR_NAME", name.to_string()),
        ("GIT_AUTHOR_EMAIL", format!("{name}@t")),
        ("GIT_AUTHOR_DATE", date.to_string()),
        ("GIT_COMMITTER_NAME", name.to_string()),
        ("GIT_COMMITTER_EMAIL", format!("{name}@t")),
        ("GIT_COMMITTER_DATE", date.to_string()),
    ]
}

fn commit_as(dir: &Path, name: &str, date: &str, msg: &str) {
    let envs = as_author(name, date);
    let envs: Vec<(&str, &str)> = envs.iter().map(|(k, v)| (*k, v.as_str())).collect();
    git(dir, &envs, &["add", "-A"]);
    git(dir, &envs, &["commit", "-q", "-m", msg]);
}

fn fixture_repo() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let d = dir.path();
    git(d, &[], &["init", "-q"]);
    git(d, &[], &["config", "user.email", "t@t"]);
    git(d, &[], &["config", "user.name", "T"]);

    // Four authors total so the Team category is applicable (MIN_TEAM_SIZE).
    std::fs::write(d.join("c.rs"), "// carol\n").unwrap();
    commit_as(d, "carol", "2026-08-01T10:00:00 +0000", "carol file");
    std::fs::write(d.join("d.rs"), "// dave\n").unwrap();
    commit_as(d, "dave", "2026-08-02T10:00:00 +0000", "dave file");

    // alice authors and owns a.rs (10 lines).
    std::fs::write(d.join("a.rs"), "fn a() {}\n".repeat(10)).unwrap();
    commit_as(d, "alice", "2026-08-03T10:00:00 +0000", "alice owns a");
    // bob authors and owns b.rs (10 lines).
    std::fs::write(d.join("b.rs"), "fn b() {}\n".repeat(10)).unwrap();
    commit_as(d, "bob", "2026-08-04T10:00:00 +0000", "bob owns b");

    // Same-day, separate commits by alice touching a.rs then b.rs (small
    // edit — bob keeps blame majority on b.rs). Repeat on a second day so
    // the pair count is 2 and the ratio comfortably clears 0.30.
    for (i, day) in ["2026-08-05", "2026-08-06"].iter().enumerate() {
        std::fs::write(d.join("a.rs"), format!("{}// churn {i}\n", "fn a() {}\n".repeat(10)))
            .unwrap();
        commit_as(d, "alice", &format!("{day}T09:00:00 +0000"), "touch a");
        std::fs::write(d.join("b.rs"), format!("{}// churn {i}\n", "fn b() {}\n".repeat(10)))
            .unwrap();
        commit_as(d, "alice", &format!("{day}T15:00:00 +0000"), "touch b");
    }
    dir
}

#[test]
fn cross_owner_day_coupling_surfaces_a_team_finding() {
    let dir = fixture_repo();
    let out = Command::new(env!("CARGO_BIN_EXE_barad-dur"))
        .arg("analyze")
        .arg(dir.path())
        .arg("--json")
        .arg("--no-cache")
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "analyze failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let team = report["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "Team")
        .expect("team category");
    let metric = team["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "Cross-team coupling")
        .expect("cross-team coupling metric");
    let list = metric["raw_value"]["List"].as_array().expect("List");
    let entry = list
        .iter()
        .filter_map(|e| e.as_str())
        .find(|e| e.contains("a.rs") && e.contains("b.rs"))
        .expect("a.rs/b.rs finding");
    assert!(
        entry.contains("alice") && entry.contains("bob"),
        "finding must name both primary owners: {entry}"
    );
    assert!(
        entry.contains("coupled 2 day(s)"),
        "finding must carry the day-bucket count: {entry}"
    );
}
```

- [ ] **Step 2: Run it — expect a real red or green and INVESTIGATE either way**

Run: `cargo test --test cross_team_coupling_walking_skeleton 2>&1 | tail -5`

This test can legitimately pass first try (Tasks 1–3 implemented everything).
If it FAILS, the failure is informative — likeliest causes, in order: blame
dominance flipped by the churn commits (make the churn edit smaller / the owned
body larger), the fixture has fewer than 4 detected authors (check emails are
distinct), or the pair ratio fell below 0.30 (recount buckets). Fix the fixture
or the implementation — never loosen the assertions to pass.

- [ ] **Step 3: Full-suite verification + dogfood sanity**

Run: `RUSTFLAGS="-D warnings" cargo test 2>&1 | grep "test result" | tail -3`
and `cargo run --release --quiet -- analyze . --json --no-cache | python3 -c
"import json,sys; r=json.load(sys.stdin); t=[c for c in r['categories'] if c['name']=='Team'][0]; print([m for m in t['metrics'] if m['name']=='Cross-team coupling'])"`.
Expected on barad-dûr itself: N/A ("Small team") or 0 findings — both fine per
the spec's Dogfood note; the fixture test is what proves correctness.

- [ ] **Step 4: Commit**

```bash
printf 'test(team): cross-team coupling walking skeleton\n\nFixture with four authors, two cross-owned files co-changing in\nsame-day separate commits -- the Team finding surfaces both owner\nnames and the day-bucket count through analyze --json.\n' > /tmp/msg.txt
git add tests/cross_team_coupling_walking_skeleton.rs && git commit -F /tmp/msg.txt
```

---

### Task 5: Book-notes update + MR

**Files:**
- Modify: `docs/crime-scene-book-notes.md` (Ch. 12 verdict + gap table row)
- Modify: `docs/superpowers/specs/2026-08-18-organizational-coupling-design.md` (Status line)

**Interfaces:** none — documentation truth-keeping.

- [ ] **Step 1: Update the tracker**

In `docs/crime-scene-book-notes.md`, Ch. 12 verdict: change `🟡` to `✅` and append
one sentence: `Closed 2026-08-19 by the Cross-team coupling Team metric
(day-bucketed pairs × primary-author mismatch); explicit team-mapping config
remains deferred future work.` Update the gap-summary table row 12 to `✅` with
"team-mapping config deferred" in the missing column.

In the spec, change `**Status:** Proposed design` to
`**Status:** Implemented 2026-08-19 (see docs/superpowers/plans/2026-08-19-organizational-coupling.md)`.

- [ ] **Step 2: Commit, push, open MR**

```bash
printf 'docs: mark Crime Scene Ch. 12 closed by cross-team coupling\n' > /tmp/msg.txt
git add docs/ && git commit -F /tmp/msg.txt
git push -u origin worktree-feat+org-coupling
glab mr create --source-branch worktree-feat+org-coupling --target-branch main \
  --title "feat(team): cross-team (Conway's-law) coupling metric" \
  --description "$(cat <<'EOF'
Implements the Ch. 12 gap from docs/crime-scene-book-notes.md per
docs/superpowers/specs/2026-08-18-organizational-coupling-design.md.

- primary_author: strict-majority blame owner per file (the piece
  bus_factor/churn_ownership computed but discarded)
- day_bucketed_pairs: co-changes grouped by (author, UTC day) — separate
  data source; exact-commit file_change_pairs and all existing coupling
  metrics untouched
- Cross-team coupling: sixth Team metric, count-banded; a finding is a
  ratio-qualified day-bucketed pair whose files have different primary
  owners. N/A without blame (backfill/ADR-005-consistent)
- No new config: reuses change_coupling_min_ratio

TDD throughout; walking-skeleton fixture proves the E2E path (dogfood
shows N/A/0 on this small-team repo, as the spec predicts).
EOF
)"
```

- [ ] **Step 3: Watch the pipeline mutation gate**

The `mutation-gate` job needs ≥ 80% kill on the diff. If survivors appear, apply
the established loop: hand-apply each surviving mutant locally, write a test
that fails under it and passes on real code, push one hardening commit.

---

## Self-review notes

- Spec coverage: Decision 1 → Task 1; Decision 2 → Task 2 (plus the "untouched"
  global constraint); Decision 3 → Task 3 filter chain; Decision 4 → Task 3
  wiring (`score_count_bands`, Team category); Decision 5 → no team config
  anywhere; Surfacing section → exact strings pinned in Task 3/4 tests;
  Interactions → N/A-without-blame test (Task 3) and untouched-coupling
  constraint; Testing strategy → every bullet has a named test.
- The spec's evidence string (`↔`, "coupled N day(s), primary owners: x vs. y")
  is pinned byte-exactly in both unit and integration tests — renderer needs no
  changes (generic `RawValue::List`).
- Type consistency: `primary_author(&[BlameLine]) -> Option<usize>`,
  `day_bucketed_pairs(&RepoSnapshot) -> Vec<(PathBuf, PathBuf, usize)>`,
  `day_bucket_counts(&RepoSnapshot) -> HashMap<PathBuf, usize>`,
  `cross_team_coupling(&RepoSnapshot, &CouplingThresholds) -> MetricValue`,
  `compute_team(&RepoSnapshot, &TeamThresholds, &CouplingThresholds)` — used
  consistently across Tasks 1–4.
