use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::metrics::CategoryResult;

#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct HotspotFile {
    pub path: String,
    pub churn_count: usize,
    pub bug_commit_count: usize,
    pub loc: usize,
    pub total_lines: usize,
    pub cyclomatic_complexity: u32,
    pub public_methods: u32,
    pub properties: u32,
    pub hotspot_score: f64,
    /// Commits touching the file per 1/12 of the analysis window (oldest first).
    pub churn_timeline: Vec<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct CouplingPair {
    pub file_a: String,
    pub file_b: String,
    pub co_changes: usize,
    pub coupling_pct: f64,
    pub cross_boundary: bool,
    pub is_test_pair: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthorShare {
    pub name: String,
    pub pct: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileOwnership {
    pub path: String,
    pub authors: Vec<AuthorShare>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileAge {
    pub path: String,
    pub last_modified: chrono::DateTime<chrono::Utc>,
    pub days_since_modified: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthorCard {
    pub name: String,
    pub email: String,
    pub commit_count: usize,
    pub files_owned: usize,
    pub lines_owned: usize,
    pub avg_commit_quality: f64,
    pub top_files: Vec<String>,
    pub last_active: chrono::DateTime<chrono::Utc>,
    pub days_since_active: i64,
    pub directories_touched: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrisisFile {
    pub path: String,
    pub crisis_commit_count: usize,
    pub total_commit_count: usize,
    pub crisis_ratio: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DirConcentration {
    pub dir: String,
    pub file_count: usize,
    pub loc: usize,
    pub pct_of_total: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeadFile {
    pub path: String,
    pub days_since_modified: i64,
    pub churn_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct VelocityBucket {
    pub week_start: String,
    pub commit_count: usize,
    pub author_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditReport {
    pub crisis_files: Vec<CrisisFile>,
    pub dir_concentration: Vec<DirConcentration>,
    pub dead_files: Vec<DeadFile>,
    pub velocity_buckets: Vec<VelocityBucket>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileCouplingMetrics {
    pub path: String,
    pub ca: usize,
    pub ce: usize,
    pub instability: f64,
}

/// One directed edge of the static import graph: `from` imports `to`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportEdge {
    pub from: String,
    pub to: String,
}

/// Metadata about the remote repository origin (populated when a URL is given).
#[derive(Debug, Clone, Serialize)]
pub struct RemoteMeta {
    pub url: String,
    pub stars: Option<u64>,
    pub description: Option<String>,
    pub language: Option<String>,
    pub open_issues: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionItem {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_tab: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct AnalysisReport {
    pub repo_name: String,
    pub branch: String,
    pub time_window_months: u32,
    pub total_commits: usize,
    pub total_authors: usize,
    pub total_files: usize,
    pub overall_score: u32,
    pub categories: Vec<CategoryResult>,
    pub top_actions: Vec<ActionItem>,
    pub remote_meta: Option<RemoteMeta>,
    pub file_hotspots: Vec<HotspotFile>,
    pub coupling_pairs: Vec<CouplingPair>,
    pub author_ownership: Vec<FileOwnership>,
    pub file_ages: Vec<FileAge>,
    pub author_cards: Vec<AuthorCard>,
    pub history: Vec<HistoryEntry>,
    pub dep_ecosystem_reports: Vec<crate::deps::EcosystemReport>,
    pub audit: Option<AuditReport>,
    pub per_file_coupling: Vec<FileCouplingMetrics>,
    pub import_edges: Vec<ImportEdge>,
    /// Import cycles as sorted member-file lists (depth 1 and 2).
    pub import_cycles: Vec<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coupling_finding_counts: Option<CouplingFindingCounts>,
    pub score_thresholds: ScoreThresholds,
}

/// Per-kind Pressman coupling finding counts for one analysis run.
/// `None` on the report means detection did not run (e.g. backfill's
/// ADR-005 snapshot) — distinct from all-zero, which means "clean".
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct CouplingFindingCounts {
    pub content: usize,
    pub common: usize,
    pub control: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistoryCounts {
    pub commits: usize,
    pub files: usize,
    pub authors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    #[serde(rename = "head", alias = "commit")]
    pub head: String,
    pub overall_score: u32,
    #[serde(rename = "category_scores", alias = "categories")]
    pub categories: HashMap<String, u32>,
    #[serde(default)]
    pub metrics: HashMap<String, u32>,
    #[serde(default)]
    pub counts: HistoryCounts,
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub schema_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Minimum score (inclusive) for the "good" band.
pub const SCORE_GOOD_MIN: u32 = 71;
/// Minimum score (inclusive) for the "warn" band; below is "danger".
pub const SCORE_WARN_MIN: u32 = 41;

/// Qualitative band for a 0–100 score. Single source of truth for every
/// renderer (CLI colors, HTML report, dashboard) — renderers must not
/// re-derive thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreBand {
    Good,
    Warn,
    Danger,
}

pub fn score_band(score: u32) -> ScoreBand {
    match score {
        s if s >= SCORE_GOOD_MIN => ScoreBand::Good,
        s if s >= SCORE_WARN_MIN => ScoreBand::Warn,
        _ => ScoreBand::Danger,
    }
}

/// Band thresholds serialized into every report so JS/TS consumers read the
/// verdict boundaries instead of hardcoding them.
#[derive(Debug, Clone, Serialize)]
pub struct ScoreThresholds {
    pub good_min: u32,
    pub warn_min: u32,
}

impl Default for ScoreThresholds {
    fn default() -> Self {
        Self {
            good_min: SCORE_GOOD_MIN,
            warn_min: SCORE_WARN_MIN,
        }
    }
}

#[cfg(test)]
mod band_tests {
    use super::*;

    #[test]
    fn band_boundaries_match_documented_thresholds() {
        assert_eq!(score_band(100), ScoreBand::Good);
        assert_eq!(score_band(SCORE_GOOD_MIN), ScoreBand::Good);
        assert_eq!(score_band(SCORE_GOOD_MIN - 1), ScoreBand::Warn);
        assert_eq!(score_band(SCORE_WARN_MIN), ScoreBand::Warn);
        assert_eq!(score_band(SCORE_WARN_MIN - 1), ScoreBand::Danger);
        assert_eq!(score_band(0), ScoreBand::Danger);
    }

    #[test]
    fn default_thresholds_serialize_for_consumers() {
        let json = serde_json::to_string(&ScoreThresholds::default()).unwrap();
        assert_eq!(json, r#"{"good_min":71,"warn_min":41}"#);
    }
}
