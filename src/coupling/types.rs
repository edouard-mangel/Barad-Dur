use crate::coupling::dependency::BlastRadiusEntry;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Output format for coupling analysis results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    Cli,
    Json,
    Html,
}

/// Configuration controlling a coupling analysis run.
#[derive(Debug, Clone)]
pub struct CouplingConfig {
    /// Root directory containing multiple repositories to analyze.
    pub root_dir: PathBuf,
    /// Maximum time window for commits to be considered "coupled" (default: 24 hours).
    pub coupling_window: Duration,
    /// How far back in history to look for commits (default: 6 months / 180 days).
    pub analysis_window: Duration,
    /// Minimum combined score to include a pair in the report (default: 30.0).
    pub min_score_threshold: f64,
    /// Output format for the report.
    pub output_format: OutputFormat,
}

impl Default for CouplingConfig {
    fn default() -> Self {
        Self {
            root_dir: PathBuf::new(),
            coupling_window: Duration::from_secs(24 * 60 * 60),
            analysis_window: Duration::from_secs(180 * 24 * 60 * 60),
            min_score_threshold: 30.0,
            output_format: OutputFormat::Cli,
        }
    }
}

/// Information about a single repository discovered under the root directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub name: String,
    pub path: PathBuf,
    pub commit_count: usize,
    pub author_count: usize,
}

/// Temporal coupling details between two repositories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalDetails {
    /// Number of times commits in both repos fell within the coupling window.
    pub co_commit_count: usize,
    /// Total commit windows analyzed. Reserved for future use; currently always 0.
    pub total_windows: usize,
}

/// Team overlap details between two repositories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamDetails {
    /// Number of authors who contributed to both repos.
    pub shared_authors: usize,
    /// Total unique authors across both repos.
    pub total_authors: usize,
}

/// Dependency details between two repositories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyDetails {
    /// Number of shared dependencies detected.
    pub shared_dependencies: usize,
    /// Description of the dependency relationship.
    pub relationship: String,
}

/// Breakdown of coupling signals between two repositories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouplingDetails {
    pub temporal: TemporalDetails,
    pub team: TeamDetails,
    pub dependency: DependencyDetails,
}

/// A pair of repositories with their coupling scores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouplingPair {
    pub repo_a: String,
    pub repo_b: String,
    pub temporal_score: f64,
    pub team_score: f64,
    pub dependency_score: f64,
    pub combined_score: f64,
    pub details: CouplingDetails,
}

/// Summary statistics for the coupling analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouplingReportSummary {
    pub total_repos: usize,
    pub total_pairs_analyzed: usize,
    pub pairs_above_threshold: usize,
    pub highest_coupling_score: f64,
}

/// Full coupling analysis report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouplingReport {
    pub repos: Vec<RepoInfo>,
    pub pairs: Vec<CouplingPair>,
    pub summary: CouplingReportSummary,
    pub blast_radius: Vec<BlastRadiusEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_coupling_config_has_24h_coupling_window() {
        let config = CouplingConfig::default();
        assert_eq!(config.coupling_window, Duration::from_secs(24 * 60 * 60));
    }

    #[test]
    fn default_coupling_config_has_6_month_analysis_window() {
        let config = CouplingConfig::default();
        assert_eq!(
            config.analysis_window,
            Duration::from_secs(180 * 24 * 60 * 60)
        );
    }

    #[test]
    fn default_coupling_config_has_30_min_score_threshold() {
        let config = CouplingConfig::default();
        assert!((config.min_score_threshold - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn default_coupling_config_has_cli_output_format() {
        let config = CouplingConfig::default();
        assert_eq!(config.output_format, OutputFormat::Cli);
    }

    #[test]
    fn default_coupling_config_has_empty_root_dir() {
        let config = CouplingConfig::default();
        assert_eq!(config.root_dir, PathBuf::new());
    }
}
