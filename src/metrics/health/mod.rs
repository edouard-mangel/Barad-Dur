mod bus_factor;
mod complex_hotspots;
mod god_objects;

use crate::config::HealthThresholds;
use crate::metrics::CategoryResult;
use crate::snapshot::RepoSnapshot;

pub fn compute_health(snapshot: &RepoSnapshot, thresholds: &HealthThresholds) -> CategoryResult {
    let metrics = vec![
        bus_factor::bus_factor(snapshot, thresholds),
        god_objects::god_objects(snapshot),
        complex_hotspots::complex_hotspots(snapshot),
    ];

    CategoryResult {
        name: "Health".to_string(),
        score: 0,
        metrics,
    }
    .compute_score()
}
