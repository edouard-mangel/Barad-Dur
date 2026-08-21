//! Test safety-net walking skeleton (Crime Scene Ch. 9): a repo with one
//! eroding source/test pair (co-change ratio below the 30% threshold) and
//! one healthy pair (always co-changing) flows through `analyze --json`
//! with the exact score, description, and evidence the metric promises —
//! and the metric survives a backfilled snapshot without panicking.
//!
//! Fixture arithmetic (worked out explicitly, not just asserted after the
//! fact):
//!
//! - `lib.rs` / `lib_test.rs` (eroding): one commit touches both files
//!   together (the only shared commit), then 9 more commits touch only
//!   `lib.rs` and 3 more touch only `lib_test.rs`. Totals: lib.rs = 1 + 9 =
//!   10 commits, lib_test.rs = 1 + 3 = 4 commits, shared = 1. Ratio =
//!   shared / min(commits_a, commits_b) = 1 / min(10, 4) = 1 / 4 = 0.25,
//!   strictly below the default 0.30 threshold → flagged as eroding.
//! - `util.rs` / `util_test.rs` (healthy, true negative): 3 commits, every
//!   one of which touches both files together. Totals: util.rs = 3,
//!   util_test.rs = 3, shared = 3. Ratio = 3 / min(3, 3) = 3 / 3 = 1.0 →
//!   NOT eroding, must not appear in the evidence list.
//!
//! Two Source files have a naming-convention Test candidate in this repo
//! (lib.rs, util.rs), so "checked" = 2 and "flagged" = 1.

use std::path::Path;
use std::process::{Command, Output};

use chrono::{Duration, Utc};

fn git(dir: &Path, date: &str, args: &[&str]) -> Output {
    let mut c = Command::new("git");
    c.current_dir(dir).args(args);
    // Isolate from the developer's global config (gpg signing, hooks).
    c.env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null");
    if !date.is_empty() {
        c.env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date);
    }
    let out = c.output().expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

/// Append `n` lines to a file (pure additions git counts exactly); creates
/// the file on first use.
fn append_lines(dir: &Path, file: &str, n: usize, tag: &str) {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(file))
        .unwrap();
    for i in 0..n {
        writeln!(f, "// {tag} {i}").unwrap();
    }
}

/// Builds the fixture described in the module doc comment into `dir`, which
/// must already be an empty directory. One captured instant, one hour in
/// the past, so the fixture cannot fail from a fixed future-dated hour —
/// every commit stamp derives from it, counting hours backward so commits
/// land in chronological order.
fn build_fixture(dir: &Path) {
    git(dir, "", &["init", "-q"]);
    git(dir, "", &["config", "user.email", "t@t"]);
    git(dir, "", &["config", "user.name", "T"]);

    let base = Utc::now() - Duration::hours(1);
    let stamp = |offset: i64| {
        (base - Duration::hours(offset))
            .format("%Y-%m-%dT%H:%M:%S +0000")
            .to_string()
    };

    // 16 commits total, oldest (largest offset) first.
    let mut offset = 16i64;
    let mut next_stamp = || {
        let s = stamp(offset);
        offset -= 1;
        s
    };

    // Commit 1 (shared, the only lib.rs/lib_test.rs co-change): create both
    // files together.
    append_lines(dir, "lib.rs", 1, "shared");
    append_lines(dir, "lib_test.rs", 1, "shared");
    let s = next_stamp();
    git(dir, &s, &["add", "-A"]);
    git(dir, &s, &["commit", "-q", "-m", "lib: initial + test"]);

    // Commits 2-10: lib.rs alone, 9 times -> lib.rs reaches 10 commits.
    for i in 0..9 {
        append_lines(dir, "lib.rs", 1, &format!("solo{i}"));
        let s = next_stamp();
        git(dir, &s, &["add", "-A"]);
        git(
            dir,
            &s,
            &["commit", "-q", "-m", &format!("lib.rs solo {i}")],
        );
    }

    // Commits 11-13: lib_test.rs alone, 3 times -> lib_test.rs reaches 4
    // commits (1 shared + 3 solo), while lib.rs stays at 10 -> ratio
    // 1/min(10,4) = 1/4 = 0.25 < 0.30, eroding.
    for i in 0..3 {
        append_lines(dir, "lib_test.rs", 1, &format!("solo{i}"));
        let s = next_stamp();
        git(dir, &s, &["add", "-A"]);
        git(
            dir,
            &s,
            &["commit", "-q", "-m", &format!("lib_test.rs solo {i}")],
        );
    }

    // Commits 14-16: util.rs/util_test.rs always co-change, 3 times ->
    // ratio 3/min(3,3) = 1.0, healthy, must not be flagged.
    for i in 0..3 {
        append_lines(dir, "util.rs", 1, &format!("both{i}"));
        append_lines(dir, "util_test.rs", 1, &format!("both{i}"));
        let s = next_stamp();
        git(dir, &s, &["add", "-A"]);
        git(dir, &s, &["commit", "-q", "-m", &format!("util pair {i}")]);
    }
}

fn analyze_json(dir: &Path) -> serde_json::Value {
    let out = Command::new(env!("CARGO_BIN_EXE_barad-dur"))
        .arg("analyze")
        .arg(dir)
        .arg("--json")
        .arg("--no-cache")
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "analyze failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("json")
}

#[test]
fn eroding_pair_flagged_healthy_pair_is_not() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    build_fixture(dir.path());

    let report = analyze_json(dir.path());

    let coupling_cat = report["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "Coupling")
        .expect("coupling category");
    let metric = coupling_cat["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "Test safety net")
        .expect("Test safety net metric row");

    assert_eq!(metric["score"], 75, "1 of 2 pairs eroding -> 75 band");
    assert_eq!(
        metric["description"],
        "1 of 2 source/test pairs below 30% co-change — safety net eroding"
    );

    let evidence = metric["raw_value"]["List"]
        .as_array()
        .expect("List raw_value")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect::<Vec<_>>();

    assert_eq!(
        evidence.len(),
        1,
        "only the eroding pair should be listed: {evidence:?}"
    );
    assert!(
        evidence[0].contains("lib.rs") && evidence[0].contains("lib_test.rs"),
        "evidence should name the eroding lib.rs/lib_test.rs pair: {evidence:?}"
    );
    assert!(
        evidence[0].contains("25%"),
        "1 shared / min(10,4)=4 -> 25% co-change: {evidence:?}"
    );
    assert!(
        !evidence.iter().any(|e| e.contains("util")),
        "the always-co-changing util.rs/util_test.rs pair must NOT appear: {evidence:?}"
    );
}

/// Interactions section: `file_change_pairs`/`commits_by_file` exist on a
/// backfilled snapshot (unlike AST data), so the metric must produce a
/// value rather than panicking or silently going N/A. The fixture is tiny,
/// so this stays cheap.
#[test]
fn survives_backfilled_snapshot_without_panicking() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    build_fixture(dir.path());

    let out = Command::new(env!("CARGO_BIN_EXE_barad-dur"))
        .arg("backfill")
        .arg(dir.path())
        .output()
        .expect("run backfill");
    assert!(
        out.status.success(),
        "backfill failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
