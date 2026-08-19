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
// ADR-005 baseline collection skips blame unconditionally.
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

        let snapshot = Collector::collect_snapshot_at(repo_path, sha, &ignore, true)?;

        // Computed once, shared by the Health category's "God objects"
        // metric and by build_report's refactoring-action generator.
        let flagged_god_objects = health::god_object_files(&snapshot, &cfg.thresholds.health);

        let categories = vec![
            health::compute_health(&snapshot, &cfg.thresholds.health, &flagged_god_objects),
            team::compute_team(&snapshot, &cfg.thresholds.team),
            evolution::compute_evolution(&snapshot, &cfg.thresholds.evolution),
            hygiene::compute_hygiene(&snapshot, &cfg.thresholds.hygiene),
        ];

        let report = scorer::build_report(
            &snapshot,
            categories,
            None,
            &weight_pairs,
            &cfg.thresholds.coupling,
            &flagged_god_objects,
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
            "[thresholds.health]\ngod_node_degree_multiplier = 0.0\n",
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
