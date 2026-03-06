use barad_dur::collector::Collector;
use barad_dur::snapshot::TimeWindow;
use std::path::PathBuf;

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
    assert_eq!(collector.repo_name(), "myTool");
}

#[test]
fn collect_commits_returns_nonempty() {
    let collector =
        Collector::open(std::path::Path::new("."), TimeWindow::full_history()).unwrap();
    let collection = collector.collect_commits().unwrap();
    assert!(!collection.commits.is_empty(), "Expected at least 1 commit");
}

#[test]
fn commits_have_required_fields() {
    let collector =
        Collector::open(std::path::Path::new("."), TimeWindow::full_history()).unwrap();
    let collection = collector.collect_commits().unwrap();
    for commit in &collection.commits {
        assert!(!commit.id.is_empty(), "Commit ID should not be empty");
        assert!(!commit.message.is_empty(), "Commit message should not be empty");
    }
}

#[test]
fn authors_are_deduplicated() {
    let collector =
        Collector::open(std::path::Path::new("."), TimeWindow::full_history()).unwrap();
    let collection = collector.collect_commits().unwrap();
    assert!(
        !collection.authors.is_empty(),
        "Expected at least 1 author"
    );
    // Check no duplicate emails
    let emails: Vec<&str> = collection.authors.iter().map(|a| a.email.as_str()).collect();
    let unique: std::collections::HashSet<&str> = emails.iter().copied().collect();
    assert_eq!(emails.len(), unique.len(), "Authors should be deduplicated by email");
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
        .any(|f| f.path == PathBuf::from("Cargo.toml"));
    assert!(has_cargo, "Expected Cargo.toml in file list");
}

#[test]
fn files_have_depth() {
    let collector = Collector::open(std::path::Path::new("."), TimeWindow::default()).unwrap();
    let files = collector.collect_files().unwrap();
    // Cargo.toml should have depth 1, src/main.rs should have depth 2
    let cargo = files
        .iter()
        .find(|f| f.path == PathBuf::from("Cargo.toml"))
        .unwrap();
    assert_eq!(cargo.depth, 1);

    let main = files
        .iter()
        .find(|f| f.path == PathBuf::from("src/main.rs"))
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
