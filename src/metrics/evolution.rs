use crate::metrics::CategoryResult;
use crate::snapshot::RepoSnapshot;

pub fn compute_evolution(_snapshot: &RepoSnapshot) -> CategoryResult {
    CategoryResult {
        name: "Evolution".to_string(),
        score: 0,
        metrics: Vec::new(),
    }
}
