//! Trends M1 walking skeleton: the day-bucketed churn timeline flows
//! through `analyze --json` with exact per-day sums, a zero-filled gap
//! day, and coupling pairs carrying per-side net growth.

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

/// Append `n` lines to a file (pure additions git counts exactly).
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

#[test]
fn churn_timeline_and_pair_growth_flow_end_to_end() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let d = dir.path();
    git(d, "", &["init", "-q"]);
    git(d, "", &["config", "user.email", "t@t"]);
    git(d, "", &["config", "user.name", "T"]);

    // One captured instant, one hour in the past: every stamp AND every
    // expected date derives from it, so the test cannot fail from a fixed
    // future-dated hour (pre-10:00-UTC runs) or a midnight crossing
    // between commit stamping and assertion.
    let base = Utc::now() - Duration::hours(1);
    let day = |offset: i64| {
        (base - Duration::days(offset))
            .format("%Y-%m-%d")
            .to_string()
    };
    let stamp = |offset: i64| {
        (base - Duration::days(offset))
            .format("%Y-%m-%dT%H:%M:%S +0000")
            .to_string()
    };

    // Day −3: a.rs +5 and b.rs +3 (their first co-change).
    append_lines(d, "a.rs", 5, "d3a");
    append_lines(d, "b.rs", 3, "d3b");
    git(d, &stamp(3), &["add", "-A"]);
    git(d, &stamp(3), &["commit", "-q", "-m", "day three"]);
    // Day −2: silence (the gap bucket).
    // Day −1: two commits — a.rs +4, then a.rs +2 b.rs +1 (co-changes 2, 3).
    append_lines(d, "a.rs", 4, "d1a");
    git(d, &stamp(1), &["add", "-A"]);
    git(d, &stamp(1), &["commit", "-q", "-m", "day one first"]);
    append_lines(d, "a.rs", 2, "d1b");
    append_lines(d, "b.rs", 1, "d1c");
    git(d, &stamp(1), &["add", "-A"]);
    git(d, &stamp(1), &["commit", "-q", "-m", "day one second"]);
    // Third a.rs+b.rs co-change so the pair clears the >=3 floor.
    append_lines(d, "a.rs", 1, "d0a");
    append_lines(d, "b.rs", 1, "d0b");
    git(d, &stamp(0), &["add", "-A"]);
    git(d, &stamp(0), &["commit", "-q", "-m", "today"]);

    let out = Command::new(env!("CARGO_BIN_EXE_barad-dur"))
        .arg("analyze")
        .arg(d)
        .arg("--json")
        .arg("--no-cache")
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "analyze failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");

    let t = &report["churn_timeline"];
    assert_eq!(t["bucket_days"], 1);
    assert_eq!(t["merge_commits_excluded"], true);
    assert_eq!(
        t["buckets"],
        serde_json::json!([
            { "date": day(3), "added": 8, "deleted": 0 },
            { "date": day(2), "added": 0, "deleted": 0 },
            { "date": day(1), "added": 7, "deleted": 0 },
            { "date": day(0), "added": 2, "deleted": 0 },
        ])
    );

    let pairs = report["coupling_pairs"].as_array().expect("pairs");
    let pair = pairs
        .iter()
        .find(|p| p["file_a"] == "a.rs" && p["file_b"] == "b.rs")
        .expect("a.rs/b.rs pair");
    assert_eq!(pair["growth_a"], 12, "a.rs net lines added in window");
    assert_eq!(pair["growth_b"], 5, "b.rs net lines added in window");
}
