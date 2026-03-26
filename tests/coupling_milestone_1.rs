use barad_dur::coupling::dependency::analyze_dependency_coupling;
use barad_dur::coupling::team::analyze_team_coupling;
use barad_dur::snapshot::{Author, RepoSnapshot, TimeWindow};
use std::path::PathBuf;

fn make_snapshot(name: &str, authors: Vec<Author>) -> (String, RepoSnapshot) {
    let mut snapshot = RepoSnapshot::new(
        PathBuf::from(format!("/tmp/{}", name)),
        name.to_string(),
        "main".to_string(),
        TimeWindow::full_history(),
    );
    snapshot.authors = authors;
    (name.to_string(), snapshot)
}

fn make_author(id: usize, name: &str, email: &str) -> Author {
    Author {
        id,
        name: name.to_string(),
        email: email.to_string(),
    }
}

#[test]
fn team_coupling_detects_shared_authors() {
    // GIVEN two repos with overlapping authors (matched by lowercase display name)
    let snapshots = vec![
        make_snapshot(
            "repo-alpha",
            vec![
                make_author(0, "Alice Smith", "alice@alpha.com"),
                make_author(1, "Bob Jones", "bob@alpha.com"),
                make_author(2, "Charlie Brown", "charlie@alpha.com"),
            ],
        ),
        make_snapshot(
            "repo-beta",
            vec![
                make_author(0, "alice smith", "alice@beta.com"),  // same person, different case
                make_author(1, "Bob Jones", "bob.j@beta.com"),    // same person, different email
                make_author(2, "Diana Prince", "diana@beta.com"), // unique to beta
            ],
        ),
    ];

    // WHEN team coupling is computed
    let pairs = analyze_team_coupling(&snapshots);

    // THEN the pair's team score equals (shared_authors / total_unique_authors) * 100
    assert_eq!(pairs.len(), 1, "should produce exactly one pair");
    let pair = &pairs[0];
    assert_eq!(pair.repo_a, "repo-alpha");
    assert_eq!(pair.repo_b, "repo-beta");

    // 2 shared (alice, bob) out of 4 unique (alice, bob, charlie, diana) => 50.0
    assert!(
        (pair.team_score - 50.0).abs() < 0.01,
        "expected team_score ~50.0, got {}",
        pair.team_score
    );

    // AND shared author names are listed
    assert_eq!(pair.shared_count, 2);
    let shared_lower: Vec<String> = pair
        .shared_authors
        .iter()
        .map(|s: &String| s.to_lowercase())
        .collect();
    assert!(shared_lower.contains(&"alice smith".to_string()));
    assert!(shared_lower.contains(&"bob jones".to_string()));

    // Bridge detection: 2 shared authors => not a single bridge
    assert!(!pair.is_single_bridge);
    assert!(pair.bridge_author.is_none());
}

#[test]
fn team_coupling_single_bridge_author() {
    // GIVEN exactly one shared author between two repos
    let snapshots = vec![
        make_snapshot(
            "repo-one",
            vec![
                make_author(0, "Alice Smith", "alice@one.com"),
                make_author(1, "Bob Jones", "bob@one.com"),
            ],
        ),
        make_snapshot(
            "repo-two",
            vec![
                make_author(0, "alice smith", "alice@two.com"),
                make_author(1, "Eve Wilson", "eve@two.com"),
            ],
        ),
    ];

    let pairs = analyze_team_coupling(&snapshots);

    // THEN is_single_bridge is true and bridge_author contains that author's name
    assert_eq!(pairs.len(), 1);
    let pair = &pairs[0];
    assert!(pair.is_single_bridge);
    assert_eq!(
        pair.bridge_author.as_deref().map(|s: &str| s.to_lowercase()),
        Some("alice smith".to_string())
    );

    // 1 shared out of 3 unique => ~33.33
    assert!(
        (pair.team_score - 33.333).abs() < 0.1,
        "expected team_score ~33.33, got {}",
        pair.team_score
    );
}

#[test]
fn team_coupling_no_shared_authors() {
    // GIVEN two repos with no shared authors
    let snapshots = vec![
        make_snapshot(
            "repo-x",
            vec![make_author(0, "Alice", "alice@x.com")],
        ),
        make_snapshot(
            "repo-y",
            vec![make_author(0, "Bob", "bob@y.com")],
        ),
    ];

    let pairs = analyze_team_coupling(&snapshots);

    // THEN the team score is 0.0 and shared_authors is empty
    assert_eq!(pairs.len(), 1);
    let pair = &pairs[0];
    assert!((pair.team_score - 0.0).abs() < 0.01);
    assert!(pair.shared_authors.is_empty());
    assert_eq!(pair.shared_count, 0);
    assert!(!pair.is_single_bridge);
    assert!(pair.bridge_author.is_none());
}
