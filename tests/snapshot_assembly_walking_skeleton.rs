//! M03 walking skeleton: the two snapshot readers agree through the binary.
//!
//! `analyze` collects HEAD from the working tree; `gate --baseline-ref HEAD`
//! and `backfill` collect the same commit from blobs. Their coupling counts
//! and finding sets must match, and history records written by `analyze`
//! and `backfill` for the same head must carry the same coupling counts.
use predicates::prelude::*;
use serde_json::Value;
use std::path::Path;
use tempfile::TempDir;

mod common;
use common::{barad_dur, read_trends_entries};

fn git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@e")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@e")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

/// A Rust file with a Common finding and a TS barrel bypass, committed once.
fn fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    for (name, content) in [
        ("src/lib.rs", "static mut CACHE: usize = 0;\npub fn f(flag: bool) -> u32 { if flag { 1 } else { 0 } }\n"),
        ("web/a/index.ts", "export * from './impl';\n"),
        ("web/a/impl.ts", "export class Impl {}\n"),
        ("web/b/user.ts", "import { Impl } from '../a/impl';\nexport class User extends Impl {}\n"),
    ] {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "init"]);
    dir
}

fn analyze_json(dir: &Path) -> Value {
    let out = barad_dur()
        .arg("analyze")
        .arg(dir)
        .args(["--json", "--all"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&out).unwrap()
}

#[test]
fn gate_sees_no_new_findings_between_working_tree_and_its_own_head_blobs() {
    let dir = fixture();
    let report = analyze_json(dir.path());
    let counts = &report["coupling_finding_counts"];
    assert_eq!(counts["common"], 1, "{counts}");
    assert_eq!(counts["content"], 1, "barrel bypass: {counts}");
    barad_dur()
        .arg("gate")
        .arg(dir.path())
        .args([
            "--min-score",
            "0",
            "--no-new-coupling",
            "--baseline-ref",
            "HEAD",
        ])
        .assert()
        .code(0)
        .stdout(predicate::str::contains(
            "RATCHET PASS: no new coupling findings vs HEAD",
        ));
}

#[test]
fn analyze_and_backfill_record_the_same_coupling_counts_for_one_head() {
    let dir = fixture();
    analyze_json(dir.path());
    let live = read_trends_entries(dir.path()).remove(0);
    std::fs::remove_file(dir.path().join(".repository-analysis/trends.json")).unwrap();
    barad_dur()
        .arg("backfill")
        .arg(dir.path())
        .assert()
        .success();
    let historical = read_trends_entries(dir.path()).remove(0);
    assert_eq!(historical["head"], live["head"]);
    for kind in [
        "content_coupling",
        "common_coupling",
        "inheritance_coupling",
        "control_coupling",
    ] {
        assert_eq!(historical["counts"][kind], live["counts"][kind], "{kind}");
    }
    assert_eq!(historical["counts"]["files"], live["counts"]["files"]);
}

#[test]
fn a_warm_cache_replays_the_cold_collection_channel_for_channel() {
    use barad_dur::collector::Collector;
    use barad_dur::runner::{resolve_snapshot, CollectOptions};
    use barad_dur::snapshot::TimeWindow;
    let dir = fixture();
    let collector = Collector::open(dir.path(), TimeWindow::full_history()).unwrap();
    let head = collector.head_commit_hash().unwrap();
    let options = CollectOptions {
        show_progress: false,
        verbose: false,
        skip_blame: false,
        no_cache: true,
        cache_only: false,
        cli_exclude_patterns: &[],
        cli_exclude_extensions: &[],
        use_default_excludes: true,
    };
    let cold = resolve_snapshot(&collector, &head, &options).unwrap();
    let warm = resolve_snapshot(
        &collector,
        &head,
        &CollectOptions {
            no_cache: false,
            cache_only: true,
            ..options
        },
    )
    .unwrap();
    let channels = |s: &barad_dur::snapshot::RepoSnapshot| {
        serde_json::to_value((
            &s.files,
            s.file_metrics
                .iter()
                .collect::<std::collections::BTreeMap<_, _>>(),
            s.import_graph
                .iter()
                .collect::<std::collections::BTreeMap<_, _>>(),
            s.unreliable_import_specifiers,
            &s.coupling_findings,
            &s.class_records,
            &s.reexports,
            &s.call_records,
            s.blame_map
                .keys()
                .collect::<std::collections::BTreeSet<_>>(),
            &s.file_change_pairs,
            s.commits_by_file
                .iter()
                .collect::<std::collections::BTreeMap<_, _>>(),
        ))
        .unwrap()
    };
    assert_eq!(channels(&cold), channels(&warm));
    assert_eq!(
        cold.created_at, warm.created_at,
        "a cache hit keeps its acquisition time"
    );
}
