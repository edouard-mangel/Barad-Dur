//! Call-graph milestone 5: Rust call-edge extraction flows end-to-end —
//! `use`-bound cross-file calls resolve through `resolve_rust_import`,
//! same-file and method calls are accounted honestly, and the dogfood
//! repo (barad-dûr itself) finally produces resolved Rust edges.

use std::path::Path;
use std::process::{Command, Output};

fn fixture_repo() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let git = |args: &[&str]| -> Output {
        let out = Command::new("git")
            .current_dir(dir.path())
            .args(args)
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "f@e.com"]);
    git(&["config", "user.name", "F"]);
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("src/tax.rs"),
        "pub fn compute_tax(x: u32) -> u32 { x + 1 }\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/lib.rs"),
        "mod tax;\n\
         use crate::tax::compute_tax;\n\
         fn round(x: u32) -> u32 { x }\n\
         pub fn process(o: u32) -> String {\n\
             let t = compute_tax(o);\n\
             let u = compute_tax(t);\n\
             round(u).to_string()\n\
         }\n",
    )
    .unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "fixture"]);
    dir
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
fn rust_call_edges_resolve_end_to_end() {
    let dir = fixture_repo();
    let report = analyze_json(dir.path());
    let cg = &report["call_graph"];
    // 3 edges: compute_tax resolved (2 sites), round same-file,
    // .to_string() honestly unresolved → rate 2/3.
    assert_eq!(cg["edges_resolved"], 1);
    assert_eq!(cg["edges_same_file"], 1);
    assert_eq!(cg["edges_unresolved"], 1);
    let rate = cg["resolution_rate"].as_f64().expect("rate");
    assert!((rate - 2.0 / 3.0).abs() < 1e-9, "rate was {rate}");
    assert_eq!(
        cg["function_hubs"],
        serde_json::json!([
            { "path": "src/lib.rs", "name": "round", "resolved_in_degree": 1 },
            { "path": "src/tax.rs", "name": "compute_tax", "resolved_in_degree": 1 },
        ])
    );
}

#[test]
fn dogfood_repo_produces_resolved_rust_call_records() {
    // The design's M5 rationale: the call graph must be useful on
    // barad-dûr itself. Loose assertions only — the codebase moves.
    let repo = std::env::var("BARAD_DUR_TEST_REPO").unwrap_or_else(|_| ".".into());
    let Ok(collector) = barad_dur::collector::Collector::open(
        Path::new(&repo),
        barad_dur::snapshot::TimeWindow::default(),
    ) else {
        return; // not a git repo (e.g. cargo-mutants temp dir) — skip
    };
    let snapshot = collector.collect_snapshot().expect("collect");
    let rust_records: Vec<_> = snapshot
        .call_records
        .iter()
        .filter(|r| r.path.extension().and_then(|e| e.to_str()) == Some("rs"))
        .collect();
    assert!(
        !rust_records.is_empty(),
        "dogfood must produce Rust call records"
    );
    assert!(
        rust_records
            .iter()
            .any(|r| matches!(r.callee, barad_dur::snapshot::CalleeRef::Resolved { .. })),
        "dogfood must contain at least one resolved cross-file Rust edge"
    );
}
