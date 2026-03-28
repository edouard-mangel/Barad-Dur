use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::Path;

use crate::cache::storage::CACHE_DIR;

const CONFIG_FILE: &str = "barad-dur.toml";

/// Output format for the report.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Cli,
    Html,
    Json,
}

/// Category weights for overall score (must sum to 100).
#[derive(Debug, Clone, Deserialize)]
pub struct CategoryWeights {
    #[serde(default = "default_health_weight")]
    pub health: u32,
    #[serde(default = "default_team_weight")]
    pub team: u32,
    #[serde(default = "default_evolution_weight")]
    pub evolution: u32,
    #[serde(default = "default_hygiene_weight")]
    pub hygiene: u32,
}

fn default_health_weight() -> u32 {
    40
}
fn default_team_weight() -> u32 {
    15
}
fn default_evolution_weight() -> u32 {
    25
}
fn default_hygiene_weight() -> u32 {
    20
}

impl Default for CategoryWeights {
    fn default() -> Self {
        Self {
            health: 40,
            team: 15,
            evolution: 25,
            hygiene: 20,
        }
    }
}

impl CategoryWeights {
    pub fn sum(&self) -> u32 {
        self.health + self.team + self.evolution + self.hygiene
    }

    /// Convert to the (name, f64) pairs format used by scorer.
    pub fn as_weight_pairs(&self) -> Vec<(&'static str, f64)> {
        let s = self.sum() as f64;
        vec![
            ("Health", self.health as f64 / s),
            ("Team", self.team as f64 / s),
            ("Evolution", self.evolution as f64 / s),
            ("Git Hygiene", self.hygiene as f64 / s),
        ]
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct HealthThresholds {
    #[serde(default = "default_max_complexity")]
    pub max_complexity: u32,
    #[serde(default = "default_hotspot_top_n")]
    pub hotspot_top_n: usize,
    #[serde(default = "default_coupling_min_commits")]
    pub coupling_min_commits: usize,
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

impl Default for HealthThresholds {
    fn default() -> Self {
        Self {
            max_complexity: default_max_complexity(),
            hotspot_top_n: default_hotspot_top_n(),
            coupling_min_commits: default_coupling_min_commits(),
        }
    }
}

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
}

/// TOML file structure — maps 1:1 to the .repository-analysis/barad-dur.toml sections.
#[derive(Debug, Clone, Deserialize, Default)]
struct TomlConfig {
    #[serde(default)]
    analysis: TomlAnalysis,
    #[serde(default)]
    exclude: TomlExclude,
    #[serde(default)]
    weights: CategoryWeights,
    #[serde(default)]
    thresholds: Thresholds,
    #[serde(default)]
    output: TomlOutput,
    #[serde(default)]
    backfill: BackfillConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct TomlAnalysis {
    since: Option<String>,
    #[serde(default)]
    skip_blame: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct TomlExclude {
    #[serde(default = "default_true")]
    use_defaults: bool,
    #[serde(default)]
    patterns: Vec<String>,
}

fn default_true() -> bool {
    true
}

impl Default for TomlExclude {
    fn default() -> Self {
        Self {
            use_defaults: true,
            patterns: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct TomlOutput {
    #[serde(default)]
    format: OutputFormat,
    #[serde(default)]
    auto_open: bool,
}

/// Configuration for the backfill subcommand.
#[derive(Debug, Clone, Deserialize)]
pub struct BackfillConfig {
    #[serde(default = "default_sample_count")]
    pub sample_count: u32,
}

fn default_sample_count() -> u32 {
    10
}

impl Default for BackfillConfig {
    fn default() -> Self {
        Self {
            sample_count: default_sample_count(),
        }
    }
}

/// Resolved configuration after merging defaults + TOML + CLI.
#[derive(Debug, Clone)]
pub struct RepoConfig {
    pub since: Option<String>,
    pub skip_blame: bool,
    pub exclude_use_defaults: bool,
    pub exclude_patterns: Vec<String>,
    pub weights: CategoryWeights,
    pub thresholds: Thresholds,
    pub output_format: OutputFormat,
    pub auto_open: bool,
    pub backfill: BackfillConfig,
}

impl Default for RepoConfig {
    fn default() -> Self {
        Self {
            since: None,
            skip_blame: false,
            exclude_use_defaults: true,
            exclude_patterns: Vec::new(),
            weights: CategoryWeights::default(),
            thresholds: Thresholds::default(),
            output_format: OutputFormat::Cli,
            auto_open: false,
            backfill: BackfillConfig::default(),
        }
    }
}

/// Load config from `.repository-analysis/barad-dur.toml` if it exists.
/// Returns default config if the file is absent.
pub fn load(repo_root: &Path) -> Result<RepoConfig> {
    let config_path = repo_root.join(CACHE_DIR).join(CONFIG_FILE);
    if !config_path.exists() {
        return Ok(RepoConfig::default());
    }

    let content = std::fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;

    // Two-pass parse: first to toml::Value for unknown-key warnings,
    // then to our typed struct.
    warn_unknown_keys(&content, &config_path);

    let toml_cfg: TomlConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", config_path.display()))?;

    Ok(RepoConfig {
        since: toml_cfg.analysis.since,
        skip_blame: toml_cfg.analysis.skip_blame,
        exclude_use_defaults: toml_cfg.exclude.use_defaults,
        exclude_patterns: toml_cfg.exclude.patterns,
        weights: toml_cfg.weights,
        thresholds: toml_cfg.thresholds,
        output_format: toml_cfg.output.format,
        auto_open: toml_cfg.output.auto_open,
        backfill: toml_cfg.backfill,
    })
}

fn warn_unknown_keys(content: &str, path: &Path) {
    let known_sections = [
        "analysis",
        "exclude",
        "weights",
        "thresholds",
        "output",
        "backfill",
    ];
    if let Ok(value) = content.parse::<toml::Value>() {
        if let Some(table) = value.as_table() {
            for key in table.keys() {
                if !known_sections.contains(&key.as_str()) {
                    eprintln!(
                        "Warning: Unknown config key '{}' in {}",
                        key,
                        path.display()
                    );
                }
            }
        }
    }
}

/// Validate the merged config.
pub fn validate(config: &RepoConfig) -> Result<()> {
    let sum = config.weights.sum();
    if sum != 100 {
        bail!(
            "Category weights must sum to 100, got {} (health={}, team={}, evolution={}, hygiene={})",
            sum,
            config.weights.health,
            config.weights.team,
            config.weights.evolution,
            config.weights.hygiene,
        );
    }
    Ok(())
}

/// Merge a single Option<String> field: CLI wins if Some.
pub fn merge_since(toml_val: Option<String>, cli_val: Option<String>) -> Option<String> {
    cli_val.or(toml_val)
}

/// Merge bool: CLI wins if Some, otherwise TOML value.
pub fn merge_bool(toml_val: bool, cli_val: Option<bool>) -> bool {
    cli_val.unwrap_or(toml_val)
}

/// Merge exclude patterns: append CLI patterns to TOML patterns.
pub fn merge_exclude_patterns(
    mut toml_patterns: Vec<String>,
    cli_patterns: &[String],
) -> Vec<String> {
    toml_patterns.extend(cli_patterns.iter().cloned());
    toml_patterns
}

/// Full merge: apply CLI overrides on top of loaded config.
pub fn merge_with_cli(config: RepoConfig, args: &crate::cli::AnalyzeArgs) -> RepoConfig {
    RepoConfig {
        since: merge_since(config.since, args.since.clone()),
        skip_blame: merge_bool(config.skip_blame, args.skip_blame),
        exclude_use_defaults: merge_bool(
            config.exclude_use_defaults,
            args.no_default_excludes.map(|v| !v),
        ),
        exclude_patterns: merge_exclude_patterns(config.exclude_patterns, &args.exclude),
        weights: config.weights,
        thresholds: config.thresholds,
        output_format: if args.json {
            OutputFormat::Json
        } else if args.html {
            OutputFormat::Html
        } else {
            config.output_format
        },
        auto_open: if args.open { true } else { config.auto_open },
        backfill: config.backfill,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn default_config_values() {
        let cfg = RepoConfig::default();
        assert_eq!(cfg.since, None);
        assert!(!cfg.skip_blame);
        assert!(cfg.exclude_use_defaults);
        assert!(cfg.exclude_patterns.is_empty());
        assert_eq!(cfg.weights.sum(), 100);
        assert_eq!(cfg.output_format, OutputFormat::Cli);
        assert!(!cfg.auto_open);
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        let dir = TempDir::new().unwrap();
        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.weights.health, 40);
        assert!(cfg.exclude_use_defaults);
    }

    #[test]
    fn load_minimal_toml() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(".repository-analysis");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(
            cache_dir.join("barad-dur.toml"),
            "[analysis]\nsince = \"3months\"\n",
        )
        .unwrap();
        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.since, Some("3months".to_string()));
        assert_eq!(cfg.weights.health, 40);
    }

    #[test]
    fn load_custom_weights() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(".repository-analysis");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(
            cache_dir.join("barad-dur.toml"),
            "[weights]\nhealth = 40\nteam = 30\nevolution = 20\nhygiene = 10\n",
        )
        .unwrap();
        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.weights.health, 40);
        assert_eq!(cfg.weights.hygiene, 10);
    }

    #[test]
    fn load_exclude_patterns() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(".repository-analysis");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(
            cache_dir.join("barad-dur.toml"),
            "[exclude]\nuse_defaults = false\npatterns = [\"*.resx\", \"**/i18n/**\"]\n",
        )
        .unwrap();
        let cfg = load(dir.path()).unwrap();
        assert!(!cfg.exclude_use_defaults);
        assert_eq!(cfg.exclude_patterns, vec!["*.resx", "**/i18n/**"]);
    }

    #[test]
    fn load_output_section() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(".repository-analysis");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(
            cache_dir.join("barad-dur.toml"),
            "[output]\nformat = \"html\"\nauto_open = true\n",
        )
        .unwrap();
        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.output_format, OutputFormat::Html);
        assert!(cfg.auto_open);
    }

    #[test]
    fn load_thresholds() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(".repository-analysis");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(
            cache_dir.join("barad-dur.toml"),
            "[thresholds.health]\nmax_complexity = 30\n",
        )
        .unwrap();
        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.thresholds.health.max_complexity, 30);
        assert_eq!(cfg.thresholds.team.silo_max_owners, 1);
    }

    #[test]
    fn load_bad_toml_returns_error() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(".repository-analysis");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(cache_dir.join("barad-dur.toml"), "not valid toml [[[").unwrap();
        assert!(load(dir.path()).is_err());
    }

    #[test]
    fn validate_weights_sum_100() {
        let cfg = RepoConfig::default();
        assert!(validate(&cfg).is_ok());
    }

    #[test]
    fn validate_weights_bad_sum() {
        let mut cfg = RepoConfig::default();
        cfg.weights.health = 50;
        let err = validate(&cfg).unwrap_err();
        assert!(err.to_string().contains("must sum to 100"));
    }

    #[test]
    fn merge_cli_since_overrides_toml() {
        let merged = merge_since(Some("6months".into()), Some("3months".into()));
        assert_eq!(merged, Some("3months".to_string()));
    }

    #[test]
    fn merge_cli_since_none_keeps_toml() {
        let merged = merge_since(Some("6months".into()), None);
        assert_eq!(merged, Some("6months".to_string()));
    }

    #[test]
    fn merge_exclude_appends() {
        let toml_patterns = vec!["*.resx".into()];
        let cli_patterns = vec!["**/vendor/**".into()];
        let merged = merge_exclude_patterns(toml_patterns, &cli_patterns);
        assert_eq!(merged, vec!["*.resx", "**/vendor/**"]);
    }

    #[test]
    fn merge_skip_blame_cli_overrides() {
        let merged = merge_bool(false, Some(true));
        assert!(merged);
    }

    #[test]
    fn merge_skip_blame_cli_absent_keeps_toml() {
        let merged = merge_bool(true, None);
        assert!(merged);
    }
}
