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
    use chrono::{DateTime, Duration, Utc};

    fn change(p: &str, add: u32) -> FileChange {
        FileChange {
            path: p.into(),
            additions: add,
            deletions: 0,
            change_type: ChangeType::Modified,
        }
    }

    /// One wall-clock reading shared by every commit in the process, so
    /// `commit_ago(_, 4, ..)` sits exactly on the midpoint between
    /// `commit_ago(_, 8, ..)` and `commit_ago(_, 0, ..)` instead of drifting
    /// by the microseconds between successive `Utc::now()` calls.
    fn base_now() -> DateTime<Utc> {
        static BASE: std::sync::OnceLock<DateTime<Utc>> = std::sync::OnceLock::new();
        *BASE.get_or_init(Utc::now)
    }

    /// Commit `hours_ago` relative to a fixed "now" — inside
    /// TimeWindow::default() and safe against the window filter at any
    /// wall-clock time.
    fn commit_ago(id: u32, hours_ago: i64, files: Vec<FileChange>, is_merge: bool) -> Commit {
        Commit {
            id: CommitId(id),
            author: 0,
            timestamp: base_now() - Duration::hours(hours_ago),
            message: String::new(),
            files_changed: files,
            is_merge,
            parent_count: if is_merge { 2 } else { 1 },
        }
    }

    fn snap(commits: Vec<Commit>) -> RepoSnapshot {
        let mut s = make_snapshot();
        s.files = vec![
            make_file("src/big.rs"),
            make_file("src/covered.rs"),
            make_file("src/smaller.rs"),
            make_file("tests/x_test.rs"),
            make_file("tests/fixtures/data.json"),
        ];
        s.commits = commits;
        s
    }

    #[test]
    fn description_carries_totals_and_per_half_ratios() {
        // Window span 8h..0h ago, midpoint 4h. First half: source 210,
        // test-file 100 -> 2.1:1. Second half: source 400, test 100 -> 4.0:1.
        let s = snap(vec![
            commit_ago(
                0,
                8,
                vec![change("src/big.rs", 210), change("tests/x_test.rs", 100)],
                false,
            ),
            commit_ago(
                1,
                2,
                vec![change("src/big.rs", 400), change("tests/x_test.rs", 100)],
                false,
            ),
            commit_ago(2, 0, vec![], false),
        ]);
        let m = growth_balance(&s);
        assert_eq!(m.name, "Code/test growth balance");
        assert_eq!(m.score, None, "annotation-first: never scored in v1");
        assert_eq!(
            m.description,
            "source +610 / test-file +200 lines this window; second half ratio 4.0:1 (first half 2.1:1)"
        );
    }

    #[test]
    fn commit_exactly_at_the_midpoint_counts_toward_the_second_half() {
        // Span 8h..0h ago, midpoint 4h: the 4h commit's source lines land
        // in the second half -> 3.0:1 there, no test growth in the first.
        let s = snap(vec![
            commit_ago(0, 8, vec![change("src/big.rs", 10)], false),
            commit_ago(1, 4, vec![change("src/big.rs", 30)], false),
            commit_ago(2, 0, vec![change("tests/x_test.rs", 10)], false),
        ]);
        let m = growth_balance(&s);
        assert_eq!(
            m.description,
            "source +40 / test-file +10 lines this window; second half ratio 3.0:1 (first half no test growth); 1 recently-grown file(s) lack test co-change"
        );
    }

    #[test]
    fn zero_test_growth_wording_in_both_halves() {
        let s = snap(vec![
            commit_ago(0, 8, vec![change("src/big.rs", 20)], false),
            commit_ago(1, 1, vec![change("src/big.rs", 30)], false),
        ]);
        let m = growth_balance(&s);
        assert!(
            m.description.starts_with(
                "source +50 / test-file +0 lines this window; second half no test growth (first half no test growth)"
            ),
            "got: {}",
            m.description
        );
    }

    #[test]
    fn merge_commits_do_not_count() {
        let s = snap(vec![
            commit_ago(0, 8, vec![change("src/big.rs", 20)], false),
            commit_ago(1, 1, vec![change("src/big.rs", 30)], false),
            commit_ago(
                2,
                4,
                vec![change("src/big.rs", 9000), change("tests/x_test.rs", 9000)],
                true,
            ),
        ]);
        let m = growth_balance(&s);
        assert!(
            m.description
                .starts_with("source +50 / test-file +0 lines this window"),
            "merge churn must not count: {}",
            m.description
        );
    }

    #[test]
    fn out_of_window_commits_do_not_count() {
        // 200 days ago is outside TimeWindow::default() (180d) — the
        // sibling-metric window filter must apply here too.
        let s = snap(vec![
            commit_ago(0, 200 * 24, vec![change("src/big.rs", 9000)], false),
            commit_ago(1, 8, vec![change("src/big.rs", 20)], false),
            commit_ago(2, 1, vec![change("src/big.rs", 30)], false),
        ]);
        let m = growth_balance(&s);
        assert!(
            m.description
                .starts_with("source +50 / test-file +0 lines this window"),
            "out-of-window churn must not count: {}",
            m.description
        );
    }

    #[test]
    fn single_active_moment_skips_the_half_comparison() {
        // One commit: both halves cannot be populated — no fabricated
        // "first half no test growth" claim.
        let s = snap(vec![commit_ago(
            0,
            4,
            vec![change("src/big.rs", 40), change("tests/x_test.rs", 10)],
            false,
        )]);
        let m = growth_balance(&s);
        assert_eq!(
            m.description,
            "source +40 / test-file +10 lines this window; too few active moments for a half-window comparison"
        );
    }

    #[test]
    fn fixture_files_under_tests_do_not_count_as_test_growth() {
        // tests/fixtures/data.json is Test-role by directory but not a
        // code file — regenerated fixtures must not inflate the ratio.
        let s = snap(vec![
            commit_ago(
                0,
                8,
                vec![change("src/big.rs", 100), change("tests/x_test.rs", 50)],
                false,
            ),
            commit_ago(
                1,
                1,
                vec![
                    change("src/big.rs", 100),
                    change("tests/fixtures/data.json", 40000),
                ],
                false,
            ),
        ]);
        let m = growth_balance(&s);
        assert!(
            m.description
                .starts_with("source +200 / test-file +50 lines this window"),
            "fixture data must not count as test growth: {}",
            m.description
        );
    }

    #[test]
    fn first_half_test_co_change_does_not_mask_recent_untested_growth() {
        // src/covered.rs co-changed with a test in the FIRST half only,
        // then grew +900 untested in the second half — it must be listed.
        let s = snap(vec![
            commit_ago(
                0,
                8,
                vec![change("src/covered.rs", 10), change("tests/x_test.rs", 5)],
                false,
            ),
            commit_ago(1, 1, vec![change("src/covered.rs", 900)], false),
        ]);
        let m = growth_balance(&s);
        match &m.raw_value {
            RawValue::List(v) => assert_eq!(
                v,
                &vec!["src/covered.rs — +900 lines (2nd half), no test co-change".to_string()]
            ),
            other => panic!("expected List, got {other:?}"),
        }
        assert!(
            m.description
                .ends_with("; 1 recently-grown file(s) lack test co-change"),
            "description must carry the full untested count: {}",
            m.description
        );
    }

    #[test]
    fn second_half_test_co_change_exempts_a_file() {
        let s = snap(vec![
            commit_ago(0, 8, vec![change("src/big.rs", 10)], false),
            commit_ago(
                1,
                1,
                vec![change("src/covered.rs", 500), change("tests/x_test.rs", 20)],
                false,
            ),
        ]);
        let m = growth_balance(&s);
        match &m.raw_value {
            RawValue::List(v) => assert!(
                v.iter().all(|e| !e.starts_with("src/covered.rs")),
                "second-half test co-change must exempt: {v:?}"
            ),
            other => panic!("expected List, got {other:?}"),
        }
    }

    #[test]
    fn not_applicable_shapes() {
        let empty = snap(vec![]);
        let m = growth_balance(&empty);
        assert_eq!(m.score, None);
        assert_eq!(m.description, "No commits in window");

        let mut no_tests = snap(vec![commit_ago(0, 8, vec![change("src/big.rs", 5)], false)]);
        no_tests.files.retain(|f| !f.path.starts_with("tests"));
        let m = growth_balance(&no_tests);
        assert_eq!(m.description, "No test files detected — not applicable");

        let mut no_source = snap(vec![commit_ago(
            0,
            8,
            vec![change("tests/x_test.rs", 5)],
            false,
        )]);
        no_source.files.retain(|f| f.path.starts_with("tests"));
        let m = growth_balance(&no_source);
        assert_eq!(m.description, "No source files detected — not applicable");
    }
}
