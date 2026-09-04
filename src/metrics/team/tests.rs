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
    // Nothing measurable: the category is unscored, not a fake 100. Gates
    // treat an unscored category as "not gated", so N/A is still not punished.
    assert_eq!(result.score, None);
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

mod knowledge_loss_tests {
    use super::*;
    use crate::metrics::testutil::make_snapshot;
    use crate::snapshot::{BlameLine, RepoSnapshot, UNKNOWN_AUTHOR};
    use chrono::Utc;

    fn line(author: usize, n: usize) -> BlameLine {
        let mut l = BlameLine::new(author, Utc::now());
        l.line_count = n;
        l
    }

    /// One file holding `unknown` unattributed lines and `known` lines by
    /// author 0, so repo-wide share is unknown / (unknown + known).
    fn snap(unknown: usize, known: usize) -> RepoSnapshot {
        let mut s = make_snapshot();
        let mut lines = vec![line(0, known)];
        if unknown > 0 {
            lines.push(line(UNKNOWN_AUTHOR, unknown));
        }
        s.blame_map.insert("a.rs".into(), lines);
        s
    }

    #[test]
    fn boundary_scores_track_the_bus_factor_style_bands() {
        // Both sides of every band edge: <10 -> 100, <25 -> 75, <50 -> 50.
        assert_eq!(knowledge_loss(&snap(99, 901)).score, Some(100)); // 9.9%
        assert_eq!(knowledge_loss(&snap(100, 900)).score, Some(75)); // 10.0%
        assert_eq!(knowledge_loss(&snap(249, 751)).score, Some(75)); // 24.9%
        assert_eq!(knowledge_loss(&snap(250, 750)).score, Some(50)); // 25.0%
        assert_eq!(knowledge_loss(&snap(499, 501)).score, Some(50)); // 49.9%
        assert_eq!(knowledge_loss(&snap(500, 500)).score, Some(25)); // 50.0%
    }

    #[test]
    fn description_carries_exact_share_and_counts() {
        let m = knowledge_loss(&snap(120, 880));
        assert_eq!(m.name, "Knowledge loss");
        assert_eq!(
            m.description,
            "12.0% of blamed lines lack an active author (120 of 1000)"
        );
    }

    #[test]
    fn zero_unattributed_lines_score_clean() {
        let m = knowledge_loss(&snap(0, 500));
        assert_eq!(m.score, Some(100));
        assert_eq!(
            m.description,
            "0.0% of blamed lines lack an active author (0 of 500)"
        );
    }

    #[test]
    fn blame_entries_with_zero_lines_score_clean_not_nan() {
        // blame_map non-empty but every entry holds no lines: total == 0
        // must take the 0.0% path, not divide 0/0 into NaN (which would
        // fall through every score band to 25).
        let mut s = make_snapshot();
        s.blame_map.insert("empty.rs".into(), vec![]);
        let m = knowledge_loss(&s);
        assert_eq!(m.score, Some(100));
        assert_eq!(
            m.description,
            "0.0% of blamed lines lack an active author (0 of 0)"
        );
    }

    #[test]
    fn fully_attributed_files_never_enter_the_evidence_list() {
        // One clean file alongside one affected file — small enough that
        // the top-10 cap cannot mask a wrongly-included clean entry (the
        // `u > 0` filter is the only thing keeping it out).
        let mut s = make_snapshot();
        s.blame_map
            .insert("old.rs".into(), vec![line(0, 50), line(UNKNOWN_AUTHOR, 50)]);
        s.blame_map.insert("clean.rs".into(), vec![line(0, 100)]);
        let m = knowledge_loss(&s);
        match &m.raw_value {
            RawValue::List(v) => {
                assert_eq!(v.len(), 1, "only the affected file: {v:?}");
                assert_eq!(v[0], "old.rs — 50.0% unattributed (50 of 100 lines)");
            }
            other => panic!("expected List, got {other:?}"),
        }
    }

    #[test]
    fn empty_blame_is_not_applicable() {
        let s = make_snapshot();
        let m = knowledge_loss(&s);
        assert_eq!(m.score, None);
        assert_eq!(m.description, "No blame data available");
    }

    #[test]
    fn evidence_lists_affected_files_by_share_then_path_capped_at_ten() {
        let mut s = make_snapshot();
        // legacy.rs 83.0% (410/494), old.rs 50.0% (50/100) — descending.
        s.blame_map.insert(
            "legacy.rs".into(),
            vec![line(0, 84), line(UNKNOWN_AUTHOR, 410)],
        );
        s.blame_map
            .insert("old.rs".into(), vec![line(0, 50), line(UNKNOWN_AUTHOR, 50)]);
        // A clean file must not appear at all.
        s.blame_map.insert("clean.rs".into(), vec![line(0, 100)]);
        // Eleven more small ones to prove the cap.
        for i in 0..11 {
            s.blame_map.insert(
                format!("f{i:02}.rs").into(),
                vec![line(0, 99), line(UNKNOWN_AUTHOR, 1)],
            );
        }
        let m = knowledge_loss(&s);
        match &m.raw_value {
            RawValue::List(v) => {
                assert_eq!(v.len(), 10, "top-10 cap");
                assert_eq!(v[0], "legacy.rs — 83.0% unattributed (410 of 494 lines)");
                assert_eq!(v[1], "old.rs — 50.0% unattributed (50 of 100 lines)");
                assert_eq!(
                    v[2], "f00.rs — 1.0% unattributed (1 of 100 lines)",
                    "equal shares must tie-break by path"
                );
                assert!(v.iter().all(|e| !e.starts_with("clean.rs")));
            }
            other => panic!("expected List, got {other:?}"),
        }
    }
}

mod knowledge_distribution_sentinel_tests {
    use super::*;
    use crate::snapshot::{BlameLine, UNKNOWN_AUTHOR};
    use chrono::Utc;

    #[test]
    fn unknown_author_lines_do_not_skew_the_gini() {
        // Two known authors with equal shares plus a large unattributable
        // legacy mass: distribution over *people* is perfectly equal, so
        // the metric must score as balanced, not as a three-way skew.
        let mut s = crate::metrics::testutil::make_snapshot();
        s.authors = (0..4)
            .map(|id| crate::snapshot::Author {
                id,
                name: format!("dev{id}"),
                email: format!("d{id}@t"),
            })
            .collect();
        let line = |author, n| {
            let mut l = BlameLine::new(author, Utc::now());
            l.line_count = n;
            l
        };
        s.blame_map.insert(
            "a.rs".into(),
            vec![line(0, 100), line(1, 100), line(UNKNOWN_AUTHOR, 500)],
        );
        let with_unknown = knowledge_distribution(&s, &crate::config::TeamThresholds::default());
        s.blame_map
            .insert("a.rs".into(), vec![line(0, 100), line(1, 100)]);
        let without = knowledge_distribution(&s, &crate::config::TeamThresholds::default());
        assert_eq!(
            with_unknown.score, without.score,
            "sentinel mass must not change the knowledge-distribution score"
        );
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
        assert_eq!(m.score, None, "author boundaries are advisory");
        match &m.raw_value {
            RawValue::List(v) => assert_eq!(
                v,
                &vec![
                    "a.rs ↔ b.rs — co-changed on 2 author-day(s), primary owners: alice vs. bob"
                        .to_string()
                ]
            ),
            other => panic!("expected List, got {other:?}"),
        }
        assert_eq!(
            m.description,
            "1 cross-owner coupling pair(s) — advisory; configure real team boundaries before treating this as cross-team risk"
        );
    }

    #[test]
    fn same_primary_owner_on_both_files_is_not_a_finding() {
        let mut s = cross_owned_snapshot();
        s.blame_map.insert("b.rs".into(), owned_lines(0)); // alice owns both
        let m = cross_team_coupling(&s, &CouplingThresholds::default());
        assert_eq!(m.score, None);
        assert_eq!(
            m.description,
            "0 cross-owner coupling pair(s) — advisory; configure real team boundaries before treating this as cross-team risk"
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
        assert_eq!(m.score, None);
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
            m.score, None,
            "ratio 1/4 = 0.25 < 0.30 default must not qualify"
        );
    }

    #[test]
    fn at_threshold_ratio_with_asymmetric_bucket_counts_qualifies_via_min() {
        // a.rs appears in 10 distinct (author, day) buckets, b.rs in 20;
        // they co-change in exactly 3 buckets -> ratio = 3 / min(10, 20)
        // = 0.30 exactly, the default threshold: >= semantics keep the
        // finding (kills a `<` -> `<=` mutant on the ratio gate). If `max`
        // were used the ratio would be 3/20 = 0.15 < 0.30 and there would
        // be no finding — the min-vs-max discrimination. co_days = 3 also
        // clears the MIN_CO_DAYS floor.
        let mut s = make_snapshot();
        s.files = vec![make_file("a.rs"), make_file("b.rs")];
        s.authors = vec![author(0, "alice"), author(1, "bob")];
        let mut commits = vec![
            // Three co-change buckets: (alice, days 19-21).
            commit(0, 0, 19, 9, &["a.rs", "b.rs"]),
            commit(1, 0, 20, 9, &["a.rs", "b.rs"]),
            commit(2, 0, 21, 9, &["a.rs", "b.rs"]),
        ];
        // Seven more a.rs-only buckets for alice -> a.rs total = 10.
        for (i, day) in (22..=28).enumerate() {
            commits.push(commit(3 + i as u32, 0, day, 9, &["a.rs"]));
        }
        // Seventeen more b.rs-only buckets for bob -> b.rs total = 20.
        for (i, day) in (1..=17).enumerate() {
            commits.push(commit(10 + i as u32, 1, day, 9, &["b.rs"]));
        }
        s.commits = commits;
        s.blame_map.insert("a.rs".into(), owned_lines(0));
        s.blame_map.insert("b.rs".into(), owned_lines(1));
        let m = cross_team_coupling(&s, &CouplingThresholds::default());
        assert_eq!(
            m.score, None,
            "ratio 3/min(10,20) = 0.30 exactly must qualify (>= semantics)"
        );
        match &m.raw_value {
            RawValue::List(v) => assert_eq!(
                v,
                &vec![
                    "a.rs ↔ b.rs — co-changed on 3 author-day(s), primary owners: alice vs. bob"
                        .to_string()
                ]
            ),
            other => panic!("expected List, got {other:?}"),
        }
    }

    #[test]
    fn single_co_day_is_below_the_support_floor() {
        // One-off same-day co-change (ratio 1.0 but co_days = 1): the
        // MIN_CO_DAYS floor rejects it — scaffolding noise, not coupling.
        let mut s = make_snapshot();
        s.files = vec![make_file("a.rs"), make_file("b.rs")];
        s.authors = vec![author(0, "alice"), author(1, "bob")];
        s.commits = vec![commit(0, 0, 19, 9, &["a.rs", "b.rs"])];
        s.blame_map.insert("a.rs".into(), owned_lines(0));
        s.blame_map.insert("b.rs".into(), owned_lines(1));
        let m = cross_team_coupling(&s, &CouplingThresholds::default());
        assert_eq!(
            m.score, None,
            "co_days = 1 must not qualify despite ratio 1.0"
        );
    }

    #[test]
    fn merge_commits_do_not_qualify_pairs() {
        // An integrator's merge diffs as the full first-parent changeset —
        // bucketing it would pair files across unrelated MRs.
        let mut s = cross_owned_snapshot();
        for c in &mut s.commits {
            c.is_merge = true;
        }
        let m = cross_team_coupling(&s, &CouplingThresholds::default());
        assert_eq!(m.score, None, "merge commits must not create day buckets");
    }

    #[test]
    fn qualifying_pair_without_blame_entry_is_counted_not_silent() {
        // b.rs never got blamed (binary / blame failure) but blame_map is
        // not empty — the qualifying pair is skipped, and the description
        // must say so instead of reporting a confident clean 100.
        let mut s = cross_owned_snapshot();
        s.blame_map.remove(&PathBuf::from("b.rs"));
        let m = cross_team_coupling(&s, &CouplingThresholds::default());
        assert_eq!(m.score, None);
        assert_eq!(
            m.description,
            "0 cross-owner coupling pair(s); 1 qualifying pair(s) lacked blame data — advisory; \
             configure real team boundaries before treating this as cross-team risk"
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
