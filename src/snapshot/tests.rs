use super::*;

#[test]
fn time_window_default_is_six_months() {
    let window = TimeWindow::default();
    assert_eq!(window.default_months, 6);
    assert!(window.since.is_some());
    assert!(window.until.is_some());

    let since = window.since.unwrap();
    let until = window.until.unwrap();
    let diff = until - since;
    // Should be approximately 180 days (allow some tolerance for test execution time)
    assert!(diff.num_days() >= 179 && diff.num_days() <= 181);
}

#[test]
fn time_window_contains_timestamp_in_range() {
    let window = TimeWindow::default();
    let now = Utc::now();
    // A timestamp from 3 months ago should be within the default 6-month window
    let three_months_ago = now - Duration::days(90);
    assert!(window.contains(&three_months_ago));
}

#[test]
fn time_window_excludes_timestamp_before_window() {
    let window = TimeWindow::default();
    let now = Utc::now();
    // A timestamp from 1 year ago should be outside the default 6-month window
    let one_year_ago = now - Duration::days(365);
    assert!(!window.contains(&one_year_ago));
}

#[test]
fn time_window_excludes_timestamp_after_window() {
    let now = Utc::now();
    let window = TimeWindow {
        since: Some(now - Duration::days(180)),
        until: Some(now - Duration::days(30)),
        default_months: 6,
    };
    // Current time should be after the window's until
    assert!(!window.contains(&now));
}

#[test]
fn time_window_full_history_contains_everything() {
    let window = TimeWindow::full_history();
    let ancient = Utc::now() - Duration::days(10000);
    let future = Utc::now() + Duration::days(10000);
    assert!(window.contains(&ancient));
    assert!(window.contains(&future));
}

#[test]
fn repo_snapshot_new_creates_empty() {
    let snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp/test"),
        "test-repo".to_string(),
        "main".to_string(),
        TimeWindow::default(),
    );
    assert_eq!(snapshot.name, "test-repo");
    assert_eq!(snapshot.default_branch, "main");
    assert!(snapshot.commits.is_empty());
    assert!(snapshot.files.is_empty());
    assert!(snapshot.authors.is_empty());
    assert!(snapshot.blame_map.is_empty());
    assert!(snapshot.file_metrics.is_empty());
}

#[test]
fn file_metrics_starts_empty() {
    let snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp/test"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    assert!(snapshot.file_metrics.is_empty());
}

fn make_commit(id: u32, author: AuthorId, files: Vec<&str>) -> Commit {
    Commit {
        id: CommitId(id),
        author,
        timestamp: Utc::now(),
        message: format!("Commit {}", id),
        files_changed: files
            .into_iter()
            .map(|f| FileChange {
                path: PathBuf::from(f),
                additions: 1,
                deletions: 0,
                change_type: ChangeType::Modified,
            })
            .collect(),
        is_merge: false,
        parent_count: 1,
    }
}

#[test]
fn build_commits_by_author_groups_correctly() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    snapshot.commits = vec![
        make_commit(0, 0, vec!["f1"]),
        make_commit(1, 0, vec!["f2"]),
        make_commit(2, 1, vec!["f1"]),
    ];
    snapshot.build_indexes();

    assert_eq!(snapshot.commits_by_author[&0].len(), 2);
    assert_eq!(snapshot.commits_by_author[&1].len(), 1);
}

#[test]
fn build_commits_by_file_maps_correctly() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    snapshot.commits = vec![
        make_commit(0, 0, vec!["a.rs", "b.rs"]),
        make_commit(1, 1, vec!["a.rs"]),
    ];
    snapshot.build_indexes();

    assert_eq!(snapshot.commits_by_file[&PathBuf::from("a.rs")].len(), 2);
    assert_eq!(snapshot.commits_by_file[&PathBuf::from("b.rs")].len(), 1);
}

fn make_file(path: &str) -> FileEntry {
    FileEntry {
        path: PathBuf::from(path),
        size_bytes: 100,
        is_binary: false,
        depth: 1,
        blob_oid: String::new(),
    }
}

#[test]
fn build_file_change_pairs_detects_coupling() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    snapshot.files = vec![make_file("a.rs"), make_file("b.rs"), make_file("c.rs")];
    // Files A and B change together in 5 commits, C only once
    snapshot.commits = vec![
        make_commit(0, 0, vec!["a.rs", "b.rs"]),
        make_commit(1, 0, vec!["a.rs", "b.rs"]),
        make_commit(2, 0, vec!["a.rs", "b.rs"]),
        make_commit(3, 0, vec!["a.rs", "b.rs", "c.rs"]),
        make_commit(4, 0, vec!["a.rs", "b.rs"]),
    ];
    snapshot.build_indexes();

    // a.rs + b.rs should appear as a coupled pair (5 co-changes >= 3)
    let ab_pair = snapshot.file_change_pairs.iter().find(|(a, b, _)| {
        (a == &PathBuf::from("a.rs") && b == &PathBuf::from("b.rs"))
            || (a == &PathBuf::from("b.rs") && b == &PathBuf::from("a.rs"))
    });
    assert!(ab_pair.is_some(), "a.rs and b.rs should be coupled");
    assert_eq!(ab_pair.unwrap().2, 5);

    // a.rs + c.rs only co-changed once, should NOT appear (threshold is 3)
    let ac_pair = snapshot.file_change_pairs.iter().find(|(a, b, _)| {
        (a == &PathBuf::from("a.rs") && b == &PathBuf::from("c.rs"))
            || (a == &PathBuf::from("c.rs") && b == &PathBuf::from("a.rs"))
    });
    assert!(
        ac_pair.is_none(),
        "a.rs and c.rs should NOT be coupled (only 1 co-change)"
    );
}

#[test]
fn build_file_change_pairs_excludes_files_not_in_tree() {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    // Only a.rs and b.rs are in the file tree; i18n file is excluded
    snapshot.files = vec![make_file("a.rs"), make_file("b.rs")];
    snapshot.commits = vec![
        make_commit(0, 0, vec!["a.rs", "b.rs", "src/i18n/en.ts"]),
        make_commit(1, 0, vec!["a.rs", "b.rs", "src/i18n/en.ts"]),
        make_commit(2, 0, vec!["a.rs", "b.rs", "src/i18n/en.ts"]),
    ];
    snapshot.build_indexes();

    // a.rs + b.rs should be coupled
    assert!(
        snapshot
            .file_change_pairs
            .iter()
            .any(|(a, b, _)| a == &PathBuf::from("a.rs") && b == &PathBuf::from("b.rs")),
        "a.rs and b.rs should be coupled"
    );
    // i18n file should NOT appear in any pair
    assert!(
        !snapshot
            .file_change_pairs
            .iter()
            .any(|(a, b, _)| a.to_string_lossy().contains("i18n")
                || b.to_string_lossy().contains("i18n")),
        "excluded i18n files should not appear in coupling pairs"
    );
}

// --- count_co_changed_pairs ---

#[test]
fn co_changed_pairs_empty_commits_returns_empty() {
    let known: std::collections::HashSet<&PathBuf> = std::collections::HashSet::new();
    assert!(count_co_changed_pairs(&[], &known).is_empty());
}

#[test]
fn co_changed_pairs_below_threshold_excluded() {
    // 2 co-changes — below the minimum of 3
    let a = PathBuf::from("a.rs");
    let b = PathBuf::from("b.rs");
    let known: std::collections::HashSet<&PathBuf> = [&a, &b].into_iter().collect();
    let commits = vec![
        make_commit(0, 0, vec!["a.rs", "b.rs"]),
        make_commit(1, 0, vec!["a.rs", "b.rs"]),
    ];
    assert!(count_co_changed_pairs(&commits, &known).is_empty());
}

#[test]
fn co_changed_pairs_at_threshold_included() {
    // Exactly 3 co-changes — should be included
    let a = PathBuf::from("a.rs");
    let b = PathBuf::from("b.rs");
    let known: std::collections::HashSet<&PathBuf> = [&a, &b].into_iter().collect();
    let commits = vec![
        make_commit(0, 0, vec!["a.rs", "b.rs"]),
        make_commit(1, 0, vec!["a.rs", "b.rs"]),
        make_commit(2, 0, vec!["a.rs", "b.rs"]),
    ];
    let pairs = count_co_changed_pairs(&commits, &known);
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].2, 3);
}

#[test]
fn co_changed_pairs_unknown_files_excluded() {
    // "unknown.rs" is not in known_files — should be ignored
    let a = PathBuf::from("a.rs");
    let b = PathBuf::from("b.rs");
    let known: std::collections::HashSet<&PathBuf> = [&a, &b].into_iter().collect();
    let commits = vec![
        make_commit(0, 0, vec!["a.rs", "b.rs", "unknown.rs"]),
        make_commit(1, 0, vec!["a.rs", "b.rs", "unknown.rs"]),
        make_commit(2, 0, vec!["a.rs", "b.rs", "unknown.rs"]),
    ];
    let pairs = count_co_changed_pairs(&commits, &known);
    // Only the a/b pair; no pairs involving unknown.rs
    assert!(pairs
        .iter()
        .all(|(x, y, _)| x != &PathBuf::from("unknown.rs") && y != &PathBuf::from("unknown.rs")));
    assert_eq!(pairs.len(), 1);
}

#[test]
fn co_changed_pairs_normalized_a_less_than_b() {
    // Regardless of commit order, pair should always be (smaller, larger)
    let a = PathBuf::from("a.rs");
    let b = PathBuf::from("z.rs");
    let known: std::collections::HashSet<&PathBuf> = [&a, &b].into_iter().collect();
    let commits = (0..3)
        .map(|i| make_commit(i, 0, vec!["z.rs", "a.rs"]))
        .collect::<Vec<_>>();
    let pairs = count_co_changed_pairs(&commits, &known);
    assert_eq!(pairs.len(), 1);
    assert!(pairs[0].0 < pairs[0].1, "pair should be normalized a < b");
}

#[test]
fn repo_snapshot_serialization_roundtrip() {
    let snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp/test"),
        "test-repo".to_string(),
        "main".to_string(),
        TimeWindow::default(),
    );
    let config = bincode::config::standard();
    let encoded = bincode::serde::encode_to_vec(&snapshot, config).expect("Failed to serialize");
    let (decoded, _): (RepoSnapshot, _) =
        bincode::serde::decode_from_slice(&encoded, config).expect("Failed to deserialize");
    assert_eq!(decoded.name, snapshot.name);
    assert_eq!(decoded.default_branch, snapshot.default_branch);
    assert_eq!(decoded.path, snapshot.path);
}
