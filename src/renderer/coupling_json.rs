use crate::coupling::types::CouplingReport;

/// Render a `CouplingReport` as a JSON string.
///
/// The output has a top-level `"coupling"` key containing:
/// - `schema_version`: always 1
/// - `repos_scanned`: number of repos analyzed
/// - `pairs_analyzed`: number of coupling pairs
/// - `pairs`: array of coupling pair objects
/// - `blast_radius`: array of hub dependency objects
///
/// When `pretty` is true, the output is indented for human readability.
pub fn render_coupling_json(report: &CouplingReport, pretty: bool) -> String {
    let pairs_json: Vec<serde_json::Value> = report
        .pairs
        .iter()
        .map(|pair| {
            serde_json::json!({
                "repo_a": pair.repo_a,
                "repo_b": pair.repo_b,
                "temporal_score": pair.temporal_score,
                "team_score": pair.team_score,
                "dependency_score": pair.dependency_score,
                "combined_score": pair.combined_score,
            })
        })
        .collect();

    let output = serde_json::json!({
        "coupling": {
            "schema_version": 1,
            "repos_scanned": report.summary.total_repos,
            "pairs_analyzed": report.summary.total_pairs_analyzed,
            "pairs": pairs_json,
            "blast_radius": report.blast_radius,
        }
    });

    if pretty {
        serde_json::to_string_pretty(&output).unwrap_or_default()
    } else {
        serde_json::to_string(&output).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coupling::dependency::BlastRadiusEntry;
    use crate::coupling::types::{
        CouplingDetails, CouplingPair, CouplingReportSummary, DependencyDetails, RepoInfo,
        TeamDetails, TemporalDetails,
    };
    use std::path::PathBuf;

    fn make_test_report() -> CouplingReport {
        CouplingReport {
            repos: vec![
                RepoInfo {
                    name: "alpha".to_string(),
                    path: PathBuf::from("/tmp/alpha"),
                    commit_count: 50,
                    author_count: 3,
                },
                RepoInfo {
                    name: "beta".to_string(),
                    path: PathBuf::from("/tmp/beta"),
                    commit_count: 30,
                    author_count: 2,
                },
            ],
            pairs: vec![CouplingPair {
                repo_a: "alpha".to_string(),
                repo_b: "beta".to_string(),
                temporal_score: 75.0,
                team_score: 50.0,
                dependency_score: 30.0,
                combined_score: 57.5,
                details: CouplingDetails {
                    temporal: TemporalDetails {
                        co_commit_count: 10,
                        total_windows: 20,
                    },
                    team: TeamDetails {
                        shared_authors: 2,
                        total_authors: 4,
                    },
                    dependency: DependencyDetails {
                        shared_dependencies: 3,
                        relationship: String::new(),
                    },
                },
            }],
            summary: CouplingReportSummary {
                total_repos: 2,
                total_pairs_analyzed: 1,
                pairs_above_threshold: 1,
                highest_coupling_score: 57.5,
            },
            blast_radius: vec![BlastRadiusEntry {
                dependency_name: "serde".to_string(),
                consumers: vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
                consumer_count: 3,
            }],
        }
    }

    #[test]
    fn json_output_has_coupling_envelope() {
        let report = make_test_report();
        let json = render_coupling_json(&report, false);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("coupling").is_some());
    }

    #[test]
    fn json_output_has_schema_version_1() {
        let report = make_test_report();
        let json = render_coupling_json(&report, false);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["coupling"]["schema_version"], 1);
    }

    #[test]
    fn json_output_has_repos_scanned_count() {
        let report = make_test_report();
        let json = render_coupling_json(&report, false);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["coupling"]["repos_scanned"], 2);
    }

    #[test]
    fn json_output_has_pairs_array() {
        let report = make_test_report();
        let json = render_coupling_json(&report, false);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let pairs = parsed["coupling"]["pairs"].as_array().unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0]["repo_a"], "alpha");
    }

    #[test]
    fn json_output_has_blast_radius_array() {
        let report = make_test_report();
        let json = render_coupling_json(&report, false);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let blast = parsed["coupling"]["blast_radius"].as_array().unwrap();
        assert_eq!(blast.len(), 1);
        assert_eq!(blast[0]["dependency_name"], "serde");
    }

    #[test]
    fn pretty_flag_produces_indented_output() {
        let report = make_test_report();
        let compact = render_coupling_json(&report, false);
        let pretty = render_coupling_json(&report, true);
        assert!(pretty.len() > compact.len());
        assert!(pretty.contains('\n'));
        assert!(pretty.contains("  "));
    }
}
