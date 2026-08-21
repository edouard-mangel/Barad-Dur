mod thresholds;
pub use thresholds::*;

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
    #[serde(default = "default_coupling_weight")]
    pub coupling: u32,
    #[serde(default = "default_deps_weight")]
    pub deps: u32,
}

fn default_health_weight() -> u32 {
    35
}
fn default_team_weight() -> u32 {
    10
}
fn default_evolution_weight() -> u32 {
    20
}
fn default_hygiene_weight() -> u32 {
    15
}
fn default_coupling_weight() -> u32 {
    20
}
fn default_deps_weight() -> u32 {
    0
} // 0 = excluded unless --deps is passed

impl Default for CategoryWeights {
    fn default() -> Self {
        Self {
            health: 35,
            team: 10,
            evolution: 20,
            hygiene: 15,
            coupling: 20,
            deps: 0,
        }
    }
}

impl CategoryWeights {
    pub fn sum(&self) -> u32 {
        self.health + self.team + self.evolution + self.hygiene + self.coupling + self.deps
    }

    /// Convert to the (name, f64) pairs format used by scorer.
    pub fn as_weight_pairs(&self) -> Vec<(&'static str, f64)> {
        let s = self.sum() as f64;
        let mut pairs = vec![
            ("Health", self.health as f64 / s),
            ("Team", self.team as f64 / s),
            ("Evolution", self.evolution as f64 / s),
            ("Git Hygiene", self.hygiene as f64 / s),
            ("Coupling", self.coupling as f64 / s),
        ];
        if self.deps > 0 {
            pairs.push(("Dependencies", self.deps as f64 / s));
        }
        pairs
    }
}

/// TOML file structure — maps 1:1 to the .repository-analysis/barad-dur.toml sections.
#[derive(Debug, Clone, Deserialize, Default)]
struct TomlConfig {
    #[serde(default)]
    analysis: TomlAnalysis,
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
    /// Whether the built-in default exclusions apply (toggled by `--no-default-excludes`).
    pub exclude_use_defaults: bool,
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
    for key in unknown_top_level_keys(&content) {
        eprintln!(
            "Warning: unknown config key '{}' in {}",
            key,
            config_path.display()
        );
    }

    let toml_cfg: TomlConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", config_path.display()))?;

    Ok(RepoConfig {
        since: toml_cfg.analysis.since,
        skip_blame: toml_cfg.analysis.skip_blame,
        // Exclusions live in `.baraddurignore` / CLI flags now, not TOML. Defaults
        // are on unless `--no-default-excludes` turns them off (applied in merge).
        exclude_use_defaults: true,
        weights: toml_cfg.weights,
        thresholds: toml_cfg.thresholds,
        output_format: toml_cfg.output.format,
        auto_open: toml_cfg.output.auto_open,
        backfill: toml_cfg.backfill,
    })
}

/// Top-level TOML sections not recognised by the config schema. Pure so it can be
/// unit-tested; the caller is responsible for reporting the result.
fn unknown_top_level_keys(content: &str) -> Vec<String> {
    const KNOWN: &[&str] = &["analysis", "weights", "thresholds", "output", "backfill"];
    let Ok(toml::Value::Table(table)) = content.parse::<toml::Value>() else {
        return Vec::new();
    };
    table
        .keys()
        .filter(|k| !KNOWN.contains(&k.as_str()))
        .cloned()
        .collect()
}

/// Validate the merged config.
pub fn validate(config: &RepoConfig) -> Result<()> {
    let sum = config.weights.sum();
    if sum != 100 {
        bail!(
            "Category weights must sum to 100, got {} (health={}, team={}, evolution={}, hygiene={}, coupling={})",
            sum,
            config.weights.health,
            config.weights.team,
            config.weights.evolution,
            config.weights.hygiene,
            config.weights.coupling,
        );
    }
    if config.thresholds.coupling.component_depth == 0 {
        bail!("thresholds.coupling.component_depth must be >= 1, got 0");
    }
    let ratio = config.thresholds.coupling.change_coupling_min_ratio;
    if !(0.0..=1.0).contains(&ratio) {
        bail!(
            "thresholds.coupling.change_coupling_min_ratio must be in [0.0, 1.0], got {}",
            ratio
        );
    }
    let multiplier = config.thresholds.coupling.hotspot_multiplier;
    if multiplier < 1.0 {
        bail!(
            "thresholds.coupling.hotspot_multiplier must be >= 1.0, got {}",
            multiplier
        );
    }
    let corroboration_weight = config.thresholds.coupling.corroboration_weight;
    if corroboration_weight.is_nan() || corroboration_weight < 1.0 {
        bail!(
            "thresholds.coupling.corroboration_weight must be >= 1.0, got {}",
            corroboration_weight
        );
    }
    if config.thresholds.coupling.decay_min_partners == 0 {
        bail!("thresholds.coupling.decay_min_partners must be >= 1, got 0");
    }
    if config.thresholds.coupling.inheritance_min_depth == 1 {
        bail!("thresholds.coupling.inheritance_min_depth must be 0 (disabled) or >= 2, got 1");
    }
    // No realistic import-graph degree needs a multiplier anywhere near this;
    // above it, `median_degree * multiplier` risks overflowing to +inf, which
    // silently disables the hub check the same way a literal Infinity would.
    const GOD_NODE_MULTIPLIER_MAX: f64 = 1_000_000.0;
    // NaN fails the range check on its own — no separate is_nan clause.
    let call_floor = config.thresholds.health.call_resolution_floor;
    if !(0.0..=1.0).contains(&call_floor) {
        bail!(
            "thresholds.health.call_resolution_floor must be in [0.0, 1.0], got {}",
            call_floor
        );
    }
    let god_multiplier = config.thresholds.health.god_node_degree_multiplier;
    if !god_multiplier.is_finite()
        || god_multiplier <= 0.0
        || god_multiplier > GOD_NODE_MULTIPLIER_MAX
    {
        bail!(
            "thresholds.health.god_node_degree_multiplier must be finite, > 0.0, and <= {}, got {}",
            GOD_NODE_MULTIPLIER_MAX,
            god_multiplier
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

/// Full merge: apply CLI overrides on top of loaded config.
pub fn merge_with_cli(config: RepoConfig, args: &crate::cli::AnalyzeArgs) -> RepoConfig {
    RepoConfig {
        since: merge_since(config.since, args.since.clone()),
        skip_blame: merge_bool(config.skip_blame, args.skip_blame),
        exclude_use_defaults: merge_bool(
            config.exclude_use_defaults,
            args.no_default_excludes.map(|v| !v),
        ),
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
        assert_eq!(cfg.weights.sum(), 100);
        assert_eq!(cfg.output_format, OutputFormat::Cli);
        assert!(!cfg.auto_open);
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        let dir = TempDir::new().unwrap();
        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.weights.health, 35);
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
        assert_eq!(cfg.weights.health, 35);
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
        assert_eq!(cfg.weights.coupling, 20); // missing key defaults to 20
    }

    #[test]
    fn unknown_top_level_keys_flags_legacy_exclude() {
        let unknown = unknown_top_level_keys("[analysis]\n[exclude]\npatterns = []\n");
        assert_eq!(unknown, vec!["exclude"]);
    }

    #[test]
    fn unknown_top_level_keys_empty_for_known_sections() {
        let unknown =
            unknown_top_level_keys("[analysis]\n[weights]\n[output]\n[thresholds.health]\n");
        assert!(unknown.is_empty(), "unexpected unknown keys: {unknown:?}");
    }

    #[test]
    fn load_ignores_legacy_exclude_section() {
        // `[exclude]` was removed in favour of `.baraddurignore` + CLI flags. A
        // leftover section must not break loading — it is ignored (and surfaces an
        // unknown-section warning via `warn_unknown_keys`).
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(".repository-analysis");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(
            cache_dir.join("barad-dur.toml"),
            "[exclude]\nuse_defaults = false\npatterns = [\"*.resx\"]\n",
        )
        .unwrap();
        let cfg = load(dir.path()).unwrap();
        // The removed `use_defaults` key no longer has any effect.
        assert!(cfg.exclude_use_defaults);
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
    fn merge_with_cli_wires_no_default_excludes() {
        use crate::cli::{Cli, Commands};
        use clap::Parser;

        let cli = Cli::parse_from(["barad-dur", "analyze", ".", "--no-default-excludes"]);
        let args = match cli.command {
            Commands::Analyze(a) => a,
            _ => panic!("expected Analyze"),
        };
        let merged = merge_with_cli(RepoConfig::default(), &args);
        assert!(!merged.exclude_use_defaults);
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

    #[test]
    fn load_coupling_thresholds_defaults() {
        let dir = TempDir::new().unwrap();
        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.thresholds.coupling.component_depth, 2);
        assert!((cfg.thresholds.coupling.change_coupling_min_ratio - 0.30).abs() < f64::EPSILON);
    }

    #[test]
    fn load_coupling_thresholds_from_toml() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(".repository-analysis");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(
            cache_dir.join("barad-dur.toml"),
            "[thresholds.coupling]\ncomponent_depth = 3\nchange_coupling_min_ratio = 0.50\n",
        )
        .unwrap();
        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.thresholds.coupling.component_depth, 3);
        assert!((cfg.thresholds.coupling.change_coupling_min_ratio - 0.50).abs() < f64::EPSILON);
    }

    #[test]
    fn decay_min_partners_defaults_and_loads() {
        let dir = TempDir::new().unwrap();
        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.thresholds.coupling.decay_min_partners, 8);
        let cache_dir = dir.path().join(".repository-analysis");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(
            cache_dir.join("barad-dur.toml"),
            "[thresholds.coupling]\ndecay_min_partners = 12\n",
        )
        .unwrap();
        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.thresholds.coupling.decay_min_partners, 12);
    }

    #[test]
    fn validate_decay_min_partners_zero_errors() {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.coupling.decay_min_partners = 0;
        let err = validate(&cfg).unwrap_err();
        assert!(err.to_string().contains("decay_min_partners"));
    }

    #[test]
    fn validate_coupling_depth_zero_errors() {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.coupling.component_depth = 0;
        let err = validate(&cfg).unwrap_err();
        assert!(err.to_string().contains("component_depth"));
    }

    #[test]
    fn validate_coupling_ratio_out_of_range_errors() {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.coupling.change_coupling_min_ratio = 1.5;
        let err = validate(&cfg).unwrap_err();
        assert!(err.to_string().contains("change_coupling_min_ratio"));
    }

    #[test]
    fn load_hotspot_multiplier_default() {
        let dir = TempDir::new().unwrap();
        let cfg = load(dir.path()).unwrap();
        assert!((cfg.thresholds.coupling.hotspot_multiplier - 1.25).abs() < f64::EPSILON);
    }

    #[test]
    fn load_hotspot_multiplier_from_toml() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(".repository-analysis");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(
            cache_dir.join("barad-dur.toml"),
            "[thresholds.coupling]\nhotspot_multiplier = 1.5\n",
        )
        .unwrap();
        let cfg = load(dir.path()).unwrap();
        assert!((cfg.thresholds.coupling.hotspot_multiplier - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn validate_hotspot_multiplier_below_one_errors() {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.coupling.hotspot_multiplier = 0.9;
        assert!(
            validate(&cfg).is_err(),
            "a discount multiplier is a config mistake"
        );
    }

    #[test]
    fn validate_hotspot_multiplier_of_exactly_one_is_valid() {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.coupling.hotspot_multiplier = 1.0;
        assert!(
            validate(&cfg).is_ok(),
            "hotspot_multiplier = 1.0 is the lower bound and must be accepted"
        );
    }

    #[test]
    fn validate_corroboration_weight_below_one_errors() {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.coupling.corroboration_weight = 0.9;
        assert!(
            validate(&cfg).is_err(),
            "a sub-1.0 corroboration weight would un-trip the severity cap"
        );
    }

    #[test]
    fn validate_corroboration_weight_of_exactly_one_is_valid() {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.coupling.corroboration_weight = 1.0;
        assert!(
            validate(&cfg).is_ok(),
            "corroboration_weight = 1.0 is the lower bound and must be accepted"
        );
    }

    #[test]
    fn validate_corroboration_weight_nan_errors() {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.coupling.corroboration_weight = f64::NAN;
        assert!(
            validate(&cfg).is_err(),
            "NaN must be rejected explicitly since NaN < 1.0 is false"
        );
    }

    #[test]
    fn inheritance_min_depth_of_one_is_rejected() {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.coupling.inheritance_min_depth = 1;
        assert!(
            validate(&cfg).is_err(),
            "1 would flag every cross-file extends"
        );
    }

    #[test]
    fn call_resolution_floor_default_is_half() {
        let cfg = RepoConfig::default();
        assert!((cfg.thresholds.health.call_resolution_floor - 0.5).abs() < f64::EPSILON);
        assert!(validate(&cfg).is_ok());
    }

    #[test]
    fn validate_call_resolution_floor_above_one_errors() {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.health.call_resolution_floor = 1.5;
        let err = validate(&cfg).unwrap_err();
        assert!(err.to_string().contains("call_resolution_floor"));
    }

    #[test]
    fn validate_call_resolution_floor_negative_errors() {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.health.call_resolution_floor = -0.1;
        assert!(validate(&cfg).is_err());
    }

    #[test]
    fn validate_call_resolution_floor_nan_errors() {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.health.call_resolution_floor = f64::NAN;
        assert!(validate(&cfg).is_err());
    }

    #[test]
    fn validate_call_resolution_floor_bounds_are_valid() {
        for floor in [0.0, 1.0] {
            let mut cfg = RepoConfig::default();
            cfg.thresholds.health.call_resolution_floor = floor;
            assert!(validate(&cfg).is_ok(), "floor {floor} must be accepted");
        }
    }

    #[test]
    fn validate_god_node_degree_multiplier_zero_errors() {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.health.god_node_degree_multiplier = 0.0;
        let err = validate(&cfg).unwrap_err();
        assert!(err.to_string().contains("god_node_degree_multiplier"));
    }

    #[test]
    fn validate_god_node_degree_multiplier_negative_errors() {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.health.god_node_degree_multiplier = -1.0;
        assert!(validate(&cfg).is_err());
    }

    #[test]
    fn validate_god_node_degree_multiplier_nan_errors() {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.health.god_node_degree_multiplier = f64::NAN;
        assert!(validate(&cfg).is_err());
    }

    #[test]
    fn validate_god_node_degree_multiplier_default_is_valid() {
        let cfg = RepoConfig::default();
        assert!(validate(&cfg).is_ok());
    }

    #[test]
    fn load_god_node_degree_multiplier_default() {
        let dir = TempDir::new().unwrap();
        let cfg = load(dir.path()).unwrap();
        assert!((cfg.thresholds.health.god_node_degree_multiplier - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn load_god_node_min_degree_default() {
        let dir = TempDir::new().unwrap();
        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.thresholds.health.god_node_min_degree, 8);
    }

    #[test]
    fn validate_god_node_degree_multiplier_infinite_errors() {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.health.god_node_degree_multiplier = f64::INFINITY;
        assert!(
            validate(&cfg).is_err(),
            "an infinite multiplier would make the hub check unsatisfiable, silently disabling it"
        );
    }

    #[test]
    fn validate_god_node_degree_multiplier_extreme_finite_value_errors() {
        let mut cfg = RepoConfig::default();
        // Finite, but median_degree * multiplier would overflow to +inf for
        // any realistic median degree — the same silent-disable failure as
        // literal Infinity, just reached via a value that passes is_finite().
        cfg.thresholds.health.god_node_degree_multiplier = 1e308;
        assert!(
            validate(&cfg).is_err(),
            "an extreme-but-finite multiplier can still overflow the hub threshold to infinity"
        );
    }

    #[test]
    fn inheritance_min_depth_zero_and_two_are_accepted() {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.coupling.inheritance_min_depth = 0;
        assert!(validate(&cfg).is_ok(), "0 disables the rule");
        cfg.thresholds.coupling.inheritance_min_depth = 2;
        assert!(validate(&cfg).is_ok());
    }
}
