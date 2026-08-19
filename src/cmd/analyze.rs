use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

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
            cli_exclude_patterns: &args.exclude,
            cli_exclude_extensions: &args.exclude_ext,
            use_default_excludes,
        },
    )?;

    // Check for empty data
    if snapshot.commits.is_empty() {
        eprintln!("Warning: No commits found in the specified time window.");
    }

    // Computed once, shared by the Health category's "God objects" metric
    // and by build_report's refactoring-action generator — avoids running
    // the O(files) god-object detection pass twice per analysis.
    let flagged_god_objects = metrics::health::god_object_files(&snapshot, &cfg.thresholds.health);

    // Compute selected metrics
    let t = std::time::Instant::now();
    let mut categories = compute_selected_metrics(&snapshot, &args, &cfg, &flagged_god_objects);
    if args.verbose > 0 {
        eprintln!("  Metrics: {}ms", t.elapsed().as_millis());
    }

    // Dependency analysis (opt-in via --deps, requires network on first run)
    let dep_reports = load_dep_reports(&args, &local_path, show_progress);

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
        &cfg.thresholds.coupling,
        &flagged_god_objects,
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
) -> Result<(
    Option<remote::clone::TempClone>,
    PathBuf,
    Option<RemoteMeta>,
)> {
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

fn load_dep_reports(
    args: &AnalyzeArgs,
    local_path: &Path,
    show_progress: bool,
) -> Vec<EcosystemReport> {
    if !args.deps {
        return vec![];
    }

    use crate::collector::deps::collect_locked_deps;
    use crate::registry;
    use crate::registry::cache as reg_cache;

    let locked = collect_locked_deps(local_path);
    if locked.is_empty() {
        return vec![];
    }

    let mut dep_cache = reg_cache::load(local_path);

    let (cached, uncached) = registry::partition_cached(&locked, &dep_cache);

    if show_progress {
        eprintln!("{}", deps_progress_start(cached.len(), uncached.len()));
    }

    let uncached_count = uncached.len();
    let counter = AtomicUsize::new(0);

    let dep_ages = registry::resolve_dep_ages(
        &locked,
        &uncached,
        &mut dep_cache,
        make_progress_fetch(
            &registry::fetch_dep_network,
            &counter,
            uncached_count,
            show_progress,
        ),
    );

    reg_cache::save(local_path, &dep_cache);

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
        config::OutputFormat::Json => {
            if args.pretty {
                renderer::json::render_pretty(report, json_trend)?
            } else {
                renderer::json::render(report, json_trend)?
            }
        }
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
    flagged_god_objects: &[(std::path::PathBuf, String)],
) -> Vec<CategoryResult> {
    use crate::metrics::{coupling, evolution, health, hygiene, team};

    let mut categories = Vec::new();

    if args.should_run("health") {
        categories.push(health::compute_health(
            snapshot,
            &cfg.thresholds.health,
            flagged_god_objects,
        ));
    }
    if args.should_run("team") {
        categories.push(team::compute_team(
            snapshot,
            &cfg.thresholds.team,
            &cfg.thresholds.coupling,
        ));
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

pub(crate) fn deps_progress_start(cached: usize, uncached: usize) -> String {
    if uncached == 0 {
        format!("Deps: {} packages (all cached)", cached)
    } else {
        format!(
            "Deps: {} cached, fetching {} from registry (timeout {}s/pkg)…",
            cached,
            uncached,
            crate::registry::client::TIMEOUT_SECS,
        )
    }
}

pub(crate) fn deps_progress_tick(n: usize, total: usize) -> String {
    format!("  [{}/{}] fetched", n, total)
}

/// Wraps `fetch_fn` so the progress counter increments for every attempt —
/// including failures — ensuring the [N/N] completion tick always fires.
pub(crate) fn make_progress_fetch<'a, F>(
    fetch_fn: &'a F,
    counter: &'a AtomicUsize,
    uncached_count: usize,
    show_progress: bool,
) -> impl Fn(&crate::collector::deps::LockedDep) -> Option<crate::registry::cache::CacheEntry> + Sync + 'a
where
    F: Fn(&crate::collector::deps::LockedDep) -> Option<crate::registry::cache::CacheEntry> + Sync,
{
    move |dep| {
        let result = fetch_fn(dep);
        if show_progress && uncached_count > 0 {
            let n = counter.fetch_add(1, Ordering::Relaxed) + 1;
            if n.is_multiple_of(10) || n == uncached_count {
                eprintln!("{}", deps_progress_tick(n, uncached_count));
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::deps::LockedDep;
    use crate::deps::{DepAge, DepTier, Ecosystem};
    use crate::registry::cache::CacheEntry;
    use chrono::{Duration, Utc};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn make_locked(name: &str) -> LockedDep {
        LockedDep {
            name: name.into(),
            version: "1.0.0".into(),
            ecosystem: Ecosystem::Cargo,
        }
    }

    fn make_entry() -> CacheEntry {
        CacheEntry {
            current_published: Some(Utc::now() - Duration::days(365)),
            latest_published: Some(Utc::now()),
            latest_version: Some("1.0.0".into()),
            vulnerabilities: vec![],
            cached_at: Utc::now(),
        }
    }

    #[test]
    fn progress_start_all_cached() {
        let msg = deps_progress_start(42, 0);
        assert!(msg.contains("42") && msg.contains("cached"), "{msg}");
        // "all cached" distinguishes this branch from the fetching branch,
        // catching the `== with !=` mutation that swaps them.
        assert!(
            msg.contains("all cached"),
            "expected 'all cached' in: {msg}"
        );
    }

    #[test]
    fn progress_start_some_uncached() {
        let msg = deps_progress_start(10, 30);
        assert!(msg.contains("10") && msg.contains("30"), "{msg}");
    }

    #[test]
    fn progress_start_embeds_timeout_constant() {
        let msg = deps_progress_start(0, 1);
        let expected = crate::registry::client::TIMEOUT_SECS.to_string();
        assert!(msg.contains(&expected), "{msg}");
    }

    #[test]
    fn progress_tick_shows_n_of_total() {
        let msg = deps_progress_tick(20, 100);
        assert!(msg.contains("20") && msg.contains("100"), "{msg}");
    }

    // The progress counter must increment for every dep attempt, including failures.
    // If it only increments on success, the [N/N] completion tick is never printed
    // when any dep fails to fetch (network error, timeout, unknown package).
    #[test]
    fn make_progress_fetch_increments_counter_on_failure() {
        let dep = make_locked("broken");
        let counter = AtomicUsize::new(0);
        let fetch_fn = |_: &LockedDep| -> Option<CacheEntry> { None };

        let wrapped = make_progress_fetch(&fetch_fn, &counter, 3, true);
        let result = wrapped(&dep);

        assert!(result.is_none());
        assert_eq!(
            counter.load(Ordering::Relaxed),
            1,
            "counter must increment even when fetch returns None"
        );
    }

    #[test]
    fn make_progress_fetch_increments_counter_on_success() {
        let dep = make_locked("ok");
        let counter = AtomicUsize::new(0);
        let fetch_fn = |_: &LockedDep| -> Option<CacheEntry> { Some(make_entry()) };

        let wrapped = make_progress_fetch(&fetch_fn, &counter, 3, true);
        let result = wrapped(&dep);

        assert!(result.is_some());
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    // When all uncached deps fail, counter still reaches uncached_count.
    #[test]
    fn make_progress_fetch_counter_reaches_total_despite_all_failures() {
        let counter = AtomicUsize::new(0);
        let uncached_count = 3;
        let fetch_fn = |_: &LockedDep| -> Option<CacheEntry> { None };

        for _ in 0..uncached_count {
            let dep = make_locked("fail");
            let wrapped = make_progress_fetch(&fetch_fn, &counter, uncached_count, true);
            wrapped(&dep);
        }

        assert_eq!(counter.load(Ordering::Relaxed), uncached_count);
    }

    fn make_dep_age(name: &str, ecosystem: Ecosystem, drift_years: f64) -> DepAge {
        DepAge {
            name: name.into(),
            ecosystem,
            current_version: "1.0.0".into(),
            drift_years,
            tier: DepTier::from_drift(drift_years),
            vulnerabilities: vec![],
        }
    }

    #[test]
    fn build_ecosystem_reports_single_dep_stats() {
        let dep = make_dep_age("foo", Ecosystem::Cargo, 3.0);
        let reports = build_ecosystem_reports(vec![dep]);
        assert_eq!(reports.len(), 1);
        let r = &reports[0];
        assert_eq!(r.total_deps, 1);
        assert!(
            (r.total_drift_years - 3.0).abs() < 0.001,
            "total_drift={}",
            r.total_drift_years
        );
        assert!(
            (r.mean_drift_years - 3.0).abs() < 0.001,
            "mean_drift={}",
            r.mean_drift_years
        );
    }

    #[test]
    fn build_ecosystem_reports_two_deps_mean_drift() {
        let deps = vec![
            make_dep_age("foo", Ecosystem::Cargo, 2.0),
            make_dep_age("bar", Ecosystem::Cargo, 4.0),
        ];
        let reports = build_ecosystem_reports(deps);
        assert_eq!(reports.len(), 1);
        let r = &reports[0];
        assert_eq!(r.total_deps, 2);
        assert!(
            (r.total_drift_years - 6.0).abs() < 0.001,
            "total_drift={}",
            r.total_drift_years
        );
        assert!(
            (r.mean_drift_years - 3.0).abs() < 0.001,
            "mean_drift={}",
            r.mean_drift_years
        );
    }

    #[test]
    fn build_ecosystem_reports_filters_critical_deps() {
        let deps = vec![
            make_dep_age("old", Ecosystem::Cargo, 6.0), // drift ≥ 5 → Critical
            make_dep_age("fresh", Ecosystem::Cargo, 0.1), // drift < 0.5 → Fresh
        ];
        let reports = build_ecosystem_reports(deps);
        assert_eq!(reports.len(), 1);
        let r = &reports[0];
        assert_eq!(r.total_deps, 2);
        assert_eq!(r.critical_deps.len(), 1);
        assert_eq!(r.critical_deps[0].name, "old");
    }

    #[test]
    fn build_ecosystem_reports_groups_by_ecosystem() {
        let deps = vec![
            make_dep_age("cargo-dep", Ecosystem::Cargo, 1.0),
            make_dep_age("npm-dep", Ecosystem::Npm, 2.0),
        ];
        let reports = build_ecosystem_reports(deps);
        assert_eq!(reports.len(), 2);
    }

    #[test]
    fn build_ecosystem_reports_empty_input_returns_empty() {
        let reports = build_ecosystem_reports(vec![]);
        assert!(reports.is_empty());
    }
}
