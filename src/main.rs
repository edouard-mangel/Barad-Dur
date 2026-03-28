use anyhow::{bail, Result};
use clap::Parser;
use std::io::IsTerminal;
use std::path::PathBuf;

use barad_dur::backfill;
use barad_dur::cache;
use barad_dur::cli::{AnalyzeArgs, Cli, Commands, CouplingArgs, GateArgs};
use barad_dur::collector::Collector;
use barad_dur::config::{self, RepoConfig};
use barad_dur::metrics::{coupling, evolution, health, hygiene, team, CategoryResult};
use barad_dur::remote;
use barad_dur::renderer;
use barad_dur::scorer::{self, RemoteMeta};
use barad_dur::snapshot::{RepoSnapshot, TimeWindow};
use barad_dur::trend;

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Analyze(args) => run_analyze(args)?,
        Commands::Backfill(args) => {
            let repo_path = std::path::PathBuf::from(&args.target);
            backfill::run(&args, &repo_path)?;
        }
        Commands::Init(args) => {
            let target = std::path::PathBuf::from(&args.target);
            barad_dur::init::run_init(&target, args.force, args.interactive)?;
        }
        Commands::Gate(args) => {
            std::process::exit(run_gate(args)?);
        }
        Commands::Coupling(args) => run_coupling(args)?,
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

    // Load and merge config (.repository-analysis/barad-dur.toml + CLI flags)
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

    // Load history BEFORE appending the current entry so compute_trend sees
    // only prior runs (the current entry is passed separately).
    // If the file exists but is fully unparseable (corruption), archive it and
    // warn the user, then start fresh.
    let (prior_history, history_warning) =
        cache::history::load_history_checked(&local_path).unwrap_or_default();
    if let Some(ref warning) = history_warning {
        println!("{}", warning);
    }

    // Build the current history entry (not yet appended).
    let history_entry = scorer::build_history_entry(&report, &current_head, None);

    // Compute trend from prior history vs current entry.
    let trend_summary = trend::compute_trend(&prior_history, &report.branch, &history_entry);

    // Record history entry (deduplicated by HEAD).
    if let Err(e) = cache::history::append_if_new_head(&history_entry, &local_path) {
        eprintln!("Warning: Failed to record history: {}", e);
    }

    // Load history for report (used by HTML Trends tab).
    report.history = cache::history::load_history(&local_path).unwrap_or_default();

    // Render
    let t = std::time::Instant::now();
    let is_html = matches!(cfg.output_format, config::OutputFormat::Html);
    let json_trend = if args.trend {
        Some(&trend_summary)
    } else {
        None
    };
    let output = match cfg.output_format {
        config::OutputFormat::Json => renderer::json::render(&report, args.pretty, json_trend)?,
        config::OutputFormat::Html => renderer::html::render(&report)?,
        config::OutputFormat::Cli => {
            renderer::cli::render(&report, args.verbose, Some(&trend_summary))
        }
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

fn run_gate(args: GateArgs) -> Result<i32> {
    let local_path = PathBuf::from(&args.target);
    let cfg = config::load(&local_path)?;
    let skip_blame = args.skip_blame.unwrap_or(cfg.skip_blame);

    let time_window = TimeWindow::default();
    let collector = Collector::open(&local_path, time_window)?;

    let exclude_patterns = &cfg.exclude_patterns;
    let use_default_excludes = cfg.exclude_use_defaults;

    let snapshot = if let Some(cached) = cache::load(collector.repo_path())? {
        let current_head = collector.head_commit_hash()?;
        if !cache::is_stale(&cached, &current_head) {
            cached
        } else {
            collect_and_cache(
                &collector,
                false,
                false,
                skip_blame,
                false,
                exclude_patterns,
                use_default_excludes,
            )?
        }
    } else {
        collect_and_cache(
            &collector,
            false,
            false,
            skip_blame,
            false,
            exclude_patterns,
            use_default_excludes,
        )?
    };

    let categories = vec![
        health::compute_health(&snapshot, &cfg.thresholds.health),
        team::compute_team(&snapshot, &cfg.thresholds.team),
        evolution::compute_evolution(&snapshot, &cfg.thresholds.evolution),
        hygiene::compute_hygiene(&snapshot, &cfg.thresholds.hygiene),
        coupling::compute_coupling(&snapshot),
    ];

    let weight_pairs = cfg.weights.as_weight_pairs();
    let report = scorer::build_report(&snapshot, categories, None, &weight_pairs);

    let threshold = args.min_score;
    let mut failed = false;

    if args.category.is_empty() {
        // Check overall score
        if report.overall_score < threshold {
            println!(
                "FAIL: overall score {} < threshold {}",
                report.overall_score, threshold
            );
            failed = true;
        } else {
            println!(
                "PASS: overall score {} >= threshold {}",
                report.overall_score, threshold
            );
        }
    } else {
        // Check each requested category
        for cat_name in &args.category {
            let cat_lower = cat_name.to_lowercase();
            if let Some(cat) = report.categories.iter().find(|c| {
                let name_lower = c.name.to_lowercase();
                name_lower == cat_lower || name_lower.contains(&cat_lower)
            }) {
                if cat.score < threshold {
                    println!(
                        "FAIL: {} score {} < threshold {}",
                        cat.name, cat.score, threshold
                    );
                    failed = true;
                } else {
                    println!(
                        "PASS: {} score {} >= threshold {}",
                        cat.name, cat.score, threshold
                    );
                }
            } else {
                println!("WARN: unknown category '{}', skipping", cat_name);
            }
        }
    }

    Ok(if failed { 1 } else { 0 })
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

fn run_coupling(args: CouplingArgs) -> Result<()> {
    use barad_dur::coupling::collector::collect_snapshots;
    use barad_dur::coupling::dependency::analyze_dependency_coupling;
    use barad_dur::coupling::discovery::discover_repos;
    use barad_dur::coupling::scorer::score_coupling_pairs;
    use barad_dur::coupling::team::analyze_team_coupling;
    use barad_dur::coupling::temporal::analyze_temporal_coupling;
    use barad_dur::coupling::{CouplingReport, CouplingReportSummary, RepoInfo};
    use barad_dur::renderer::coupling_cli::render_coupling_table;
    use barad_dur::renderer::coupling_json::render_coupling_json;
    use indicatif::{ProgressBar, ProgressStyle};

    let is_tty = std::io::stderr().is_terminal();

    let make_spinner = |msg: &str| -> ProgressBar {
        if !is_tty {
            return ProgressBar::hidden();
        }
        let sp = ProgressBar::new_spinner();
        sp.set_style(
            ProgressStyle::default_spinner()
                .template("  {spinner:.cyan} {msg}")
                .unwrap(),
        );
        sp.set_message(msg.to_string());
        sp.enable_steady_tick(std::time::Duration::from_millis(80));
        sp
    };

    // Step 1: Discover repos under root directory
    let sp = make_spinner("Discovering repositories...");
    let discovery = discover_repos(&args.root_dir);
    sp.finish_with_message(format!(
        "Discovered {} repos (skipped {})",
        discovery.discovered.len(),
        discovery.skipped.len()
    ));

    if discovery.discovered.len() < 2 {
        eprintln!(
            "Found {} repos under {}. Need at least 2 for coupling analysis.",
            discovery.discovered.len(),
            args.root_dir.display()
        );
        return Ok(());
    }

    // Step 2: Collect snapshots (skip-blame, parallel)
    let sp = make_spinner(&format!(
        "Collecting snapshots from {} repos...",
        discovery.discovered.len()
    ));
    let config = barad_dur::coupling::CouplingConfig {
        root_dir: args.root_dir.clone(),
        ..Default::default()
    };
    let collection = collect_snapshots(&discovery.discovered, &config);
    sp.finish_with_message(format!(
        "Collected {} snapshots ({} failed)",
        collection.snapshots.len(),
        collection.failed.len()
    ));

    // Step 3: Analyze all three coupling dimensions
    let sp = make_spinner("Analyzing temporal coupling...");
    let window = std::time::Duration::from_secs(24 * 60 * 60);
    let temporal_pairs = analyze_temporal_coupling(&collection.snapshots, window);
    sp.finish_with_message(format!("Temporal: {} coupled pairs", temporal_pairs.len()));

    let sp = make_spinner("Analyzing team coupling...");
    let team_pairs = analyze_team_coupling(&collection.snapshots);
    sp.finish_with_message(format!("Team: {} pairs", team_pairs.len()));

    let sp = make_spinner("Analyzing dependency coupling...");
    let repo_paths: Vec<(String, std::path::PathBuf)> = collection
        .snapshots
        .iter()
        .map(|(name, snap)| (name.clone(), snap.path.clone()))
        .collect();
    let dep_analysis = analyze_dependency_coupling(&repo_paths);
    sp.finish_with_message(format!("Dependencies: {} pairs", dep_analysis.pairs.len()));

    // Step 4: Combine scores from all dimensions
    let sp = make_spinner("Computing combined scores...");
    let combined_pairs = score_coupling_pairs(&temporal_pairs, &team_pairs, &dep_analysis);
    sp.finish_with_message(format!("Scored {} pairs", combined_pairs.len()));

    // Step 5: Render output
    let use_html = args.html || args.open;
    if args.json || use_html {
        let repos: Vec<RepoInfo> = collection
            .snapshots
            .iter()
            .map(|(name, snap)| RepoInfo {
                name: name.clone(),
                path: snap.path.clone(),
                commit_count: snap.commits.len(),
                author_count: snap.authors.len(),
            })
            .collect();

        let highest = combined_pairs
            .first()
            .map(|p| p.combined_score)
            .unwrap_or(0.0);

        let pairs_above = combined_pairs
            .iter()
            .filter(|p| p.combined_score >= args.min_score)
            .count();

        let report = CouplingReport {
            repos: repos.clone(),
            pairs: combined_pairs,
            summary: CouplingReportSummary {
                total_repos: repos.len(),
                total_pairs_analyzed: repos.len() * (repos.len().saturating_sub(1)) / 2,
                pairs_above_threshold: pairs_above,
                highest_coupling_score: highest,
            },
            blast_radius: dep_analysis.blast_radius,
        };

        if use_html {
            let output = renderer::coupling_html::render_coupling_html(&report);
            let path = if let Some(ref p) = args.output {
                std::fs::write(p, &output)?;
                p.clone()
            } else {
                let default_path = PathBuf::from("coupling-report.html");
                std::fs::write(&default_path, &output)?;
                default_path
            };
            eprintln!("Report written to {}", path.display());
            if args.open {
                open_in_browser(&path)?;
            }
        } else {
            let output = render_coupling_json(&report, args.pretty);
            if let Some(path) = &args.output {
                std::fs::write(path, &output)?;
                eprintln!("Report written to {}", path.display());
            } else {
                print!("{}", output);
            }
        }
    } else {
        let output = render_coupling_table(&temporal_pairs);
        if let Some(path) = &args.output {
            std::fs::write(path, &output)?;
            eprintln!("Report written to {}", path.display());
        } else {
            print!("{}", output);
        }
    }

    Ok(())
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
    if args.should_run("coupling") {
        categories.push(coupling::compute_coupling(snapshot));
    }

    categories
}
