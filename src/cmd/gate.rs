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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::CategoryResult;
    use crate::scorer::AnalysisReport;
    use crate::trend::{TrendDelta, TrendSummary, TrendVelocity, VelocityDirection};
    use std::collections::HashMap;

    fn make_report(overall: u32, categories: &[(&str, u32)]) -> AnalysisReport {
        let cats: Vec<CategoryResult> = categories
            .iter()
            .map(|(name, score)| CategoryResult {
                name: name.to_string(),
                score: *score,
                metrics: vec![],
            })
            .collect();
        AnalysisReport {
            repo_name: "test".into(),
            branch: "main".into(),
            time_window_months: 6,
            total_commits: 1,
            total_authors: 1,
            total_files: 1,
            overall_score: overall,
            categories: cats,
            top_actions: vec![],
            remote_meta: None,
            file_hotspots: vec![],
            coupling_pairs: vec![],
            author_ownership: vec![],
            file_ages: vec![],
            author_cards: vec![],
            history: vec![],
            dep_ecosystem_reports: vec![],
        }
    }

    fn make_gate_args(min_score: u32, categories: Vec<String>) -> GateArgs {
        GateArgs {
            target: ".".into(),
            min_score,
            category: categories,
            skip_blame: None,
            max_decline: None,
        }
    }

    fn first_summary() -> TrendSummary {
        TrendSummary {
            delta: TrendDelta {
                overall: 0,
                delta_vs_oldest: 0,
                categories: HashMap::new(),
                is_first: true,
            },
            sparkline: vec![],
            velocity: None,
            branch_mismatch_warning: false,
            history: vec![],
        }
    }

    fn summary_with_velocity(direction: VelocityDirection, points_per_run: f64) -> TrendSummary {
        TrendSummary {
            delta: TrendDelta {
                overall: -5,
                delta_vs_oldest: -5,
                categories: HashMap::new(),
                is_first: false,
            },
            sparkline: vec![],
            velocity: Some(TrendVelocity {
                direction,
                points_per_run,
                window_size: 3,
            }),
            branch_mismatch_warning: false,
            history: vec![],
        }
    }

    // ── check_gate_categories ────────────────────────────────────────

    #[test]
    fn overall_pass() {
        let report = make_report(75, &[]);
        let args = make_gate_args(60, vec![]);
        assert!(!check_gate_categories(&report, &args, 60));
    }

    #[test]
    fn overall_fail() {
        let report = make_report(50, &[]);
        let args = make_gate_args(60, vec![]);
        assert!(check_gate_categories(&report, &args, 60));
    }

    #[test]
    fn category_pass() {
        let report = make_report(80, &[("Health", 75)]);
        let args = make_gate_args(60, vec!["health".into()]);
        assert!(!check_gate_categories(&report, &args, 60));
    }

    #[test]
    fn category_fail() {
        let report = make_report(80, &[("Health", 40)]);
        let args = make_gate_args(60, vec!["health".into()]);
        assert!(check_gate_categories(&report, &args, 60));
    }

    #[test]
    fn unknown_category_skipped() {
        let report = make_report(80, &[("Health", 80)]);
        let args = make_gate_args(60, vec!["nonexistent".into()]);
        assert!(!check_gate_categories(&report, &args, 60));
    }

    // ── check_trend_gate ────────────────────────────────────────────

    #[test]
    fn trend_first_run_passes() {
        assert!(!check_trend_gate(&first_summary(), 2.0));
    }

    #[test]
    fn trend_declining_above_limit_fails() {
        let s = summary_with_velocity(VelocityDirection::Declining, -5.0);
        assert!(check_trend_gate(&s, 2.0));
    }

    #[test]
    fn trend_declining_below_limit_passes() {
        let s = summary_with_velocity(VelocityDirection::Declining, -1.0);
        assert!(!check_trend_gate(&s, 2.0));
    }

    #[test]
    fn trend_improving_passes() {
        let s = summary_with_velocity(VelocityDirection::Improving, 3.0);
        assert!(!check_trend_gate(&s, 2.0));
    }

    #[test]
    fn trend_stable_passes() {
        let s = summary_with_velocity(VelocityDirection::Stable, 0.1);
        assert!(!check_trend_gate(&s, 2.0));
    }

    #[test]
    fn trend_no_velocity_passes() {
        let mut s = first_summary();
        s.delta.is_first = false; // not first, but no velocity computed
        assert!(!check_trend_gate(&s, 2.0));
    }

    #[test]
    fn trend_branch_mismatch_warning_does_not_fail() {
        let mut s = summary_with_velocity(VelocityDirection::Stable, 0.1);
        s.branch_mismatch_warning = true;
        assert!(!check_trend_gate(&s, 2.0));
    }
}
