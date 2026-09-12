use anyhow::{Context, Result};
use chrono::Utc;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::metrics::complexity;
use crate::snapshot::{FileEntry, RepoSnapshot, TimeWindow};

use super::ignore_file::{should_include, BaradDurIgnore};
use super::progress::{NoProgress, Progress};
use super::source_assembly::{RawSourceChannels, ResolvedSourceChannels};
use super::{Collector, CommitCollection, SnapshotOptions};

/// Spinner for a fast phase; `None` when progress display is off.
fn phase_spinner(show_progress: bool, msg: &str) -> Option<ProgressBar> {
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
}

fn finish_spinner(sp: Option<ProgressBar>) {
    if let Some(s) = sp {
        s.finish_and_clear();
    }
}

/// Progress bar for a slow phase; `None` when progress display is off.
fn phase_bar(show_progress: bool, len: u64, msg: &str) -> Option<ProgressBar> {
    if !show_progress {
        return None;
    }
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("  {spinner:.cyan} {msg} [{bar:30.cyan/dim}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("━╸─"),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    Some(pb)
}

/// Non-binary files in the tree — the denominator shown by the blame and
/// complexity progress displays.
fn non_binary_count(files: &[FileEntry]) -> u64 {
    files.iter().filter(|f| !f.is_binary).count() as u64
}

// The announce_* helpers below are presentation-only stderr notes: they
// steer no data, so no assertion can observe their internals. They are
// excluded from mutation testing in .cargo/mutants.toml.

fn announce_exclusions(show_progress: bool, before: usize, remaining: usize) {
    let excluded = before - remaining;
    if show_progress && excluded > 0 {
        eprintln!("  Excluded {} files ({} remaining)", excluded, remaining);
    }
}

fn announce_blame_skipped(show_progress: bool, non_binary_total: u64) {
    if show_progress {
        eprintln!(
            "  Skipping blame ({} files) — use without --skip-blame for full analysis",
            non_binary_total
        );
    }
}

fn announce_blame_plan(
    show_progress: bool,
    changed: u64,
    total: u64,
    cache: &crate::cache::blame::BlameCache,
    blame_files: &[FileEntry],
) {
    if !show_progress {
        return;
    }
    if changed < total {
        eprintln!(
            "  Selective blame: {}/{} files changed in window",
            changed, total
        );
    }
    let cached = blame_files
        .iter()
        .filter(|f| cache.entries.contains_key(&f.blob_oid))
        .count();
    if cached > 0 {
        eprintln!("  Blame cache: {}/{} files cached", cached, changed);
    }
}

impl Collector {
    pub(super) fn collect_file_metrics_with_progress(
        &self,
        files: &[FileEntry],
        progress: &dyn Progress,
    ) -> RawSourceChannels {
        let root = self.repo_path();
        // Parallel read-and-parse; aggregation is shared with the
        // historical reader and does not depend on completion order.
        let analyses: Vec<(PathBuf, complexity::SourceAnalysis)> = files
            .par_iter()
            .filter(|entry| !entry.is_binary)
            .filter_map(|entry| {
                let abs_path = root.join(&entry.path);
                let content = std::fs::read_to_string(&abs_path).ok()?;
                let analysis = complexity::analyse_source(&entry.path, &content);
                progress.inc(1);
                Some((entry.path.clone(), analysis))
            })
            .collect();
        RawSourceChannels::aggregate(analyses)
    }

    /// Orchestrates the five collection phases; each phase manages its own
    /// progress display and the orchestrator keeps the timing spine.
    pub(super) fn collect_snapshot_inner(
        &self,
        opts: &SnapshotOptions<'_>,
    ) -> Result<RepoSnapshot> {
        // Phase 1: commits (fast, spinner only)
        let sp = phase_spinner(opts.show_progress, "Walking commits...");
        let t = Instant::now();
        let collection = self.collect_commits()?;
        let commits_ms = t.elapsed().as_millis();
        finish_spinner(sp);

        // Phase 2: file tree (fast, spinner only)
        let sp = phase_spinner(
            opts.show_progress,
            &format!(
                "Found {} commits. Collecting file tree...",
                collection.commits.len()
            ),
        );
        let t = Instant::now();
        let files = self.collect_filtered_files(opts)?;
        let files_ms = t.elapsed().as_millis();
        finish_spinner(sp);

        // Phase 3: blame (slow — real progress bar, skippable, with per-blob cache)
        let t = Instant::now();
        let blame_map = self.resolve_blame_map(&collection, &files, opts)?;
        let blame_ms = t.elapsed().as_millis();

        // Phase 4: complexity (can be slow on large repos — progress bar)
        let complexity_bar = phase_bar(
            opts.show_progress,
            non_binary_count(&files),
            "Analysing complexity",
        );
        let t = Instant::now();
        let complexity_progress: &dyn Progress = match &complexity_bar {
            Some(pb) => pb,
            None => &NoProgress,
        };
        let ast = self.collect_file_metrics_with_progress(&files, complexity_progress);
        let complexity_ms = t.elapsed().as_millis();
        if let Some(pb) = complexity_bar {
            pb.finish_and_clear();
        }

        // Phase 5: indexes (fast, spinner only)
        let sp = phase_spinner(opts.show_progress, "Building indexes...");
        let t = Instant::now();
        let snapshot = self.assemble_snapshot(collection, files, blame_map, ast)?;
        let indexes_ms = t.elapsed().as_millis();
        finish_spinner(sp);

        if opts.verbose {
            eprintln!(
                "  Timings: commits {}ms, files {}ms, blame {}ms, complexity {}ms, indexes {}ms",
                commits_ms, files_ms, blame_ms, complexity_ms, indexes_ms
            );
        }

        Ok(snapshot)
    }

    /// Phase 2 body: the tracked file tree with every exclusion layer applied
    /// in a single pass. `.baraddurignore` (repo root) sits between the CLI
    /// flags (highest) and the built-in defaults (lowest); its `!` rules
    /// re-include default-excluded files. When nothing excludes a path
    /// `should_include` returns true, so no separate short-circuit is needed.
    /// See `ignore_file::should_include` for the precedence composition.
    fn collect_filtered_files(&self, opts: &SnapshotOptions<'_>) -> Result<Vec<FileEntry>> {
        let all_files = self.collect_files()?;
        let ignore = BaradDurIgnore::load(self.repo_path())?;
        let before = all_files.len();
        let files: Vec<FileEntry> = all_files
            .into_iter()
            .filter(|f| {
                should_include(
                    &ignore,
                    &f.path,
                    opts.exclude_patterns,
                    opts.exclude_extensions,
                    opts.use_default_excludes,
                )
            })
            .collect();
        announce_exclusions(opts.show_progress, before, files.len());
        Ok(files)
    }

    /// Phase 3 body: blame the files changed within the window, reusing and
    /// re-saving the per-blob cache. Empty when `skip_blame` is set.
    ///
    /// Selective blame: files untouched in the window don't affect churn,
    /// coupling, or recent ownership metrics. For bus factor / knowledge
    /// distribution the cached blame from previous runs covers the rest.
    fn resolve_blame_map(
        &self,
        collection: &CommitCollection,
        files: &[FileEntry],
        opts: &SnapshotOptions<'_>,
    ) -> Result<HashMap<PathBuf, Vec<crate::snapshot::BlameLine>>> {
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

        if opts.skip_blame {
            announce_blame_skipped(opts.show_progress, non_binary_count(files));
            return Ok(HashMap::new());
        }

        let blame_cache = if opts.no_cache {
            crate::cache::blame::BlameCache::default()
        } else {
            crate::cache::blame::load(self.repo_path()).unwrap_or_default()
        };
        announce_blame_plan(
            opts.show_progress,
            non_binary_changed,
            non_binary_count(files),
            &blame_cache,
            &blame_files,
        );

        let blame_bar = phase_bar(opts.show_progress, non_binary_changed, "Blaming files");
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

        // Prune stale entries, then persist for the next run
        let current_oids: std::collections::HashSet<String> =
            files.iter().map(|f| f.blob_oid.clone()).collect();
        updated_cache.prune(&current_oids);
        if let Err(e) = crate::cache::blame::save(&updated_cache, self.repo_path()) {
            eprintln!("Warning: Failed to save blame cache: {}", e);
        }
        Ok(map)
    }

    /// Phase 5 body: resolve the AST pass's raw output against the file set
    /// and assemble the snapshot with derived indexes.
    fn assemble_snapshot(
        &self,
        collection: CommitCollection,
        files: Vec<FileEntry>,
        blame_map: HashMap<PathBuf, Vec<crate::snapshot::BlameLine>>,
        ast: RawSourceChannels,
    ) -> Result<RepoSnapshot> {
        let head = self.head_commit_hash()?;
        // Working-tree pass: manifests come from disk.
        let root = self.repo_path().to_path_buf();
        let resolved = ast.resolve(&files, |entry| {
            std::fs::read_to_string(root.join(&entry.path)).ok()
        });
        let provenance = SnapshotProvenance {
            path: self.repo_path().to_path_buf(),
            name: self.repo_name(),
            default_branch: self.default_branch(),
            time_window: self.time_window.clone(),
            head_commit: head,
        };
        Ok(assemble(provenance, collection, files, blame_map, resolved))
    }

    /// Collect a snapshot at a specific commit SHA without touching the
    /// working tree or running the AST pass —
    /// `file_metrics`/`import_graph`/`coupling_findings` stay empty.
    ///
    /// `ignore` is passed in (not loaded here) so a caller doing a historical
    /// sweep can parse the repo's `.baraddurignore` once and reuse it across
    /// every sample.
    ///
    /// `use_default_excludes` mirrors the caller's `cfg.exclude_use_defaults` —
    /// callers must pass the same value they use for their live/HEAD snapshot
    /// so a baseline snapshot is comparable to it.
    ///
    /// No production caller remains as of the per-entity trend feature —
    /// `backfill` switched to [`Self::collect_snapshot_at_with_ast`] to get
    /// per-sample complexity, matching `gate`'s baseline collection. Kept
    /// `#[cfg(test)]` as the AST-free half of this module's own coverage
    /// (e.g. `collect_snapshot_at_without_ast_stays_empty`).
    #[cfg(test)]
    pub(crate) fn collect_snapshot_at(
        repo_path: &Path,
        sha: &str,
        ignore: &BaradDurIgnore,
        use_default_excludes: bool,
    ) -> Result<RepoSnapshot> {
        Self::collect_snapshot_at_inner(repo_path, sha, ignore, use_default_excludes, |_, _| {
            Ok(ResolvedSourceChannels::default())
        })
    }

    /// Same as the AST-free collection above, plus the AST pass over blob
    /// contents — both `gate`'s baseline and `backfill`'s per-sample
    /// collection need coupling/complexity findings from it.
    pub(crate) fn collect_snapshot_at_with_ast(
        repo_path: &Path,
        sha: &str,
        ignore: &BaradDurIgnore,
        use_default_excludes: bool,
    ) -> Result<RepoSnapshot> {
        Self::collect_snapshot_at_inner(repo_path, sha, ignore, use_default_excludes, ast_pass_at)
    }

    fn collect_snapshot_at_inner(
        repo_path: &Path,
        sha: &str,
        ignore: &BaradDurIgnore,
        use_default_excludes: bool,
        ast_pass: impl FnOnce(&git2::Repository, &[FileEntry]) -> Result<ResolvedSourceChannels>,
    ) -> Result<RepoSnapshot> {
        let repo = git2::Repository::discover(repo_path)
            .with_context(|| format!("'{}' is not a git repository", repo_path.display()))?;
        let time_window = TimeWindow::full_history();
        let collection = super::libgit::collect_commits_at(&repo, sha, &time_window)?;

        // Apply the same exclusion policy as `analyze`/`gate` so backfilled history
        // is comparable to live scores: `use_default_excludes` (as configured by
        // the caller) + `.baraddurignore`. Neither backfill nor the gate ratchet's
        // baseline collection has CLI exclude flags. The *current* `.baraddurignore`
        // is applied uniformly to every historical snapshot, so the trend reflects
        // one consistent definition of "relevant files" rather than each commit's own.
        let all_files = super::libgit::collect_files_at(&repo, sha)?;
        let files: Vec<FileEntry> = all_files
            .into_iter()
            .filter(|f| should_include(ignore, &f.path, &[], &[], use_default_excludes))
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

        let resolved = ast_pass(&repo, &files)?;
        let provenance = SnapshotProvenance {
            path: repo_path.to_path_buf(),
            name: repo_name,
            default_branch: branch,
            time_window,
            head_commit: sha.to_string(),
        };
        Ok(assemble(provenance, collection, files, blame_map, resolved))
    }
}

/// What a reader knows about where a snapshot came from; the final
/// assembly copies it verbatim and stamps `created_at` itself.
struct SnapshotProvenance {
    path: PathBuf,
    name: String,
    default_branch: String,
    time_window: TimeWindow,
    head_commit: String,
}

/// The one construction path for both readers: provenance, commit and
/// author data, the exclusion-filtered file list, blame (empty when the
/// reader skipped it), and the resolved source channels, with the derived
/// indexes built once from that final core data.
fn assemble(
    provenance: SnapshotProvenance,
    collection: CommitCollection,
    files: Vec<FileEntry>,
    blame_map: HashMap<PathBuf, Vec<crate::snapshot::BlameLine>>,
    source: ResolvedSourceChannels,
) -> RepoSnapshot {
    let mut snapshot = RepoSnapshot {
        path: provenance.path,
        name: provenance.name,
        default_branch: provenance.default_branch,
        time_window: provenance.time_window,
        head_commit: provenance.head_commit,
        created_at: Utc::now(),
        commits: collection.commits,
        files,
        authors: collection.authors,
        blame_map,
        commits_by_author: HashMap::new(),
        commits_by_file: HashMap::new(),
        file_change_pairs: Vec::new(),
        file_metrics: source.file_metrics,
        import_graph: source.import_graph,
        unreliable_import_specifiers: source.unreliable_import_specifiers,
        coupling_findings: source.coupling_findings,
        class_records: source.class_records,
        reexports: source.reexports,
        call_records: source.call_records,
        commit_interner: collection.interner,
    };
    snapshot.build_indexes();
    snapshot
}

/// AST pass over blob contents at a historical commit — the object-DB
/// equivalent of `collect_file_metrics_with_progress` (which reads the
/// working tree). Used by the gate ratchet's baseline collection; backfill
/// keeps this off per ADR-005. Runs sequentially (no rayon): baseline trees
/// are collected once per gate run, not once per commit like backfill's
/// historical sweep, so the parallelism isn't worth the added complexity here.
fn ast_pass_at(repo: &git2::Repository, files: &[FileEntry]) -> Result<ResolvedSourceChannels> {
    // Bad object ids, missing blobs and non-UTF-8 content are skipped, not
    // errors: a historical tree can legitimately hold what the working
    // tree reader would also fail to read.
    let blob_text = |entry: &FileEntry| -> Option<String> {
        let oid = git2::Oid::from_str(&entry.blob_oid).ok()?;
        let blob = repo.find_blob(oid).ok()?;
        std::str::from_utf8(blob.content()).ok().map(str::to_owned)
    };
    let analyses = files
        .iter()
        .filter(|entry| !entry.is_binary)
        .filter_map(|entry| {
            let content = blob_text(entry)?;
            Some((
                entry.path.clone(),
                complexity::analyse_source(&entry.path, &content),
            ))
        });
    // Historical pass: manifests must come from the tree AT THAT COMMIT.
    // Reading the working tree here would make gate baselines incomparable.
    Ok(RawSourceChannels::aggregate(analyses).resolve(files, blob_text))
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
    fn collect_snapshot_populates_coupling_findings_deterministically() {
        let Ok(collector) = Collector::open(&test_repo_path(), TimeWindow::default()) else {
            return;
        };
        let files = collector.collect_files().expect("should collect files");
        // NoProgress is already imported at the top of snapshot_builder.rs
        // (`use super::progress::{NoProgress, Progress};`) and reaches the
        // tests module via `use super::*`.
        let findings = collector
            .collect_file_metrics_with_progress(&files, &NoProgress)
            .coupling_findings;
        // barad-dur's own code should produce a deterministic, sorted list
        let mut sorted = findings.clone();
        sorted.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
        assert_eq!(findings, sorted, "findings must be sorted by (path, line)");
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

    #[test]
    fn non_binary_count_ignores_binaries() {
        let text = FileEntry {
            path: PathBuf::from("a.rs"),
            size_bytes: 10,
            is_binary: false,
            depth: 1,
            blob_oid: String::new(),
        };
        let bin = FileEntry {
            is_binary: true,
            path: PathBuf::from("a.png"),
            ..text.clone()
        };
        assert_eq!(
            non_binary_count(&[text.clone(), bin.clone(), text.clone()]),
            2
        );
        assert_eq!(non_binary_count(&[bin]), 0);
        assert_eq!(non_binary_count(&[]), 0);
    }

    #[test]
    fn resolve_blame_map_skip_blame_is_empty() {
        let (dir, _head) = make_single_commit_repo_with(&[("src/lib.rs", "pub fn f() {}\n")]);
        let collector = Collector::open(dir.path(), TimeWindow::default()).unwrap();
        let collection = collector.collect_commits().unwrap();
        let files = collector.collect_files().unwrap();
        let opts = SnapshotOptions {
            skip_blame: true,
            ..SnapshotOptions::default()
        };
        let map = collector
            .resolve_blame_map(&collection, &files, &opts)
            .unwrap();
        assert!(map.is_empty(), "skip_blame must produce an empty blame map");
    }

    #[test]
    fn resolve_blame_map_blames_changed_text_files_only() {
        let (dir, _head) = make_single_commit_repo_with(&[("src/lib.rs", "pub fn f() {}\n")]);
        // Add a committed binary file so the !is_binary filter has something
        // to reject.
        std::fs::write(dir.path().join("blob.bin"), [0u8, 159, 146, 150, 0, 7]).unwrap();
        let git = |args: &[&str]| {
            assert!(std::process::Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .status()
                .unwrap()
                .success());
        };
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "add binary"]);

        let collector = Collector::open(dir.path(), TimeWindow::default()).unwrap();
        let collection = collector.collect_commits().unwrap();
        let files = collector.collect_files().unwrap();
        assert!(
            files.iter().any(|f| f.is_binary),
            "fixture must contain a binary file"
        );
        let opts = SnapshotOptions {
            no_cache: true,
            ..SnapshotOptions::default()
        };
        let map = collector
            .resolve_blame_map(&collection, &files, &opts)
            .unwrap();
        assert!(
            map.contains_key(&PathBuf::from("src/lib.rs")),
            "changed text file must be blamed, got keys: {:?}",
            map.keys().collect::<Vec<_>>()
        );
        assert!(
            !map.contains_key(&PathBuf::from("blob.bin")),
            "binary files must never be blamed"
        );
    }

    /// A throwaway git repo with one commit, for `collect_snapshot_at` (backfill
    /// and the gate ratchet's baseline collection).
    fn make_single_commit_repo_with(files: &[(&str, &str)]) -> (tempfile::TempDir, String) {
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
        let (dir, head) =
            make_single_commit_repo_with(&[("main.rs", "fn main() {}\n"), ("Cargo.lock", "x\n")]);
        let ignore = BaradDurIgnore::load(dir.path()).unwrap();
        let snap = Collector::collect_snapshot_at(dir.path(), &head, &ignore, true).unwrap();
        let paths = snapshot_paths(&snap);
        assert!(paths.iter().any(|p| p == "main.rs"));
        assert!(
            !paths.iter().any(|p| p == "Cargo.lock"),
            "Cargo.lock should be excluded by default in backfill too"
        );
    }

    #[test]
    fn collect_snapshot_at_honors_use_default_excludes_flag() {
        // Gate's ratchet baseline must respect `cfg.exclude_use_defaults`, just
        // like the HEAD snapshot does — not hardcode `true`. With the flag off,
        // a default-excluded lockfile must appear in the snapshot; with it on,
        // it must not.
        let (dir, head) = make_single_commit_repo_with(&[
            ("src/lib.rs", "fn lib() {}\n"),
            ("package-lock.json", "{}\n"),
        ]);
        let ignore = BaradDurIgnore::load(dir.path()).unwrap();

        let snap_without_defaults =
            Collector::collect_snapshot_at(dir.path(), &head, &ignore, false).unwrap();
        let paths_without_defaults = snapshot_paths(&snap_without_defaults);
        assert!(
            paths_without_defaults
                .iter()
                .any(|p| p == "package-lock.json"),
            "use_default_excludes=false must keep the lockfile in the snapshot"
        );

        let snap_with_defaults =
            Collector::collect_snapshot_at(dir.path(), &head, &ignore, true).unwrap();
        let paths_with_defaults = snapshot_paths(&snap_with_defaults);
        assert!(
            !paths_with_defaults.iter().any(|p| p == "package-lock.json"),
            "use_default_excludes=true must drop the lockfile from the snapshot"
        );
    }

    #[test]
    fn collect_snapshot_at_honors_baraddurignore() {
        // A working-tree `.baraddurignore` filters historical snapshots as well.
        let (dir, head) =
            make_single_commit_repo_with(&[("main.rs", "fn main() {}\n"), ("keep.rs", "//\n")]);
        std::fs::write(dir.path().join(".baraddurignore"), "keep.rs\n").unwrap();
        let ignore = BaradDurIgnore::load(dir.path()).unwrap();
        let snap = Collector::collect_snapshot_at(dir.path(), &head, &ignore, true).unwrap();
        let paths = snapshot_paths(&snap);
        assert!(!paths.iter().any(|p| p == "keep.rs"));
    }

    #[test]
    fn collect_snapshot_at_with_ast_populates_findings() {
        let (dir, head) = make_single_commit_repo_with(&[(
            "src/lib.rs",
            "static mut CACHE: usize = 0;\npub fn f() {}\n",
        )]);
        let ignore = BaradDurIgnore::load(dir.path()).unwrap();
        let snap =
            Collector::collect_snapshot_at_with_ast(dir.path(), &head, &ignore, true).unwrap();
        assert!(
            !snap.file_metrics.is_empty(),
            "AST pass must populate file_metrics"
        );
        assert_eq!(snap.coupling_findings.len(), 1);
        assert_eq!(
            snap.coupling_findings[0].kind,
            crate::snapshot::CouplingKind::Common
        );
    }

    #[test]
    fn collect_snapshot_at_without_ast_stays_empty() {
        let (dir, head) =
            make_single_commit_repo_with(&[("src/lib.rs", "static mut CACHE: usize = 0;\n")]);
        let ignore = BaradDurIgnore::load(dir.path()).unwrap();
        let snap = Collector::collect_snapshot_at(dir.path(), &head, &ignore, true).unwrap();
        assert!(snap.file_metrics.is_empty(), "ADR-005 contract unchanged");
        assert!(snap.coupling_findings.is_empty());
    }

    #[test]
    fn ast_pass_at_skips_bad_oid_missing_blob_and_non_utf8() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let good = repo.blob(b"static mut CACHE: usize = 0;\n").unwrap();
        let non_utf8 = repo.blob(&[0xff, 0xfe, 0x9f, 0x00]).unwrap();
        let entry = |path: &str, oid: String| FileEntry {
            path: PathBuf::from(path),
            size_bytes: 1,
            is_binary: false,
            depth: 2,
            blob_oid: oid,
        };
        let files = vec![
            entry("src/good.rs", good.to_string()),
            entry("src/bad_oid.rs", "not-a-sha".to_string()),
            // Well-formed oid that exists in no ODB entry:
            entry(
                "src/missing.rs",
                "0123456789abcdef0123456789abcdef01234567".to_string(),
            ),
            entry("src/non_utf8.rs", non_utf8.to_string()),
        ];
        let ResolvedSourceChannels {
            file_metrics: metrics,
            coupling_findings: findings,
            ..
        } = ast_pass_at(&repo, &files).unwrap();
        assert_eq!(
            findings.len(),
            1,
            "only the parseable blob contributes findings; the rest skip silently"
        );
        assert!(metrics.contains_key(Path::new("src/good.rs")));
        assert!(!metrics.contains_key(Path::new("src/bad_oid.rs")));
        assert!(!metrics.contains_key(Path::new("src/missing.rs")));
        assert!(!metrics.contains_key(Path::new("src/non_utf8.rs")));
    }

    #[test]
    fn ast_pass_at_extracts_and_resolves_call_records() {
        use crate::snapshot::{CallRecord, CalleeRef};
        let dir = tempfile::TempDir::new().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let caller_blob = repo
            .blob(b"import { f } from './lib';\nexport function g() { f(); }\n")
            .unwrap();
        let lib_blob = repo.blob(b"export function f() {}\n").unwrap();
        let entry = |path: &str, oid: git2::Oid| FileEntry {
            path: PathBuf::from(path),
            size_bytes: 1,
            is_binary: false,
            depth: 2,
            blob_oid: oid.to_string(),
        };
        let files = vec![
            entry("src/a.ts", caller_blob),
            entry("src/lib.ts", lib_blob),
        ];
        let calls = ast_pass_at(&repo, &files).unwrap().call_records;
        assert_eq!(
            calls,
            vec![CallRecord {
                path: "src/a.ts".into(),
                caller: "g".into(),
                callee: CalleeRef::Resolved {
                    path: "src/lib.ts".into(),
                    name: "f".into(),
                },
                count: 1,
            }],
            "the at-SHA AST pass must extract and resolve call records"
        );
    }
}

#[cfg(test)]
mod assembly_tests {
    use super::*;
    use crate::snapshot::{ChangeType, Commit, FileChange};

    #[test]
    fn assemble_builds_indexes_once_from_final_core_data_and_keeps_provenance() {
        let mut interner = crate::snapshot::CommitInterner::default();
        let change = |path: &str| FileChange {
            path: path.into(),
            additions: 0,
            deletions: 0,
            change_type: ChangeType::Modified,
        };
        // Three co-changes: the pair index keeps pairs seen at least three times.
        let ids: Vec<_> = ["abc123", "def456", "0123ab"]
            .into_iter()
            .map(|sha| interner.intern(sha))
            .collect();
        let commits = ids
            .iter()
            .map(|&id| Commit {
                id,
                author: 0,
                timestamp: Utc::now(),
                message: "m".into(),
                files_changed: vec![
                    change("src/a.rs"),
                    change("src/b.rs"),
                    change("excluded.rs"),
                ],
                is_merge: false,
                parent_count: 1,
            })
            .collect();
        let id = ids[0];
        let collection = CommitCollection {
            commits,
            authors: vec![crate::snapshot::Author {
                id: 0,
                name: "a".into(),
                email: "a@e".into(),
            }],
            interner,
            raw_email_to_id: HashMap::new(),
        };
        let files = vec![
            crate::metrics::testutil::make_file("src/a.rs"),
            crate::metrics::testutil::make_file("src/b.rs"),
        ];
        let provenance = SnapshotProvenance {
            path: PathBuf::from("/tmp/repo"),
            name: "repo".into(),
            default_branch: "trunk".into(),
            time_window: TimeWindow::full_history(),
            head_commit: "abc123".into(),
        };
        let snapshot = assemble(
            provenance,
            collection,
            files,
            HashMap::new(),
            ResolvedSourceChannels::default(),
        );
        assert_eq!(snapshot.name, "repo");
        assert_eq!(snapshot.default_branch, "trunk");
        assert_eq!(snapshot.head_commit, "abc123");
        assert_eq!(snapshot.path, PathBuf::from("/tmp/repo"));
        assert_eq!(snapshot.resolve_commit(id), "abc123");
        assert_eq!(snapshot.commits_by_author[&0], ids);
        assert_eq!(snapshot.commits_by_file.len(), 3, "index over commit data");
        assert_eq!(
            snapshot.file_change_pairs,
            vec![("src/a.rs".into(), "src/b.rs".into(), 3)],
            "pairs only over listed files, excluded path dropped"
        );
    }
}
