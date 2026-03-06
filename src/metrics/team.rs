use crate::metrics::CategoryResult;
use crate::snapshot::RepoSnapshot;

pub fn compute_team(_snapshot: &RepoSnapshot) -> CategoryResult {
    CategoryResult {
        name: "Team".to_string(),
        score: 0,
        metrics: Vec::new(),
    }
}
