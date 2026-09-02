//! Throwaway `git worktree`s used to isolate field-test analysis from the
//! corpus repositories it inspects.
//!
//! `barad-dur analyze` writes into the repository it analyses — it creates
//! `.repository-analysis/` and appends to `.gitignore`. Running it directly
//! against a corpus repository (the maintainer's real working repositories,
//! on whatever branch, possibly with uncommitted work) would dirty it. Every
//! analysis in the harness runs inside a [`Worktree`] instead: a detached
//! checkout of a pinned commit in a throwaway directory, removed on drop.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// A throwaway `git worktree` checked out at a pinned commit.
#[derive(Debug)]
pub struct Worktree {
    repo: PathBuf,
    dir: PathBuf,
}

fn run_git(repo: &Path, args: &[&str]) -> Result<()> {
    let out = Command::new("git").current_dir(repo).args(args).output()?;
    if !out.status.success() {
        bail!(
            "git {} failed in {}: {}",
            args.join(" "),
            repo.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

impl Worktree {
    /// Check `pin` out into `dir` as a detached worktree of `repo`.
    ///
    /// On failure, no `Worktree` value is produced and nothing is left
    /// behind: `git worktree add` either creates a complete worktree or (per
    /// git's own behavior) fails without partially creating one.
    pub fn add(repo: &Path, pin: &str, dir: &Path) -> Result<Self> {
        let dir_s = dir.to_string_lossy().to_string();
        run_git(
            repo,
            &["worktree", "add", "--detach", "--quiet", &dir_s, pin],
        )
        .with_context(|| {
            format!(
                "adding worktree for {} at pin {pin} in {}",
                repo.display(),
                dir.display()
            )
        })?;
        Ok(Self {
            repo: repo.to_path_buf(),
            dir: dir.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Worktree {
    fn drop(&mut self) {
        // `Drop` cannot propagate a `Result`, and this runs on every
        // exit path (including panics unwinding through analysis) where
        // the whole point is to guarantee removal of the throwaway
        // checkout. Panicking here would either abort the process (if
        // already unwinding) or mask the original failure with a
        // cleanup failure; failing the harness's own error reporting
        // isn't the corpus repository's problem. So: best-effort
        // removal — but *reported*, never silent. `--force` because
        // analysis is expected to dirty the worktree (that's the whole
        // reason it's isolated here).
        //
        // `git worktree remove` already deletes this worktree's own
        // administrative files, so there is deliberately no follow-up
        // `git worktree prune`: prune is unscoped, and it will happily
        // drop the registration of a *live* worktree belonging to
        // somebody else whose directory is momentarily unreachable (an
        // unmounted volume, a network share). It once did exactly that
        // to an unrelated session. The cost of dropping it is that a
        // failed removal now leaves a stale registration behind, and the
        // worktree directory name repeats every run — so the next run's
        // `worktree add` fails on that path. Hence the message: it is the
        // operator's only warning that manual cleanup is needed.
        let dir_s = self.dir.to_string_lossy().to_string();
        if let Err(err) = run_git(&self.repo, &["worktree", "remove", "--force", &dir_s]) {
            eprintln!(
                "field-test: failed to remove the throwaway worktree {} of {}: {err:#}\n\
                 the stale registration will make the next run's `git worktree add` \
                 fail on that path — clean it up with `git -C {} worktree prune`",
                self.dir.display(),
                self.repo.display(),
                self.repo.display()
            );
        }
    }
}
