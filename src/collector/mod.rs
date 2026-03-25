pub mod gitcli;
mod libgit;

use anyhow::{Context, Result};
use chrono::Utc;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::metrics::complexity;
use crate::snapshot::{
    Author, BlameLine, Commit, FileComplexity, FileEntry, RepoSnapshot, TimeWindow,
};

/// Default file extensions excluded from analysis (translation/resource files).
/// These files change together by definition and inflate coupling/churn metrics.
const DEFAULT_EXCLUDE_EXTENSIONS: &[&str] = &[
    "resx", "po", "pot", "xlf", "xliff", "strings", "arb", "lproj",
];

/// Default path patterns excluded from analysis (tooling config, lockfiles).
/// Lockfiles inflate churn/coupling metrics without reflecting real code changes.
const DEFAULT_EXCLUDE_PATTERNS: &[&str] = &[
    // Lockfiles
    "pnpm-lock.yaml",
    "package-lock.json",
    "yarn.lock",
    "Cargo.lock",
    "Gemfile.lock",
    "poetry.lock",
    "composer.lock",
    "go.sum",
    "flake.lock",
    // Tooling directories
    ".claude/**",
    ".cursor/**",
    ".idea/**",
    ".vscode/**",
    // ORM migrations / generated schemas (auto-generated, inflate churn)
    "**/Migrations/*.Designer.cs",
    "**/Migrations/*ModelSnapshot.cs",
    "**/migrations/*.py",
    "db/schema.rb",
    "prisma/migrations/**",
    "alembic/versions/**",
    // Internationalization / translation directories
    "**/i18n/**",
    "**/l10n/**",
    "**/locales/**",
    "**/locale/**",
];

/// Returns true if the file should be excluded based on the given glob patterns
/// and (optionally) the built-in default extension list.
pub fn is_excluded(path: &Path, patterns: &[String], use_defaults: bool) -> bool {
    let path_str = path.to_string_lossy();

    // Check built-in defaults (by extension and path pattern)
    if use_defaults {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_lower = ext.to_lowercase();
            if DEFAULT_EXCLUDE_EXTENSIONS.iter().any(|&e| e == ext_lower) {
                return true;
            }
        }
        if DEFAULT_EXCLUDE_PATTERNS
            .iter()
            .any(|p| glob_match::glob_match(p, &path_str))
        {
            return true;
        }
    }

    // Check user-provided glob patterns
    for pattern in patterns {
        if glob_match::glob_match(pattern, &path_str) {
            return true;
        }
    }

    false
}

/// Trait for reporting progress during collection phases.
/// Implemented by `ProgressBar` for real display and `NoProgress` for silent operation.
pub trait Progress: Send + Sync {
    fn inc(&self, delta: u64);
}

impl Progress for ProgressBar {
    fn inc(&self, delta: u64) {
        ProgressBar::inc(self, delta);
    }
}

/// No-op progress reporter — used in tests and when progress display is disabled.
pub struct NoProgress;

impl Progress for NoProgress {
    fn inc(&self, _delta: u64) {}
}

/// Result of collecting commits — includes deduplicated author list.
pub struct CommitCollection {
    pub commits: Vec<Commit>,
    pub authors: Vec<Author>,
}

pub struct Collector {
    repo: git2::Repository,
    pub time_window: TimeWindow,
}

impl Collector {
    pub fn open(path: &Path, time_window: TimeWindow) -> Result<Self> {
        let repo = git2::Repository::discover(path).with_context(|| {
            format!(
                "'{}' is not a git repository. Run from a repo root or pass a path.",
                path.display()
            )
        })?;
        Ok(Collector { repo, time_window })
    }

    pub fn repo_name(&self) -> String {
        self.repo
            .workdir()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    pub fn default_branch(&self) -> String {
        self.repo
            .head()
            .ok()
            .and_then(|h| h.shorthand().map(String::from))
            .unwrap_or_else(|| "main".to_string())
    }

    pub fn head_commit_hash(&self) -> Result<String> {
        let head = self.repo.head().context("Failed to get HEAD")?;
        let oid = head.target().context("HEAD has no target")?;
        Ok(oid.to_string())
    }

    /// Collect all commits in the time window, along with deduplicated authors.
    pub fn collect_commits(&self) -> Result<CommitCollection> {
        libgit::collect_commits(&self.repo, &self.time_window)
    }

    /// Collect the current file tree from HEAD.
    pub fn collect_files(&self) -> Result<Vec<FileEntry>> {
        libgit::collect_files(&self.repo)
    }

    /// Collect blame data for all non-binary files.
    pub fn collect_blame(
        &self,
        files: &[FileEntry],
        authors: &[Author],
        progress: &dyn Progress,
    ) -> Result<HashMap<PathBuf, Vec<BlameLine>>> {
        gitcli::collect_blame(self.repo_path(), files, authors, progress)
    }

    /// Collect blame data, reusing cached entries for unchanged blobs.
    /// Returns (blame_map, updated_cache).
    pub fn collect_blame_cached(
        &self,
        files: &[FileEntry],
        authors: &[Author],
        cache: &crate::cache::blame::BlameCache,
        progress: &dyn Progress,
    ) -> Result<(
        HashMap<PathBuf, Vec<BlameLine>>,
        crate::cache::blame::BlameCache,
    )> {
        gitcli::collect_blame_cached(self.repo_path(), files, authors, cache, progress)
    }

    /// Check if this is a shallow clone.
    pub fn is_shallow(&self) -> bool {
        gitcli::is_shallow_clone(self.repo_path())
    }

    /// Analyse working-tree files for static complexity metrics.
    pub fn collect_file_metrics(&self, files: &[FileEntry]) -> HashMap<PathBuf, FileComplexity> {
        self.collect_file_metrics_with_progress(files, &NoProgress)
    }

    fn collect_file_metrics_with_progress(
        &self,
        files: &[FileEntry],
        progress: &dyn Progress,
    ) -> HashMap<PathBuf, FileComplexity> {
        let root = self.repo_path();
        let mut map = HashMap::new();
        for entry in files {
            if entry.is_binary {
                continue;
            }
            let abs_path = root.join(&entry.path);
            if let Ok(content) = std::fs::read_to_string(&abs_path) {
                let metrics = complexity::analyse_file(&entry.path, &content);
                map.insert(entry.path.clone(), metrics);
            }
            progress.inc(1);
        }
        map
    }

    /// Build a complete RepoSnapshot with all data and derived indexes.
    pub fn collect_snapshot(&self) -> Result<RepoSnapshot> {
        self.collect_snapshot_with_progress(false)
    }

    /// Build a complete RepoSnapshot, optionally showing progress indicators.
    pub fn collect_snapshot_with_progress(&self, show_progress: bool) -> Result<RepoSnapshot> {
        self.collect_snapshot_inner(show_progress, false, false, false, &[], true)
    }

    /// Build a complete RepoSnapshot with full control over display and phases.
    pub fn collect_snapshot_verbose(
        &self,
        show_progress: bool,
        verbose: bool,
        skip_blame: bool,
        no_cache: bool,
        exclude_patterns: &[String],
        use_default_excludes: bool,
    ) -> Result<RepoSnapshot> {
        self.collect_snapshot_inner(
            show_progress,
            verbose,
            skip_blame,
            no_cache,
            exclude_patterns,
            use_default_excludes,
        )
    }

    fn collect_snapshot_inner(
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
        let file_metrics = self.collect_file_metrics_with_progress(&files, complexity_progress);
        let complexity_ms = t.elapsed().as_millis();
        if let Some(pb) = complexity_bar {
            pb.finish_and_clear();
        }

        // Phase 5: indexes (fast, spinner only)
        let sp = make_spinner("Building indexes...");
        let t = Instant::now();
        let head = self.head_commit_hash()?;

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
        let collection = libgit::collect_commits_at(&repo, sha, &time_window)?;
        let files = libgit::collect_files_at(&repo, sha)?;

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
        };
        snapshot.build_indexes();
        Ok(snapshot)
    }

    pub fn repo_path(&self) -> &Path {
        self.repo.workdir().unwrap_or_else(|| self.repo.path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::TimeWindow;

    #[test]
    fn collect_files_populates_blob_oid() {
        let collector = Collector::open(std::path::Path::new("."), TimeWindow::default())
            .expect("should open repo");
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
        let collector = Collector::open(std::path::Path::new("."), TimeWindow::default())
            .expect("should open repo");
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
        let collector = Collector::open(std::path::Path::new("."), TimeWindow::default())
            .expect("should open repo");
        let files = collector.collect_files().expect("should collect files");
        let metrics = collector.collect_file_metrics(&files);
        assert!(!metrics.is_empty());
        let rs_file = metrics
            .keys()
            .find(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"));
        assert!(rs_file.is_some(), "expected at least one .rs file");
    }

    #[test]
    fn is_excluded_matches_default_extensions() {
        let p = Path::new("src/Resources/Strings.resx");
        assert!(is_excluded(p, &[], true));
        assert!(!is_excluded(p, &[], false));
    }

    #[test]
    fn is_excluded_matches_po_files() {
        assert!(is_excluded(Path::new("locale/fr/messages.po"), &[], true));
        assert!(is_excluded(Path::new("lang/en.pot"), &[], true));
        assert!(is_excluded(Path::new("i18n/strings.xlf"), &[], true));
    }

    #[test]
    fn is_excluded_matches_user_globs() {
        let patterns = vec!["**/i18n/**".to_string()];
        assert!(is_excluded(
            Path::new("src/assets/i18n/sfk-messages/fr-FR.ts"),
            &patterns,
            false
        ));
        assert!(!is_excluded(Path::new("src/main.rs"), &patterns, false));
    }

    #[test]
    fn is_excluded_combines_defaults_and_user_patterns() {
        let patterns = vec!["**/i18n/**".to_string()];
        // Matched by default extension
        assert!(is_excluded(Path::new("foo.resx"), &patterns, true));
        // Matched by user pattern
        assert!(is_excluded(Path::new("src/i18n/en.ts"), &patterns, true));
        // Not matched by either
        assert!(!is_excluded(Path::new("src/main.rs"), &patterns, true));
    }

    #[test]
    fn is_excluded_matches_i18n_directories_by_default() {
        assert!(is_excluded(
            Path::new("src/client/src/assets/i18n/sfk-messages/en-US.ts"),
            &[],
            true
        ));
        assert!(is_excluded(Path::new("app/l10n/strings_fr.arb"), &[], true));
        assert!(is_excluded(Path::new("src/locales/en.json"), &[], true));
        assert!(is_excluded(Path::new("config/locale/fr.yml"), &[], true));
        // Non-i18n .ts files should NOT be excluded
        assert!(!is_excluded(Path::new("src/main.ts"), &[], true));
    }

    #[test]
    fn is_excluded_case_insensitive_extension() {
        assert!(is_excluded(Path::new("Strings.RESX"), &[], true));
        assert!(is_excluded(Path::new("lang.Resx"), &[], true));
    }

    #[test]
    fn is_excluded_matches_default_lockfiles() {
        assert!(is_excluded(Path::new("pnpm-lock.yaml"), &[], true));
        assert!(is_excluded(Path::new("package-lock.json"), &[], true));
        assert!(is_excluded(Path::new("yarn.lock"), &[], true));
        assert!(is_excluded(Path::new("Cargo.lock"), &[], true));
        assert!(is_excluded(Path::new("go.sum"), &[], true));
        assert!(is_excluded(Path::new("poetry.lock"), &[], true));
        // Not excluded when defaults disabled
        assert!(!is_excluded(Path::new("pnpm-lock.yaml"), &[], false));
    }

    #[test]
    fn is_excluded_matches_orm_generated_files() {
        // EF Core
        assert!(is_excluded(
            Path::new("Data/Migrations/20240101_Init.Designer.cs"),
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new("Data/Migrations/AppDbContextModelSnapshot.cs"),
            &[],
            true
        ));
        // Django
        assert!(is_excluded(
            Path::new("myapp/migrations/0001_initial.py"),
            &[],
            true
        ));
        // Rails
        assert!(is_excluded(Path::new("db/schema.rb"), &[], true));
        // Prisma
        assert!(is_excluded(
            Path::new("prisma/migrations/20240101/migration.sql"),
            &[],
            true
        ));
        // Regular source should not match
        assert!(!is_excluded(Path::new("src/Models/User.cs"), &[], true));
    }

    #[test]
    fn is_excluded_matches_default_tooling_dirs() {
        assert!(is_excluded(Path::new(".claude/settings.json"), &[], true));
        assert!(is_excluded(Path::new(".cursor/rules/my-rule"), &[], true));
        assert!(is_excluded(Path::new(".idea/workspace.xml"), &[], true));
        assert!(is_excluded(Path::new(".vscode/settings.json"), &[], true));
        // Not excluded when defaults disabled
        assert!(!is_excluded(Path::new(".claude/settings.json"), &[], false));
    }
}
