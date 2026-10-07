mod deps;

use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::analysis::{self, AnalysisInputs, CategorySelection};
use crate::cache;
use crate::cli::AnalyzeArgs;
use crate::config::{self, RepoConfig};
use crate::metrics;
use crate::remote;
use crate::renderer;
use crate::runner::{self, CollectOptions};
use crate::scorer::{self, AnalysisReport, RemoteMeta};
use crate::trend;

pub fn run_analyze(args: AnalyzeArgs) -> Result<()> {
    let reference_time = chrono::Utc::now();
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

    let time_window = runner::build_time_window_from_config(reference_time, &cfg, &args);
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

    // Computed once, shared by the Coupling category's reach-trend metric
    // and by build_report's hotspot rows (same pattern as above).
    let coupling_reach = metrics::coupling::growing_coupling_reach(
        &snapshot,
        cfg.thresholds.coupling.decay_min_partners,
    );

    // Dependency analysis (opt-in via --deps, requires network on first run)
    let dep_reports = deps::load_dep_reports(&args, &local_path, show_progress);

    // The CLI filters become an explicit selection here; the calculation
    // itself never sees arguments. Opting into dependencies also gives them
    // a weight — the command's policy, not the calculation's.
    let selection = CategorySelection::from_filters(
        args.health,
        args.team,
        args.evolution,
        args.hygiene,
        args.deps,
    );
    let mut cfg_weights = cfg.weights.clone();
    if args.deps {
        cfg_weights.deps = 20;
    }
    let weight_pairs = cfg_weights.as_weight_pairs();

    // Compute selected metrics
    let t = std::time::Instant::now();
    let analysis = analysis::calculate(&AnalysisInputs {
        reference_time,
        snapshot: &snapshot,
        selection,
        thresholds: &cfg.thresholds,
        weights: &weight_pairs,
        dependency_evidence: &dep_reports,
        god_objects: &flagged_god_objects,
        coupling_reach: &coupling_reach,
    });
    if args.verbose > 0 {
        eprintln!("  Metrics: {}ms", t.elapsed().as_millis());
    }

    // The history record needs only the result; build it before the
    // report takes ownership of the categories.
    let history_entry =
        scorer::build_history_entry(reference_time, &analysis, &snapshot, &current_head, None);

    // Enrich the result into the display report. Timed under the
    // historical "Scoring" label, which `-v` consumers pin.
    let t = std::time::Instant::now();
    let mut report = scorer::build_report(
        reference_time,
        &snapshot,
        analysis,
        remote_meta,
        &cfg.thresholds,
        &flagged_god_objects,
        &coupling_reach,
    );
    report.dep_ecosystem_reports = dep_reports;
    if args.verbose > 0 {
        eprintln!("  Scoring: {}ms", t.elapsed().as_millis());
    }

    // An Err here is a genuine I/O failure (unreadable cache dir, a failed
    // archive rename), distinct from "no history yet", which comes back as an
    // empty Vec. Degrading both to silence hid the reason every direction was
    // missing, so say so and carry on without trends.
    let entity_input_fingerprint = cache::entity_history::entity_history_input_fingerprint(
        &local_path,
        cfg.backfill.sample_count,
        cfg.backfill.entity_trend_top_n,
        &cfg.thresholds.coupling,
        cfg.exclude_use_defaults,
    );
    let (entity_history, entity_history_warning) =
        match cache::entity_history::load_entity_history_checked(
            &local_path,
            entity_input_fingerprint,
        ) {
            Ok(loaded) => loaded,
            Err(e) => {
                eprintln!("Warning: could not read entity trend history: {e}");
                Default::default()
            }
        };
    if let Some(ref warning) = entity_history_warning {
        eprintln!("{}", warning);
    }
    cache::entity_history::attach_entity_trends(
        &mut report.file_hotspots,
        &mut report.coupling_pairs,
        &entity_history,
    );

    let trend_summary = compute_trend_and_update_history(&mut report, &local_path, history_entry);

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

/// Load prior history, compute the trend summary, append the current entry,
/// and populate `report.history` for the HTML Trends tab.
pub fn compute_trend_and_update_history(
    report: &mut AnalysisReport,
    local_path: &Path,
    history_entry: scorer::HistoryEntry,
) -> trend::TrendSummary {
    // Load BEFORE appending so compute_trend sees only prior runs.
    // On corruption, archive the file and start fresh.
    let (prior_history, history_warning) =
        cache::history::load_history_checked(local_path).unwrap_or_default();
    // Stays on stdout: AC-01.4 (`tests/trend_milestone_1.rs`) specifies this
    // warning as stdout output. That makes `analyze --json` emit unparseable
    // output when trends.json is corrupt — a real but pre-existing conflict
    // between that acceptance criterion and the stdout-purity convention,
    // and a spec decision rather than something to change in passing.
    if let Some(ref warning) = history_warning {
        println!("{}", warning);
    }

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
