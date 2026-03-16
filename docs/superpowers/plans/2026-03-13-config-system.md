# Config System Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `.repository-analysis/barad-dur.toml` config file system with 3-layer merge (defaults -> TOML -> CLI), configurable weights/thresholds, and a `barad-dur init` scaffold/wizard command.

**Architecture:** New `src/config.rs` module owns the `RepoConfig` struct, TOML loading, CLI merge, and validation. The `init` subcommand lives in `src/init.rs` with repo-scanning heuristics. Existing metric modules gain optional threshold parameters threaded from config.

**Tech Stack:** Rust, `toml` crate for parsing, `clap` derive API with `Option<bool>` for merge-aware flags.

**Spec:** `docs/superpowers/specs/2026-03-13-config-system-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/config.rs` | Create | `RepoConfig` struct, TOML deserialization, `load()`, `merge_with_cli()`, `validate()` |
| `src/init.rs` | Create | `barad-dur init` logic: repo scanning heuristics, TOML generation, interactive wizard |
| `src/cli.rs` | Modify | Add `Init` subcommand, change `skip_blame`/`no_default_excludes` to `Option<bool>` |
| `src/main.rs` | Modify | Load config after path resolution, merge with CLI, thread config to scorer/metrics |
| `src/lib.rs` | Modify | Add `pub mod config; pub mod init;` |
| `src/scorer.rs` | Modify | `build_report()` and `compute_overall_score()` accept weights from config |
| `src/metrics/health.rs` | Modify | `compute_health()` accepts `HealthThresholds` |
| `src/metrics/team.rs` | Modify | `compute_team()` accepts `TeamThresholds` |
| `src/metrics/evolution.rs` | Modify | `compute_evolution()` accepts `EvolutionThresholds` |
| `src/metrics/hygiene.rs` | Modify | `compute_hygiene()` accepts `HygieneThresholds` |
| `Cargo.toml` | Modify | Add `toml = "0.8"` dependency |

---

## Chunk 1: Config Struct, Loading, and Validation

### Task 1: Add `toml` dependency

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add toml dependency**

In `Cargo.toml` under `[dependencies]`, add:
```toml
toml = "0.8"
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: OK

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "deps: add toml crate for config file parsing"
```

---

### Task 2: Create `src/config.rs` with `RepoConfig` struct and defaults

**Files:**
- Create: `src/config.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write tests for RepoConfig defaults and TOML loading**

Create `src/config.rs` with the struct, `Default` impl, and test module:

```rust
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::Path;

use crate::cache::storage::CACHE_DIR;

const CONFIG_FILE: &str = "barad-dur.toml";

/// Output format for the report.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Cli,
    Html,
    Json,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Cli
    }
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

fn default_health_weight() -> u32 { 30 }
fn default_team_weight() -> u32 { 30 }
fn default_evolution_weight() -> u32 { 20 }
fn default_hygiene_weight() -> u32 { 20 }

impl Default for CategoryWeights {
    fn default() -> Self {
        Self { health: 30, team: 30, evolution: 20, hygiene: 20 }
    }
}

impl CategoryWeights {
    pub fn sum(&self) -> u32 {
        self.health + self.team + self.evolution + self.hygiene
    }

    /// Convert to the (name, f64) slice format used by scorer.
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

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HealthThresholds {
    #[serde(default = "default_max_complexity")]
    pub max_complexity: u32,
    #[serde(default = "default_hotspot_top_n")]
    pub hotspot_top_n: usize,
    #[serde(default = "default_coupling_min_commits")]
    pub coupling_min_commits: usize,
    #[serde(default = "default_bus_factor_warning")]
    pub bus_factor_warning: usize,
}

fn default_max_complexity() -> u32 { 20 }
fn default_hotspot_top_n() -> usize { 10 }
fn default_coupling_min_commits() -> usize { 5 }
fn default_bus_factor_warning() -> usize { 2 }

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TeamThresholds {
    #[serde(default = "default_silo_max_owners")]
    pub silo_max_owners: usize,
    #[serde(default = "default_activity_window_days")]
    pub activity_window_days: u32,
}

fn default_silo_max_owners() -> usize { 1 }
fn default_activity_window_days() -> u32 { 30 }

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EvolutionThresholds {
    #[serde(default = "default_growth_baseline_months")]
    pub growth_baseline_months: u32,
    #[serde(default = "default_refactor_ratio_target")]
    pub refactor_ratio_target: f64,
}

fn default_growth_baseline_months() -> u32 { 3 }
fn default_refactor_ratio_target() -> f64 { 0.1 }

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HygieneThresholds {
    #[serde(default = "default_min_message_length")]
    pub min_message_length: usize,
    #[serde(default = "default_max_message_length")]
    pub max_message_length: usize,
}

fn default_min_message_length() -> usize { 10 }
fn default_max_message_length() -> usize { 72 }

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

fn default_true() -> bool { true }

impl Default for TomlExclude {
    fn default() -> Self {
        Self { use_defaults: true, patterns: Vec::new() }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct TomlOutput {
    #[serde(default)]
    format: OutputFormat,
    #[serde(default)]
    auto_open: bool,
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
    })
}

fn warn_unknown_keys(content: &str, path: &Path) {
    let known_sections = ["analysis", "exclude", "weights", "thresholds", "output"];
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

/// Validate the merged config. Returns error messages for invalid values.
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
        assert_eq!(cfg.weights.health, 30);
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
        assert_eq!(cfg.weights.health, 30); // default preserved
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
            "[thresholds.health]\nmax_complexity = 30\nbus_factor_warning = 3\n",
        )
        .unwrap();
        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.thresholds.health.max_complexity, 30);
        assert_eq!(cfg.thresholds.health.bus_factor_warning, 3);
        // Unset thresholds get defaults
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
}
```

- [ ] **Step 2: Register module in lib.rs**

Add `pub mod config;` to `src/lib.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo test --lib config`
Expected: all 9 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/config.rs src/lib.rs
git commit -m "feat(config): add RepoConfig struct with TOML loading and validation"
```

---

### Task 3: Add `merge_with_cli()` and update `AnalyzeArgs`

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/config.rs`

- [ ] **Step 1: Write merge tests in config.rs**

Add these tests to the `config::tests` module:

```rust
#[test]
fn merge_cli_since_overrides_toml() {
    let config = RepoConfig {
        since: Some("6months".into()),
        ..Default::default()
    };
    // Simulating CLI with explicit --since
    let merged = merge_since(config.since, Some("3months".into()));
    assert_eq!(merged, Some("3months".to_string()));
}

#[test]
fn merge_cli_since_none_keeps_toml() {
    let config = RepoConfig {
        since: Some("6months".into()),
        ..Default::default()
    };
    let merged = merge_since(config.since, None);
    assert_eq!(merged, Some("6months".to_string()));
}

#[test]
fn merge_exclude_appends() {
    let config = RepoConfig {
        exclude_patterns: vec!["*.resx".into()],
        ..Default::default()
    };
    let cli_patterns = vec!["**/vendor/**".into()];
    let merged = merge_exclude_patterns(config.exclude_patterns, &cli_patterns);
    assert_eq!(merged, vec!["*.resx", "**/vendor/**"]);
}

#[test]
fn merge_skip_blame_cli_overrides() {
    // CLI explicitly passed --skip-blame → Some(true)
    let merged = merge_bool(false, Some(true));
    assert!(merged);
}

#[test]
fn merge_skip_blame_cli_absent_keeps_toml() {
    // CLI didn't pass flag → None, TOML says true
    let merged = merge_bool(true, None);
    assert!(merged);
}
```

- [ ] **Step 2: Implement merge helpers in config.rs**

```rust
/// Merge a single Option<String> field: CLI wins if Some.
pub fn merge_since(toml_val: Option<String>, cli_val: Option<String>) -> Option<String> {
    cli_val.or(toml_val)
}

/// Merge bool: CLI wins if Some, otherwise TOML value.
pub fn merge_bool(toml_val: bool, cli_val: Option<bool>) -> bool {
    cli_val.unwrap_or(toml_val)
}

/// Merge exclude patterns: append CLI patterns to TOML patterns.
pub fn merge_exclude_patterns(mut toml_patterns: Vec<String>, cli_patterns: &[String]) -> Vec<String> {
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
        weights: config.weights,       // No CLI override for weights
        thresholds: config.thresholds, // No CLI override for thresholds
        output_format: if args.json {
            OutputFormat::Json
        } else if args.html {
            OutputFormat::Html
        } else {
            config.output_format
        },
        auto_open: if args.open { true } else { config.auto_open },
    }
}
```

- [ ] **Step 3: Modify `AnalyzeArgs` in cli.rs**

Change `skip_blame: bool` and `no_default_excludes: bool` to `Option<bool>`:

```rust
// Before:
//   #[arg(long, help_heading = "Performance")]
//   pub skip_blame: bool,
//   #[arg(long, help_heading = "Filtering")]
//   pub no_default_excludes: bool,

// After:
/// Skip git blame (the slowest phase) for a faster partial analysis
#[arg(long, help_heading = "Performance")]
pub skip_blame: Option<bool>,

/// Disable built-in exclusion of translation/resource files
#[arg(long, help_heading = "Filtering")]
pub no_default_excludes: Option<bool>,
```

Update all call sites in `main.rs` that reference `args.skip_blame` and `args.no_default_excludes` to use `.unwrap_or(false)` until Task 5 replaces them with the merged config.

Also update `cli.rs` tests that assert `assert!(!args.skip_blame)` etc. to check `assert!(args.skip_blame.is_none())` and `assert_eq!(args.skip_blame, Some(true))`.

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add src/config.rs src/cli.rs src/main.rs
git commit -m "feat(config): add merge_with_cli and Option<bool> CLI fields"
```

---

## Chunk 2: Wire Config Into Main + Scorer + Metrics

### Task 4: Wire config loading into `main.rs`

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Load config after path resolution, merge with CLI**

In `run_analyze()`, after `local_path` is resolved but before `build_time_window`:

```rust
// Load repo config (.repository-analysis/barad-dur.toml)
let config = barad_dur::config::load(&local_path)?;
let config = barad_dur::config::merge_with_cli(config, &args);
barad_dur::config::validate(&config)?;
```

- [ ] **Step 2: Replace direct `args` usage with merged config**

Replace these patterns in `run_analyze()`:

```rust
// Time window: use config.since instead of args.since
// (args.until and args.all still come from CLI only)
let time_window = build_time_window_from_config(&config, &args);

// Exclusions: use config values
let exclude_patterns = &config.exclude_patterns;
let use_default_excludes = config.exclude_use_defaults;

// Skip blame: use config value
// args.skip_blame -> config.skip_blame

// Output format: use config
// The --json/--html/--open flags already handled in merge_with_cli
```

Add a `build_time_window_from_config` function that uses `config.since` but still checks `args.all` and `args.until` from CLI:

```rust
fn build_time_window_from_config(
    config: &barad_dur::config::RepoConfig,
    args: &AnalyzeArgs,
) -> TimeWindow {
    if args.all {
        return TimeWindow::full_history();
    }
    let now = chrono::Utc::now();
    let since = config.since.as_ref().and_then(|s| parse_time_spec(s, now));
    let until = args.until.as_ref().and_then(|s| parse_time_spec(s, now));
    if since.is_some() || until.is_some() {
        TimeWindow {
            since,
            until: until.or(Some(now)),
            default_months: 0,
        }
    } else {
        TimeWindow::default()
    }
}
```

- [ ] **Step 3: Thread config into output decision**

Replace the output format logic to use `config.output_format` and `config.auto_open`:

```rust
let output = match config.output_format {
    barad_dur::config::OutputFormat::Json => {
        renderer::json::render(&report, args.pretty)?
    }
    barad_dur::config::OutputFormat::Html => {
        renderer::html::render(&report)?
    }
    barad_dur::config::OutputFormat::Cli => {
        renderer::cli::render(&report, args.verbose)
    }
};

if config.auto_open && matches!(config.output_format, barad_dur::config::OutputFormat::Html) {
    // open in browser logic (same as current --open)
}
```

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 5: Test manually**

Run: `cargo run -- analyze .`
Expected: same output as before (no config file present = defaults)

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat(config): wire config loading and merge into analyze command"
```

---

### Task 5: Thread configurable weights into scorer

**Files:**
- Modify: `src/scorer.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write test for configurable weights**

Add to `scorer::tests`:

```rust
#[test]
fn overall_score_custom_weights() {
    let categories = vec![
        make_category("Health", 100),
        make_category("Team", 0),
        make_category("Evolution", 0),
        make_category("Git Hygiene", 0),
    ];
    let weights = vec![
        ("Health", 1.0),
        ("Team", 0.0),
        ("Evolution", 0.0),
        ("Git Hygiene", 0.0),
    ];
    let score = compute_overall_score_with_weights(&categories, &weights);
    assert_eq!(score, 100);
}
```

- [ ] **Step 2: Add `compute_overall_score_with_weights` and update `build_report`**

```rust
// Keep the const WEIGHTS for backward compat in tests
pub fn compute_overall_score_with_weights(
    categories: &[CategoryResult],
    weights: &[(&str, f64)],
) -> u32 {
    if categories.is_empty() {
        return 0;
    }
    let mut weighted_sum = 0.0;
    let mut total_weight = 0.0;
    for cat in categories {
        let weight = weights
            .iter()
            .find(|(name, _)| *name == cat.name)
            .map(|(_, w)| *w)
            .unwrap_or(0.25);
        weighted_sum += cat.score as f64 * weight;
        total_weight += weight;
    }
    if total_weight > 0.0 {
        (weighted_sum / total_weight).round() as u32
    } else {
        0
    }
}

fn compute_overall_score(categories: &[CategoryResult]) -> u32 {
    compute_overall_score_with_weights(categories, WEIGHTS)
}
```

Update `build_report` to accept an optional weights parameter:

```rust
pub fn build_report(
    snapshot: &RepoSnapshot,
    categories: Vec<CategoryResult>,
    remote_meta: Option<RemoteMeta>,
    weights: &[(&str, f64)],
) -> AnalysisReport {
    let overall_score = compute_overall_score_with_weights(&categories, weights);
    // ... rest unchanged
}
```

- [ ] **Step 3: Update main.rs call site**

```rust
let weight_pairs = config.weights.as_weight_pairs();
let mut report = scorer::build_report(&snapshot, categories, remote_meta, &weight_pairs);
```

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: all pass (update existing `build_report` calls in tests to pass `WEIGHTS`)

- [ ] **Step 5: Commit**

```bash
git add src/scorer.rs src/main.rs
git commit -m "feat(config): thread configurable weights into scorer"
```

---

### Task 6: Thread thresholds into metric modules

**Files:**
- Modify: `src/metrics/health.rs`
- Modify: `src/metrics/team.rs`
- Modify: `src/metrics/evolution.rs`
- Modify: `src/metrics/hygiene.rs`
- Modify: `src/main.rs`

This task modifies each `compute_*` function to accept a thresholds struct. The approach is the same for all 4 modules:

1. Add a thresholds parameter to `compute_*`
2. Thread relevant threshold values to internal metric functions
3. Update call sites

- [ ] **Step 1: Update health.rs**

Change signature from:
```rust
pub fn compute_health(snapshot: &RepoSnapshot) -> CategoryResult
```
to:
```rust
pub fn compute_health(
    snapshot: &RepoSnapshot,
    thresholds: &crate::config::HealthThresholds,
) -> CategoryResult
```

Thread `thresholds.bus_factor_warning` into `bus_factor()`, `thresholds.max_complexity` into `file_complexity()`, `thresholds.coupling_min_commits` into `temporal_coupling()`, `thresholds.hotspot_top_n` into `churn_hotspots()`.

Update the existing tests to pass `HealthThresholds::default()`.

- [ ] **Step 2: Update team.rs**

Same pattern — add `TeamThresholds` parameter, thread `silo_max_owners` and `activity_window_days`.

- [ ] **Step 3: Update evolution.rs**

Add `EvolutionThresholds` parameter, thread `growth_baseline_months` and `refactor_ratio_target`.

- [ ] **Step 4: Update hygiene.rs**

Add `HygieneThresholds` parameter, thread `min_message_length` and `max_message_length`.

- [ ] **Step 5: Update main.rs call sites**

```rust
if args.should_run("health") {
    categories.push(health::compute_health(snapshot, &config.thresholds.health));
}
if args.should_run("team") {
    categories.push(team::compute_team(snapshot, &config.thresholds.team));
}
if args.should_run("evolution") {
    categories.push(evolution::compute_evolution(snapshot, &config.thresholds.evolution));
}
if args.should_run("hygiene") {
    categories.push(hygiene::compute_hygiene(snapshot, &config.thresholds.hygiene));
}
```

The `compute_selected_metrics` function needs to accept `&RepoConfig`:

```rust
fn compute_selected_metrics(
    snapshot: &RepoSnapshot,
    args: &AnalyzeArgs,
    config: &barad_dur::config::RepoConfig,
) -> Vec<CategoryResult>
```

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: all 132+ tests pass

- [ ] **Step 7: Commit**

```bash
git add src/metrics/ src/main.rs
git commit -m "feat(config): thread configurable thresholds into all metric modules"
```

---

## Chunk 3: `barad-dur init` Command

### Task 7: Add `Init` subcommand to CLI

**Files:**
- Modify: `src/cli.rs`

- [ ] **Step 1: Add InitArgs struct and Init variant**

```rust
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Analyze a git repository
    Analyze(AnalyzeArgs),
    /// Generate a .repository-analysis/barad-dur.toml configuration file
    Init(InitArgs),
}

#[derive(clap::Args, Debug)]
#[command(
    about = "Generate a .repository-analysis/barad-dur.toml config file with smart defaults",
    long_about = "Scans the repository to detect translation files, generated code, \
        vendored dependencies, and team patterns, then generates a commented config file \
        with recommended settings.\n\n\
        Use --interactive for a guided wizard that walks through each setting."
)]
pub struct InitArgs {
    /// Path to the git repository [default: .]
    #[arg(default_value = ".")]
    pub target: String,

    /// Run interactive wizard instead of auto-detecting
    #[arg(short, long)]
    pub interactive: bool,

    /// Overwrite existing config file
    #[arg(long)]
    pub force: bool,
}
```

- [ ] **Step 2: Add CLI test**

```rust
#[test]
fn init_subcommand() {
    let cli = Cli::parse_from(["barad-dur", "init"]);
    assert!(matches!(cli.command, Commands::Init(_)));
}

#[test]
fn init_interactive_flag() {
    let cli = Cli::parse_from(["barad-dur", "init", "-i"]);
    match cli.command {
        Commands::Init(args) => assert!(args.interactive),
        _ => panic!("expected Init"),
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib cli`
Expected: pass

- [ ] **Step 4: Commit**

```bash
git add src/cli.rs
git commit -m "feat(cli): add init subcommand with --interactive and --force flags"
```

---

### Task 8: Create `src/init.rs` — repo scanning and TOML generation

**Files:**
- Create: `src/init.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write tests for detection heuristics**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_translation_extensions() {
        let files = vec!["src/main.rs", "Resources/Strings.resx", "i18n/fr.po"];
        let patterns = detect_exclude_patterns(&files);
        assert!(patterns.iter().any(|p| p.contains("resx")));
        assert!(patterns.iter().any(|p| p.contains("po")));
    }

    #[test]
    fn detect_i18n_directories() {
        let files = vec!["src/assets/i18n/en.ts", "src/assets/i18n/fr.ts"];
        let patterns = detect_exclude_patterns(&files);
        assert!(patterns.iter().any(|p| p.contains("i18n")));
    }

    #[test]
    fn detect_generated_code() {
        let files = vec!["Models/Foo.generated.cs", "Views/Bar.designer.cs"];
        let patterns = detect_exclude_patterns(&files);
        assert!(patterns.iter().any(|p| p.contains("generated")));
    }

    #[test]
    fn detect_vendor_dirs() {
        let files = vec!["vendor/lib/foo.go", "node_modules/pkg/index.js"];
        let patterns = detect_exclude_patterns(&files);
        assert!(patterns.iter().any(|p| p.contains("vendor")));
        assert!(patterns.iter().any(|p| p.contains("node_modules")));
    }

    #[test]
    fn generate_toml_is_valid() {
        let scan = ScanResult::default();
        let toml_str = generate_toml(&scan);
        assert!(toml_str.contains("[analysis]"));
        assert!(toml_str.contains("[weights]"));
        assert!(toml_str.contains("since ="));
    }
}
```

- [ ] **Step 2: Implement scanning and generation**

```rust
use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::cache::storage::CACHE_DIR;

const CONFIG_FILE: &str = "barad-dur.toml";

/// Results of scanning the repo for smart defaults.
#[derive(Debug, Default)]
pub struct ScanResult {
    pub exclude_patterns: Vec<(String, usize)>, // (pattern, file count)
    pub total_files: usize,
    pub total_commits: usize,
    pub distinct_authors: usize,
    pub suggest_skip_blame: bool,
}

/// Detect exclude patterns from a list of file paths.
pub fn detect_exclude_patterns(file_paths: &[&str]) -> Vec<String> {
    // ... detection logic for translation files, generated code,
    // vendored deps, i18n directories
}

/// Scan a repository and return smart defaults.
pub fn scan_repo(repo_path: &Path) -> Result<ScanResult> {
    // Uses Collector to get files and commits, then runs detection
}

/// Generate the TOML config string from scan results.
pub fn generate_toml(scan: &ScanResult) -> String {
    // Builds the commented TOML string with detected values
}

/// Run the init command.
pub fn run_init(target: &Path, force: bool, interactive: bool) -> Result<()> {
    let config_path = target.join(CACHE_DIR).join(CONFIG_FILE);
    if config_path.exists() && !force {
        bail!(
            "Config already exists at {}. Use --force to overwrite.",
            config_path.display()
        );
    }

    let scan = scan_repo(target)?;

    let toml_content = if interactive && std::io::stdin().is_terminal() {
        run_wizard(&scan)?
    } else {
        if interactive {
            eprintln!("Warning: stdin is not a terminal, falling back to auto-detect mode.");
        }
        generate_toml(&scan)
    };

    std::fs::create_dir_all(target.join(CACHE_DIR))?;
    std::fs::write(&config_path, &toml_content)?;
    eprintln!("Config written to {}", config_path.display());
    Ok(())
}
```

- [ ] **Step 3: Register module in lib.rs**

Add `pub mod init;` to `src/lib.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib init`
Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add src/init.rs src/lib.rs
git commit -m "feat(init): add repo scanning heuristics and TOML generation"
```

---

### Task 9: Implement interactive wizard

**Files:**
- Modify: `src/init.rs`

- [ ] **Step 1: Implement prompt helper and wizard flow**

```rust
use std::io::{self, BufRead, IsTerminal, Write};

fn prompt(question: &str, default: &str) -> String {
    eprint!("     ? {} [{}]: ", question, default);
    io::stderr().flush().unwrap();
    let mut input = String::new();
    io::stdin().lock().read_line(&mut input).unwrap();
    let trimmed = input.trim();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}

fn prompt_yn(question: &str, default_yes: bool) -> bool {
    let hint = if default_yes { "Y/n" } else { "y/N" };
    let answer = prompt(question, hint);
    match answer.to_lowercase().as_str() {
        "y" | "yes" | "Y/n" => true,
        "n" | "no" | "y/N" => false,
        _ => default_yes,
    }
}

pub fn run_wizard(scan: &ScanResult) -> Result<String> {
    eprintln!("\n  barad-dur config wizard");
    eprintln!("  ───────────────────────\n");

    // Mode selection
    let mode = prompt("Configuration mode: [S]imple or [A]dvanced", "S");
    let advanced = mode.to_lowercase().starts_with('a');

    if advanced {
        run_advanced_wizard(scan)
    } else {
        run_simple_wizard(scan)
    }
}

fn run_simple_wizard(scan: &ScanResult) -> Result<String> {
    // Q1: Time window
    let since = prompt("Analysis window", "6months");

    // Q2: Exclusions
    let use_detected_excludes = if !scan.exclude_patterns.is_empty() {
        for (pattern, count) in &scan.exclude_patterns {
            eprintln!("       - {} ({} files)", pattern, count);
        }
        prompt_yn("Exclude these from analysis?", true)
    } else {
        false
    };

    // Q3: Output format
    let format = prompt("Default output format (cli/html/json)", "cli");

    // Build TOML with these values + defaults for everything else
    generate_toml_with_overrides(scan, &since, use_detected_excludes, &format, false)
}

fn run_advanced_wizard(scan: &ScanResult) -> Result<String> {
    // All 5 groups as described in the spec
    // ... (full implementation)
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test`
Expected: all pass

- [ ] **Step 3: Commit**

```bash
git add src/init.rs
git commit -m "feat(init): add interactive wizard with simple/advanced modes"
```

---

### Task 10: Wire `Init` command in main.rs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add Init match arm**

```rust
fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Analyze(args) => run_analyze(args)?,
        Commands::Init(args) => {
            let target = std::path::PathBuf::from(&args.target);
            barad_dur::init::run_init(&target, args.force, args.interactive)?;
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Run all tests**

Run: `cargo test`
Expected: all pass

- [ ] **Step 3: Test manually**

```bash
cargo run -- init .
# Should generate .repository-analysis/barad-dur.toml with detected patterns
cat .repository-analysis/barad-dur.toml

cargo run -- init .
# Should error: "Config already exists"

cargo run -- init . --force
# Should overwrite
```

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat(init): wire init subcommand into main"
```

---

## Chunk 4: Integration Testing and Polish

### Task 11: Integration test — config affects analysis

**Files:**
- Create or extend: `tests/integration.rs` (or existing integration test file)

- [ ] **Step 1: Write integration test**

```rust
#[test]
fn config_file_affects_analysis() {
    // 1. Create temp dir, init git repo
    // 2. Write .repository-analysis/barad-dur.toml with custom weights
    // 3. Run barad-dur analyze . --json
    // 4. Parse JSON, verify weights affected overall score
}
```

- [ ] **Step 2: Run full test suite**

Run: `cargo test`
Expected: all pass including integration

- [ ] **Step 3: Commit**

```bash
git add tests/
git commit -m "test: add integration test for config file affecting analysis"
```

---

### Task 12: End-to-end verification

- [ ] **Step 1: Test on current repo (no config file)**

```bash
cargo run -- analyze . -v
```
Expected: identical output to before (no config = defaults).

- [ ] **Step 2: Generate config and test**

```bash
cargo run -- init .
cargo run -- analyze . -v
```
Expected: output unchanged (generated config uses detected defaults).

- [ ] **Step 3: Modify config and verify effect**

Edit `.repository-analysis/barad-dur.toml` to set `health = 90, team = 10, evolution = 0, hygiene = 0`, then:

```bash
cargo run -- analyze . -v
```
Expected: overall score shifts toward Health category score.

- [ ] **Step 4: Test CLI override**

```bash
cargo run -- analyze . --since 1month -v
```
Expected: CLI `--since` overrides TOML `since`.

- [ ] **Step 5: Clean up and final commit**

```bash
rm .repository-analysis/barad-dur.toml  # Remove test config
git add -A
git commit -m "feat(config): complete config system implementation"
```
