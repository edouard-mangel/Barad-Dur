use barad_dur::field_test::worktree::Worktree;
use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git runs");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Build a throwaway repo with two commits and return (dir, first_sha).
fn fixture_repo() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    git(p, &["config", "user.email", "t@example.com"]);
    git(p, &["config", "user.name", "Test"]);
    std::fs::write(p.join("a.txt"), "one").expect("write");
    git(p, &["add", "."]);
    git(p, &["commit", "-qm", "first"]);
    let first = git(p, &["rev-parse", "HEAD"]);
    std::fs::write(p.join("a.txt"), "two").expect("write");
    git(p, &["commit", "-qam", "second"]);
    (dir, first)
}

#[test]
fn worktree_checks_out_the_pin_and_cleans_up_after_itself() {
    let (repo, first) = fixture_repo();
    let scratch = tempfile::tempdir().expect("tempdir");
    let wt_dir = scratch.path().join("wt");

    {
        let wt = Worktree::add(repo.path(), &first, &wt_dir).expect("worktree added");
        assert_eq!(
            std::fs::read_to_string(wt.path().join("a.txt")).expect("read"),
            "one",
            "worktree must be checked out at the pinned commit, not HEAD"
        );
    }

    assert!(!wt_dir.exists(), "worktree directory removed on drop");
    assert_eq!(
        git(repo.path(), &["status", "--short"]),
        "",
        "the source repository must be left untouched"
    );
}

#[test]
fn worktree_removal_survives_a_dirtied_working_tree() {
    let (repo, first) = fixture_repo();
    let scratch = tempfile::tempdir().expect("tempdir");
    let wt_dir = scratch.path().join("wt");

    {
        let wt = Worktree::add(repo.path(), &first, &wt_dir).expect("worktree added");
        // barad-dur analyze does exactly this to its target.
        std::fs::write(wt.path().join(".gitignore"), ".repository-analysis/\n").expect("write");
        std::fs::create_dir_all(wt.path().join(".repository-analysis")).expect("mkdir");
    }

    assert!(!wt_dir.exists(), "a dirtied worktree must still be removed");
    assert_eq!(git(repo.path(), &["status", "--short"]), "");
}
