//! Cross-team coupling E2E: two files, each blame-dominated by a
//! different author, co-changing across same-day separate commits,
//! surface a Team-category finding naming both owners.

use chrono::{Duration, Utc};
use std::path::Path;
use std::process::{Command, Output};

fn git(dir: &Path, envs: &[(&str, &str)], args: &[&str]) -> Output {
    let mut c = Command::new("git");
    c.current_dir(dir).args(args);
    for (k, v) in envs {
        c.env(k, v);
    }
    let out = c.output().expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn as_author(name: &str, date: &str) -> Vec<(&'static str, String)> {
    vec![
        ("GIT_AUTHOR_NAME", name.to_string()),
        ("GIT_AUTHOR_EMAIL", format!("{name}@t")),
        ("GIT_AUTHOR_DATE", date.to_string()),
        ("GIT_COMMITTER_NAME", name.to_string()),
        ("GIT_COMMITTER_EMAIL", format!("{name}@t")),
        ("GIT_COMMITTER_DATE", date.to_string()),
    ]
}

fn commit_as(dir: &Path, name: &str, date: &str, msg: &str) {
    let envs = as_author(name, date);
    let envs: Vec<(&str, &str)> = envs.iter().map(|(k, v)| (*k, v.as_str())).collect();
    git(dir, &envs, &["add", "-A"]);
    git(dir, &envs, &["commit", "-q", "-m", msg]);
}

fn fixture_repo() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let d = dir.path();
    git(d, &[], &["init", "-q"]);
    git(d, &[], &["config", "user.email", "t@t"]);
    git(d, &[], &["config", "user.name", "T"]);

    // Dates are derived relative to "now" (rather than hardcoded) so this
    // fixture never ages out of `analyze`'s default 180-day window.
    let carol_date = (Utc::now() - Duration::days(20))
        .format("%Y-%m-%d")
        .to_string();
    let dave_date = (Utc::now() - Duration::days(19))
        .format("%Y-%m-%d")
        .to_string();
    let alice_owns_date = (Utc::now() - Duration::days(18))
        .format("%Y-%m-%d")
        .to_string();
    let bob_owns_date = (Utc::now() - Duration::days(17))
        .format("%Y-%m-%d")
        .to_string();
    // The two coupling days must each be a single derived UTC calendar date
    // (same day-offset, hours 09:00/15:00) to preserve same-day semantics.
    let coupling_day_1 = (Utc::now() - Duration::days(16))
        .format("%Y-%m-%d")
        .to_string();
    let coupling_day_2 = (Utc::now() - Duration::days(15))
        .format("%Y-%m-%d")
        .to_string();

    // Four authors total so the Team category is applicable (MIN_TEAM_SIZE).
    std::fs::write(d.join("c.rs"), "// carol\n").unwrap();
    commit_as(
        d,
        "carol",
        &format!("{carol_date}T10:00:00 +0000"),
        "carol file",
    );
    std::fs::write(d.join("d.rs"), "// dave\n").unwrap();
    commit_as(
        d,
        "dave",
        &format!("{dave_date}T10:00:00 +0000"),
        "dave file",
    );

    // alice authors and owns a.rs (10 lines).
    std::fs::write(d.join("a.rs"), "fn a() {}\n".repeat(10)).unwrap();
    commit_as(
        d,
        "alice",
        &format!("{alice_owns_date}T10:00:00 +0000"),
        "alice owns a",
    );
    // bob authors and owns b.rs (10 lines).
    std::fs::write(d.join("b.rs"), "fn b() {}\n".repeat(10)).unwrap();
    commit_as(
        d,
        "bob",
        &format!("{bob_owns_date}T10:00:00 +0000"),
        "bob owns b",
    );

    // Same-day, separate commits by alice touching a.rs then b.rs (small
    // edit — bob keeps blame majority on b.rs). Repeat on a second day so
    // the pair count is 2 and the ratio comfortably clears 0.30.
    for (i, day) in [coupling_day_1, coupling_day_2].iter().enumerate() {
        std::fs::write(
            d.join("a.rs"),
            format!("{}// churn {i}\n", "fn a() {}\n".repeat(10)),
        )
        .unwrap();
        commit_as(d, "alice", &format!("{day}T09:00:00 +0000"), "touch a");
        std::fs::write(
            d.join("b.rs"),
            format!("{}// churn {i}\n", "fn b() {}\n".repeat(10)),
        )
        .unwrap();
        commit_as(d, "alice", &format!("{day}T15:00:00 +0000"), "touch b");
    }
    dir
}

#[test]
fn cross_owner_day_coupling_surfaces_a_team_finding() {
    let dir = fixture_repo();
    let out = Command::new(env!("CARGO_BIN_EXE_barad-dur"))
        .arg("analyze")
        .arg(dir.path())
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
    let team = report["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "Team")
        .expect("team category");
    let metric = team["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "Cross-team coupling")
        .expect("cross-team coupling metric");
    let list = metric["raw_value"]["List"].as_array().expect("List");
    let entry = list
        .iter()
        .filter_map(|e| e.as_str())
        .find(|e| e.contains("a.rs") && e.contains("b.rs"))
        .expect("a.rs/b.rs finding");
    assert!(
        entry.contains("alice") && entry.contains("bob"),
        "finding must name both primary owners: {entry}"
    );
    assert!(
        entry.contains("coupled 2 day(s)"),
        "finding must carry the day-bucket count: {entry}"
    );
}
