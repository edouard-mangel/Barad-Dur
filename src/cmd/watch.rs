use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

use crate::cli::WatchArgs;

const HOOK_MARKER: &str = "# barad-dur watch hook";

pub fn run_watch(args: WatchArgs) -> Result<()> {
    let repo_path = PathBuf::from(&args.target);
    let hook_path = find_hook_path(&repo_path)?;

    if args.uninstall {
        uninstall_hook(&hook_path)
    } else {
        install_hook(&hook_path, args.skip_blame, args.force)
    }
}

fn find_hook_path(repo_path: &Path) -> Result<PathBuf> {
    // Walk up to find the .git directory.
    let git_dir = repo_path
        .canonicalize()
        .context("could not resolve repo path")?;
    let git_dir = git_dir.join(".git");
    if !git_dir.exists() {
        bail!(
            "no .git directory found under '{}'",
            repo_path.display()
        );
    }
    let hooks_dir = git_dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir).context("failed to create .git/hooks")?;
    Ok(hooks_dir.join("post-commit"))
}

fn install_hook(hook_path: &Path, skip_blame: bool, force: bool) -> Result<()> {
    if hook_path.exists() && !force {
        // Allow overwrite only if the existing hook was installed by us.
        let existing = std::fs::read_to_string(hook_path)
            .context("failed to read existing hook")?;
        if !existing.contains(HOOK_MARKER) {
            bail!(
                "a post-commit hook already exists at '{}' and was not installed by barad-dur.\n\
                 Use --force to overwrite it.",
                hook_path.display()
            );
        }
    }

    let skip_flag = if skip_blame { " --skip-blame" } else { "" };
    let script = format!(
        "#!/bin/sh\n\
         {marker}\n\
         # Runs after every commit to track repository score changes.\n\
         # Remove with: barad-dur watch --uninstall\n\
         \n\
         barad-dur analyze .{skip_flag} --trend 2>/dev/null || true\n",
        marker = HOOK_MARKER,
        skip_flag = skip_flag,
    );

    std::fs::write(hook_path, &script)
        .with_context(|| format!("failed to write hook to '{}'", hook_path.display()))?;

    // Make the hook executable (Unix only; Windows ignores this).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(hook_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(hook_path, perms)
            .context("failed to set hook executable")?;
    }

    println!(
        "Hook installed at '{}'. barad-dur will run after each commit.",
        hook_path.display()
    );
    if skip_blame {
        println!("Note: --skip-blame is active; blame-dependent metrics will use defaults.");
    }
    Ok(())
}

fn uninstall_hook(hook_path: &Path) -> Result<()> {
    if !hook_path.exists() {
        println!("No post-commit hook found — nothing to uninstall.");
        return Ok(());
    }

    let content = std::fs::read_to_string(hook_path)
        .context("failed to read hook file")?;
    if !content.contains(HOOK_MARKER) {
        bail!(
            "the hook at '{}' was not installed by barad-dur and will not be removed.\n\
             Remove it manually if you no longer need it.",
            hook_path.display()
        );
    }

    std::fs::remove_file(hook_path)
        .with_context(|| format!("failed to remove hook at '{}'", hook_path.display()))?;

    println!("Hook removed from '{}'.", hook_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fake_git_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".git/hooks")).unwrap();
        dir
    }

    #[test]
    fn install_creates_executable_hook() {
        let dir = fake_git_repo();
        let hook = dir.path().join(".git/hooks/post-commit");

        install_hook(&hook, false, false).unwrap();

        assert!(hook.exists());
        let content = std::fs::read_to_string(&hook).unwrap();
        assert!(content.contains(HOOK_MARKER));
        assert!(content.contains("barad-dur analyze"));
        assert!(!content.contains("--skip-blame"));
    }

    #[test]
    fn install_with_skip_blame_adds_flag() {
        let dir = fake_git_repo();
        let hook = dir.path().join(".git/hooks/post-commit");

        install_hook(&hook, true, false).unwrap();

        let content = std::fs::read_to_string(&hook).unwrap();
        assert!(content.contains("--skip-blame"));
    }

    #[test]
    fn install_refuses_to_overwrite_foreign_hook_without_force() {
        let dir = fake_git_repo();
        let hook = dir.path().join(".git/hooks/post-commit");
        std::fs::write(&hook, "#!/bin/sh\necho foreign hook\n").unwrap();

        let result = install_hook(&hook, false, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not installed by barad-dur"));
    }

    #[test]
    fn install_force_overwrites_foreign_hook() {
        let dir = fake_git_repo();
        let hook = dir.path().join(".git/hooks/post-commit");
        std::fs::write(&hook, "#!/bin/sh\necho foreign hook\n").unwrap();

        install_hook(&hook, false, true).unwrap();

        let content = std::fs::read_to_string(&hook).unwrap();
        assert!(content.contains(HOOK_MARKER));
    }

    #[test]
    fn reinstall_own_hook_without_force() {
        let dir = fake_git_repo();
        let hook = dir.path().join(".git/hooks/post-commit");
        install_hook(&hook, false, false).unwrap();

        // Second install without --force should succeed (it's our own hook).
        install_hook(&hook, true, false).unwrap();
        let content = std::fs::read_to_string(&hook).unwrap();
        assert!(content.contains("--skip-blame"));
    }

    #[test]
    fn uninstall_removes_own_hook() {
        let dir = fake_git_repo();
        let hook = dir.path().join(".git/hooks/post-commit");
        install_hook(&hook, false, false).unwrap();
        assert!(hook.exists());

        uninstall_hook(&hook).unwrap();
        assert!(!hook.exists());
    }

    #[test]
    fn uninstall_no_hook_is_noop() {
        let dir = fake_git_repo();
        let hook = dir.path().join(".git/hooks/post-commit");
        // Should not panic or error when no hook exists.
        uninstall_hook(&hook).unwrap();
    }

    #[test]
    fn uninstall_refuses_foreign_hook() {
        let dir = fake_git_repo();
        let hook = dir.path().join(".git/hooks/post-commit");
        std::fs::write(&hook, "#!/bin/sh\necho foreign\n").unwrap();

        let result = uninstall_hook(&hook);
        assert!(result.is_err());
        assert!(hook.exists(), "foreign hook must not be deleted");
    }
}
