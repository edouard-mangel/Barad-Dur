use crate::snapshot::RepoSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A detected team coupling between two repositories based on shared authors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamCouplingPair {
    pub repo_a: String,
    pub repo_b: String,
    /// (shared_authors / total_unique_authors) * 100
    pub team_score: f64,
    /// Lowercased names of authors contributing to both repos.
    pub shared_authors: Vec<String>,
    /// Number of shared authors.
    pub shared_count: usize,
    /// True when exactly one author bridges the two repos.
    pub is_single_bridge: bool,
    /// The bridge author's name (lowercased) when `is_single_bridge` is true.
    pub bridge_author: Option<String>,
}

/// Extract unique author names (lowercased) from a snapshot.
fn extract_normalized_authors(snapshot: &RepoSnapshot) -> HashSet<String> {
    snapshot
        .authors
        .iter()
        .map(|author| author.name.to_lowercase())
        .collect()
}

/// Compute the team score: (shared / total_unique) * 100.
fn compute_team_score(shared_count: usize, total_unique: usize) -> f64 {
    if total_unique == 0 {
        return 0.0;
    }
    (shared_count as f64 / total_unique as f64) * 100.0
}

/// Detect bridge author when exactly one author is shared.
fn detect_bridge(shared_authors: &[String]) -> (bool, Option<String>) {
    if shared_authors.len() == 1 {
        (true, Some(shared_authors[0].clone()))
    } else {
        (false, None)
    }
}

/// Analyze a single pair of snapshots for team coupling.
fn analyze_pair(
    name_a: &str,
    snapshot_a: &RepoSnapshot,
    name_b: &str,
    snapshot_b: &RepoSnapshot,
) -> TeamCouplingPair {
    let authors_a = extract_normalized_authors(snapshot_a);
    let authors_b = extract_normalized_authors(snapshot_b);

    let shared: Vec<String> = authors_a.intersection(&authors_b).cloned().collect();

    let total_unique = authors_a.union(&authors_b).count();
    let shared_count = shared.len();
    let team_score = compute_team_score(shared_count, total_unique);
    let (is_single_bridge, bridge_author) = detect_bridge(&shared);

    TeamCouplingPair {
        repo_a: name_a.to_string(),
        repo_b: name_b.to_string(),
        team_score,
        shared_authors: shared,
        shared_count,
        is_single_bridge,
        bridge_author,
    }
}

/// Analyze team coupling across all pairs of repository snapshots.
///
/// For each pair (A, B), computes the ratio of shared authors (by lowercase
/// display name) to total unique authors across both repos, expressed as a
/// percentage. Returns all pairs sorted by team_score descending.
pub fn analyze_team_coupling(snapshots: &[(String, RepoSnapshot)]) -> Vec<TeamCouplingPair> {
    let mut pairs: Vec<TeamCouplingPair> = Vec::new();

    for i in 0..snapshots.len() {
        for j in (i + 1)..snapshots.len() {
            let (name_a, snap_a) = &snapshots[i];
            let (name_b, snap_b) = &snapshots[j];
            pairs.push(analyze_pair(name_a, snap_a, name_b, snap_b));
        }
    }

    pairs.sort_by(|a, b| {
        b.team_score
            .partial_cmp(&a.team_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_team_score_formula() {
        // 2 shared out of 4 unique => 50.0
        assert!((compute_team_score(2, 4) - 50.0).abs() < 0.01);
    }

    #[test]
    fn compute_team_score_zero_unique_returns_zero() {
        assert!((compute_team_score(0, 0)).abs() < 0.01);
    }

    #[test]
    fn detect_bridge_single_author() {
        let shared = vec!["alice".to_string()];
        let (is_bridge, author) = detect_bridge(&shared);
        assert!(is_bridge);
        assert_eq!(author, Some("alice".to_string()));
    }

    #[test]
    fn detect_bridge_multiple_authors() {
        let shared = vec!["alice".to_string(), "bob".to_string()];
        let (is_bridge, author) = detect_bridge(&shared);
        assert!(!is_bridge);
        assert!(author.is_none());
    }

    #[test]
    fn detect_bridge_empty() {
        let shared: Vec<String> = vec![];
        let (is_bridge, author) = detect_bridge(&shared);
        assert!(!is_bridge);
        assert!(author.is_none());
    }

    #[test]
    fn extract_normalized_authors_lowercases_names() {
        use crate::snapshot::{Author, TimeWindow};
        use std::path::PathBuf;

        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".to_string(),
            "main".to_string(),
            TimeWindow::full_history(),
        );
        snapshot.authors = vec![
            Author {
                id: 0,
                name: "Alice Smith".to_string(),
                email: "a@x.com".to_string(),
            },
            Author {
                id: 1,
                name: "BOB JONES".to_string(),
                email: "b@x.com".to_string(),
            },
        ];

        let normalized = extract_normalized_authors(&snapshot);
        assert!(normalized.contains("alice smith"));
        assert!(normalized.contains("bob jones"));
        assert_eq!(normalized.len(), 2);
    }
}
