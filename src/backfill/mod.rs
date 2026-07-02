pub mod sampling;

use anyhow::Result;
use std::collections::HashSet;
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

    // Collect all commits (newest-first) to get SHAs + timestamps for sampling
    let collection = collector.collect_commits()?;
    let commit_refs: Vec<sampling::CommitRef> = collection
        .commits
        .iter()
        .map(|c| (collection.interner.resolve(c.id).to_string(), c.timestamp))
        .collect();

    if commit_refs.is_empty() {
        anyhow::bail!("No commits found — nothing to backfill");
    }

    let selected_shas = sampling::select_samples(&commit_refs, sample_count);

    // Build a set of SHAs already present in trends.json to skip duplicates
    let existing_entries = history::load_history(repo_path)?;
    let existing_heads: HashSet<String> = existing_entries.into_iter().map(|e| e.head).collect();

    let total = selected_shas.len();
    let mut written = 0usize;

    // Parse `.baraddurignore` once and reuse it for every historical sample.
    let ignore = crate::collector::BaradDurIgnore::load(repo_path)?;

    for (idx, sha) in selected_shas.iter().enumerate() {
        println!("[{}/{}] Analyzing {}...", idx + 1, total, &sha[..8]);

        if existing_heads.contains(sha) {
            continue;
        }

        let snapshot = Collector::collect_snapshot_at(repo_path, sha, args.no_blame, &ignore)?;

        let categories = vec![
            health::compute_health(&snapshot, &cfg.thresholds.health),
            team::compute_team(&snapshot, &cfg.thresholds.team),
            evolution::compute_evolution(&snapshot, &cfg.thresholds.evolution),
            hygiene::compute_hygiene(&snapshot, &cfg.thresholds.hygiene),
        ];

        let report = scorer::build_report(
            &snapshot,
            categories,
            None,
            &weight_pairs,
            cfg.thresholds.coupling.component_depth,
        );
        let mut entry = scorer::build_history_entry(&report, sha, Some("backfill".to_string()));

        // Use the commit's actual timestamp instead of "now" so the trend
        // chart spaces backfill points by their real dates.
        let commit_ts = snapshot
            .commits
            .iter()
            .find(|c| snapshot.resolve_commit(c.id) == sha.as_str())
            .map(|c| c.timestamp);
        if let Some(ts) = commit_ts {
            entry.timestamp = ts;
        }

        history::append_if_new_head(&entry, repo_path)?;
        written += 1;
    }

    if written == 0 && !existing_heads.is_empty() {
        println!("Backfill already complete");
    } else {
        println!("{} entries written", written);
    }
    Ok(())
}
