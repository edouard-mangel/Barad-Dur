use anyhow::Result;
use std::path::PathBuf;

use crate::cache;
use crate::cli::GateArgs;
use crate::collector::Collector;
use crate::config;
use crate::metrics::{coupling, evolution, health, hygiene, team};
use crate::runner::{self, CollectOptions};
use crate::scorer::{self, AnalysisReport};
use crate::snapshot::TimeWindow;
use crate::trend::{self, VelocityDirection};

pub fn run_gate(args: GateArgs) -> Result<i32> {
    let local_path = PathBuf::from(&args.target);
    let cfg = config::load(&local_path)?;
    let skip_blame = args.skip_blame.unwrap_or(cfg.skip_blame);

    let time_window = TimeWindow::default();
    let collector = Collector::open(&local_path, time_window)?;

    let exclude_patterns = &cfg.exclude_patterns;
    let use_default_excludes = cfg.exclude_use_defaults;
    let current_head = collector.head_commit_hash()?;

    let snapshot = runner::resolve_snapshot(
        &collector,
        &current_head,
        &CollectOptions {
            show_progress: false,
            verbose: false,
            skip_blame,
            no_cache: false,
            cache_only: false,
            exclude_patterns,
            use_default_excludes,
        },
    )?;

    let categories = vec![
        health::compute_health(&snapshot, &cfg.thresholds.health),
        team::compute_team(&snapshot, &cfg.thresholds.team),
        evolution::compute_evolution(&snapshot, &cfg.thresholds.evolution),
        hygiene::compute_hygiene(&snapshot, &cfg.thresholds.hygiene),
        coupling::compute_coupling(&snapshot, &cfg.thresholds.coupling),
    ];

    let weight_pairs = cfg.weights.as_weight_pairs();
    let report = scorer::build_report(
        &snapshot,
        categories,
        None,
        &weight_pairs,
        cfg.thresholds.coupling.component_depth,
    );

    let threshold = args.min_score;
    let score_failed = check_gate_categories(&report, &args, threshold);

    let trend_failed = if let Some(max_decline) = args.max_decline {
        let history = cache::history::load_history(&local_path).unwrap_or_default();
        let current_entry = scorer::build_history_entry(&report, &current_head, None);
        let summary = trend::compute_trend(&history, &report.branch, &current_entry);
        check_trend_gate(&summary, max_decline)
    } else {
        false
    };

    Ok(if score_failed || trend_failed { 1 } else { 0 })
}

fn check_trend_gate(summary: &trend::TrendSummary, max_decline: f64) -> bool {
    if summary.delta.is_first {
        println!("TREND: no prior history on this branch — skipping trend check");
        return false;
    }

    if summary.branch_mismatch_warning {
        println!("TREND WARN: prior history is from a different branch");
    }

    match &summary.velocity {
        Some(v) if v.direction == VelocityDirection::Declining => {
            let rate = v.points_per_run.abs();
            if rate > max_decline {
                println!(
                    "FAIL: score declining at {:.1} points/run (limit: {:.1})",
                    rate, max_decline
                );
                true
            } else {
                println!(
                    "PASS: score declining at {:.1} points/run (within limit {:.1})",
                    rate, max_decline
                );
                false
            }
        }
        Some(v) => {
            println!(
                "PASS: score trend {:?} ({:+.1} points/run)",
                v.direction, v.points_per_run
            );
            false
        }
        None => {
            println!("TREND: not enough history to compute velocity");
            false
        }
    }
}

fn check_gate_categories(report: &AnalysisReport, args: &GateArgs, threshold: u32) -> bool {
    let mut failed = false;

    if args.category.is_empty() {
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

    failed
}
