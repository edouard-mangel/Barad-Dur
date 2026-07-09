//! M5 milestone E2E: a `static mut` common-coupling finding whose file
//! co-changes cross-component across several commits is reported as
//! "corroborated" and scored one severity band below the dormant baseline.
//!
//! `compute_coupling` is crate-internal, so this drives the installed binary
//! (`CARGO_BIN_EXE_barad-dur`) against a throwaway fixture repo and parses
//! the JSON report.

use std::process::{Command, Output};

/// Build a fixture repo whose `src/config.rs` holds a `static mut` (Common
/// finding) and co-changes with `lib/helper.rs` (a different depth-2
/// component) across 4 of 4 commits — ratio 1.0, well above the 0.30
/// threshold — so the finding corroborates.
fn fixture_repo() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let git = |args: &[&str]| -> Output {
        let out = Command::new("git")
            .current_dir(dir.path())
            .args(args)
            .output()
            .expect("git must spawn");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "fixture@example.com"]);
    git(&["config", "user.name", "Fixture"]);

    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::create_dir_all(dir.path().join("lib")).unwrap();
    let config_rs = dir.path().join("src/config.rs");
    let helper_rs = dir.path().join("lib/helper.rs");

    for i in 0..4 {
        std::fs::write(
            &config_rs,
            format!("static mut FLAG: bool = false;\npub fn v() -> u32 {{ {i} }}\n"),
        )
        .unwrap();
        std::fs::write(&helper_rs, format!("pub fn h() -> u32 {{ {i} }}\n")).unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", &format!("change {i}")]);
    }
    dir
}

fn common_metric_score(report: &serde_json::Value) -> (i64, Vec<String>) {
    let cats = report["categories"].as_array().expect("categories array");
    let coupling = cats
        .iter()
        .find(|c| c["name"] == "Coupling")
        .expect("Coupling category");
    let metric = coupling["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "Common coupling")
        .expect("Common coupling metric");
    let score = metric["score"].as_i64().expect("score");
    let list = metric["raw_value"]["List"]
        .as_array()
        .expect("List raw_value")
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect();
    (score, list)
}

#[test]
fn corroborated_common_finding_is_annotated_and_downscored() {
    let dir = fixture_repo();
    let out = Command::new(env!("CARGO_BIN_EXE_barad-dur"))
        .arg("analyze")
        .arg(dir.path())
        .args(["--json", "--no-cache"])
        .output()
        .expect("binary must run");
    assert!(
        out.status.success(),
        "analyze failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let report: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be JSON");
    let (score, evidence) = common_metric_score(&report);

    // Dormant baseline for 1 Common finding is 60; corroborated (weight 2.0)
    // scores the count-2 band, 40.
    assert_eq!(
        score, 40,
        "corroborated Common finding must score one band worse"
    );
    assert!(
        evidence
            .iter()
            .any(|s| s.contains("corroborated (co-changes with")),
        "evidence must be annotated: {evidence:?}"
    );
}
