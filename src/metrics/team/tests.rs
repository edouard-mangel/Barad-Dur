use super::*;
use crate::snapshot::*;
use chrono::{Duration, Utc};
use std::path::PathBuf;

#[test]
fn compute_team_small_team_metrics_unscored() {
    // Fewer than MIN_TEAM_SIZE authors → all metrics N/A, category scores 100
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    for i in 0..3 {
        snapshot.authors.push(Author {
            id: i,
            name: format!("Author {i}"),
            email: format!("a{i}@t.com"),
        });
    }
    let result = compute_team(
        &snapshot,
        &crate::config::TeamThresholds::default(),
        &crate::config::CouplingThresholds::default(),
    );
    // Category keeps 100 (gates must not punish N/A), but the individual
    // metrics carry no score — renderers show a dash, not a fake 100.
    assert_eq!(result.score, 100);
    assert!(result.metrics.iter().all(|m| m.score.is_none()));
    assert!(result
        .metrics
        .iter()
        .all(|m| m.description.contains("not applicable")));
}

fn make_solo_snapshot() -> RepoSnapshot {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    snapshot.authors = vec![Author {
        id: 0,
        name: "Alice".into(),
        email: "a@t.com".into(),
    }];
    snapshot
}

#[test]
fn knowledge_distribution_solo_project_has_no_score() {
    let snapshot = make_solo_snapshot();
    let result = knowledge_distribution(&snapshot, &crate::config::TeamThresholds::default());
    assert_eq!(result.score, None);
    assert!(result.description.contains("Solo project"));
}

#[test]
fn ownership_clarity_solo_project_has_no_score() {
    let snapshot = make_solo_snapshot();
    let result = ownership_clarity(&snapshot, &crate::config::TeamThresholds::default());
    assert_eq!(result.score, None);
    assert!(result.description.contains("Solo project"));
}

#[test]
fn collaboration_patterns_solo_project_has_no_score() {
    let snapshot = make_solo_snapshot();
    let result = collaboration_patterns(&snapshot, &crate::config::TeamThresholds::default());
    assert_eq!(result.score, None);
    assert!(result.description.contains("Solo project"));
}

#[test]
fn knowledge_distribution_detects_concentration() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );

    snapshot.authors = vec![
        Author {
            id: 0,
            name: "Alice".into(),
            email: "a@t.com".into(),
        },
        Author {
            id: 1,
            name: "Bob".into(),
            email: "b@t.com".into(),
        },
        Author {
            id: 2,
            name: "Carol".into(),
            email: "c@t.com".into(),
        },
    ];

    let now = Utc::now();
    // Alice owns 95 lines, Bob 4, Carol 1 → very high Gini
    let mut blame = Vec::new();
    for _ in 0..95 {
        blame.push(BlameLine::new(0, now));
    }
    for _ in 0..4 {
        blame.push(BlameLine::new(1, now));
    }
    for _ in 0..1 {
        blame.push(BlameLine::new(2, now));
    }
    snapshot.blame_map.insert(PathBuf::from("file.rs"), blame);

    let result = knowledge_distribution(&snapshot, &crate::config::TeamThresholds::default());
    match result.raw_value {
        RawValue::Float(gini) => assert!(gini >= 0.5, "Expected Gini >= 0.5, got {}", gini),
        _ => panic!("Expected Float"),
    }
    assert!(result.score.unwrap() <= 50);
}

#[test]
fn contributor_activity_counts_active() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );

    snapshot.authors = vec![
        Author {
            id: 0,
            name: "Alice".into(),
            email: "a@t.com".into(),
        },
        Author {
            id: 1,
            name: "Bob".into(),
            email: "b@t.com".into(),
        },
        Author {
            id: 2,
            name: "Carol".into(),
            email: "c@t.com".into(),
        },
        Author {
            id: 3,
            name: "Dave".into(),
            email: "d@t.com".into(),
        },
        Author {
            id: 4,
            name: "Eve".into(),
            email: "e@t.com".into(),
        },
    ];

    let now = Utc::now();
    // Only 3 authors have recent commits
    for i in 0..3 {
        snapshot.commits.push(Commit {
            id: CommitId(i as u32),
            author: i,
            timestamp: now - Duration::days(10),
            message: "msg".into(),
            files_changed: vec![],
            is_merge: false,
            parent_count: 1,
        });
    }
    snapshot.build_indexes();

    let result = contributor_activity(&snapshot, &crate::config::TeamThresholds::default());
    match result.raw_value {
        RawValue::Percentage(p) => assert!((p - 60.0).abs() < 1.0, "Expected ~60%, got {}", p),
        _ => panic!("Expected Percentage"),
    }
}

#[test]
fn ownership_clarity_detects_owners() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    snapshot.authors = vec![
        Author {
            id: 0,
            name: "Alice".into(),
            email: "a@t.com".into(),
        },
        Author {
            id: 1,
            name: "Bob".into(),
            email: "b@t.com".into(),
        },
    ];

    let now = Utc::now();
    // File 1: Alice 80%, Bob 20% → clear owner
    let mut blame1 = Vec::new();
    for _ in 0..80 {
        blame1.push(BlameLine::new(0, now));
    }
    for _ in 0..20 {
        blame1.push(BlameLine::new(1, now));
    }
    snapshot.blame_map.insert(PathBuf::from("f1.rs"), blame1);

    // File 2: 50/50 → no clear owner
    let mut blame2 = Vec::new();
    for _ in 0..50 {
        blame2.push(BlameLine::new(0, now));
    }
    for _ in 0..50 {
        blame2.push(BlameLine::new(1, now));
    }
    snapshot.blame_map.insert(PathBuf::from("f2.rs"), blame2);

    let result = ownership_clarity(&snapshot, &crate::config::TeamThresholds::default());
    // 1 out of 2 files has clear owner = 50%
    match result.raw_value {
        RawValue::Percentage(p) => assert!((p - 50.0).abs() < 1.0),
        _ => panic!("Expected Percentage"),
    }
}

#[test]
fn collaboration_detects_silos() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    snapshot.authors = vec![
        Author {
            id: 0,
            name: "Alice".into(),
            email: "a@t.com".into(),
        },
        Author {
            id: 1,
            name: "Bob".into(),
            email: "b@t.com".into(),
        },
    ];

    let now = Utc::now();
    // "auth" directory: 100% Alice → silo
    let mut blame_auth = Vec::new();
    for _ in 0..100 {
        blame_auth.push(BlameLine::new(0, now));
    }
    snapshot
        .blame_map
        .insert(PathBuf::from("auth/login.rs"), blame_auth);

    // "api" directory: 60/40 split → NOT a silo
    let mut blame_api = Vec::new();
    for _ in 0..60 {
        blame_api.push(BlameLine::new(0, now));
    }
    for _ in 0..40 {
        blame_api.push(BlameLine::new(1, now));
    }
    snapshot
        .blame_map
        .insert(PathBuf::from("api/routes.rs"), blame_api);

    let result = collaboration_patterns(&snapshot, &crate::config::TeamThresholds::default());
    match result.raw_value {
        RawValue::Count(c) => assert_eq!(c, 1, "Should detect 1 silo (auth)"),
        _ => panic!("Expected Count"),
    }
}

#[test]
fn merge_patterns_counts_merges() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );

    let now = Utc::now();
    for i in 0..20 {
        snapshot.commits.push(Commit {
            id: CommitId(i as u32),
            author: 0,
            timestamp: now - Duration::hours(i * 24),
            message: "msg".into(),
            files_changed: vec![],
            is_merge: i < 5, // First 5 are merges
            parent_count: if i < 5 { 2 } else { 1 },
        });
    }

    let result = merge_patterns(&snapshot, &crate::config::TeamThresholds::default());
    match result.raw_value {
        RawValue::Count(c) => assert_eq!(c, 5),
        _ => panic!("Expected Count"),
    }
}

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
        assert_eq!(
            m.score,
            Some(75),
            "1 finding -> band 75 (score_count_bands)"
        );
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
    fn at_threshold_ratio_with_asymmetric_bucket_counts_qualifies_via_min() {
        // a.rs appears in 3 distinct (author, day) buckets, b.rs in 9;
        // they co-change in exactly 1 bucket -> ratio = 1 / min(3, 9) =
        // 1/3 ~= 0.333 >= 0.30 default, so the pair qualifies. If `max`
        // were used instead of `min` the ratio would be 1/9 ~= 0.111 <
        // 0.30 and there would be no finding — that's the discrimination
        // this test targets.
        let mut s = make_snapshot();
        s.files = vec![make_file("a.rs"), make_file("b.rs")];
        s.authors = vec![author(0, "alice"), author(1, "bob")];
        s.commits = vec![
            // The one co-change bucket: (alice, day 19).
            commit(0, 0, 19, 9, &["a.rs", "b.rs"]),
            // Two more a.rs-only buckets for alice -> a.rs total = 3.
            commit(1, 0, 20, 9, &["a.rs"]),
            commit(2, 0, 21, 9, &["a.rs"]),
            // Eight more b.rs-only buckets for bob -> b.rs total = 9.
            commit(3, 1, 22, 9, &["b.rs"]),
            commit(4, 1, 23, 9, &["b.rs"]),
            commit(5, 1, 24, 9, &["b.rs"]),
            commit(6, 1, 25, 9, &["b.rs"]),
            commit(7, 1, 26, 9, &["b.rs"]),
            commit(8, 1, 27, 9, &["b.rs"]),
            commit(9, 1, 28, 9, &["b.rs"]),
            commit(10, 1, 29, 9, &["b.rs"]),
        ];
        s.blame_map.insert("a.rs".into(), owned_lines(0));
        s.blame_map.insert("b.rs".into(), owned_lines(1));
        let m = cross_team_coupling(&s, &CouplingThresholds::default());
        assert_eq!(
            m.score,
            Some(75),
            "ratio 1/min(3,9) = 1/3 ~= 0.333 >= 0.30 default must qualify"
        );
        match &m.raw_value {
            RawValue::List(v) => assert_eq!(
                v,
                &vec!["a.rs ↔ b.rs — coupled 1 day(s), primary owners: alice vs. bob".to_string()]
            ),
            other => panic!("expected List, got {other:?}"),
        }
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
