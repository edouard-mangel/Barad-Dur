use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::metrics::CategoryResult;

#[derive(Debug, Clone, Serialize)]
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
}

#[derive(Debug, Clone, Serialize)]
pub struct CouplingPair {
    pub file_a: String,
    pub file_b: String,
    pub co_changes: usize,
    pub coupling_pct: f64,
    pub cross_boundary: bool,
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
