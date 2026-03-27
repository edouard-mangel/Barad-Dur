use barad_dur::coupling::{
    CouplingDetails, CouplingPair, CouplingReport, CouplingReportSummary, DependencyDetails,
    RepoInfo, TeamDetails, TemporalDetails,
};
use barad_dur::renderer::coupling_html::render_coupling_html;
use std::path::PathBuf;

fn sample_report() -> CouplingReport {
    CouplingReport {
        repos: vec![
            RepoInfo {
                name: "auth-service".to_string(),
                path: PathBuf::from("/repos/auth-service"),
                commit_count: 120,
                author_count: 5,
            },
            RepoInfo {
                name: "user-service".to_string(),
                path: PathBuf::from("/repos/user-service"),
                commit_count: 95,
                author_count: 4,
            },
            RepoInfo {
                name: "api-gateway".to_string(),
                path: PathBuf::from("/repos/api-gateway"),
                commit_count: 200,
                author_count: 8,
            },
        ],
        pairs: vec![
            CouplingPair {
                repo_a: "auth-service".to_string(),
                repo_b: "user-service".to_string(),
                temporal_score: 45.0,
                team_score: 30.0,
                dependency_score: 20.0,
                combined_score: 72.5,
                details: CouplingDetails {
                    temporal: TemporalDetails {
                        co_commit_count: 18,
                        total_windows: 40,
                    },
                    team: TeamDetails {
                        shared_authors: 3,
                        total_authors: 6,
                    },
                    dependency: DependencyDetails {
                        shared_dependencies: 4,
                        relationship: "shared-proto".to_string(),
                    },
                },
            },
            CouplingPair {
                repo_a: "auth-service".to_string(),
                repo_b: "api-gateway".to_string(),
                temporal_score: 25.0,
                team_score: 15.0,
                dependency_score: 10.0,
                combined_score: 38.0,
                details: CouplingDetails {
                    temporal: TemporalDetails {
                        co_commit_count: 10,
                        total_windows: 40,
                    },
                    team: TeamDetails {
                        shared_authors: 2,
                        total_authors: 11,
                    },
                    dependency: DependencyDetails {
                        shared_dependencies: 1,
                        relationship: "api-client".to_string(),
                    },
                },
            },
        ],
        summary: CouplingReportSummary {
            total_repos: 3,
            total_pairs_analyzed: 3,
            pairs_above_threshold: 2,
            highest_coupling_score: 72.5,
        },
        blast_radius: vec![],
    }
}

#[test]
fn html_graph_renders_self_contained() {
    let report = sample_report();
    let html = render_coupling_html(&report);

    // Must be a complete HTML document
    assert!(html.contains("<!DOCTYPE html>"), "missing DOCTYPE");
    assert!(html.contains("<html"), "missing html tag");
    assert!(html.contains("</html>"), "missing closing html tag");

    // All CSS and JS must be inlined (no external dependencies)
    assert!(
        !html.contains("href=\"http"),
        "found external CSS link -- must be self-contained"
    );
    assert!(
        !html.contains("src=\"http"),
        "found external JS script -- must be self-contained"
    );
    assert!(
        !html.contains("href=\"//"),
        "found protocol-relative CSS link"
    );
    assert!(
        !html.contains("src=\"//"),
        "found protocol-relative JS script"
    );

    // Must contain inlined CSS
    assert!(html.contains("<style>"), "missing inlined CSS");

    // Must contain inlined JS
    assert!(html.contains("<script>"), "missing inlined JS");

    // Must embed coupling data as JSON
    assert!(
        html.contains("auth-service"),
        "missing repo name auth-service in output"
    );
    assert!(
        html.contains("user-service"),
        "missing repo name user-service in output"
    );
    assert!(
        html.contains("api-gateway"),
        "missing repo name api-gateway in output"
    );

    // Must contain SVG element for the force-directed graph
    assert!(
        html.contains("<svg") || html.contains("createElementNS") || html.contains("canvas"),
        "missing graph rendering element (svg or canvas)"
    );

    // Must contain force simulation logic
    assert!(
        html.contains("force") || html.contains("simulation"),
        "missing force-directed simulation logic"
    );

    // Must contain tooltip/hover logic
    assert!(
        html.contains("tooltip") || html.contains("hover") || html.contains("mouseover"),
        "missing tooltip/hover interaction"
    );
}

#[test]
fn html_graph_edge_thickness_proportional_to_score() {
    let report = sample_report();
    let html = render_coupling_html(&report);

    // The JSON data must contain the combined scores so JS can use them for edge thickness
    assert!(html.contains("72.5"), "missing high coupling score 72.5");
    assert!(
        html.contains("38.0") || html.contains("38"),
        "missing lower coupling score 38"
    );
}

#[test]
fn html_graph_node_hover_shows_repo_info() {
    let report = sample_report();
    let html = render_coupling_html(&report);

    // Data must be embedded such that hover can display repo name and coupling pair count
    // The JSON data section should contain all repo names
    assert!(html.contains("auth-service"));
    assert!(html.contains("user-service"));
    assert!(html.contains("api-gateway"));
}

#[test]
fn html_renders_with_empty_pairs() {
    let report = CouplingReport {
        repos: vec![RepoInfo {
            name: "solo-repo".to_string(),
            path: PathBuf::from("/repos/solo"),
            commit_count: 50,
            author_count: 2,
        }],
        pairs: vec![],
        summary: CouplingReportSummary {
            total_repos: 1,
            total_pairs_analyzed: 0,
            pairs_above_threshold: 0,
            highest_coupling_score: 0.0,
        },
        blast_radius: vec![],
    };

    let html = render_coupling_html(&report);
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("solo-repo"));
}
