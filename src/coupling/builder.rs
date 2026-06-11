use crate::coupling::dependency::BlastRadiusEntry;
use crate::coupling::types::{CouplingPair, CouplingReport, CouplingReportSummary, RepoInfo};

/// Assemble a `CouplingReport` from analysis outputs.
///
/// Single shaping point for every coupling renderer (CLI/JSON/HTML):
/// pairs are sorted by combined score descending and the summary is derived
/// here, so renderers consume the report without re-deriving anything.
pub fn build_coupling_report(
    repos: Vec<RepoInfo>,
    pairs: Vec<CouplingPair>,
    blast_radius: Vec<BlastRadiusEntry>,
    min_score: f64,
) -> CouplingReport {
    let mut pairs = pairs;
    pairs.sort_by(|a, b| {
        b.combined_score
            .partial_cmp(&a.combined_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let highest = pairs
        .iter()
        .map(|p| p.combined_score)
        .fold(0.0_f64, f64::max);

    let pairs_above = pairs
        .iter()
        .filter(|p| p.combined_score >= min_score)
        .count();

    let summary = CouplingReportSummary {
        total_repos: repos.len(),
        total_pairs_analyzed: repos.len() * repos.len().saturating_sub(1) / 2,
        pairs_above_threshold: pairs_above,
        highest_coupling_score: highest,
    };

    CouplingReport {
        repos,
        pairs,
        summary,
        blast_radius,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coupling::types::{
        CouplingDetails, DependencyDetails, TeamDetails, TemporalDetails,
    };
    use std::path::PathBuf;

    fn make_repo(name: &str) -> RepoInfo {
        RepoInfo {
            name: name.to_string(),
            path: PathBuf::from(format!("/tmp/{name}")),
            commit_count: 10,
            author_count: 2,
        }
    }

    fn make_pair(a: &str, b: &str, combined: f64) -> CouplingPair {
        CouplingPair {
            repo_a: a.to_string(),
            repo_b: b.to_string(),
            temporal_score: combined,
            team_score: combined,
            dependency_score: combined,
            combined_score: combined,
            details: CouplingDetails {
                temporal: TemporalDetails {
                    co_commit_count: 1,
                    total_windows: 2,
                },
                team: TeamDetails {
                    shared_authors: 1,
                    total_authors: 2,
                },
                dependency: DependencyDetails {
                    shared_dependencies: 0,
                    relationship: String::new(),
                },
            },
        }
    }

    #[test]
    fn pairs_are_sorted_descending_even_from_unsorted_input() {
        let report = build_coupling_report(
            vec![make_repo("a"), make_repo("b"), make_repo("c")],
            vec![
                make_pair("a", "b", 20.0),
                make_pair("b", "c", 90.0),
                make_pair("a", "c", 55.0),
            ],
            vec![],
            0.0,
        );
        let scores: Vec<f64> = report.pairs.iter().map(|p| p.combined_score).collect();
        assert_eq!(scores, vec![90.0, 55.0, 20.0]);
    }

    #[test]
    fn highest_score_is_the_max_not_the_first_input() {
        let report = build_coupling_report(
            vec![make_repo("a"), make_repo("b"), make_repo("c")],
            vec![make_pair("a", "b", 10.0), make_pair("b", "c", 80.0)],
            vec![],
            0.0,
        );
        assert_eq!(report.summary.highest_coupling_score, 80.0);
    }

    #[test]
    fn summary_counts_pairs_above_threshold_inclusively() {
        let report = build_coupling_report(
            vec![make_repo("a"), make_repo("b"), make_repo("c")],
            vec![
                make_pair("a", "b", 50.0),
                make_pair("b", "c", 49.9),
                make_pair("a", "c", 70.0),
            ],
            vec![],
            50.0,
        );
        assert_eq!(report.summary.pairs_above_threshold, 2);
    }

    #[test]
    fn summary_derives_repo_and_pair_universe_counts() {
        let report = build_coupling_report(
            vec![
                make_repo("a"),
                make_repo("b"),
                make_repo("c"),
                make_repo("d"),
            ],
            vec![],
            vec![],
            0.0,
        );
        assert_eq!(report.summary.total_repos, 4);
        assert_eq!(report.summary.total_pairs_analyzed, 6);
        assert_eq!(report.summary.highest_coupling_score, 0.0);
    }
}
