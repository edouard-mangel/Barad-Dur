//! `--json` and `--html` write their document to stdout, so every warning
//! must go to stderr. `analyze.rs` states that convention itself ("progress
//! goes to stderr, so it never interferes with JSON/HTML output on stdout"),
//! but the entity-history corruption warning was printed with `println!`,
//! putting its text ahead of the document and making it unparseable.
//!
//! Note the sibling `trends.json` warning deliberately stays on stdout:
//! AC-01.4 in `tests/trend_milestone_1.rs` specifies it there. That conflict
//! predates this feature and is left as a spec decision.

use std::process::Command;

fn git(dir: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

/// A repo with one commit and a deliberately corrupt cache file.
fn repo_with_corrupt_cache(file_name: &str) -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path();

    git(path, &["init", "-q"]);
    git(path, &["config", "user.email", "t@e"]);
    git(path, &["config", "user.name", "t"]);
    std::fs::write(path.join("lib.rs"), "fn f(x: i32) -> i32 { x }\n").unwrap();
    git(path, &["add", "-A"]);
    git(path, &["commit", "-q", "-m", "init"]);

    let cache = path.join(".repository-analysis");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(cache.join(file_name), "NOT VALID JSON\n").unwrap();

    dir
}

fn analyze_json_stdout(repo: &std::path::Path) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_barad-dur"))
        .args(["analyze", &repo.to_string_lossy(), "--json"])
        .output()
        .expect("analyze should run");
    String::from_utf8(out.stdout).expect("stdout should be utf-8")
}

#[test]
fn corrupt_entity_trends_does_not_pollute_json_stdout() {
    let dir = repo_with_corrupt_cache("entity_trends.json");
    let stdout = analyze_json_stdout(dir.path());

    serde_json::from_str::<serde_json::Value>(&stdout).unwrap_or_else(|e| {
        panic!(
            "stdout must be parseable JSON when entity_trends.json is corrupt, \
             but parsing failed ({e}); stdout began: {:?}",
            stdout.chars().take(120).collect::<String>()
        )
    });
}
