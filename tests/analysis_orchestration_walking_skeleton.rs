//! Characterization of the three command policies before the shared analysis
//! layer (M02) exists.
//!
//! `analyze`, `gate`, and `backfill` each assemble their own categories and
//! call the same report builder. These tests pin what each one selects,
//! weighs, persists, and decides *today*, through the binary and the files it
//! writes, so the extraction can prove parity without knowing the internals.
//! Where the commands intentionally differ, the difference is the assertion.
//!
//! Driving port: `barad-dur <cmd>` via assert_cmd. No internal Rust API.
use chrono::{Duration, Utc};
use predicates::prelude::*;
use serde_json::Value;
use std::path::Path;
use tempfile::TempDir;

mod common;
use common::{barad_dur, init_git_repo_with_commits, read_trends_entries};

// ---------------------------------------------------------------------------
// Fixture: a small Rust repository with recent, dated commits from four
// authors (Team needs 4+) so every category has evidence inside the default
// 6-month window.
// ---------------------------------------------------------------------------

const AUTHORS: [(&str, &str); 4] = [
    ("Alice", "alice@example.test"),
    ("Bob", "bob@example.test"),
    ("Carol", "carol@example.test"),
    ("Dave", "dave@example.test"),
];

/// Runs git the way the CI runner sees it: no global or system config, so a
/// fixture that forgets to supply its own author identity fails here too,
/// not only on a runner without a `~/.gitconfig`.
fn git(dir: &Path, args: &[&str], env: &[(&str, String)]) {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    for (key, value) in env {
        cmd.env(key, value);
    }
    let out = cmd.output().expect("git spawn");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// `authors` commits per file change, rotating through `AUTHORS`; commits
/// are dated `days_ago[i]` days before now.
fn fixture_with(authors: usize, days_ago: &[i64]) -> TempDir {
    let dir = TempDir::new().unwrap();
    let path = dir.path();
    git(path, &["init", "-q", "-b", "main"], &[]);
    for (i, days) in days_ago.iter().enumerate() {
        let (name, email) = AUTHORS[i % authors];
        let file = format!("src/mod_{}.rs", i % 3);
        std::fs::create_dir_all(path.join("src")).unwrap();
        std::fs::write(
            path.join(&file),
            format!(
                "pub fn value_{i}(flag: bool) -> u32 {{\n    if flag {{ {i} }} else {{ 0 }}\n}}\n"
            ),
        )
        .unwrap();
        let date = (Utc::now() - Duration::days(*days)).to_rfc3339();
        git(path, &["add", "-A"], &[]);
        git(
            path,
            &["commit", "-q", "-m", &format!("feat: value {i}")],
            &[
                ("GIT_AUTHOR_NAME", name.to_string()),
                ("GIT_AUTHOR_EMAIL", email.to_string()),
                ("GIT_COMMITTER_NAME", name.to_string()),
                ("GIT_COMMITTER_EMAIL", email.to_string()),
                ("GIT_AUTHOR_DATE", date.clone()),
                ("GIT_COMMITTER_DATE", date),
            ],
        );
    }
    dir
}

fn fixture() -> TempDir {
    fixture_with(4, &[40, 30, 20, 12, 5, 1])
}

fn write_config(dir: &Path, toml: &str) {
    let cache = dir.join(".repository-analysis");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(cache.join("barad-dur.toml"), toml).unwrap();
}

fn analyze_json(dir: &Path, flags: &[&str]) -> Value {
    let out = barad_dur()
        .arg("analyze")
        .arg(dir)
        .arg("--json")
        .args(flags)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&out).expect("analyze --json emits one JSON document")
}

fn category_names(report: &Value) -> Vec<String> {
    report["categories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect()
}

fn category_score(report: &Value, name: &str) -> Option<u64> {
    report["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("category {name} present"))["score"]
        .as_u64()
}

/// The overall score as the scorer defines it: the weighted mean of the
/// *scored* categories, weights renormalized over those present, rounded
/// half away from zero; `None` when nothing scored.
fn weighted_overall(report: &Value, weights: &[(&str, f64)]) -> Option<u64> {
    let (sum, total) = report["categories"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| {
            let score = c["score"].as_u64()? as f64;
            let weight = weights
                .iter()
                .find(|(n, _)| *n == c["name"].as_str().unwrap())
                .map(|(_, w)| *w)
                .expect("every category has a weight");
            Some((score, weight))
        })
        .fold((0.0, 0.0), |(s, t), (score, w)| (s + score * w, t + w));
    (total > 0.0).then(|| (sum / total).round() as u64)
}

/// `CategoryWeights::default()` — pinned here so a silent change to the
/// defaults fails the overall-score characterizations.
const DEFAULT_WEIGHTS: [(&str, f64); 5] = [
    ("Health", 35.0),
    ("Team", 10.0),
    ("Evolution", 20.0),
    ("Git Hygiene", 15.0),
    ("Coupling", 20.0),
];

const FIVE: [&str; 5] = ["Health", "Team", "Evolution", "Git Hygiene", "Coupling"];

// ---------------------------------------------------------------------------
// analyze — category selection and order
// ---------------------------------------------------------------------------

#[test]
fn analyze_default_selects_five_categories_in_fixed_order() {
    let dir = fixture();
    let report = analyze_json(dir.path(), &[]);
    assert_eq!(category_names(&report), FIVE);
}

#[test]
fn analyze_single_filter_selects_exactly_that_category() {
    let dir = fixture();
    for (flag, name) in [
        ("--health", "Health"),
        ("--team", "Team"),
        ("--evolution", "Evolution"),
        ("--hygiene", "Git Hygiene"),
    ] {
        let report = analyze_json(dir.path(), &[flag]);
        assert_eq!(category_names(&report), [name], "{flag}");
    }
}

#[test]
fn analyze_filters_combine_in_canonical_order_and_never_select_coupling() {
    // There is no `--coupling` filter: Coupling only runs when no filter is
    // given. Any filter, in any order, drops it.
    let dir = fixture();
    let report = analyze_json(dir.path(), &["--hygiene", "--health"]);
    assert_eq!(category_names(&report), ["Health", "Git Hygiene"]);
}

#[test]
fn analyze_deps_without_manifests_adds_nothing_and_leaves_scores_unchanged() {
    // `--deps` is an opt-in whose evidence may be unavailable: no lockfile
    // means no Dependencies category, no network, and the same overall.
    let dir = fixture();
    let without = analyze_json(dir.path(), &[]);
    let with = analyze_json(dir.path(), &["--deps"]);
    assert_eq!(category_names(&with), FIVE);
    assert_eq!(with["overall_score"], without["overall_score"]);
    assert_eq!(with["dep_ecosystem_reports"], serde_json::json!([]));
}

#[test]
fn analyze_deps_does_not_widen_a_category_filter() {
    let dir = fixture();
    let report = analyze_json(dir.path(), &["--deps", "--team"]);
    assert_eq!(category_names(&report), ["Team"]);
}

// ---------------------------------------------------------------------------
// analyze — weighting policy
// ---------------------------------------------------------------------------

#[test]
fn analyze_overall_is_the_default_weighted_mean_of_scored_categories() {
    let dir = fixture();
    let report = analyze_json(dir.path(), &[]);
    assert_eq!(
        report["overall_score"].as_u64(),
        weighted_overall(&report, &DEFAULT_WEIGHTS)
    );
}

#[test]
fn analyze_configured_weights_change_overall_but_not_category_scores() {
    let dir = fixture();
    let default = analyze_json(dir.path(), &[]);
    write_config(
        dir.path(),
        "[weights]\nhealth = 70\nteam = 10\nevolution = 5\nhygiene = 5\ncoupling = 10\n",
    );
    let weighted = analyze_json(dir.path(), &[]);
    for name in FIVE {
        assert_eq!(
            category_score(&weighted, name),
            category_score(&default, name),
            "{name} score is independent of weights"
        );
    }
    let configured = [
        ("Health", 70.0),
        ("Team", 10.0),
        ("Evolution", 5.0),
        ("Git Hygiene", 5.0),
        ("Coupling", 10.0),
    ];
    assert_eq!(
        weighted["overall_score"].as_u64(),
        weighted_overall(&weighted, &configured)
    );
}

#[test]
fn analyze_filtered_report_renormalizes_overall_over_present_categories() {
    let dir = fixture();
    let report = analyze_json(dir.path(), &["--health"]);
    assert_eq!(
        report["overall_score"].as_u64(),
        category_score(&report, "Health")
    );
}

// ---------------------------------------------------------------------------
// analyze — unavailable evidence stays distinct from a measured zero
// ---------------------------------------------------------------------------

fn metric<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["categories"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["metrics"].as_array().unwrap())
        .find(|m| m["name"] == name)
        .unwrap_or_else(|| panic!("metric {name} present"))
}

#[test]
fn analyze_skip_blame_leaves_blame_metrics_unscored_not_zero() {
    let dir = fixture();
    let with_blame = analyze_json(dir.path(), &[]);
    assert!(metric(&with_blame, "Code age")["score"].is_u64());
    assert!(metric(&with_blame, "Knowledge distribution")["score"].is_u64());

    write_config(dir.path(), "[analysis]\nskip_blame = true\n");
    let report = analyze_json(dir.path(), &["--no-cache"]);
    for name in ["Code age", "Knowledge distribution"] {
        let m = metric(&report, name);
        assert!(m["score"].is_null(), "unscored, not 0: {m}");
        assert!(
            m["description"]
                .as_str()
                .unwrap()
                .starts_with("No blame data"),
            "{m}"
        );
    }
    assert_eq!(
        report["overall_score"].as_u64(),
        weighted_overall(&report, &DEFAULT_WEIGHTS)
    );
}

#[test]
fn analyze_solo_author_keeps_team_in_the_report_but_unscored() {
    let dir = fixture_with(1, &[20, 10, 1]);
    let report = analyze_json(dir.path(), &[]);
    assert_eq!(category_names(&report), FIVE);
    assert_eq!(category_score(&report, "Team"), None);
    assert_eq!(
        report["overall_score"].as_u64(),
        weighted_overall(&report, &DEFAULT_WEIGHTS)
    );
}

#[test]
fn analyze_window_without_commits_keeps_categories_and_scores_only_what_it_can() {
    // All commits predate the 6-month window: the report still lists every
    // category; a category whose metrics all need commits (Evolution) is
    // unscored, while one with a window-free metric (Git Hygiene's gitignore
    // coverage) still scores — and the overall is the weighted mean of
    // whatever scored, so an evidence-free window can score 100.
    let dir = TempDir::new().unwrap();
    init_git_repo_with_commits(dir.path(), "main", 3);
    let report = analyze_json(dir.path(), &[]);
    assert_eq!(category_names(&report), FIVE);
    assert_eq!(report["total_commits"], 0);
    assert_eq!(category_score(&report, "Evolution"), None);
    let hygiene_scored: Vec<&str> = report["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "Git Hygiene")
        .unwrap()["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| !m["score"].is_null())
        .map(|m| m["name"].as_str().unwrap())
        .collect();
    assert_eq!(hygiene_scored, ["Gitignore coverage"]);
    assert_eq!(
        report["overall_score"].as_u64(),
        weighted_overall(&report, &DEFAULT_WEIGHTS)
    );
}

#[test]
fn analyze_without_parseable_sources_leaves_coupling_findings_absent_not_zero() {
    // No tree-sitter language in the tree: nothing to detect on, so the
    // Pressman metrics are unscored, the report carries no finding counts at
    // all, and the history entry omits the coupling count fields.
    let dir = TempDir::new().unwrap();
    let path = dir.path();
    git(path, &["init", "-q", "-b", "main"], &[]);
    for (i, file) in ["notes.txt", "data.csv", "more.txt"].iter().enumerate() {
        std::fs::write(path.join(file), format!("line {i}\n")).unwrap();
        let date = (Utc::now() - Duration::days(20 - i as i64 * 5)).to_rfc3339();
        git(path, &["add", "-A"], &[]);
        git(
            path,
            &["commit", "-q", "-m", &format!("docs: {file}")],
            &[
                ("GIT_AUTHOR_NAME", AUTHORS[0].0.to_string()),
                ("GIT_AUTHOR_EMAIL", AUTHORS[0].1.to_string()),
                ("GIT_COMMITTER_NAME", AUTHORS[0].0.to_string()),
                ("GIT_COMMITTER_EMAIL", AUTHORS[0].1.to_string()),
                ("GIT_AUTHOR_DATE", date.clone()),
                ("GIT_COMMITTER_DATE", date),
            ],
        );
    }
    let report = analyze_json(path, &[]);
    let control = metric(&report, "Control coupling");
    assert!(control["score"].is_null(), "{control}");
    assert_eq!(
        control["description"],
        "No files in detectable languages (Rust, TS/JS)"
    );
    assert!(
        report.get("coupling_finding_counts").is_none(),
        "absent, not zero: {:?}",
        report.get("coupling_finding_counts")
    );
    let entries = read_trends_entries(path);
    assert_eq!(
        sorted_keys(&entries[0]["counts"]),
        ["authors", "commits", "files"]
    );
}

#[test]
fn analyze_without_a_head_commit_is_an_error() {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"], &[]);
    barad_dur()
        .arg("analyze")
        .arg(dir.path())
        .arg("--json")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to get HEAD"));
}

// ---------------------------------------------------------------------------
// history — what each command persists
// ---------------------------------------------------------------------------

fn scored_metric_names(report: &Value) -> Vec<String> {
    let mut names: Vec<String> = report["categories"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["metrics"].as_array().unwrap())
        .filter(|m| !m["score"].is_null())
        .map(|m| m["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    names
}

fn sorted_keys(value: &Value) -> Vec<String> {
    let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    keys
}

#[test]
fn analyze_appends_one_history_entry_built_from_its_own_report() {
    let dir = fixture();
    let report = analyze_json(dir.path(), &[]);
    let entries = read_trends_entries(dir.path());
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];

    let head = std::process::Command::new("git")
        .args(["-C", dir.path().to_str().unwrap(), "rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert_eq!(
        entry["head"],
        String::from_utf8(head.stdout).unwrap().trim()
    );
    assert_eq!(entry["branch"], "main");
    assert_eq!(
        entry["source"],
        Value::Null,
        "analyze entries carry no source marker"
    );
    assert_eq!(entry["overall_score"], report["overall_score"]);

    let mut expected_categories = FIVE.map(String::from).to_vec();
    expected_categories.sort();
    assert_eq!(sorted_keys(&entry["category_scores"]), expected_categories);
    for name in FIVE {
        assert_eq!(
            entry["category_scores"][name].as_u64(),
            category_score(&report, name)
        );
    }
    assert_eq!(
        sorted_keys(&entry["metrics"]),
        scored_metric_names(&report),
        "history keeps scored metrics only"
    );
    assert_eq!(entry["counts"]["commits"], report["total_commits"]);
    assert_eq!(entry["counts"]["files"], report["total_files"]);
    assert_eq!(entry["counts"]["authors"], report["total_authors"]);
    assert_eq!(
        entry["counts"]["control_coupling"],
        report["coupling_finding_counts"]["control"]
    );
}

#[test]
fn analyze_appends_a_history_entry_on_every_run_even_for_the_same_head() {
    // `append_if_new_head` does not check the head: every analyze run adds
    // an entry, and a category-filtered run records only its categories,
    // with the overall renormalized over them.
    let dir = fixture();
    analyze_json(dir.path(), &[]);
    let filtered = analyze_json(dir.path(), &["--health"]);
    let entries = read_trends_entries(dir.path());
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["head"], entries[1]["head"]);
    assert_eq!(sorted_keys(&entries[1]["category_scores"]), ["Health"]);
    assert_eq!(entries[1]["overall_score"], filtered["overall_score"]);
}

#[test]
fn backfill_entries_omit_coupling_scores_but_keep_coupling_counts() {
    // Backfill scores four categories — no Coupling — yet its entries still
    // carry the finding counts, because the report builder derives them from
    // the snapshot regardless of which categories were selected.
    let dir = fixture();
    barad_dur()
        .arg("backfill")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("6 entries written"));
    let entries = read_trends_entries(dir.path());
    assert_eq!(entries.len(), 6);
    let mut four = ["Health", "Team", "Evolution", "Git Hygiene"]
        .map(String::from)
        .to_vec();
    four.sort();
    for entry in &entries {
        assert_eq!(sorted_keys(&entry["category_scores"]), four);
        assert_eq!(entry["source"], "backfill");
        assert_eq!(entry["branch"], "main");
        assert!(
            entry["counts"]["control_coupling"].is_u64(),
            "coupling counts persist without the category: {entry}"
        );
    }
}

#[test]
fn backfill_rerun_writes_nothing_and_says_so() {
    let dir = fixture();
    barad_dur()
        .arg("backfill")
        .arg(dir.path())
        .assert()
        .success();
    barad_dur()
        .arg("backfill")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Backfill already complete"));
    assert_eq!(read_trends_entries(dir.path()).len(), 6);
}

#[test]
fn backfill_without_commits_is_an_error() {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"], &[]);
    barad_dur()
        .arg("backfill")
        .arg(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("No commits found"));
}

#[test]
fn gate_persists_no_history() {
    let dir = fixture();
    barad_dur()
        .args(["gate", "--min-score", "0"])
        .arg(dir.path())
        .assert()
        .success();
    assert!(
        !dir.path().join(".repository-analysis/trends.json").exists(),
        "gate reads history; only analyze and backfill write it"
    );
}

// ---------------------------------------------------------------------------
// gate — score, decline, and ratchet checks, alone and together
// ---------------------------------------------------------------------------

fn gate(dir: &Path, flags: &[&str]) -> assert_cmd::assert::Assert {
    barad_dur().arg("gate").arg(dir).args(flags).assert()
}

#[test]
fn gate_scores_all_five_categories_including_coupling() {
    let dir = fixture();
    let report = analyze_json(dir.path(), &[]);
    let coupling = category_score(&report, "Coupling").expect("fixture scores Coupling");
    gate(
        dir.path(),
        &[
            "--category",
            "Coupling",
            "--min-score",
            &coupling.to_string(),
        ],
    )
    .code(0)
    .stdout(predicate::str::contains("Coupling"));
}

#[test]
fn gate_overall_threshold_is_exact_and_matches_analyze() {
    let dir = fixture();
    let overall = analyze_json(dir.path(), &[])["overall_score"]
        .as_u64()
        .expect("fixture scores");
    assert!(overall < 100, "fixture must leave headroom: {overall}");
    gate(dir.path(), &["--min-score", &overall.to_string()]).code(0);
    gate(dir.path(), &["--min-score", &(overall + 1).to_string()]).code(1);
}

#[test]
fn gate_category_threshold_replaces_the_overall_check() {
    let dir = fixture();
    let report = analyze_json(dir.path(), &[]);
    let overall = report["overall_score"].as_u64().unwrap();
    let health = category_score(&report, "Health").unwrap();
    // A threshold the overall clears but Health does not, or vice versa,
    // proves which score is being gated.
    let (threshold, expected) = if health < overall {
        (overall, 1)
    } else {
        (health, 0)
    };
    gate(
        dir.path(),
        &[
            "--category",
            "Health",
            "--min-score",
            &threshold.to_string(),
        ],
    )
    .code(expected);
}

#[test]
fn gate_unknown_category_is_skipped_not_failed() {
    let dir = fixture();
    gate(
        dir.path(),
        &["--category", "Dependencies", "--min-score", "100"],
    )
    .code(0);
}

#[test]
fn gate_ratchet_against_head_passes_with_no_new_findings() {
    let dir = fixture();
    gate(
        dir.path(),
        &[
            "--min-score",
            "0",
            "--no-new-coupling",
            "--baseline-ref",
            "HEAD",
        ],
    )
    .code(0)
    .stdout(predicate::str::contains(
        "RATCHET PASS: no new coupling findings vs HEAD",
    ));
}

#[test]
fn gate_ratchet_with_unresolvable_baseline_is_an_error() {
    let dir = fixture();
    gate(
        dir.path(),
        &[
            "--min-score",
            "0",
            "--no-new-coupling",
            "--baseline-ref",
            "no-such-ref",
        ],
    )
    .failure()
    .stderr(predicate::str::contains(
        "cannot resolve baseline ref 'no-such-ref'",
    ));
}

#[test]
fn gate_score_failure_and_ratchet_pass_fold_into_one_failing_exit() {
    let dir = fixture();
    gate(
        dir.path(),
        &[
            "--min-score",
            "100",
            "--no-new-coupling",
            "--baseline-ref",
            "HEAD",
        ],
    )
    .code(1)
    .stdout(predicate::str::contains("RATCHET PASS"));
}

#[test]
fn gate_max_decline_passes_on_first_run_and_against_stable_history() {
    let dir = fixture();
    gate(dir.path(), &["--min-score", "0", "--max-decline", "0"]).code(0);
    analyze_json(dir.path(), &[]);
    gate(dir.path(), &["--min-score", "0", "--max-decline", "0"]).code(0);
}

#[test]
fn gate_max_decline_fails_against_a_higher_history_and_names_the_rate() {
    // Seed history with four earlier, higher-scoring runs on this branch,
    // shaped exactly like analyze's own entry so the schema check passes.
    let dir = fixture();
    analyze_json(dir.path(), &[]);
    let own = read_trends_entries(dir.path()).remove(0);
    assert!(own["overall_score"].as_u64().unwrap() < 100);
    let seeded: Vec<String> = (0..4)
        .map(|i| {
            let mut entry = own.clone();
            entry["head"] = Value::String(format!("{i:040x}"));
            entry["overall_score"] = Value::from(100);
            entry["timestamp"] = Value::String((Utc::now() - Duration::days(30 - i)).to_rfc3339());
            entry.to_string()
        })
        .collect();
    std::fs::write(
        dir.path().join(".repository-analysis/trends.json"),
        seeded.join("\n") + "\n",
    )
    .unwrap();
    gate(dir.path(), &["--min-score", "0", "--max-decline", "0"])
        .code(1)
        .stdout(predicate::str::contains("PASS: overall score"))
        .stdout(predicate::str::contains("FAIL: score declining at"));
}

#[test]
fn gate_rejects_invalid_weights_before_scoring() {
    let dir = fixture();
    write_config(dir.path(), "[weights]\nhealth = 90\n");
    gate(dir.path(), &["--min-score", "0"])
        .failure()
        .stderr(predicate::str::contains("Category weights must sum to 100"));
}
