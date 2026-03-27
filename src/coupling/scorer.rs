use std::collections::HashMap;

use crate::coupling::dependency::DependencyAnalysis;
use crate::coupling::team::TeamCouplingPair;
use crate::coupling::temporal::TemporalCouplingPair;
use crate::coupling::types::{
    CouplingDetails, CouplingPair, DependencyDetails, TeamDetails, TemporalDetails,
};

/// Weights for combining the three coupling dimensions.
const TEMPORAL_WEIGHT: f64 = 0.50;
const TEAM_WEIGHT: f64 = 0.25;
const DEPENDENCY_WEIGHT: f64 = 0.25;

/// Canonical key for a repo pair (alphabetically sorted).
fn pair_key(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

/// Compute the combined weighted score from three dimension scores.
fn compute_combined_score(temporal: f64, team: f64, dependency: f64) -> f64 {
    temporal * TEMPORAL_WEIGHT + team * TEAM_WEIGHT + dependency * DEPENDENCY_WEIGHT
}

/// Build default temporal details.
fn default_temporal_details() -> TemporalDetails {
    TemporalDetails {
        co_commit_count: 0,
        total_windows: 0,
    }
}

/// Build default team details.
fn default_team_details() -> TeamDetails {
    TeamDetails {
        shared_authors: 0,
        total_authors: 0,
    }
}

/// Build default dependency details.
fn default_dependency_details() -> DependencyDetails {
    DependencyDetails {
        shared_dependencies: 0,
        relationship: String::new(),
    }
}

/// Merge results from all three coupling dimensions into combined `CouplingPair` values.
///
/// Each dimension may cover different sets of repo pairs. Pairs are merged
/// by canonical key (repo_a, repo_b sorted alphabetically). Missing dimension
/// scores default to 0.0. Results are sorted by combined_score descending.
pub fn score_coupling_pairs(
    temporal: &[TemporalCouplingPair],
    team: &[TeamCouplingPair],
    dependency: &DependencyAnalysis,
) -> Vec<CouplingPair> {
    let mut temporal_map: HashMap<(String, String), &TemporalCouplingPair> =
        HashMap::with_capacity(temporal.len());
    for pair in temporal {
        let key = pair_key(&pair.repo_a, &pair.repo_b);
        temporal_map.insert(key, pair);
    }

    let mut team_map: HashMap<(String, String), &TeamCouplingPair> =
        HashMap::with_capacity(team.len());
    for pair in team {
        let key = pair_key(&pair.repo_a, &pair.repo_b);
        team_map.insert(key, pair);
    }

    let mut dep_map: HashMap<
        (String, String),
        &crate::coupling::dependency::DependencyCouplingPair,
    > = HashMap::with_capacity(dependency.pairs.len());
    for pair in &dependency.pairs {
        let key = pair_key(&pair.repo_a, &pair.repo_b);
        dep_map.insert(key, pair);
    }

    // Collect all unique pair keys
    let total_estimate = temporal_map.len() + team_map.len() + dep_map.len();
    let mut all_keys: Vec<(String, String)> = Vec::with_capacity(total_estimate);
    let mut seen: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::with_capacity(total_estimate);

    for key in temporal_map.keys() {
        if seen.insert(key.clone()) {
            all_keys.push(key.clone());
        }
    }
    for key in team_map.keys() {
        if seen.insert(key.clone()) {
            all_keys.push(key.clone());
        }
    }
    for key in dep_map.keys() {
        if seen.insert(key.clone()) {
            all_keys.push(key.clone());
        }
    }

    let mut pairs: Vec<CouplingPair> = all_keys
        .into_iter()
        .map(|(repo_a, repo_b)| {
            let key = (repo_a.clone(), repo_b.clone());

            let temporal_score = temporal_map
                .get(&key)
                .map(|p| p.temporal_score)
                .unwrap_or(0.0);
            let team_score = team_map.get(&key).map(|p| p.team_score).unwrap_or(0.0);
            let dep_score = dep_map.get(&key).map(|p| p.dep_score).unwrap_or(0.0);

            let combined_score = compute_combined_score(temporal_score, team_score, dep_score);

            let temporal_details = temporal_map
                .get(&key)
                .map(|p| TemporalDetails {
                    co_commit_count: p.co_changes,
                    total_windows: 0,
                })
                .unwrap_or_else(default_temporal_details);

            let team_details = team_map
                .get(&key)
                .map(|p| TeamDetails {
                    shared_authors: p.shared_count,
                    total_authors: p.shared_count, // best available
                })
                .unwrap_or_else(default_team_details);

            let dep_details = dep_map
                .get(&key)
                .map(|p| {
                    let relationship = p
                        .direct_dependency
                        .as_ref()
                        .map(|d| format!("{} -> {}", d.from, d.to))
                        .unwrap_or_default();
                    DependencyDetails {
                        shared_dependencies: p.shared_count,
                        relationship,
                    }
                })
                .unwrap_or_else(default_dependency_details);

            CouplingPair {
                repo_a,
                repo_b,
                temporal_score,
                team_score,
                dependency_score: dep_score,
                combined_score,
                details: CouplingDetails {
                    temporal: temporal_details,
                    team: team_details,
                    dependency: dep_details,
                },
            }
        })
        .collect();

    pairs.sort_by(|a, b| {
        b.combined_score
            .partial_cmp(&a.combined_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    pairs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coupling::dependency::DependencyCouplingPair;
    use crate::coupling::temporal::Confidence;

    fn make_temporal(repo_a: &str, repo_b: &str, score: f64) -> TemporalCouplingPair {
        TemporalCouplingPair {
            repo_a: repo_a.to_string(),
            repo_b: repo_b.to_string(),
            co_changes: 10,
            temporal_score: score,
            confidence: Confidence::Medium,
        }
    }

    fn make_team(repo_a: &str, repo_b: &str, score: f64) -> TeamCouplingPair {
        TeamCouplingPair {
            repo_a: repo_a.to_string(),
            repo_b: repo_b.to_string(),
            team_score: score,
            shared_authors: vec![],
            shared_count: 0,
            is_single_bridge: false,
            bridge_author: None,
        }
    }

    fn make_dep(repo_a: &str, repo_b: &str, score: f64) -> DependencyCouplingPair {
        DependencyCouplingPair {
            repo_a: repo_a.to_string(),
            repo_b: repo_b.to_string(),
            shared_deps: vec![],
            shared_count: 0,
            dep_score: score,
            direct_dependency: None,
        }
    }

    fn empty_dep_analysis() -> DependencyAnalysis {
        DependencyAnalysis {
            pairs: vec![],
            blast_radius: vec![],
        }
    }

    #[test]
    fn combined_score_formula_applies_weights() {
        let temporal = vec![make_temporal("a", "b", 80.0)];
        let team = vec![make_team("a", "b", 60.0)];
        let dep = DependencyAnalysis {
            pairs: vec![make_dep("a", "b", 40.0)],
            blast_radius: vec![],
        };

        let pairs = score_coupling_pairs(&temporal, &team, &dep);

        assert_eq!(pairs.len(), 1);
        let expected = 80.0 * 0.50 + 60.0 * 0.25 + 40.0 * 0.25;
        assert!(
            (pairs[0].combined_score - expected).abs() < 0.01,
            "expected {}, got {}",
            expected,
            pairs[0].combined_score
        );
    }

    #[test]
    fn missing_dimensions_default_to_zero() {
        // Only temporal present
        let temporal = vec![make_temporal("a", "b", 100.0)];
        let pairs = score_coupling_pairs(&temporal, &[], &empty_dep_analysis());

        assert_eq!(pairs.len(), 1);
        let expected = 100.0 * 0.50;
        assert!(
            (pairs[0].combined_score - expected).abs() < 0.01,
            "expected {}, got {}",
            expected,
            pairs[0].combined_score
        );
        assert!((pairs[0].team_score - 0.0).abs() < 0.01);
        assert!((pairs[0].dependency_score - 0.0).abs() < 0.01);
    }

    #[test]
    fn pairs_sorted_by_combined_score_descending() {
        let temporal = vec![make_temporal("a", "b", 40.0), make_temporal("c", "d", 80.0)];
        let pairs = score_coupling_pairs(&temporal, &[], &empty_dep_analysis());

        assert_eq!(pairs.len(), 2);
        assert!(pairs[0].combined_score >= pairs[1].combined_score);
        assert_eq!(pairs[0].repo_a, "c");
    }

    #[test]
    fn merges_different_pair_sets_across_dimensions() {
        // temporal has (a,b), team has (c,d)
        let temporal = vec![make_temporal("a", "b", 50.0)];
        let team = vec![make_team("c", "d", 70.0)];
        let pairs = score_coupling_pairs(&temporal, &team, &empty_dep_analysis());

        assert_eq!(pairs.len(), 2);
        // (c,d) has higher combined: 0 + 70*0.25 + 0 = 17.5
        // (a,b) has: 50*0.50 + 0 + 0 = 25.0
        let ab = pairs.iter().find(|p| p.repo_a == "a").unwrap();
        let cd = pairs.iter().find(|p| p.repo_a == "c").unwrap();
        assert!((ab.combined_score - 25.0).abs() < 0.01);
        assert!((cd.combined_score - 17.5).abs() < 0.01);
    }

    #[test]
    fn canonical_key_normalizes_order() {
        // temporal has (b,a), team has (a,b) -- should merge
        let temporal = vec![make_temporal("b", "a", 60.0)];
        let team = vec![make_team("a", "b", 40.0)];
        let pairs = score_coupling_pairs(&temporal, &team, &empty_dep_analysis());

        assert_eq!(pairs.len(), 1, "should merge into single pair");
        assert_eq!(pairs[0].repo_a, "a");
        assert_eq!(pairs[0].repo_b, "b");
    }

    #[test]
    fn compute_combined_score_pure_function() {
        assert!((compute_combined_score(100.0, 100.0, 100.0) - 100.0).abs() < 0.01);
        assert!((compute_combined_score(0.0, 0.0, 0.0) - 0.0).abs() < 0.01);
        assert!((compute_combined_score(50.0, 50.0, 50.0) - 50.0).abs() < 0.01);
    }
}
