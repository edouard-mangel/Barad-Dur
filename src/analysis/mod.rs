//! Pure analysis orchestration shared by `analyze`, `gate`, and `backfill`.
//!
//! Commands own I/O, collection, and policy: which categories to select,
//! which weights apply, whether dependency evidence was acquired. This layer
//! turns those explicit inputs into ordered categories and a weighted
//! summary. It performs no I/O and reads neither the clock nor CLI
//! arguments, so equivalent inputs yield equivalent results whichever
//! command supplied them.
mod result;
mod selection;

pub use result::{compute_overall_score_with_weights, AnalysisResult};
pub use selection::CategorySelection;

use std::path::PathBuf;

use chrono::{DateTime, Utc};

use crate::config::Thresholds;
use crate::deps::EcosystemReport;
use crate::metrics::coupling::{CouplingEvidence, CouplingReach};
use crate::metrics::{coupling, deps, evolution, health, hygiene, team, CategoryResult};
use crate::snapshot::RepoSnapshot;

pub struct AnalysisInputs<'a> {
    /// Captured once at the application boundary; drives every
    /// age-dependent calculation.
    pub reference_time: DateTime<Utc>,
    pub snapshot: &'a RepoSnapshot,
    pub selection: CategorySelection,
    pub thresholds: &'a Thresholds,
    /// Effective weights, including any command-level adjustment such as
    /// `analyze`'s dependency opt-in.
    pub weights: &'a [(&'a str, f64)],
    /// Dependency evidence the command already acquired; empty when it was
    /// not requested or not available. Never fetched here.
    pub dependency_evidence: &'a [EcosystemReport],
    /// Derivations the command computes once and shares with the report
    /// (`health::god_object_files`, `coupling::growing_coupling_reach`).
    pub god_objects: &'a [(PathBuf, String)],
    pub coupling_reach: &'a CouplingReach,
}

/// The selected categories in canonical order — Health, Team, Evolution,
/// Git Hygiene, Coupling, then Dependencies — and their weighted summary.
/// An unselected category costs nothing; Dependencies join only when
/// selected *and* evidence is present, so an opted-in run without a
/// lockfile stays a five-category report.
pub fn calculate(inputs: &AnalysisInputs) -> AnalysisResult {
    let selection = inputs.selection;
    let thresholds = inputs.thresholds;
    let snapshot = inputs.snapshot;
    // Derived whether or not Coupling is selected: the report's finding
    // counts, hotspot badges, and actions consume it for every command.
    let coupling_evidence = CouplingEvidence::derive(snapshot, &thresholds.coupling);
    let categories: Vec<CategoryResult> = [
        selection
            .health
            .then(|| health::compute_health(snapshot, &thresholds.health, inputs.god_objects)),
        selection
            .team
            .then(|| team::compute_team(snapshot, &thresholds.team, &thresholds.coupling)),
        selection.evolution.then(|| {
            evolution::compute_evolution(inputs.reference_time, snapshot, &thresholds.evolution)
        }),
        selection
            .hygiene
            .then(|| hygiene::compute_hygiene(snapshot, &thresholds.hygiene)),
        selection.coupling.then(|| {
            coupling::compute_coupling_with_evidence(
                snapshot,
                &thresholds.coupling,
                inputs.coupling_reach,
                &coupling_evidence,
            )
        }),
        (selection.deps && !inputs.dependency_evidence.is_empty())
            .then(|| deps::compute_deps(inputs.dependency_evidence)),
    ]
    .into_iter()
    .flatten()
    .collect();
    let overall_score = compute_overall_score_with_weights(&categories, inputs.weights);
    AnalysisResult {
        categories,
        overall_score,
        coupling_evidence,
    }
}
