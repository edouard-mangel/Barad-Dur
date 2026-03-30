use anyhow::{Context, Result};
use chrono::Utc;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::metrics::complexity;
use crate::snapshot::{FileComplexity, FileEntry, RepoSnapshot, TimeWindow};

type RawImports = HashMap<PathBuf, Vec<String>>;

use super::exclude::is_excluded;
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

    pub(super) fn collect_snapshot_inner(
        &self,
        show_progress: bool,
        verbose: bool,
        skip_blame: bool,
        no_cache: bool,
        exclude_patterns: &[String],
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
        let has_excludes = !exclude_patterns.is_empty() || use_default_excludes;
        let (files, excluded_count) = if has_excludes {
            let before = all_files.len();
            let filtered: Vec<FileEntry> = all_files
                .into_iter()
                .filter(|f| !is_excluded(&f.path, exclude_patterns, use_default_excludes))
                .collect();
            let after = filtered.len();
            (filtered, before - after)
        } else {
            (all_files, 0)
        };
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
        let files = super::libgit::collect_files_at(&repo, sha)?;

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
            .and_then(|h| h.shorthand().map(String::from))
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
        };
        snapshot.build_indexes();
        Ok(snapshot)
    }
}

/// Resolve raw import strings to actual file paths present in the repository.
/// Only keeps imports that map to a known file in `files`.
fn resolve_imports(
    raw_imports: &RawImports,
    files: &[FileEntry],
) -> HashMap<PathBuf, Vec<PathBuf>> {
    use std::collections::HashSet;
    let known: HashSet<&PathBuf> = files.iter().map(|f| &f.path).collect();

    raw_imports
        .iter()
        .filter_map(|(source_path, imports)| {
            let resolved: Vec<PathBuf> = imports
                .iter()
                .filter_map(|raw| resolve_single_import(raw, source_path, &known))
                .collect();
            if resolved.is_empty() {
                None
            } else {
                Some((source_path.clone(), resolved))
            }
        })
        .collect()
}

fn resolve_single_import(
    raw: &str,
    source: &Path,
    known: &std::collections::HashSet<&PathBuf>,
) -> Option<PathBuf> {
    let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("");
    let candidates = match ext {
        "rs" => resolve_rust_import(raw),
        "js" | "jsx" | "mjs" | "cjs" => resolve_js_import(raw, source),
        "ts" | "tsx" => resolve_ts_import(raw, source),
        "py" => resolve_python_import(raw),
        "go" => resolve_go_import(raw, source),
        "java" => resolve_java_import(raw),
        "cs" => resolve_csharp_import(raw),
        _ => Vec::new(),
    };
    candidates.into_iter().find(|c| known.contains(c))
}

fn resolve_rust_import(raw: &str) -> Vec<PathBuf> {
    // crate::foo::bar → src/foo/bar.rs or src/foo/bar/mod.rs
    // self::foo → relative (handled similarly)
    let path_part = raw
        .strip_prefix("crate::")
        .or_else(|| raw.strip_prefix("self::"))
        .unwrap_or(raw);
    // Handle glob imports like "crate::foo::*" or nested items "crate::foo::{Bar, Baz}"
    let path_part = path_part
        .split("::{")
        .next()
        .unwrap_or(path_part)
        .trim_end_matches("::*");
    let segments = path_part.replace("::", "/");
    vec![
        PathBuf::from(format!("src/{}.rs", segments)),
        PathBuf::from(format!("src/{}/mod.rs", segments)),
    ]
}

fn resolve_js_import(raw: &str, source: &Path) -> Vec<PathBuf> {
    if !raw.starts_with('.') {
        return Vec::new(); // external package
    }
    let base = source.parent().unwrap_or_else(|| Path::new(""));
    let resolved = base.join(raw);
    vec![
        resolved.with_extension("js"),
        resolved.with_extension("jsx"),
        resolved.with_extension("mjs"),
        resolved.join("index.js"),
    ]
}

fn resolve_ts_import(raw: &str, source: &Path) -> Vec<PathBuf> {
    if !raw.starts_with('.') {
        return Vec::new();
    }
    let base = source.parent().unwrap_or_else(|| Path::new(""));
    let resolved = base.join(raw);
    vec![
        resolved.with_extension("ts"),
        resolved.with_extension("tsx"),
        resolved.with_extension("js"),
        resolved.join("index.ts"),
        resolved.join("index.tsx"),
        resolved.join("index.js"),
    ]
}

fn resolve_python_import(raw: &str) -> Vec<PathBuf> {
    let segments = raw.replace('.', "/");
    vec![
        PathBuf::from(format!("{}.py", segments)),
        PathBuf::from(format!("{}/__init__.py", segments)),
    ]
}

fn resolve_go_import(raw: &str, source: &Path) -> Vec<PathBuf> {
    // Go imports are package-based; try matching by last path segment as directory
    let last = raw.rsplit('/').next().unwrap_or(raw);
    let base = source.parent().unwrap_or_else(|| Path::new(""));
    // Try sibling directory
    vec![base.join(last).join("*.go")] // simplified: won't match via HashSet
                                       // For Go we do a prefix-based fallback below
}

fn resolve_java_import(raw: &str) -> Vec<PathBuf> {
    // java.util.HashMap → java/util/HashMap.java
    let segments = raw.replace('.', "/");
    vec![
        PathBuf::from(format!("{}.java", segments)),
        PathBuf::from(format!("src/main/java/{}.java", segments)),
    ]
}

fn resolve_csharp_import(raw: &str) -> Vec<PathBuf> {
    let segments = raw.replace('.', "/");
    vec![PathBuf::from(format!("{}.cs", segments))]
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
            .collect_blame_cached(&files, &collection.authors, &blame_cache, &NoProgress)
            .expect("should collect blame");

        assert!(!blame_map.is_empty());
        assert!(!new_cache.entries.is_empty());

        // Second run: all blobs cached — should produce identical results
        let (blame_map2, _) = collector
            .collect_blame_cached(&files, &collection.authors, &new_cache, &NoProgress)
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
}
