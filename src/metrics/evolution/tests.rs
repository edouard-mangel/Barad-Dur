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
    assert_eq!(result.score, 92);
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
    assert!(result.score >= 80);
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
    assert_eq!(result.score, 55);
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
    assert_eq!(result.score, 25);
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
    assert!(result.score >= 70);
}
