//! M02 milestone 1: one pure analysis calculation serves every command.
//!
//! `analysis::calculate` takes an explicit category selection, effective
//! thresholds and weights, already-acquired dependency evidence, and a
//! reference time — never CLI arguments — and returns the ordered categories
//! with their weighted summary. Which command asked is not an input.
use barad_dur::{
    analysis::{calculate, AnalysisInputs, CategorySelection},
    config::RepoConfig,
    deps::{Ecosystem, EcosystemReport},
    metrics::coupling::CouplingReach,
    scorer::compute_overall_score_with_weights,
    snapshot::{RepoSnapshot, TimeWindow},
};
use chrono::{TimeZone, Utc};

const FIVE: [&str; 5] = ["Health", "Team", "Evolution", "Git Hygiene", "Coupling"];

fn snapshot() -> RepoSnapshot {
    RepoSnapshot::new(
        "/tmp".into(),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    )
}

fn ecosystem_report() -> EcosystemReport {
    EcosystemReport {
        ecosystem: Ecosystem::Cargo,
        total_deps: 3,
        mean_drift_years: 0.2,
        total_drift_years: 0.6,
        critical_deps: vec![],
    }
}

/// Categories, by name, that `calculate` returns for `selection` given
/// `evidence`; everything else held at defaults.
fn names_for(selection: CategorySelection, evidence: &[EcosystemReport]) -> Vec<String> {
    let cfg = RepoConfig::default();
    let weights = cfg.weights.as_weight_pairs();
    let snapshot = snapshot();
    let reach = CouplingReach::default();
    let result = calculate(&AnalysisInputs {
        reference_time: Utc.with_ymd_and_hms(2026, 9, 12, 0, 0, 0).unwrap(),
        snapshot: &snapshot,
        selection,
        thresholds: &cfg.thresholds,
        weights: &weights,
        dependency_evidence: evidence,
        god_objects: &[],
        coupling_reach: &reach,
    });
    result.categories.into_iter().map(|c| c.name).collect()
}

#[test]
fn no_filter_selects_the_five_categories_in_canonical_order() {
    let selection = CategorySelection::from_filters(false, false, false, false, false);
    assert_eq!(names_for(selection, &[]), FIVE);
}

#[test]
fn filters_select_their_categories_in_canonical_order_and_never_coupling() {
    let selection = CategorySelection::from_filters(true, false, false, true, false);
    assert!(!selection.coupling, "no filter can select Coupling");
    assert_eq!(names_for(selection, &[]), ["Health", "Git Hygiene"]);
}

#[test]
fn gate_and_backfill_selections_are_fixed_sets() {
    assert_eq!(names_for(CategorySelection::GATE, &[]), FIVE);
    assert_eq!(
        names_for(CategorySelection::BACKFILL, &[]),
        ["Health", "Team", "Evolution", "Git Hygiene"]
    );
}

#[test]
fn dependencies_join_last_only_when_selected_and_evidence_is_present() {
    let with_deps = CategorySelection::from_filters(false, false, false, false, true);
    assert_eq!(
        names_for(with_deps, &[]),
        FIVE,
        "selected but unavailable evidence adds nothing"
    );
    assert_eq!(
        names_for(with_deps, &[ecosystem_report()]),
        [
            "Health",
            "Team",
            "Evolution",
            "Git Hygiene",
            "Coupling",
            "Dependencies"
        ]
    );
    let filtered = CategorySelection::from_filters(false, true, false, false, true);
    assert_eq!(
        names_for(filtered, &[ecosystem_report()]),
        ["Team", "Dependencies"]
    );
    let unselected = CategorySelection::from_filters(false, false, false, false, false);
    assert_eq!(
        names_for(unselected, &[ecosystem_report()]),
        FIVE,
        "evidence without selection is ignored"
    );
}

#[test]
fn overall_is_the_shared_weighted_policy_over_the_returned_categories() {
    let cfg = RepoConfig::default();
    let weights = cfg.weights.as_weight_pairs();
    let snapshot = snapshot();
    let reach = CouplingReach::default();
    let result = calculate(&AnalysisInputs {
        reference_time: Utc.with_ymd_and_hms(2026, 9, 12, 0, 0, 0).unwrap(),
        snapshot: &snapshot,
        selection: CategorySelection::GATE,
        thresholds: &cfg.thresholds,
        weights: &weights,
        dependency_evidence: &[],
        god_objects: &[],
        coupling_reach: &reach,
    });
    assert_eq!(
        result.overall_score,
        compute_overall_score_with_weights(&result.categories, &weights)
    );
}

#[test]
fn equivalent_inputs_yield_equivalent_results() {
    let cfg = RepoConfig::default();
    let weights = cfg.weights.as_weight_pairs();
    let snapshot = snapshot();
    let reach = CouplingReach::default();
    let inputs = AnalysisInputs {
        reference_time: Utc.with_ymd_and_hms(2026, 9, 12, 0, 0, 0).unwrap(),
        snapshot: &snapshot,
        selection: CategorySelection::GATE,
        thresholds: &cfg.thresholds,
        weights: &weights,
        dependency_evidence: &[],
        god_objects: &[],
        coupling_reach: &reach,
    };
    let (first, second) = (calculate(&inputs), calculate(&inputs));
    assert_eq!(
        serde_json::to_value(&first.categories).unwrap(),
        serde_json::to_value(&second.categories).unwrap()
    );
    assert_eq!(first.overall_score, second.overall_score);
}
