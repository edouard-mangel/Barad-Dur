use serde::Serialize;

/// Per-kind Pressman coupling finding counts for one analysis run.
/// `None` on the report means detection did not run (e.g. backfill's
/// ADR-005 snapshot) — distinct from all-zero, which means "clean".
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "export-types", derive(ts_rs::TS))]
pub struct CouplingFindingCounts {
    pub content: usize,
    pub common: usize,
    pub inheritance: usize,
    pub control: usize,
}
