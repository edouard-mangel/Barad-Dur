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

// `args` currently carries only `--no-blame`, which was always a no-op here:
// ADR-005 baseline collection skips blame unconditionally. Blame is the
// expensive part ADR-005 exists to avoid; the AST pass is NOT skipped as
// of the per-entity trend feature (Decision 5) — it's needed for
// per-sample complexity, and it's the same cost `analyze`/`gate` already
// pay on every normal invocation, amortized across `sample_count` points.
pub fn run(_args: &BackfillArgs, repo_path: &Path) -> Result<()> {
    let cfg = config::load(repo_path)?;
    config::validate(&cfg)?;
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

    // Build a set of SHAs already present in trends.json to skip duplicates.
    // Version-aware: a scoring change archives the old file and leaves this
    // empty, so every commit is regenerated. Deduping against entries the
    // bump just invalidated would make `backfill` — the command the warning
    // names as the fix — silently do nothing.
    let (existing_entries, warning) = history::load_history_checked(repo_path)?;
    if let Some(warning) = warning {
        println!("{warning}");
    }
    let existing_heads: HashSet<String> = existing_entries.into_iter().map(|e| e.head).collect();

    // Tracked separately from `existing_heads`: the two files are written and
    // reset independently. `analyze` appends HEAD to trends.json on every run,
    // and a corrupt entity_trends.json is archived and replaced with an empty
    // one — in both cases a SHA present in trends.json says nothing about
    // whether this sample's entity data exists.
    let existing_entity_heads: HashSet<String> =
        crate::cache::entity_history::load_entity_history(repo_path)?
            .into_iter()
            .map(|e| e.head)
            .collect();

    let total = selected_shas.len();
    let mut written = 0usize;

    // Parse `.baraddurignore` once and reuse it for every historical sample.
    let ignore = crate::collector::BaradDurIgnore::load(repo_path)?;

    for (idx, sha) in selected_shas.iter().enumerate() {
        println!("[{}/{}] Analyzing {}...", idx + 1, total, &sha[..8]);

        let needs_trend_entry = !existing_heads.contains(sha);
        let needs_entity_entry = !existing_entity_heads.contains(sha);
        if !needs_trend_entry && !needs_entity_entry {
            continue;
        }

        let snapshot = Collector::collect_snapshot_at_with_ast(
            repo_path,
            sha,
            &ignore,
            cfg.exclude_use_defaults,
        )?;

        // Computed once, shared by the Health category's "God objects"
        // metric and by build_report's refactoring-action generator.
        let flagged_god_objects = health::god_object_files(&snapshot, &cfg.thresholds.health);

        let categories = vec![
            health::compute_health(&snapshot, &cfg.thresholds.health, &flagged_god_objects),
            team::compute_team(&snapshot, &cfg.thresholds.team, &cfg.thresholds.coupling),
            evolution::compute_evolution(&snapshot, &cfg.thresholds.evolution),
            hygiene::compute_hygiene(&snapshot, &cfg.thresholds.hygiene),
        ];

        // Backfill keeps only scores from the report — skip the coupling
        // reach computation whose hotspot annotations it would discard.
        let report = scorer::build_report(
            &snapshot,
            categories,
            None,
            &weight_pairs,
            &cfg.thresholds,
            &flagged_god_objects,
            &Default::default(),
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

        if needs_trend_entry {
            history::append_if_new_head(&entry, repo_path)?;
        }

        if needs_entity_entry {
            let entity_entry = crate::cache::entity_history::build_entity_trend_entry(
                &snapshot,
                &report.file_hotspots,
                &cfg.thresholds.coupling,
                cfg.backfill.entity_trend_top_n,
                sha,
                entry.timestamp,
                &report.branch,
            );
            crate::cache::entity_history::append_entity_entry(&entity_entry, repo_path)?;
        }

        written += 1;
    }

    if written == 0 && !existing_heads.is_empty() {
        println!("Backfill already complete");
    } else {
        println!("{} entries written", written);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::BackfillArgs;

    fn temp_git_repo() -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?} failed");
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@e"]);
        git(&["config", "user.name", "t"]);
        std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "init"]);
        dir
    }

    #[test]
    fn run_rejects_invalid_config() {
        let dir = temp_git_repo();
        let cache_dir = dir.path().join(".repository-analysis");
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(
            cache_dir.join("barad-dur.toml"),
            "[thresholds.health]\ngod_node_degree_percentile = 0.0\n",
        )
        .unwrap();
        let args = BackfillArgs {
            target: dir.path().to_string_lossy().into_owned(),
            no_blame: false,
        };
        assert!(
            run(&args, dir.path()).is_err(),
            "backfill must reject a config that analyze would also reject"
        );
    }
}
