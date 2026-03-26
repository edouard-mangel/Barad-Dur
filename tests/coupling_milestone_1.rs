use barad_dur::coupling::dependency::analyze_dependency_coupling;
use barad_dur::coupling::scorer::score_coupling_pairs;
use barad_dur::coupling::team::analyze_team_coupling;
use barad_dur::coupling::temporal::TemporalCouplingPair;
use barad_dur::coupling::team::TeamCouplingPair;
use barad_dur::coupling::dependency::{DependencyAnalysis, DependencyCouplingPair, BlastRadiusEntry};
use barad_dur::coupling::{CouplingReport, CouplingReportSummary, RepoInfo};
use barad_dur::renderer::coupling_json::render_coupling_json;
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

// ===========================================================================
// Dependency coupling tests
// ===========================================================================

fn create_repo_with_manifest(
    parent: &std::path::Path,
    name: &str,
    filename: &str,
    content: &str,
) -> PathBuf {
    let repo_path = parent.join(name);
    std::fs::create_dir_all(&repo_path).unwrap();
    std::fs::write(repo_path.join(filename), content).unwrap();
    repo_path
}

#[test]
fn dependency_coupling_scans_cargo_toml_shared_deps() {
    let root = tempfile::TempDir::new().unwrap();

    let cargo_a = "[package]\nname = \"repo-a\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\ntokio = \"1\"\nanyhow = \"1\"\n";
    let cargo_b = "[package]\nname = \"repo-b\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\ntokio = \"1\"\nclap = \"4\"\n";

    let path_a = create_repo_with_manifest(root.path(), "repo-a", "Cargo.toml", cargo_a);
    let path_b = create_repo_with_manifest(root.path(), "repo-b", "Cargo.toml", cargo_b);

    let repo_paths: Vec<(String, PathBuf)> = vec![
        ("repo-a".to_string(), path_a),
        ("repo-b".to_string(), path_b),
    ];

    let analysis = analyze_dependency_coupling(&repo_paths);

    assert_eq!(analysis.pairs.len(), 1, "expected 1 coupling pair");
    let pair = &analysis.pairs[0];
    let mut shared = pair.shared_deps.clone();
    shared.sort();
    assert_eq!(shared, vec!["serde", "tokio"]);
    assert_eq!(pair.shared_count, 2);
    assert!(pair.dep_score > 0.0, "dep_score should be positive");
}

#[test]
fn dependency_coupling_detects_direct_path_dependency() {
    let root = tempfile::TempDir::new().unwrap();

    let cargo_a = "[package]\nname = \"repo-a\"\nversion = \"0.1.0\"\n\n[dependencies]\nrepo-b = { path = \"../repo-b\" }\nserde = \"1\"\n";
    let cargo_b = "[package]\nname = \"repo-b\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n";

    let path_a = create_repo_with_manifest(root.path(), "repo-a", "Cargo.toml", cargo_a);
    let path_b = create_repo_with_manifest(root.path(), "repo-b", "Cargo.toml", cargo_b);

    let repo_paths: Vec<(String, PathBuf)> = vec![
        ("repo-a".to_string(), path_a),
        ("repo-b".to_string(), path_b),
    ];

    let analysis = analyze_dependency_coupling(&repo_paths);

    let pair = analysis.pairs.iter().find(|p| {
        (p.repo_a == "repo-a" && p.repo_b == "repo-b")
            || (p.repo_a == "repo-b" && p.repo_b == "repo-a")
    });
    assert!(pair.is_some(), "should find repo-a / repo-b pair");
    let pair = pair.unwrap();

    assert!(pair.direct_dependency.is_some(), "should detect direct dependency");
    let direct = pair.direct_dependency.as_ref().unwrap();
    assert_eq!(direct.from, "repo-a");
    assert_eq!(direct.to, "repo-b");
}

#[test]
fn dependency_coupling_detects_direct_git_dependency() {
    let root = tempfile::TempDir::new().unwrap();

    let cargo_a = "[package]\nname = \"repo-a\"\nversion = \"0.1.0\"\n\n[dependencies]\nrepo-b = { git = \"https://github.com/org/repo-b.git\" }\n";
    let cargo_b = "[package]\nname = \"repo-b\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n";

    let path_a = create_repo_with_manifest(root.path(), "repo-a", "Cargo.toml", cargo_a);
    let path_b = create_repo_with_manifest(root.path(), "repo-b", "Cargo.toml", cargo_b);

    let repo_paths: Vec<(String, PathBuf)> = vec![
        ("repo-a".to_string(), path_a),
        ("repo-b".to_string(), path_b),
    ];

    let analysis = analyze_dependency_coupling(&repo_paths);

    let pair = analysis.pairs.iter().find(|p| p.direct_dependency.is_some());
    assert!(pair.is_some(), "should detect git-based direct dependency");
    let direct = pair.unwrap().direct_dependency.as_ref().unwrap();
    assert_eq!(direct.from, "repo-a");
    assert_eq!(direct.to, "repo-b");
}

#[test]
fn dependency_coupling_scans_package_json() {
    let root = tempfile::TempDir::new().unwrap();

    let pkg_a = r#"{"name":"app-a","dependencies":{"express":"^4.18.0","lodash":"^4.17.0"},"devDependencies":{"jest":"^29.0.0"}}"#;
    let pkg_b = r#"{"name":"app-b","dependencies":{"express":"^4.18.0","axios":"^1.0.0"},"devDependencies":{"jest":"^29.0.0"}}"#;

    let path_a = create_repo_with_manifest(root.path(), "app-a", "package.json", pkg_a);
    let path_b = create_repo_with_manifest(root.path(), "app-b", "package.json", pkg_b);

    let repo_paths: Vec<(String, PathBuf)> = vec![
        ("app-a".to_string(), path_a),
        ("app-b".to_string(), path_b),
    ];

    let analysis = analyze_dependency_coupling(&repo_paths);

    assert_eq!(analysis.pairs.len(), 1);
    let pair = &analysis.pairs[0];
    let mut shared = pair.shared_deps.clone();
    shared.sort();
    assert_eq!(shared, vec!["express", "jest"]);
    assert_eq!(pair.shared_count, 2);
}

#[test]
fn dependency_coupling_scans_go_mod() {
    let root = tempfile::TempDir::new().unwrap();

    let go_mod_a = "module github.com/org/service-a\n\ngo 1.21\n\nrequire (\n\tgithub.com/gin-gonic/gin v1.9.0\n\tgithub.com/go-redis/redis v6.15.0\n\tgoogle.golang.org/grpc v1.58.0\n)\n";
    let go_mod_b = "module github.com/org/service-b\n\ngo 1.21\n\nrequire (\n\tgithub.com/gin-gonic/gin v1.9.0\n\tgoogle.golang.org/grpc v1.58.0\n\tgithub.com/stretchr/testify v1.8.0\n)\n";

    let path_a = create_repo_with_manifest(root.path(), "service-a", "go.mod", go_mod_a);
    let path_b = create_repo_with_manifest(root.path(), "service-b", "go.mod", go_mod_b);

    let repo_paths: Vec<(String, PathBuf)> = vec![
        ("service-a".to_string(), path_a),
        ("service-b".to_string(), path_b),
    ];

    let analysis = analyze_dependency_coupling(&repo_paths);

    assert_eq!(analysis.pairs.len(), 1);
    let pair = &analysis.pairs[0];
    let mut shared = pair.shared_deps.clone();
    shared.sort();
    assert_eq!(shared, vec!["github.com/gin-gonic/gin", "google.golang.org/grpc"]);
    assert_eq!(pair.shared_count, 2);
}

#[test]
fn dependency_coupling_scans_requirements_txt() {
    let root = tempfile::TempDir::new().unwrap();

    let req_a = "flask==2.3.0\nrequests>=2.28\nnumpy\n";
    let req_b = "django==4.2\nrequests>=2.28\nnumpy==1.24.0\n";

    let path_a = create_repo_with_manifest(root.path(), "py-a", "requirements.txt", req_a);
    let path_b = create_repo_with_manifest(root.path(), "py-b", "requirements.txt", req_b);

    let repo_paths: Vec<(String, PathBuf)> = vec![
        ("py-a".to_string(), path_a),
        ("py-b".to_string(), path_b),
    ];

    let analysis = analyze_dependency_coupling(&repo_paths);

    assert_eq!(analysis.pairs.len(), 1);
    let pair = &analysis.pairs[0];
    let mut shared = pair.shared_deps.clone();
    shared.sort();
    assert_eq!(shared, vec!["numpy", "requests"]);
    assert_eq!(pair.shared_count, 2);
}

#[test]
fn blast_radius_lists_hub_dependencies_with_3_plus_consumers() {
    let root = tempfile::TempDir::new().unwrap();

    let cargo_a = "[dependencies]\nserde = \"1\"\ntokio = \"1\"\n";
    let cargo_b = "[dependencies]\nserde = \"1\"\ntokio = \"1\"\n";
    let cargo_c = "[dependencies]\nserde = \"1\"\nclap = \"4\"\n";

    let path_a = create_repo_with_manifest(root.path(), "repo-a", "Cargo.toml", cargo_a);
    let path_b = create_repo_with_manifest(root.path(), "repo-b", "Cargo.toml", cargo_b);
    let path_c = create_repo_with_manifest(root.path(), "repo-c", "Cargo.toml", cargo_c);

    let repo_paths: Vec<(String, PathBuf)> = vec![
        ("repo-a".to_string(), path_a),
        ("repo-b".to_string(), path_b),
        ("repo-c".to_string(), path_c),
    ];

    let analysis = analyze_dependency_coupling(&repo_paths);

    let serde_entry = analysis.blast_radius.iter().find(|e| e.dependency_name == "serde");
    assert!(serde_entry.is_some(), "serde should be in blast_radius");
    let serde_entry = serde_entry.unwrap();
    assert_eq!(serde_entry.consumer_count, 3);
    let mut consumers = serde_entry.consumers.clone();
    consumers.sort();
    assert_eq!(consumers, vec!["repo-a", "repo-b", "repo-c"]);

    let tokio_entry = analysis.blast_radius.iter().find(|e| e.dependency_name == "tokio");
    assert!(tokio_entry.is_none(), "tokio (only 2 consumers) should NOT be in blast_radius");
}

// ===========================================================================
// Combined scoring and JSON output (Step 02-03)
// ===========================================================================

fn make_temporal_pair(repo_a: &str, repo_b: &str, score: f64) -> TemporalCouplingPair {
    use barad_dur::coupling::temporal::Confidence;
    TemporalCouplingPair {
        repo_a: repo_a.to_string(),
        repo_b: repo_b.to_string(),
        co_changes: 10,
        temporal_score: score,
        confidence: Confidence::Medium,
    }
}

fn make_team_pair(repo_a: &str, repo_b: &str, score: f64) -> TeamCouplingPair {
    TeamCouplingPair {
        repo_a: repo_a.to_string(),
        repo_b: repo_b.to_string(),
        team_score: score,
        shared_authors: vec!["alice".to_string()],
        shared_count: 1,
        is_single_bridge: true,
        bridge_author: Some("alice".to_string()),
    }
}

fn make_dep_pair(repo_a: &str, repo_b: &str, score: f64) -> DependencyCouplingPair {
    DependencyCouplingPair {
        repo_a: repo_a.to_string(),
        repo_b: repo_b.to_string(),
        shared_deps: vec!["serde".to_string()],
        shared_count: 1,
        dep_score: score,
        direct_dependency: None,
    }
}

#[test]
fn combined_scoring_and_json_output() {
    // GIVEN all three coupling dimensions are available for a pair
    let temporal = vec![make_temporal_pair("repo-a", "repo-b", 80.0)];
    let team = vec![make_team_pair("repo-a", "repo-b", 60.0)];
    let dependency = DependencyAnalysis {
        pairs: vec![make_dep_pair("repo-a", "repo-b", 40.0)],
        blast_radius: vec![BlastRadiusEntry {
            dependency_name: "serde".to_string(),
            consumers: vec!["repo-a".to_string(), "repo-b".to_string(), "repo-c".to_string()],
            consumer_count: 3,
        }],
    };

    // WHEN the combined score is computed
    let pairs = score_coupling_pairs(&temporal, &team, &dependency);

    // THEN it equals temporal * 0.50 + team * 0.25 + dependency * 0.25
    assert_eq!(pairs.len(), 1);
    let pair = &pairs[0];
    let expected = 80.0 * 0.50 + 60.0 * 0.25 + 40.0 * 0.25;
    assert!(
        (pair.combined_score - expected).abs() < 0.01,
        "expected combined_score ~{}, got {}",
        expected,
        pair.combined_score
    );
    assert!((pair.temporal_score - 80.0).abs() < 0.01);
    assert!((pair.team_score - 60.0).abs() < 0.01);
    assert!((pair.dependency_score - 40.0).abs() < 0.01);

    // GIVEN --json flag is passed to the coupling subcommand
    let report = CouplingReport {
        repos: vec![
            RepoInfo {
                name: "repo-a".to_string(),
                path: PathBuf::from("/tmp/repo-a"),
                commit_count: 100,
                author_count: 5,
            },
            RepoInfo {
                name: "repo-b".to_string(),
                path: PathBuf::from("/tmp/repo-b"),
                commit_count: 80,
                author_count: 3,
            },
        ],
        pairs: pairs.clone(),
        summary: CouplingReportSummary {
            total_repos: 2,
            total_pairs_analyzed: 1,
            pairs_above_threshold: 1,
            highest_coupling_score: pairs[0].combined_score,
        },
        blast_radius: vec![BlastRadiusEntry {
            dependency_name: "serde".to_string(),
            consumers: vec!["repo-a".to_string(), "repo-b".to_string(), "repo-c".to_string()],
            consumer_count: 3,
        }],
    };

    // WHEN the report is rendered as JSON
    let json_output = render_coupling_json(&report, false);

    // THEN output is valid JSON with expected schema
    let parsed: serde_json::Value = serde_json::from_str(&json_output)
        .expect("should be valid JSON");

    let coupling = parsed.get("coupling").expect("should have top-level 'coupling' key");
    assert_eq!(coupling["schema_version"], 1);
    assert_eq!(coupling["repos_scanned"], 2);
    assert_eq!(coupling["pairs_analyzed"], 1);

    let json_pairs = coupling["pairs"].as_array().expect("pairs should be an array");
    assert_eq!(json_pairs.len(), 1);
    assert_eq!(json_pairs[0]["repo_a"], "repo-a");
    assert_eq!(json_pairs[0]["repo_b"], "repo-b");

    let blast = coupling["blast_radius"].as_array().expect("blast_radius should be an array");
    assert_eq!(blast.len(), 1);
    assert_eq!(blast[0]["dependency_name"], "serde");

    // GIVEN --json --pretty flags
    let pretty_output = render_coupling_json(&report, true);
    // THEN the JSON output is pretty-printed (contains newlines and indentation)
    assert!(pretty_output.contains('\n'), "pretty-printed should have newlines");
    assert!(pretty_output.contains("  "), "pretty-printed should have indentation");

    // Both should parse to same data
    let parsed_pretty: serde_json::Value = serde_json::from_str(&pretty_output)
        .expect("pretty JSON should also be valid");
    assert_eq!(parsed, parsed_pretty);
}
