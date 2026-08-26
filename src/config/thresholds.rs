use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct HealthThresholds {
    #[serde(default = "default_max_complexity")]
    pub max_complexity: u32,
    #[serde(default = "default_hotspot_top_n")]
    pub hotspot_top_n: usize,
    #[serde(default = "default_coupling_min_commits")]
    pub coupling_min_commits: usize,
    #[serde(default = "default_long_method_loc")]
    pub long_method_loc: usize,
    #[serde(default = "default_long_method_cc")]
    pub long_method_cc: u32,
    #[serde(default = "default_biomarker_max_depth")]
    pub biomarker_max_depth: u32,
    #[serde(default = "default_biomarker_max_variance")]
    pub biomarker_max_variance: f64,
    /// A source file's import-graph degree (incoming + outgoing) must exceed
    /// the repo's median degree by this multiple to be flagged a structural
    /// hub, alongside `god_node_min_degree`. Default 4.0.
    #[serde(default = "default_god_node_degree_multiplier")]
    pub god_node_degree_multiplier: f64,
    /// Absolute floor on import-graph degree before a file can be flagged a
    /// structural hub — keeps small repos, where even a modest degree is
    /// "4x the median", from producing spurious flags. Default 8.
    #[serde(default = "default_god_node_min_degree")]
    pub god_node_min_degree: usize,
    /// Trust floor for call-graph analysis: when the snapshot-wide call
    /// resolution rate (resolved + same-file over all edges) falls below
    /// this fraction, function-hub output is suppressed rather than built
    /// on mostly-unresolved data. Must be in [0.0, 1.0]. Default 0.5.
    #[serde(default = "default_call_resolution_floor")]
    pub call_resolution_floor: f64,
}

fn default_max_complexity() -> u32 {
    20
}
fn default_hotspot_top_n() -> usize {
    10
}
fn default_coupling_min_commits() -> usize {
    5
}
fn default_long_method_loc() -> usize {
    40
}
fn default_long_method_cc() -> u32 {
    10
}
fn default_biomarker_max_depth() -> u32 {
    4
}
fn default_biomarker_max_variance() -> f64 {
    2.0
}
fn default_god_node_degree_multiplier() -> f64 {
    4.0
}
fn default_god_node_min_degree() -> usize {
    8
}
fn default_call_resolution_floor() -> f64 {
    0.5
}

impl Default for HealthThresholds {
    fn default() -> Self {
        Self {
            max_complexity: default_max_complexity(),
            hotspot_top_n: default_hotspot_top_n(),
            coupling_min_commits: default_coupling_min_commits(),
            long_method_loc: default_long_method_loc(),
            long_method_cc: default_long_method_cc(),
            biomarker_max_depth: default_biomarker_max_depth(),
            biomarker_max_variance: default_biomarker_max_variance(),
            god_node_degree_multiplier: default_god_node_degree_multiplier(),
            god_node_min_degree: default_god_node_min_degree(),
            call_resolution_floor: default_call_resolution_floor(),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
pub struct TeamThresholds {
    #[serde(default = "default_silo_max_owners")]
    pub silo_max_owners: usize,
    #[serde(default = "default_activity_window_days")]
    pub activity_window_days: u32,
}

fn default_silo_max_owners() -> usize {
    1
}
fn default_activity_window_days() -> u32 {
    30
}

impl Default for TeamThresholds {
    fn default() -> Self {
        Self {
            silo_max_owners: default_silo_max_owners(),
            activity_window_days: default_activity_window_days(),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
pub struct EvolutionThresholds {
    #[serde(default = "default_growth_baseline_months")]
    pub growth_baseline_months: u32,
    #[serde(default = "default_refactor_ratio_target")]
    pub refactor_ratio_target: f64,
}

fn default_growth_baseline_months() -> u32 {
    3
}
fn default_refactor_ratio_target() -> f64 {
    0.1
}

impl Default for EvolutionThresholds {
    fn default() -> Self {
        Self {
            growth_baseline_months: default_growth_baseline_months(),
            refactor_ratio_target: default_refactor_ratio_target(),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
pub struct HygieneThresholds {
    #[serde(default = "default_min_message_length")]
    pub min_message_length: usize,
    #[serde(default = "default_max_message_length")]
    pub max_message_length: usize,
}

fn default_min_message_length() -> usize {
    10
}
fn default_max_message_length() -> usize {
    72
}

impl Default for HygieneThresholds {
    fn default() -> Self {
        Self {
            min_message_length: default_min_message_length(),
            max_message_length: default_max_message_length(),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
pub struct CouplingThresholds {
    #[serde(default = "default_component_depth")]
    pub component_depth: usize,
    #[serde(default = "default_change_coupling_min_ratio")]
    pub change_coupling_min_ratio: f64,
    /// Enable the TS/JS barrel-bypass content-coupling rule. Teams whose
    /// culture prefers deep imports can turn it off.
    #[serde(default = "default_content_barrel_rule")]
    pub content_barrel_rule: bool,
    /// Hotspot-score multiplier applied when a file carries Content or
    /// Common coupling findings — severity × change frequency = risk.
    /// Control findings never multiply (least severe rung).
    #[serde(default = "default_hotspot_multiplier")]
    pub hotspot_multiplier: f64,
    /// How much a corroborated finding (its file co-changes cross-boundary)
    /// weighs versus a dormant one when scoring a Pressman metric. 1.0 = no
    /// nudge (reproduces pre-M5 scores); 2.0 = a corroborated finding counts
    /// double toward the severity band. Values < 1.0 (and NaN) are rejected
    /// by `validate()` — corroboration may only raise severity, never lower it.
    #[serde(default = "default_corroboration_weight")]
    pub corroboration_weight: f64,
    /// Minimum project-local inheritance depth (DIT) for a class to be
    /// flagged as inheritance coupling. 0 disables the rule; 1 is rejected
    /// by `validate()` (it would flag every cross-file `extends` — ordinary
    /// OO, not the deep-chain hazard this rung targets). Default 2.
    #[serde(default = "default_inheritance_min_depth")]
    pub inheritance_min_depth: usize,
    /// Use Louvain communities of the import graph to *refute* change-coupling
    /// smells: a pair whose two files sit in the same community is dropped
    /// from the score, while pairs in different communities — and pairs where
    /// either file is absent from the graph — are kept. Never changes the
    /// reported smell count, but it does change the score, and disabling it
    /// can only lower that score (refuted pairs come back in). With no import
    /// data at all nothing can be refuted, so the switch has no effect.
    /// Teams that find the annotation noisy can turn it off.
    #[serde(default = "default_community_corroboration")]
    pub community_corroboration: bool,
    /// Minimum distinct co-change partners a file must reach in the second
    /// half of the window before half-over-half growth counts as growing
    /// reach (trends M3). Keeps small absolute jumps (1 → 2) from flagging.
    /// Must be >= 1 (`validate()`). Default 8.
    #[serde(default = "default_decay_min_partners")]
    pub decay_min_partners: usize,
    /// Expected-tight source↔test co-change floor; the minimum ratio of
    /// test files co-changed within the source pair's coupling window.
    /// Distinct from `change_coupling_min_ratio` — the two measure different
    /// relationships (expected-tight test safety net vs arbitrary cross-component smell).
    /// Must be in [0.0, 1.0]. Default 0.30.
    #[serde(default = "default_test_safety_net_min_ratio")]
    pub test_safety_net_min_ratio: f64,
}

fn default_component_depth() -> usize {
    2
}
fn default_change_coupling_min_ratio() -> f64 {
    0.30
}
fn default_content_barrel_rule() -> bool {
    true
}
fn default_hotspot_multiplier() -> f64 {
    1.25
}
fn default_corroboration_weight() -> f64 {
    2.0
}
fn default_inheritance_min_depth() -> usize {
    2
}
fn default_decay_min_partners() -> usize {
    8
}
fn default_test_safety_net_min_ratio() -> f64 {
    0.30
}
fn default_community_corroboration() -> bool {
    true
}

impl Default for CouplingThresholds {
    fn default() -> Self {
        Self {
            component_depth: default_component_depth(),
            change_coupling_min_ratio: default_change_coupling_min_ratio(),
            content_barrel_rule: default_content_barrel_rule(),
            hotspot_multiplier: default_hotspot_multiplier(),
            corroboration_weight: default_corroboration_weight(),
            inheritance_min_depth: default_inheritance_min_depth(),
            community_corroboration: default_community_corroboration(),
            decay_min_partners: default_decay_min_partners(),
            test_safety_net_min_ratio: default_test_safety_net_min_ratio(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Thresholds {
    #[serde(default)]
    pub health: HealthThresholds,
    #[serde(default)]
    pub team: TeamThresholds,
    #[serde(default)]
    pub evolution: EvolutionThresholds,
    #[serde(default)]
    pub hygiene: HygieneThresholds,
    #[serde(default)]
    pub coupling: CouplingThresholds,
}
