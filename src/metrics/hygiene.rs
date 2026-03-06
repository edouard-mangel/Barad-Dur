use crate::metrics::CategoryResult;
use crate::snapshot::RepoSnapshot;

pub fn compute_hygiene(_snapshot: &RepoSnapshot) -> CategoryResult {
    CategoryResult {
        name: "Git Hygiene".to_string(),
        score: 0,
        metrics: Vec::new(),
    }
}
