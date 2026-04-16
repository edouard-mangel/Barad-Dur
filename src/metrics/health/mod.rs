mod biomarkers;
mod bus_factor;
mod churn_ownership;
mod complex_hotspots;
mod god_objects;
mod long_methods;

use crate::config::HealthThresholds;
use crate::metrics::CategoryResult;
use crate::snapshot::RepoSnapshot;

pub fn compute_health(snapshot: &RepoSnapshot, thresholds: &HealthThresholds) -> CategoryResult {
    let metrics = vec![
        bus_factor::bus_factor(snapshot, thresholds),
        god_objects::god_objects(snapshot),
        complex_hotspots::complex_hotspots(snapshot),
        long_methods::long_methods(snapshot),
        biomarkers::biomarkers(snapshot),
        churn_ownership::churn_ownership_risk(snapshot),
    ];

    CategoryResult {
        name: "Health".to_string(),
        score: 0,
        metrics,
    }
    .compute_score()
}
