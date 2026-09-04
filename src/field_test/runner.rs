//! Runs `barad-dur analyze` against a pinned corpus repository inside a
//! throwaway [`Worktree`], optionally twice, to catch nondeterministic
//! output before it reaches a committed baseline.

use crate::field_test::diff::{diff_surfaces, SurfaceDiff};
use crate::field_test::surface::{extract_surface, DecisionSurface};
use crate::field_test::worktree::Worktree;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Fixed lower bound used to seed the committed corpus baselines. Keeping it
/// in source makes repeated runs independent of the current date without the
/// prohibitive cost of scanning the complete history of very large repos.
const CORPUS_HISTORY_SINCE: &str = "2026-03-01";

/// The result of analysing one corpus repository.
#[derive(Debug, Clone)]
pub struct RepoOutcome {
    pub name: String,
    pub surface: DecisionSurface,
    /// `Some` when two passes over identical input disagreed.
    pub nondeterminism: Option<SurfaceDiff>,
}

/// Fold N measured passes into an outcome. Pure, so determinism handling is
/// testable without running an analysis. Private: the only public entry
/// point is `analyze_pinned`, which always supplies at least one pass
/// (`passes == 0` is floored to 1) — keeping this function module-internal
/// removes the empty-`Vec` footgun from the public API instead of just
/// documenting around it.
fn outcome_from_passes(name: &str, passes: Vec<DecisionSurface>) -> RepoOutcome {
    let nondeterminism = passes
        .first()
        .zip(passes.get(1))
        .map(|(a, b)| diff_surfaces(a, b))
        .filter(|d| !d.is_empty());

    RepoOutcome {
        name: name.to_string(),
        surface: passes
            .into_iter()
            .next()
            .expect("callers always supply at least one pass"),
        nondeterminism,
    }
}

/// Resolve `path` to an absolute path without requiring it to exist yet.
///
/// `Worktree::add` runs `git` with its working directory set to the corpus
/// repository being checked out, not the caller's cwd — a relative worktree
/// path would silently resolve against the wrong directory. Every path this
/// module hands to `Worktree::add` goes through here first.
fn ensure_absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .context("resolving current directory to make the worktree scratch path absolute")
            .map(|cwd| cwd.join(path))
    }
}

/// Where one repo/pass's full normalized report is archived on disk.
///
/// The archive is what makes historical drill-down possible without
/// rerunning the analysis (`archive/` is gitignored scratch, not the
/// committed baseline — see `field-test/archive/` in the harness layout).
/// Pure and independent of `analyze_once` so the naming scheme is testable
/// without shelling out to the binary.
fn archive_path(archive: &Path, name: &str, pass: u8) -> PathBuf {
    archive.join(format!("{name}-{pass}.json"))
}

/// Run one analysis pass against `target`, parsing the report from stdout
/// and archiving the raw JSON at `archive_file`.
fn analyze_once(
    binary: &Path,
    target: &Path,
    repo_name: &str,
    archive_file: &Path,
) -> Result<DecisionSurface> {
    let output = Command::new(binary)
        .arg("analyze")
        .arg(target)
        .arg("--json")
        .arg("--no-cache")
        .arg("--since")
        .arg(CORPUS_HISTORY_SINCE)
        .output()
        .with_context(|| format!("running {} against {}", binary.display(), target.display()))?;

    if !output.status.success() {
        bail!(
            "analysis of {repo_name} failed (exit {}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let stdout = String::from_utf8(output.stdout)
        .with_context(|| format!("decoding stdout from analysing {repo_name}"))?;

    std::fs::write(archive_file, &stdout).with_context(|| {
        format!(
            "archiving report for {repo_name} at {}",
            archive_file.display()
        )
    })?;

    let report: serde_json::Value = serde_json::from_str(&stdout)
        .with_context(|| format!("parsing JSON report for {repo_name}"))?;
    Ok(extract_surface(&report))
}

/// Analyse `repo` at `pin` inside a throwaway worktree under `archive`,
/// `passes` times (`passes == 0` is floored to 1 — there's always at least
/// one measured pass). Each pass gets its own worktree so `--no-cache`
/// isn't the only thing standing between the two runs and a
/// trivially-matching cache hit — the second pass starts from a completely
/// fresh checkout. The full raw report from each pass is written under
/// `archive` for later drill-down.
pub fn analyze_pinned(
    binary: &Path,
    name: &str,
    repo: &Path,
    pin: &str,
    archive: &Path,
    passes: u8,
) -> Result<RepoOutcome> {
    let archive = ensure_absolute(archive)?;
    std::fs::create_dir_all(&archive)
        .with_context(|| format!("creating archive directory {}", archive.display()))?;

    let measured = (0..passes.max(1))
        .map(|i| {
            let wt_dir = archive.join(format!("{name}-{i}"));
            let worktree = Worktree::add(repo, pin, &wt_dir)
                .with_context(|| format!("checking out {name} at {pin} for pass {i}"))?;
            let archive_file = archive_path(&archive, name, i);
            analyze_once(binary, worktree.path(), name, &archive_file)
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(outcome_from_passes(name, measured))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field_test::surface::DecisionSurface;
    use std::collections::BTreeMap;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn surface(score: i64) -> DecisionSurface {
        DecisionSurface {
            overall_score: Some(score),
            total_files: 1,
            total_commits: 1,
            total_authors: 1,
            score_thresholds: BTreeMap::new(),
            coupling_finding_counts: BTreeMap::new(),
            categories: vec![],
            actions: vec![],
            top_hotspots: vec![],
        }
    }

    #[test]
    fn two_identical_passes_report_no_nondeterminism() {
        let outcome = outcome_from_passes("ripgrep", vec![surface(55), surface(55)]);
        assert!(outcome.nondeterminism.is_none());
        assert_eq!(outcome.surface.overall_score, Some(55));
    }

    #[test]
    fn differing_passes_are_flagged_as_nondeterminism() {
        let outcome = outcome_from_passes("ripgrep", vec![surface(55), surface(56)]);
        let nd = outcome.nondeterminism.expect("nondeterminism detected");
        assert!(nd.render().contains("overall_score"));
    }

    #[test]
    fn a_single_pass_cannot_report_nondeterminism() {
        let outcome = outcome_from_passes("ripgrep", vec![surface(55)]);
        assert!(outcome.nondeterminism.is_none());
    }

    #[test]
    fn archive_path_names_the_report_by_repo_and_pass() {
        let p = archive_path(Path::new("/scratch/archive"), "ripgrep", 1);
        assert_eq!(p, PathBuf::from("/scratch/archive/ripgrep-1.json"));
    }

    #[test]
    fn ensure_absolute_leaves_an_absolute_path_unchanged() {
        let abs = std::env::current_dir().unwrap().join("already-absolute");
        assert_eq!(ensure_absolute(&abs).unwrap(), abs);
    }

    #[test]
    fn ensure_absolute_joins_a_relative_path_onto_the_current_directory() {
        let cwd = std::env::current_dir().unwrap();
        let resolved = ensure_absolute(Path::new("relative/scratch")).unwrap();
        assert_eq!(resolved, cwd.join("relative/scratch"));
    }

    #[cfg(unix)]
    #[test]
    fn analysis_uses_a_committed_time_stable_history_window() {
        let dir = tempfile::tempdir().expect("tempdir");
        let binary = dir.path().join("fake-barad-dur");
        let captured = dir.path().join("args");
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nprintf '{{}}'\n",
            captured.display()
        );
        std::fs::write(&binary, script).expect("write fake binary");
        let mut permissions = std::fs::metadata(&binary).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&binary, permissions).expect("make fake binary executable");

        analyze_once(
            &binary,
            Path::new("/pinned/repository"),
            "fixture",
            &dir.path().join("archive.json"),
        )
        .expect("fake analysis succeeds");

        let args = std::fs::read_to_string(captured).expect("read captured arguments");
        let args = args.lines().collect::<Vec<_>>();
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--since", "2026-03-01"]),
            "a pinned corpus must use its committed history boundary, got: {args:?}"
        );
        assert!(!args.contains(&"--all"));
    }
}
