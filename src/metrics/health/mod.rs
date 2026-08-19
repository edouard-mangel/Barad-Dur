mod biomarkers;
mod bus_factor;
mod churn_ownership;
mod complex_hotspots;
mod god_objects;
pub use god_objects::god_object_files;
mod long_methods;

use crate::config::HealthThresholds;
use crate::metrics::CategoryResult;
use crate::snapshot::RepoSnapshot;

/// `flagged_god_objects` — the caller's own `god_object_files(snapshot,
/// thresholds)` result — is threaded in rather than recomputed here, since
/// callers that also build a report need the same data again for
/// refactoring-action generation; computing it once and sharing it avoids
/// running the O(files) god-object detection pass twice per analysis.
pub fn compute_health(
    snapshot: &RepoSnapshot,
    thresholds: &HealthThresholds,
    flagged_god_objects: &[(std::path::PathBuf, String)],
) -> CategoryResult {
    let metrics = vec![
        bus_factor::bus_factor(snapshot, thresholds),
        god_objects::god_objects(snapshot, flagged_god_objects),
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
