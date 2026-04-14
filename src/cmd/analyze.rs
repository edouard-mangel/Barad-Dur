use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::cache;
use crate::cli::AnalyzeArgs;
use crate::config::{self, RepoConfig};
use crate::deps::{DepAge, EcosystemReport};
use crate::metrics::{self, CategoryResult};
use crate::remote;
use crate::renderer;
use crate::runner::{self, CollectOptions};
use crate::scorer::{self, AnalysisReport, RemoteMeta};
use crate::snapshot::RepoSnapshot;
use crate::trend;

pub fn run_analyze(args: AnalyzeArgs) -> Result<()> {
    use anyhow::bail;
    use std::io::IsTerminal;

    use crate::collector::Collector;

    if args.json && args.html {
        bail!("--json and --html are mutually exclusive");
    }

    // Resolve target: URL → clone to temp dir, otherwise treat as local path.
    // _temp_clone must stay alive until the end of the function so the dir
    // isn't deleted before we finish analysis.
    let (_temp_clone, local_path, remote_meta) = resolve_remote_target(&args)?;

    // Load and merge config (.repository-analysis/barad-dur.toml + CLI flags)
    let cfg = config::load(&local_path)?;
    let cfg = config::merge_with_cli(cfg, &args);
    config::validate(&cfg)?;

    let time_window = runner::build_time_window_from_config(&cfg, &args);
    let collector = Collector::open(&local_path, time_window)?;

    // Warn about shallow clones
    if collector.is_shallow() {
        eprintln!("Warning: This is a shallow clone. Metrics may be incomplete.");
    }

    // Show progress whenever stderr is a terminal (progress goes to stderr,
    // so it never interferes with JSON/HTML output on stdout or -o file).
    let show_progress = std::io::stderr().is_terminal();

    let current_head = collector.head_commit_hash()?;
    let exclude_patterns = &cfg.exclude_patterns;
    let use_default_excludes = cfg.exclude_use_defaults;

    let snapshot = runner::resolve_snapshot(
        &collector,
        &current_head,
        &CollectOptions {
            show_progress,
            verbose: args.verbose > 0,
            skip_blame: cfg.skip_blame,
            no_cache: args.no_cache,
            cache_only: args.cache_only,
            exclude_patterns,
            use_default_excludes,
        },
    )?;

    // Check for empty data
    if snapshot.commits.is_empty() {
        eprintln!("Warning: No commits found in the specified time window.");
    }

    // Compute selected metrics
    let t = std::time::Instant::now();
    let mut categories = compute_selected_metrics(&snapshot, &args, &cfg);
    if args.verbose > 0 {
        eprintln!("  Metrics: {}ms", t.elapsed().as_millis());
    }

    // Dependency analysis (opt-in via --deps, requires network on first run)
    let dep_reports = load_dep_reports(&args, &local_path);

    if args.deps && !dep_reports.is_empty() {
        categories.push(metrics::deps::compute_deps(&dep_reports));
    }

    // Score
    let t = std::time::Instant::now();
    let mut cfg_weights = cfg.weights.clone();
    if args.deps {
        cfg_weights.deps = 20;
    }
    let weight_pairs = cfg_weights.as_weight_pairs();
    let mut report = scorer::build_report(
        &snapshot,
        categories,
        remote_meta,
        &weight_pairs,
        cfg.thresholds.coupling.component_depth,
    );
    report.dep_ecosystem_reports = dep_reports;
    if args.verbose > 0 {
        eprintln!("  Scoring: {}ms", t.elapsed().as_millis());
    }

    let trend_summary = compute_trend_and_update_history(&mut report, &local_path, &current_head);

    render_and_write(&report, &args, &cfg, &trend_summary, &local_path)?;

    Ok(())
}

fn resolve_remote_target(
    args: &AnalyzeArgs,
) -> Result<(Option<remote::clone::TempClone>, PathBuf, Option<RemoteMeta>)> {
    if remote::is_url(&args.target) {
        let clone = remote::clone::clone_remote(&args.target)?;
        let gh_meta = args.token.as_deref().and_then(|t| {
            if remote::github::is_github_url(&args.target) {
                match remote::github::fetch_meta(&args.target, t) {
                    Ok(m) => Some(m),
                    Err(e) => {
                        eprintln!("Warning: GitHub API error: {}", e);
                        None
                    }
                }
            } else {
                None
            }
        });
        let path = clone.path.clone();
        let meta = gh_meta.map(|m| RemoteMeta {
            url: args.target.clone(),
            stars: Some(m.stars),
            description: m.description,
            language: m.language,
            open_issues: Some(m.open_issues),
        });
        Ok((Some(clone), path, meta))
    } else {
        Ok((None, PathBuf::from(&args.target), None))
    }
}

fn load_dep_reports(args: &AnalyzeArgs, local_path: &Path) -> Vec<EcosystemReport> {
    if !args.deps {
        return vec![];
    }

    use crate::collector::deps::collect_locked_deps;
    use crate::registry;
    use crate::registry::cache as reg_cache;

    let locked = collect_locked_deps(local_path);
    let mut dep_cache = reg_cache::load(local_path);
    let dep_ages: Vec<DepAge> = locked
        .iter()
        .filter_map(|dep| registry::fetch_dep(dep, &mut dep_cache, local_path))
        .collect();
    build_ecosystem_reports(dep_ages)
}

/// Load prior history, compute the trend summary, append the current entry,
/// and populate `report.history` for the HTML Trends tab.
pub fn compute_trend_and_update_history(
    report: &mut AnalysisReport,
    local_path: &Path,
    current_head: &str,
) -> trend::TrendSummary {
    // Load BEFORE appending so compute_trend sees only prior runs.
    // On corruption, archive the file and start fresh.
    let (prior_history, history_warning) =
        cache::history::load_history_checked(local_path).unwrap_or_default();
    if let Some(ref warning) = history_warning {
        println!("{}", warning);
    }

    let history_entry = scorer::build_history_entry(report, current_head, None);
    let trend_summary = trend::compute_trend(&prior_history, &report.branch, &history_entry);

    if let Err(e) = cache::history::append_if_new_head(&history_entry, local_path) {
        eprintln!("Warning: Failed to record history: {}", e);
    }

    report.history = cache::history::load_history(local_path).unwrap_or_default();
    trend_summary
}

/// Render the report to a string and write it to stdout, a file, or open it in a browser.
pub fn render_and_write(
    report: &AnalysisReport,
    args: &AnalyzeArgs,
    cfg: &RepoConfig,
    trend_summary: &trend::TrendSummary,
    local_path: &Path,
) -> Result<()> {
    let t = std::time::Instant::now();
    let is_html = matches!(cfg.output_format, config::OutputFormat::Html);
    let json_trend = if args.trend {
        Some(trend_summary)
    } else {
        None
    };
    let output = match cfg.output_format {
        config::OutputFormat::Json => renderer::json::render(report, args.pretty, json_trend)?,
        config::OutputFormat::Html => renderer::html::render(report)?,
        config::OutputFormat::Cli => {
            renderer::cli::render(report, args.verbose, Some(trend_summary))
        }
    };
    if args.verbose > 0 {
        eprintln!("  Render: {}ms", t.elapsed().as_millis());
    }

    let should_open = cfg.auto_open && is_html;
    if should_open {
        let path = if let Some(ref p) = args.output {
            std::fs::write(p, &output)?;
            p.clone()
        } else {
            let dir = local_path.join(cache::CACHE_DIR);
            std::fs::create_dir_all(&dir)?;
            let path = dir.join("report.html");
            std::fs::write(&path, &output)?;
            path
        };
        eprintln!("Opening {}", path.display());
        runner::open_in_browser(&path)?;
    } else if let Some(path) = &args.output {
        std::fs::write(path, &output)?;
        if matches!(cfg.output_format, config::OutputFormat::Cli) {
            eprintln!("Report written to {}", path.display());
        }
    } else if is_html {
        let dir = local_path.join(cache::CACHE_DIR);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("report.html");
        std::fs::write(&path, &output)?;
        eprintln!("Report written to {}", path.display());
    } else {
        print!("{}", output);
    }

    Ok(())
}

pub fn compute_selected_metrics(
    snapshot: &RepoSnapshot,
    args: &AnalyzeArgs,
    cfg: &RepoConfig,
) -> Vec<CategoryResult> {
    use crate::metrics::{coupling, evolution, health, hygiene, team};

    let mut categories = Vec::new();

    if args.should_run("health") {
        categories.push(health::compute_health(snapshot, &cfg.thresholds.health));
    }
    if args.should_run("team") {
        categories.push(team::compute_team(snapshot, &cfg.thresholds.team));
    }
    if args.should_run("evolution") {
        categories.push(evolution::compute_evolution(
            snapshot,
            &cfg.thresholds.evolution,
        ));
    }
    if args.should_run("hygiene") {
        categories.push(hygiene::compute_hygiene(snapshot, &cfg.thresholds.hygiene));
    }
    if args.should_run("coupling") {
        categories.push(coupling::compute_coupling(
            snapshot,
            &cfg.thresholds.coupling,
        ));
    }

    categories
}

pub fn build_ecosystem_reports(dep_ages: Vec<DepAge>) -> Vec<EcosystemReport> {
    use std::collections::HashMap;

    let mut by_ecosystem: HashMap<String, Vec<DepAge>> = HashMap::new();
    for dep in dep_ages {
        by_ecosystem
            .entry(dep.ecosystem.display_name().to_string())
            .or_default()
            .push(dep);
    }

    by_ecosystem
        .into_values()
        .map(|deps| {
            let total = deps.len();
            let total_drift: f64 = deps.iter().map(|d| d.drift_years).sum();
            let mean_drift = if total > 0 {
                total_drift / total as f64
            } else {
                0.0
            };
            let critical_deps: Vec<DepAge> = deps
                .iter()
                .filter(|d| d.is_critical_callout())
                .cloned()
                .collect();
            let ecosystem = deps[0].ecosystem.clone();
            EcosystemReport {
                ecosystem,
                total_deps: total,
                mean_drift_years: mean_drift,
                total_drift_years: total_drift,
                critical_deps,
            }
        })
        .collect()
}
