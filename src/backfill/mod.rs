pub mod sampling;

use anyhow::Result;
use std::path::Path;

use crate::cache::history;
use crate::cli::BackfillArgs;
use crate::collector::Collector;
use crate::config;
use crate::metrics::{evolution, health, hygiene, team};
use crate::scorer;
use crate::snapshot::TimeWindow;

pub fn run(args: &BackfillArgs, repo_path: &Path) -> Result<()> {
    let cfg = config::load(repo_path)?;
    let sample_count = cfg.backfill.sample_count as usize;

    let time_window = TimeWindow::full_history();
    let collector = Collector::open(repo_path, time_window)?;

    let weight_pairs = cfg.weights.as_weight_pairs();

    // Collect all commits (newest-first) to get SHAs for sampling
    let collection = collector.collect_commits()?;
    let all_shas: Vec<String> = collection.commits.iter().map(|c| c.id.clone()).collect();

    let selected_shas = sampling::select_samples(&all_shas, sample_count);

    let mut written = 0usize;

    for sha in &selected_shas {
        let snapshot = Collector::collect_snapshot_at(repo_path, sha, args.no_blame)?;

        let categories = vec![
            health::compute_health(&snapshot, &cfg.thresholds.health),
            team::compute_team(&snapshot, &cfg.thresholds.team),
            evolution::compute_evolution(&snapshot, &cfg.thresholds.evolution),
            hygiene::compute_hygiene(&snapshot, &cfg.thresholds.hygiene),
        ];

        let report = scorer::build_report(&snapshot, categories, None, &weight_pairs);
        let entry =
            scorer::build_history_entry(&report, sha, Some("backfill".to_string()));

        history::append_if_new_head(&entry, repo_path)?;
        written += 1;
    }

    println!("{} entries written", written);
    Ok(())
}
