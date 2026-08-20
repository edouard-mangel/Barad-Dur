use super::*;
use crate::metrics::testutil::make_snapshot;
use crate::snapshot::*;
use chrono::Duration;
use std::path::PathBuf;

#[test]
fn growth_trend_detects_net_growth() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );

    let now = Utc::now();
    snapshot.files = (0..100)
        .map(|i| FileEntry {
            path: PathBuf::from(format!("f{}.rs", i)),
            size_bytes: 100,
            is_binary: false,
            depth: 1,
            blob_oid: String::new(),
        })
        .collect();

    // 15 files added, 0 deleted → +15% growth
    snapshot.commits.push(Commit {
        id: CommitId(0),
        author: 0,
        timestamp: now - Duration::days(10),
        message: "add files".into(),
        files_changed: (0..15)
            .map(|i| FileChange {
                path: PathBuf::from(format!("new{}.rs", i)),
                additions: 50,
                deletions: 0,
                change_type: ChangeType::Added,
            })
            .collect(),
        is_merge: false,
        parent_count: 1,
    });

    let result = growth_trend(&snapshot, &crate::config::EvolutionThresholds::default());
    match result.raw_value {
        RawValue::Integer(v) => assert_eq!(v, 15),
        _ => panic!("Expected Integer"),
    }
}

fn plain_commit(id: u32, msg: &str, ts: chrono::DateTime<Utc>) -> Commit {
    Commit {
        id: CommitId(id),
        author: 0,
        timestamp: ts,
        message: msg.into(),
        files_changed: vec![FileChange {
            path: PathBuf::from("src/lib.rs"),
            additions: 5,
            deletions: 3,
            change_type: ChangeType::Modified,
        }],
        is_merge: false,
        parent_count: 1,
    }
}

#[test]
fn structural_investment_keyword_commits() {
    // 3 "refactor" commits out of 10 total → ratio 0.30 → score 92
    let mut snapshot = make_snapshot();
    let now = Utc::now();
    for i in 0..7 {
        snapshot.commits.push(plain_commit(
            i as u32,
            "add feature",
            now - Duration::days(i + 1),
        ));
    }
    for i in 0..3 {
        snapshot.commits.push(plain_commit(
            (i + 7) as u32,
            "refactor module layout",
            now - Duration::days(i + 8),
        ));
    }
    let result = refactoring_ratio(&snapshot, &crate::config::EvolutionThresholds::default());
    match result.raw_value {
        RawValue::Float(r) => assert!((r - 0.30).abs() < 0.01, "Expected ~0.30, got {}", r),
        _ => panic!("Expected Float"),
    }
    assert_eq!(result.score, Some(92));
}

#[test]
fn structural_investment_rename_commits() {
    // commits with ChangeType::Renamed are counted as structural
    let mut snapshot = make_snapshot();
    let now = Utc::now();
    for i in 0..8 {
        snapshot.commits.push(plain_commit(
            i as u32,
            "fix bug",
            now - Duration::days(i + 1),
        ));
    }
    for i in 0..2 {
        snapshot.commits.push(Commit {
            id: CommitId((i + 8) as u32),
            author: 0,
            timestamp: now - Duration::days(i as i64 + 9),
            message: "update path".into(),
            files_changed: vec![FileChange {
                path: PathBuf::from("old.rs"),
                additions: 0,
                deletions: 0,
                change_type: ChangeType::Renamed,
            }],
            is_merge: false,
            parent_count: 1,
        });
    }
    let result = refactoring_ratio(&snapshot, &crate::config::EvolutionThresholds::default());
    match result.raw_value {
        RawValue::Float(r) => assert!((r - 0.20).abs() < 0.01, "Expected ~0.20, got {}", r),
        _ => panic!("Expected Float"),
    }
    assert!(result.score.unwrap() >= 80);
}

#[test]
fn structural_investment_deletion_commits() {
    // commits with ChangeType::Deleted files are counted as structural
    let mut snapshot = make_snapshot();
    let now = Utc::now();
    for i in 0..9 {
        snapshot.commits.push(plain_commit(
            i as u32,
            "add stuff",
            now - Duration::days(i + 1),
        ));
    }
    snapshot.commits.push(Commit {
        id: CommitId(9),
        author: 0,
        timestamp: now - Duration::days(10),
        message: "remove unused module".into(),
        files_changed: vec![FileChange {
            path: PathBuf::from("old_module.rs"),
            additions: 0,
            deletions: 200,
            change_type: ChangeType::Deleted,
        }],
        is_merge: false,
        parent_count: 1,
    });
    let result = refactoring_ratio(&snapshot, &crate::config::EvolutionThresholds::default());
    match result.raw_value {
        RawValue::Float(r) => assert!((r - 0.10).abs() < 0.01, "Expected ~0.10, got {}", r),
        _ => panic!("Expected Float"),
    }
    assert_eq!(result.score, Some(55));
}

#[test]
fn structural_investment_none_scores_low() {
    // all ChangeType::Added commits → ratio 0.0 → score 25
    let mut snapshot = make_snapshot();
    let now = Utc::now();
    for i in 0..10 {
        snapshot.commits.push(Commit {
            id: CommitId(i as u32),
            author: 0,
            timestamp: now - Duration::days(i + 1),
            message: "add new file".into(),
            files_changed: vec![FileChange {
                path: PathBuf::from(format!("new{}.rs", i)),
                additions: 20,
                deletions: 0,
                change_type: ChangeType::Added,
            }],
            is_merge: false,
            parent_count: 1,
        });
    }
    let result = refactoring_ratio(&snapshot, &crate::config::EvolutionThresholds::default());
    match result.raw_value {
        RawValue::Float(r) => assert_eq!(r, 0.0),
        _ => panic!("Expected Float"),
    }
    assert_eq!(result.score, Some(25));
}

#[test]
fn code_age_computes_median() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::full_history(),
    );

    let now = Utc::now();
    let eight_months_ago = now - Duration::days(240);
    let mut blame = Vec::new();
    for _ in 0..100 {
        blame.push(BlameLine::new(0, eight_months_ago));
    }
    snapshot.blame_map.insert(PathBuf::from("f.rs"), blame);

    let result = code_age(&snapshot, &crate::config::EvolutionThresholds::default());
    match result.raw_value {
        RawValue::Float(months) => {
            assert!(
                months > 7.0 && months < 9.0,
                "Expected ~8 months, got {}",
                months
            )
        }
        _ => panic!("Expected Float"),
    }
}

#[test]
fn commit_cadence_detects_regularity() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );

    let now = Utc::now();
    // 4 commits per day for 30 days → regular
    for day in 0..30 {
        for i in 0..4 {
            snapshot.commits.push(Commit {
                id: CommitId((day * 4 + i) as u32),
                author: 0,
                timestamp: now - Duration::days(day) + Duration::hours(i),
                message: "work".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            });
        }
    }

    let result = commit_cadence(&snapshot, &crate::config::EvolutionThresholds::default());
    assert!(result.description.contains("regular") || result.description.contains("moderate"));
    assert!(result.score.unwrap() >= 70);
}

mod growth_balance_tests {
    use super::*;
    use crate::metrics::testutil::{make_file, make_snapshot};
    use chrono::{TimeZone, Utc};

    fn change(p: &str, add: u32) -> FileChange {
        FileChange {
            path: p.into(),
            additions: add,
            deletions: 0,
            change_type: ChangeType::Modified,
        }
    }

    fn commit_at(id: u32, hour: u32, files: Vec<FileChange>, is_merge: bool) -> Commit {
        Commit {
            id: CommitId(id),
            author: 0,
            timestamp: Utc.with_ymd_and_hms(2026, 8, 20, hour, 0, 0).unwrap(),
            message: String::new(),
            files_changed: files,
            is_merge,
            parent_count: if is_merge { 2 } else { 1 },
        }
    }

    /// Snapshot with one source and one test file known to the tree.
    fn snap(commits: Vec<Commit>) -> RepoSnapshot {
        let mut s = make_snapshot();
        s.files = vec![
            make_file("src/big.rs"),
            make_file("src/covered.rs"),
            make_file("src/smaller.rs"),
            make_file("tests/x_test.rs"),
        ];
        s.commits = commits;
        s
    }

    #[test]
    fn description_carries_totals_and_per_half_ratios() {
        // Window 10:00..14:00, midpoint 12:00. First half: source 210,
        // test 100 -> 2.1:1. Second half: source 400, test 100 -> 4.0:1.
        let s = snap(vec![
            commit_at(
                0,
                10,
                vec![change("src/big.rs", 210), change("tests/x_test.rs", 100)],
                false,
            ),
            commit_at(
                1,
                13,
                vec![change("src/big.rs", 400), change("tests/x_test.rs", 100)],
                false,
            ),
            commit_at(2, 14, vec![], false),
        ]);
        let m = growth_balance(&s);
        assert_eq!(m.name, "Code/test growth balance");
        assert_eq!(m.score, None, "annotation-first: never scored in v1");
        assert_eq!(
            m.description,
            "source +610 / test +200 lines this window; second half ratio 4.0:1 (first half 2.1:1)"
        );
    }

    #[test]
    fn commit_exactly_at_the_midpoint_counts_toward_the_second_half() {
        // Window 10:00..12:00, midpoint 11:00. The 11:00 commit's source
        // lines must land in the second half: 30/10 -> 3.0:1 there, and
        // no test growth in the first.
        let s = snap(vec![
            commit_at(0, 10, vec![change("src/big.rs", 10)], false),
            commit_at(1, 11, vec![change("src/big.rs", 30)], false),
            commit_at(2, 12, vec![change("tests/x_test.rs", 10)], false),
        ]);
        let m = growth_balance(&s);
        assert_eq!(
            m.description,
            "source +40 / test +10 lines this window; second half ratio 3.0:1 (first half no test growth)"
        );
    }

    #[test]
    fn zero_test_growth_wording_in_both_halves() {
        let s = snap(vec![
            commit_at(0, 10, vec![change("src/big.rs", 20)], false),
            commit_at(1, 14, vec![change("src/big.rs", 30)], false),
        ]);
        let m = growth_balance(&s);
        assert_eq!(
            m.description,
            "source +50 / test +0 lines this window; second half no test growth (first half no test growth)"
        );
    }

    #[test]
    fn merge_commits_do_not_count() {
        let s = snap(vec![
            commit_at(0, 10, vec![change("src/big.rs", 20)], false),
            commit_at(1, 14, vec![change("src/big.rs", 30)], false),
            commit_at(
                2,
                12,
                vec![change("src/big.rs", 9000), change("tests/x_test.rs", 9000)],
                true,
            ),
        ]);
        let m = growth_balance(&s);
        assert_eq!(
            m.description,
            "source +50 / test +0 lines this window; second half no test growth (first half no test growth)"
        );
    }

    #[test]
    fn evidence_lists_untested_second_half_source_files() {
        // big.rs and smaller.rs grow in the second half with no test-role
        // co-change; covered.rs grows but shares a commit with a test file.
        let s = snap(vec![
            commit_at(0, 10, vec![change("tests/x_test.rs", 5)], false),
            commit_at(1, 13, vec![change("src/big.rs", 234)], false),
            commit_at(2, 13, vec![change("src/smaller.rs", 10)], false),
            commit_at(
                3,
                14,
                vec![change("src/covered.rs", 500), change("tests/x_test.rs", 20)],
                false,
            ),
        ]);
        let m = growth_balance(&s);
        match &m.raw_value {
            RawValue::List(v) => {
                assert_eq!(
                    v,
                    &vec![
                        "src/big.rs — +234 lines (2nd half), no test co-change".to_string(),
                        "src/smaller.rs — +10 lines (2nd half), no test co-change".to_string(),
                    ]
                );
            }
            other => panic!("expected List, got {other:?}"),
        }
    }

    #[test]
    fn not_applicable_shapes() {
        let empty = snap(vec![]);
        let m = growth_balance(&empty);
        assert_eq!(m.score, None);
        assert_eq!(m.description, "No commits in window");

        let mut no_tests = snap(vec![commit_at(0, 10, vec![change("src/big.rs", 5)], false)]);
        no_tests.files.retain(|f| !f.path.starts_with("tests"));
        let m = growth_balance(&no_tests);
        assert_eq!(m.description, "No test files detected — not applicable");

        let mut no_source = snap(vec![commit_at(
            0,
            10,
            vec![change("tests/x_test.rs", 5)],
            false,
        )]);
        no_source.files.retain(|f| f.path.starts_with("tests"));
        let m = growth_balance(&no_source);
        assert_eq!(m.description, "No source files detected — not applicable");
    }
}
