use anyhow::{bail, Result};
use clap::Parser;
use std::io::IsTerminal;
use std::path::PathBuf;

use barad_dur::cache;
use barad_dur::cli::{AnalyzeArgs, Cli, Commands};
use barad_dur::collector::Collector;
use barad_dur::config::{self, RepoConfig};
use barad_dur::metrics::{evolution, health, hygiene, team, CategoryResult};
use barad_dur::remote;
use barad_dur::renderer;
use barad_dur::scorer::{self, RemoteMeta};
use barad_dur::snapshot::{RepoSnapshot, TimeWindow};

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Analyze(args) => run_analyze(args)?,
    }
    Ok(())
}

fn run_analyze(args: AnalyzeArgs) -> Result<()> {
    if args.json && args.html {
        bail!("--json and --html are mutually exclusive");
    }

    // Resolve target: URL → clone to temp dir, otherwise treat as local path.
    // _temp_clone must stay alive until the end of the function so the dir
    // isn't deleted before we finish analysis.
    let _temp_clone: Option<remote::clone::TempClone>;
    let (local_path, remote_meta): (PathBuf, Option<RemoteMeta>) = if remote::is_url(&args.target) {
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
        _temp_clone = Some(clone);
        let meta = gh_meta.map(|m| RemoteMeta {
            url: args.target.clone(),
            stars: Some(m.stars),
            description: m.description,
            language: m.language,
            open_issues: Some(m.open_issues),
        });
        (path, meta)
    } else {
        _temp_clone = None;
        (PathBuf::from(&args.target), None)
    };

    // Load and merge config (.ncrunch/barad-dur.toml + CLI flags)
    let cfg = config::load(&local_path)?;
    let cfg = config::merge_with_cli(cfg, &args);
    config::validate(&cfg)?;

    let time_window = build_time_window_from_config(&cfg, &args);
    let collector = Collector::open(&local_path, time_window)?;

    // Warn about shallow clones
    if collector.is_shallow() {
        eprintln!("Warning: This is a shallow clone. Metrics may be incomplete.");
    }

    // Show progress whenever stderr is a terminal (progress goes to stderr,
    // so it never interferes with JSON/HTML output on stdout or -o file).
    let show_progress = std::io::stderr().is_terminal();

    // Cache logic
    let current_head = collector.head_commit_hash()?;
    let exclude_patterns = &cfg.exclude_patterns;
    let use_default_excludes = cfg.exclude_use_defaults;

    let snapshot = if args.no_cache {
        collect_and_cache(
            &collector,
            show_progress,
            args.verbose > 0,
            cfg.skip_blame,
            true,
            exclude_patterns,
            use_default_excludes,
        )?
    } else if let Some(cached) = cache::load(collector.repo_path())? {
        if !cache::is_stale(&cached, &current_head) {
            if args.verbose > 0 {
                eprintln!("Using cached snapshot.");
            }
            cached
        } else {
            if args.verbose > 0 {
                eprintln!("Cache stale, re-collecting...");
            }
            collect_and_cache(
                &collector,
                show_progress,
                args.verbose > 0,
                cfg.skip_blame,
                false,
                exclude_patterns,
                use_default_excludes,
            )?
        }
    } else if args.cache_only {
        bail!("No cache found. Run without --cache-only first.");
    } else {
        collect_and_cache(
            &collector,
            show_progress,
            args.verbose > 0,
            cfg.skip_blame,
            false,
            exclude_patterns,
            use_default_excludes,
        )?
    };

    // Check for empty data
    if snapshot.commits.is_empty() {
        eprintln!("Warning: No commits found in the specified time window.");
    }

    // Compute selected metrics
    let t = std::time::Instant::now();
    let categories = compute_selected_metrics(&snapshot, &args, &cfg);
    if args.verbose > 0 {
        eprintln!("  Metrics: {}ms", t.elapsed().as_millis());
    }

    // Score
    let t = std::time::Instant::now();
    let weight_pairs = cfg.weights.as_weight_pairs();
    let mut report = scorer::build_report(&snapshot, categories, remote_meta, &weight_pairs);
    if args.verbose > 0 {
        eprintln!("  Scoring: {}ms", t.elapsed().as_millis());
    }

    // Record history entry (deduplicated by HEAD)
    let history_entry = scorer::build_history_entry(&report, &current_head);
    if let Err(e) = cache::history::append_if_new_head(&history_entry, &local_path) {
        eprintln!("Warning: Failed to record history: {}", e);
    }

    // Load history for report (used by HTML Trends tab)
    report.history = cache::history::load_history(&local_path).unwrap_or_default();

    // Render
    let t = std::time::Instant::now();
    let is_html = matches!(cfg.output_format, config::OutputFormat::Html);
    let output = match cfg.output_format {
        config::OutputFormat::Json => renderer::json::render(&report, args.pretty)?,
        config::OutputFormat::Html => renderer::html::render(&report)?,
        config::OutputFormat::Cli => renderer::cli::render(&report, args.verbose),
    };
    if args.verbose > 0 {
        eprintln!("  Render: {}ms", t.elapsed().as_millis());
    }

    // Write output
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
        open_in_browser(&path)?;
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

fn open_in_browser(path: &std::path::Path) -> Result<()> {
    let path_str = path.to_string_lossy();

    #[cfg(target_os = "linux")]
    let result = std::process::Command::new("xdg-open")
        .arg(path_str.as_ref())
        .spawn();

    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open")
        .arg(path_str.as_ref())
        .spawn();

    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/C", "start", "", &path_str])
        .spawn();

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let result: std::io::Result<std::process::Child> = Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Cannot detect platform for browser open",
    ));

    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Warning: Could not open browser: {}", e);
            eprintln!("Report saved to: {}", path.display());
            Ok(())
        }
    }
}

fn build_time_window_from_config(cfg: &RepoConfig, args: &AnalyzeArgs) -> TimeWindow {
    if args.all {
        return TimeWindow::full_history();
    }

    let now = chrono::Utc::now();

    // config.since already has the merged value (TOML + CLI override)
    let since = cfg.since.as_ref().and_then(|s| parse_time_spec(s, now));
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

fn parse_time_spec(
    spec: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<chrono::DateTime<chrono::Utc>> {
    // Try relative format: "3months", "6months", "1year", "30days"
    if let Some(months) = spec
        .strip_suffix("months")
        .or_else(|| spec.strip_suffix("month"))
    {
        if let Ok(m) = months.trim().parse::<i64>() {
            return Some(now - chrono::Duration::days(m * 30));
        }
    }
    if let Some(days) = spec
        .strip_suffix("days")
        .or_else(|| spec.strip_suffix("day"))
    {
        if let Ok(d) = days.trim().parse::<i64>() {
            return Some(now - chrono::Duration::days(d));
        }
    }
    if let Some(years) = spec
        .strip_suffix("years")
        .or_else(|| spec.strip_suffix("year"))
    {
        if let Ok(y) = years.trim().parse::<i64>() {
            return Some(now - chrono::Duration::days(y * 365));
        }
    }

    // Try ISO date format: "2024-01-01"
    if let Ok(date) = chrono::NaiveDate::parse_from_str(spec, "%Y-%m-%d") {
        return Some(date.and_hms_opt(0, 0, 0).unwrap().and_utc());
    }

    eprintln!(
        "Warning: Could not parse time spec '{}', using default.",
        spec
    );
    None
}

fn collect_and_cache(
    collector: &Collector,
    show_progress: bool,
    verbose: bool,
    skip_blame: bool,
    no_cache: bool,
    exclude_patterns: &[String],
    use_default_excludes: bool,
) -> Result<RepoSnapshot> {
    let snapshot = collector.collect_snapshot_verbose(
        show_progress,
        verbose,
        skip_blame,
        no_cache,
        exclude_patterns,
        use_default_excludes,
    )?;
    if let Err(e) = cache::save(&snapshot, collector.repo_path()) {
        eprintln!("Warning: Failed to save cache: {}", e);
    }
    Ok(snapshot)
}

fn compute_selected_metrics(
    snapshot: &RepoSnapshot,
    args: &AnalyzeArgs,
    cfg: &RepoConfig,
) -> Vec<CategoryResult> {
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

    categories
}
