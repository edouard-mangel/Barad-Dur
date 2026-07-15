//! M7 milestone E2E: a TS fixture with a depth-2 inheritance chain surfaces
//! an Inheritance finding through counts, the metric row, actions, and
//! hotspots via `analyze --json`, while Rust trait impls produce nothing.
//! A warm-cache re-run pins the CACHE_VERSION bump (a stale pre-M7 snapshot
//! shape would silently drop class_records).

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
    std::fs::write(dir.path().join("src/a.ts"), "export class A {}\n").unwrap();
    std::fs::write(
        dir.path().join("src/b.ts"),
        "import { A } from './a';\nexport class B extends A {}\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/c.ts"),
        "import { B } from './b';\nexport class C extends B {}\n",
    )
    .unwrap();
    // Rust trait impls are interface inheritance — must yield no findings.
    std::fs::write(
        dir.path().join("src/lib.rs"),
        "pub trait T { fn f(&self); }\npub struct S;\nimpl T for S { fn f(&self) {} }\n",
    )
    .unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "fixture"]);
    dir
}

fn analyze_json(dir: &Path, extra: &[&str]) -> serde_json::Value {
    let out = Command::new(env!("CARGO_BIN_EXE_barad-dur"))
        .arg("analyze")
        .arg(dir)
        .arg("--json")
        .args(extra)
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
fn inheritance_finding_surfaces_end_to_end() {
    let dir = fixture_repo();
    let report = analyze_json(dir.path(), &["--no-cache"]);

    // Exactly one finding: C (depth 2); B (depth 1) stays clean; Rust silent.
    assert_eq!(report["coupling_finding_counts"]["inheritance"], 1);
    assert_eq!(report["coupling_finding_counts"]["content"], 0);
    assert_eq!(report["coupling_finding_counts"]["common"], 0);

    // Metric row: band score + real line + chain evidence.
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
        .find(|m| m["name"] == "Inheritance coupling")
        .expect("inheritance metric row");
    assert_eq!(metric["score"], 70, "1 finding → 70 band");
    let list = metric["raw_value"]["List"]
        .as_array()
        .expect("List raw_value");
    let entry = list[0].as_str().unwrap();
    assert!(
        entry.contains("src/c.ts:2") && entry.contains("class C extends B → A (depth 2)"),
        "evidence with line: {entry}"
    );

    // Action: ranked with the inheritance label + composition advice.
    let action = report["coupling_actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["text"].as_str().unwrap())
        .find(|t| t.contains("worst: inheritance"))
        .expect("inheritance action");
    assert!(action.contains("src/c.ts") && action.contains("composition"));

    // Hotspot badge counts: c.ts flagged, the Rust file clean.
    let hotspots = report["file_hotspots"].as_array().unwrap();
    let c = hotspots.iter().find(|h| h["path"] == "src/c.ts").unwrap();
    assert_eq!(c["inheritance_findings"], 1);
    let rs = hotspots.iter().find(|h| h["path"] == "src/lib.rs").unwrap();
    assert_eq!(rs["inheritance_findings"], 0);
}

#[test]
fn inheritance_finding_survives_warm_cache() {
    let dir = fixture_repo();
    let first = analyze_json(dir.path(), &[]); // collects + writes cache
    let second = analyze_json(dir.path(), &[]); // must serve the cache
    assert_eq!(first["coupling_finding_counts"]["inheritance"], 1);
    assert_eq!(
        second["coupling_finding_counts"]["inheritance"], 1,
        "cached snapshot must round-trip class_records (CACHE_VERSION 2)"
    );
}
