use serde::Serialize;

/// Call-graph summary for one analysis run (design D7). `None` on the
/// report means no call data — the AST pass did not run (ADR-005) or no
/// supported-language file produced call edges — never "zero calls".
/// `resolution_rate` counts same-file callees as resolved.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct CallGraphReport {
    pub resolution_rate: f64,
    pub edges_resolved: usize,
    pub edges_same_file: usize,
    pub edges_unresolved: usize,
    /// The trust floor `resolution_rate` was checked against — lets a
    /// reader tell "no hubs exist" apart from "hubs were suppressed below
    /// this threshold" instead of guessing from an empty list alone.
    pub call_resolution_floor: f64,
    /// Top functions by distinct-caller in-degree (barrel-chased), empty
    /// when the resolution rate sits below the configured trust floor.
    pub function_hubs: Vec<FunctionHub>,
}

/// One function-hub row: a call target and how many distinct functions
/// call it through resolved (or same-file) edges.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct FunctionHub {
    pub path: String,
    pub name: String,
    pub resolved_in_degree: usize,
}
