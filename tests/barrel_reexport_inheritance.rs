//! Barrel re-export E2E: an inheritance chain whose middle link is imported
//! through a barrel (`index.ts` re-exporting the class) still surfaces a
//! depth-2 Inheritance finding via `analyze --json`. Before re-export
//! resolution the base resolved to the barrel file, found no class record
//! there, and the chain undercounted to depth 1.

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
    std::fs::create_dir_all(dir.path().join("src/models")).unwrap();
    std::fs::write(dir.path().join("src/models/a.ts"), "export class A {}\n").unwrap();
    std::fs::write(
        dir.path().join("src/models/b.ts"),
        "import { A } from './a';\nexport class B extends A {}\n",
    )
    .unwrap();
    // The barrel: forwards B without declaring anything itself.
    std::fs::write(
        dir.path().join("src/models/index.ts"),
        "export { B } from './b';\nexport * from './a';\n",
    )
    .unwrap();
    // C imports B through the barrel — real chain C → B → A (depth 2).
    std::fs::write(
        dir.path().join("src/c.ts"),
        "import { B } from './models';\nexport class C extends B {}\n",
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
fn barrel_imported_base_still_counts_full_depth() {
    let dir = fixture_repo();
    let report = analyze_json(dir.path(), &["--no-cache"]);

    assert_eq!(
        report["coupling_finding_counts"]["inheritance"], 1,
        "C's chain through the barrel must reach depth 2"
    );

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
    let list = metric["raw_value"]["List"]
        .as_array()
        .expect("List raw_value");
    let entry = list[0].as_str().expect("evidence entry is a string");
    assert!(
        entry.contains("src/c.ts:2") && entry.contains("class C extends B → A (depth 2)"),
        "chain evidence must name the real ancestors, not the barrel: {entry}"
    );
}

#[test]
fn barrel_resolution_survives_warm_cache() {
    let dir = fixture_repo();
    let first = analyze_json(dir.path(), &[]); // collects + writes cache
    let second = analyze_json(dir.path(), &[]); // must serve the cache
    assert_eq!(first["coupling_finding_counts"]["inheritance"], 1);
    assert_eq!(
        second["coupling_finding_counts"]["inheritance"], 1,
        "cached snapshot must round-trip the reexports field"
    );
}
