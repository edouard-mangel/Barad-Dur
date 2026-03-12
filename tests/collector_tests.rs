use barad_dur::collector::Collector;
use barad_dur::snapshot::TimeWindow;
use std::path::{Path, PathBuf};

#[test]
fn open_repo_succeeds() {
    let collector = Collector::open(std::path::Path::new("."), TimeWindow::default());
    assert!(collector.is_ok());
    let collector = collector.unwrap();
    assert!(!collector.repo_name().is_empty());
}

#[test]
fn repo_name_is_correct() {
    let collector = Collector::open(std::path::Path::new("."), TimeWindow::default()).unwrap();
    assert_eq!(collector.repo_name(), "barad-dur");
}

#[test]
fn collect_commits_returns_nonempty() {
    let collector = Collector::open(std::path::Path::new("."), TimeWindow::full_history()).unwrap();
    let collection = collector.collect_commits().unwrap();
    assert!(!collection.commits.is_empty(), "Expected at least 1 commit");
}

#[test]
fn commits_have_required_fields() {
    let collector = Collector::open(std::path::Path::new("."), TimeWindow::full_history()).unwrap();
    let collection = collector.collect_commits().unwrap();
    for commit in &collection.commits {
        assert!(!commit.id.is_empty(), "Commit ID should not be empty");
        assert!(
            !commit.message.is_empty(),
            "Commit message should not be empty"
        );
    }
}

#[test]
fn authors_are_deduplicated() {
    let collector = Collector::open(std::path::Path::new("."), TimeWindow::full_history()).unwrap();
    let collection = collector.collect_commits().unwrap();
    assert!(!collection.authors.is_empty(), "Expected at least 1 author");
    // Check no duplicate emails
    let emails: Vec<&str> = collection
        .authors
        .iter()
        .map(|a| a.email.as_str())
        .collect();
    let unique: std::collections::HashSet<&str> = emails.iter().copied().collect();
    assert_eq!(
        emails.len(),
        unique.len(),
        "Authors should be deduplicated by email"
    );
}

#[test]
fn collect_files_returns_nonempty() {
    let collector = Collector::open(std::path::Path::new("."), TimeWindow::default()).unwrap();
    let files = collector.collect_files().unwrap();
    assert!(!files.is_empty(), "Expected at least 1 file");
}

#[test]
fn collect_files_includes_cargo_toml() {
    let collector = Collector::open(std::path::Path::new("."), TimeWindow::default()).unwrap();
    let files = collector.collect_files().unwrap();
    let has_cargo = files
        .iter()
        .any(|f| f.path.as_path() == Path::new("Cargo.toml"));
    assert!(has_cargo, "Expected Cargo.toml in file list");
}

#[test]
fn files_have_depth() {
    let collector = Collector::open(std::path::Path::new("."), TimeWindow::default()).unwrap();
    let files = collector.collect_files().unwrap();
    // Cargo.toml should have depth 1, src/main.rs should have depth 2
    let cargo = files
        .iter()
        .find(|f| f.path.as_path() == Path::new("Cargo.toml"))
        .unwrap();
    assert_eq!(cargo.depth, 1);

    let main = files
        .iter()
        .find(|f| f.path.as_path() == Path::new("src/main.rs"))
        .unwrap();
    assert_eq!(main.depth, 2);
}

#[test]
fn head_commit_hash_is_valid() {
    let collector = Collector::open(std::path::Path::new("."), TimeWindow::default()).unwrap();
    let hash = collector.head_commit_hash().unwrap();
    assert_eq!(hash.len(), 40, "Git hash should be 40 hex chars");
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "Git hash should be hex"
    );
}

// --- Blame tests ---

#[test]
fn collect_blame_returns_data_for_cargo_toml() {
    let collector = Collector::open(std::path::Path::new("."), TimeWindow::full_history()).unwrap();
    let collection = collector.collect_commits().unwrap();
    let files = collector.collect_files().unwrap();
    let blame_map = collector
        .collect_blame(&files, &collection.authors, None)
        .unwrap();

    let cargo_blame = blame_map.get(&PathBuf::from("Cargo.toml"));
    assert!(cargo_blame.is_some(), "Expected blame data for Cargo.toml");
    assert!(
        !cargo_blame.unwrap().is_empty(),
        "Cargo.toml blame should have lines"
    );
}

#[test]
fn blame_lines_have_valid_author_ids() {
    let collector = Collector::open(std::path::Path::new("."), TimeWindow::full_history()).unwrap();
    let collection = collector.collect_commits().unwrap();
    let files = collector.collect_files().unwrap();
    let blame_map = collector
        .collect_blame(&files, &collection.authors, None)
        .unwrap();

    let max_author_id = collection.authors.len();
    for lines in blame_map.values() {
        for line in lines {
            assert!(
                line.author_id < max_author_id,
                "Blame line author_id {} should be < {}",
                line.author_id,
                max_author_id
            );
        }
    }
}

#[test]
fn shallow_clone_detection() {
    let collector = Collector::open(std::path::Path::new("."), TimeWindow::default()).unwrap();
    // This repo is not shallow
    assert!(!collector.is_shallow());
}

// --- Full snapshot tests ---

#[test]
fn collect_snapshot_returns_populated_snapshot() {
    let collector = Collector::open(std::path::Path::new("."), TimeWindow::full_history()).unwrap();
    let snapshot = collector.collect_snapshot().unwrap();

    assert!(!snapshot.commits.is_empty(), "Snapshot should have commits");
    assert!(!snapshot.files.is_empty(), "Snapshot should have files");
    assert!(!snapshot.authors.is_empty(), "Snapshot should have authors");
    assert!(
        !snapshot.blame_map.is_empty(),
        "Snapshot should have blame data"
    );
    assert!(
        !snapshot.head_commit.is_empty(),
        "Snapshot should have HEAD hash"
    );
    assert_eq!(snapshot.head_commit.len(), 40);
}

#[test]
fn collect_snapshot_has_derived_indexes() {
    let collector = Collector::open(std::path::Path::new("."), TimeWindow::full_history()).unwrap();
    let snapshot = collector.collect_snapshot().unwrap();

    assert!(
        !snapshot.commits_by_author.is_empty(),
        "commits_by_author should be populated"
    );
    assert!(
        !snapshot.commits_by_file.is_empty(),
        "commits_by_file should be populated"
    );
    // file_change_pairs may be empty if no files co-change >= 3 times yet
}
