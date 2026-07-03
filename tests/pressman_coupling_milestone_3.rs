//! M3 milestone E2E: `gate --no-new-coupling` / `--max-new-coupling` ratchet
//! checks against the actual installed-from-source binary.
//!
//! `run_gate` and `Collector::collect_snapshot_at` are crate-internal, so
//! this drives the binary itself (`CARGO_BIN_EXE_barad-dur`, cargo's
//! standard integration-test binary path) against a throwaway two-commit
//! fixture repo: a clean baseline `src/lib.rs`, then a commit that
//! introduces a `static mut` shared mutable global (a Pressman
//! common-coupling finding).

use std::path::Path;
use std::process::{Command, Output};

/// Build a two-commit fixture repo in a fresh `TempDir`: commit 1 is a clean
/// `src/lib.rs`, commit 2 adds `static mut CACHE: usize = 0;` to it. Returns
/// the temp dir (must be kept alive by the caller) and the base commit's SHA
/// (`HEAD~1` at the end of setup).
fn fixture_repo() -> (tempfile::TempDir, String) {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let git = |args: &[&str]| -> Output {
        let output = Command::new("git")
            .current_dir(dir.path())
            .args(args)
            .output()
            .expect("git invocation must spawn");
        assert!(
            output.status.success(),
            "git {args:?} failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    };

    git(&["init", "-q"]);
    git(&["config", "user.email", "fixture@example.com"]);
    git(&["config", "user.name", "Fixture"]);

    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("mkdir src");
    let lib_rs = src_dir.join("lib.rs");

    std::fs::write(&lib_rs, "pub fn f() {}\n").expect("write clean lib.rs");
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "clean baseline"]);

    let base_sha = {
        let out = git(&["rev-parse", "HEAD"]);
        String::from_utf8(out.stdout)
            .expect("git rev-parse output must be utf8")
            .trim()
            .to_string()
    };

    std::fs::write(&lib_rs, "static mut CACHE: usize = 0;\npub fn f() {}\n")
        .expect("write mutable-global lib.rs");
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "introduce shared mutable global"]);

    (dir, base_sha)
}

/// Run the built binary against `target_dir` with `gate` and the given
/// extra args.
fn run_gate(target_dir: &Path, extra_args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_barad-dur"))
        .arg("gate")
        .arg(target_dir)
        .args(extra_args)
        .output()
        .expect("barad-dur binary invocation must spawn")
}

/// stdout + stderr concatenated, for panic messages and substring checks
/// that don't care which stream a hint landed on.
fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn ratchet_fails_and_names_new_common_coupling_finding() {
    let (dir, base_sha) = fixture_repo();
    let output = run_gate(
        dir.path(),
        &[
            "--min-score",
            "0",
            "--no-new-coupling",
            "--baseline-ref",
            &base_sha,
        ],
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "expected non-zero exit for a new coupling finding; full output:\n{}",
        combined(&output)
    );
    assert!(
        stdout.contains("static mut CACHE"),
        "expected stdout to name the new finding's evidence; full output:\n{}",
        combined(&output)
    );
    assert!(
        stdout.contains("common"),
        "expected stdout to mention the 'common' coupling kind; full output:\n{}",
        combined(&output)
    );
}

#[test]
fn max_new_coupling_allowance_of_one_passes() {
    let (dir, base_sha) = fixture_repo();
    let output = run_gate(
        dir.path(),
        &[
            "--min-score",
            "0",
            "--max-new-coupling",
            "1",
            "--baseline-ref",
            &base_sha,
        ],
    );

    assert!(
        output.status.success(),
        "expected exit 0: exactly one new finding, allowance of one; full output:\n{}",
        combined(&output)
    );
}

#[test]
fn unresolvable_baseline_ref_hints_git_depth() {
    let (dir, _base_sha) = fixture_repo();
    let output = run_gate(
        dir.path(),
        &[
            "--min-score",
            "0",
            "--no-new-coupling",
            "--baseline-ref",
            "does-not-exist",
        ],
    );

    assert!(
        !output.status.success(),
        "expected non-zero exit for an unresolvable baseline ref; full output:\n{}",
        combined(&output)
    );
    assert!(
        combined(&output).contains("GIT_DEPTH"),
        "expected the GIT_DEPTH shallow-clone remediation hint; full output:\n{}",
        combined(&output)
    );
}

#[test]
fn no_new_coupling_without_baseline_ref_is_a_clap_usage_error() {
    let (dir, _base_sha) = fixture_repo();
    let output = run_gate(dir.path(), &["--min-score", "0", "--no-new-coupling"]);

    assert!(
        !output.status.success(),
        "expected non-zero exit: --no-new-coupling requires --baseline-ref; full output:\n{}",
        combined(&output)
    );
    assert!(
        combined(&output).contains("--baseline-ref"),
        "expected clap's usage error to name --baseline-ref; full output:\n{}",
        combined(&output)
    );
}
