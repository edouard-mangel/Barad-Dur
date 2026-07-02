use anyhow::{Context, Result};
use chrono::Utc;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::metrics::complexity;
use crate::snapshot::{FileComplexity, FileEntry, RepoSnapshot, TimeWindow};

use super::ignore_file::{should_include, BaradDurIgnore};
use super::import_resolver::{resolve_imports, RawImports};
use super::progress::{NoProgress, Progress};
use super::Collector;

impl Collector {
    pub(super) fn collect_file_metrics_with_progress(
        &self,
        files: &[FileEntry],
        progress: &dyn Progress,
    ) -> (HashMap<PathBuf, FileComplexity>, RawImports) {
        let root = self.repo_path();
        let results: Vec<(PathBuf, FileComplexity, Vec<String>)> = files
            .par_iter()
            .filter(|entry| !entry.is_binary)
            .filter_map(|entry| {
                let abs_path = root.join(&entry.path);
                let content = std::fs::read_to_string(&abs_path).ok()?;
                let metrics = complexity::analyse_file(&entry.path, &content);
                let imports = complexity::extract_file_imports(&entry.path, &content);
                progress.inc(1);
                Some((entry.path.clone(), metrics, imports))
            })
            .collect();
        let mut file_metrics = HashMap::new();
        let mut raw_imports = HashMap::new();
        for (path, metrics, imports) in results {
            file_metrics.insert(path.clone(), metrics);
            if !imports.is_empty() {
                raw_imports.insert(path, imports);
            }
        }
        (file_metrics, raw_imports)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn collect_snapshot_inner(
        &self,
        show_progress: bool,
        verbose: bool,
        skip_blame: bool,
        no_cache: bool,
        cli_exclude_patterns: &[String],
        cli_exclude_extensions: &[String],
        use_default_excludes: bool,
    ) -> Result<RepoSnapshot> {
        let make_spinner = |msg: &str| -> Option<ProgressBar> {
            if !show_progress {
                return None;
            }
            let sp = ProgressBar::new_spinner();
            sp.set_style(
                ProgressStyle::default_spinner()
                    .template("  {spinner:.cyan} {msg}")
                    .unwrap(),
            );
            sp.set_message(msg.to_string());
            sp.enable_steady_tick(std::time::Duration::from_millis(80));
            Some(sp)
        };

        let bar_style = ProgressStyle::default_bar()
            .template("  {spinner:.cyan} {msg} [{bar:30.cyan/dim}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("━╸─");

        // Phase 1: commits (fast, spinner only)
        let sp = make_spinner("Walking commits...");
        let t = Instant::now();
        let collection = self.collect_commits()?;
        let commits_ms = t.elapsed().as_millis();
        if let Some(s) = sp {
            s.finish_and_clear();
        }

        // Phase 2: file tree (fast, spinner only)
        let sp = make_spinner(&format!(
            "Found {} commits. Collecting file tree...",
            collection.commits.len()
        ));
        let t = Instant::now();
        let all_files = self.collect_files()?;
        // Apply every exclusion layer in a single pass. `.baraddurignore` (repo root)
        // sits between the CLI flags (highest) and the built-in defaults (lowest); its
        // `!` rules re-include default-excluded files. When nothing excludes a path
        // `should_include` returns true, so no separate short-circuit is needed. See
        // `ignore_file::should_include` for the precedence composition.
        let ignore = BaradDurIgnore::load(self.repo_path())?;
        let before = all_files.len();
        let files: Vec<FileEntry> = all_files
            .into_iter()
            .filter(|f| {
                should_include(
                    &ignore,
                    &f.path,
                    cli_exclude_patterns,
                    cli_exclude_extensions,
                    use_default_excludes,
                )
            })
            .collect();
        let excluded_count = before - files.len();
        let files_ms = t.elapsed().as_millis();
        if let Some(s) = sp {
            s.finish_and_clear();
        }
        if show_progress && excluded_count > 0 {
            eprintln!(
                "  Excluded {} files ({} remaining)",
                excluded_count,
                files.len()
            );
        }

        // Phase 3: blame (slow — real progress bar, skippable, with per-blob cache)
        //
        // Selective blame: only blame files modified in the time window.
        // Files untouched in the window don't affect churn, coupling, or recent
        // ownership metrics. For bus factor / knowledge distribution the cached
        // blame from previous runs covers the rest.
        let changed_paths: std::collections::HashSet<PathBuf> = collection
            .commits
            .iter()
            .flat_map(|c| c.files_changed.iter().map(|fc| fc.path.clone()))
            .collect();
        let blame_files: Vec<FileEntry> = files
            .iter()
            .filter(|f| !f.is_binary && changed_paths.contains(&f.path))
            .cloned()
            .collect();
        let non_binary_changed: u64 = blame_files.len() as u64;
        let non_binary_total: u64 = files.iter().filter(|f| !f.is_binary).count() as u64;
        let t = Instant::now();
        let blame_map = if skip_blame {
            if show_progress {
                eprintln!(
                    "  Skipping blame ({} files) — use without --skip-blame for full analysis",
                    non_binary_total
                );
            }
            HashMap::new()
        } else {
            let blame_cache = if no_cache {
                crate::cache::blame::BlameCache::default()
            } else {
                crate::cache::blame::load(self.repo_path()).unwrap_or_default()
            };
            if show_progress && non_binary_changed < non_binary_total {
                eprintln!(
                    "  Selective blame: {}/{} files changed in window",
                    non_binary_changed, non_binary_total
                );
            }
            let cached_count = blame_files
                .iter()
                .filter(|f| blame_cache.entries.contains_key(&f.blob_oid))
                .count();
            if show_progress && cached_count > 0 {
                eprintln!(
                    "  Blame cache: {}/{} files cached",
                    cached_count, non_binary_changed
                );
            }
            let blame_bar = if show_progress {
                let pb = ProgressBar::new(non_binary_changed);
                pb.set_style(bar_style.clone());
                pb.set_message("Blaming files");
                pb.enable_steady_tick(std::time::Duration::from_millis(80));
                Some(pb)
            } else {
                None
            };
            let blame_progress: &dyn Progress = match &blame_bar {
                Some(pb) => pb,
                None => &NoProgress,
            };
            let (map, mut updated_cache) = self.collect_blame_cached(
                &blame_files,
                &collection.authors,
                &collection.raw_email_to_id,
                &blame_cache,
                blame_progress,
            )?;
            if let Some(pb) = blame_bar {
                pb.finish_and_clear();
            }
            // Prune stale entries
            let current_oids: std::collections::HashSet<String> =
                files.iter().map(|f| f.blob_oid.clone()).collect();
            updated_cache.prune(&current_oids);
            // Save blame cache
            if let Err(e) = crate::cache::blame::save(&updated_cache, self.repo_path()) {
                eprintln!("Warning: Failed to save blame cache: {}", e);
            }
            map
        };
        let blame_ms = t.elapsed().as_millis();

        // Phase 4: complexity (can be slow on large repos — progress bar)
        let complexity_bar = if show_progress {
            let pb = ProgressBar::new(non_binary_total);
            pb.set_style(bar_style);
            pb.set_message("Analysing complexity");
            pb.enable_steady_tick(std::time::Duration::from_millis(80));
            Some(pb)
        } else {
            None
        };
        let t = Instant::now();
        let complexity_progress: &dyn Progress = match &complexity_bar {
            Some(pb) => pb,
            None => &NoProgress,
        };
        let (file_metrics, raw_imports) =
            self.collect_file_metrics_with_progress(&files, complexity_progress);
        let complexity_ms = t.elapsed().as_millis();
        if let Some(pb) = complexity_bar {
            pb.finish_and_clear();
        }

        // Phase 5: indexes (fast, spinner only)
        let sp = make_spinner("Building indexes...");
        let t = Instant::now();
        let head = self.head_commit_hash()?;

        let import_graph = resolve_imports(&raw_imports, &files);
        let mut snapshot = RepoSnapshot {
            path: self.repo_path().to_path_buf(),
            name: self.repo_name(),
            default_branch: self.default_branch(),
            time_window: self.time_window.clone(),
            head_commit: head,
            created_at: Utc::now(),
            commits: collection.commits,
            files,
            authors: collection.authors,
            blame_map,
            commits_by_author: HashMap::new(),
            commits_by_file: HashMap::new(),
            file_change_pairs: Vec::new(),
            file_metrics,
            import_graph,
            commit_interner: collection.interner,
        };
        snapshot.build_indexes();
        let indexes_ms = t.elapsed().as_millis();

        if let Some(s) = sp {
            s.finish_and_clear();
        }

        if verbose {
            eprintln!(
                "  Timings: commits {}ms, files {}ms, blame {}ms, complexity {}ms, indexes {}ms",
                commits_ms, files_ms, blame_ms, complexity_ms, indexes_ms
            );
        }

        Ok(snapshot)
    }

    /// Collect a snapshot at a specific commit SHA without touching the working tree.
    /// file_metrics is always empty (ADR-005).
    pub fn collect_snapshot_at(
        repo_path: &Path,
        sha: &str,
        _skip_blame: bool,
    ) -> Result<RepoSnapshot> {
        let repo = git2::Repository::discover(repo_path)
            .with_context(|| format!("'{}' is not a git repository", repo_path.display()))?;
        let time_window = TimeWindow::full_history();
        let collection = super::libgit::collect_commits_at(&repo, sha, &time_window)?;

        // Apply the same exclusion policy as `analyze`/`gate` so backfilled history
        // is comparable to live scores: built-in defaults + `.baraddurignore`.
        // Backfill has no CLI exclude flags. The *current* `.baraddurignore` is
        // applied uniformly to every historical snapshot, so the trend reflects one
        // consistent definition of "relevant files" rather than each commit's own.
        let all_files = super::libgit::collect_files_at(&repo, sha)?;
        let ignore = BaradDurIgnore::load(repo_path)?;
        let files: Vec<FileEntry> = all_files
            .into_iter()
            .filter(|f| should_include(&ignore, &f.path, &[], &[], true))
            .collect();

        // ADR-005: backfill always skips blame for performance.
        let blame_map: HashMap<_, _> = HashMap::new();

        let repo_name = repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let branch = repo
            .head()
            .ok()
            .and_then(|h| h.shorthand().ok().map(String::from))
            .unwrap_or_else(|| "main".to_string());

        let mut snapshot = RepoSnapshot {
            path: repo_path.to_path_buf(),
            name: repo_name,
            default_branch: branch,
            time_window,
            head_commit: sha.to_string(),
            created_at: Utc::now(),
            commits: collection.commits,
            files,
            authors: collection.authors,
            blame_map,
            commits_by_author: HashMap::new(),
            commits_by_file: HashMap::new(),
            file_change_pairs: Vec::new(),
            file_metrics: HashMap::new(),
            import_graph: HashMap::new(),
            commit_interner: collection.interner,
        };
        snapshot.build_indexes();
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::TimeWindow;

    fn test_repo_path() -> std::path::PathBuf {
        std::env::var("BARAD_DUR_TEST_REPO")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
    }

    #[test]
    fn collect_files_populates_blob_oid() {
        // Requires a real git repo — skips gracefully under cargo-mutants (temp dir).
        // In CI, BARAD_DUR_TEST_REPO points to CI_PROJECT_DIR for dogfooding.
        let Ok(collector) = Collector::open(&test_repo_path(), TimeWindow::default()) else {
            return;
        };
        let files = collector.collect_files().expect("should collect files");
        assert!(!files.is_empty());
        for f in &files {
            assert!(
                !f.blob_oid.is_empty(),
                "blob_oid should be populated for {}",
                f.path.display()
            );
            assert_eq!(f.blob_oid.len(), 40, "blob_oid should be 40 hex chars");
        }
    }

    #[test]
    fn collect_blame_uses_cache_for_known_blobs() {
        // Requires a real git repo — skips gracefully under cargo-mutants (temp dir).
        // In CI, BARAD_DUR_TEST_REPO points to CI_PROJECT_DIR for dogfooding.
        let Ok(collector) = Collector::open(&test_repo_path(), TimeWindow::default()) else {
            return;
        };
        let files = collector.collect_files().expect("should collect files");
        let collection = collector.collect_commits().expect("should collect commits");

        // First run: no cache
        let blame_cache = crate::cache::blame::BlameCache::default();
        let (blame_map, new_cache) = collector
            .collect_blame_cached(
                &files,
                &collection.authors,
                &collection.raw_email_to_id,
                &blame_cache,
                &NoProgress,
            )
            .expect("should collect blame");

        assert!(!blame_map.is_empty());
        assert!(!new_cache.entries.is_empty());

        // Second run: all blobs cached — should produce identical results
        let (blame_map2, _) = collector
            .collect_blame_cached(
                &files,
                &collection.authors,
                &collection.raw_email_to_id,
                &new_cache,
                &NoProgress,
            )
            .expect("should collect blame from cache");

        assert_eq!(blame_map.len(), blame_map2.len());
    }

    #[test]
    fn collect_file_metrics_does_not_panic_on_real_repo() {
        // Requires a real git repo — skips gracefully under cargo-mutants (temp dir).
        // In CI, BARAD_DUR_TEST_REPO points to CI_PROJECT_DIR for dogfooding.
        let Ok(collector) = Collector::open(&test_repo_path(), TimeWindow::default()) else {
            return;
        };
        let files = collector.collect_files().expect("should collect files");
        let metrics = collector.collect_file_metrics(&files);
        assert!(!metrics.is_empty());
        let rs_file = metrics
            .keys()
            .find(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"));
        assert!(rs_file.is_some(), "expected at least one .rs file");
    }

    /// A throwaway git repo with one commit, for `collect_snapshot_at` (backfill).
    fn temp_git_repo(files: &[(&str, &str)]) -> (tempfile::TempDir, String) {
        let dir = tempfile::TempDir::new().unwrap();
        let git = |args: &[&str]| {
            assert!(std::process::Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .status()
                .unwrap()
                .success());
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@e"]);
        git(&["config", "user.name", "t"]);
        for (name, contents) in files {
            let p = dir.path().join(name);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(p, contents).unwrap();
        }
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "init"]);
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        let head = String::from_utf8(out.stdout).unwrap().trim().to_string();
        (dir, head)
    }

    fn snapshot_paths(snap: &RepoSnapshot) -> Vec<String> {
        snap.files
            .iter()
            .map(|f| f.path.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn collect_snapshot_at_applies_default_exclusions() {
        // Backfill must drop built-in default exclusions (e.g. Cargo.lock) so its
        // history is comparable to live analyze/gate scores.
        let (dir, head) = temp_git_repo(&[("main.rs", "fn main() {}\n"), ("Cargo.lock", "x\n")]);
        let snap = Collector::collect_snapshot_at(dir.path(), &head, true).unwrap();
        let paths = snapshot_paths(&snap);
        assert!(paths.iter().any(|p| p == "main.rs"));
        assert!(
            !paths.iter().any(|p| p == "Cargo.lock"),
            "Cargo.lock should be excluded by default in backfill too"
        );
    }

    #[test]
    fn collect_snapshot_at_honors_baraddurignore() {
        // A working-tree `.baraddurignore` filters historical snapshots as well.
        let (dir, head) = temp_git_repo(&[("main.rs", "fn main() {}\n"), ("keep.rs", "//\n")]);
        std::fs::write(dir.path().join(".baraddurignore"), "keep.rs\n").unwrap();
        let snap = Collector::collect_snapshot_at(dir.path(), &head, true).unwrap();
        let paths = snapshot_paths(&snap);
        assert!(!paths.iter().any(|p| p == "keep.rs"));
    }
}
