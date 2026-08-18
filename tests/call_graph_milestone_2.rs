//! Call-graph milestone 2: the `call_graph` report section (design D7)
//! flows through `analyze --json` — resolution-rate accounting with
//! same-file counted as resolved, function hubs by distinct-caller
//! in-degree, and trust-floor suppression when most calls are unresolved.

use std::path::Path;
use std::process::{Command, Output};

fn init_repo(dir: &Path) -> impl Fn(&[&str]) -> Output + '_ {
    let git = move |args: &[&str]| -> Output {
        let out = Command::new("git")
            .current_dir(dir)
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
    git
}

fn commit_all(dir: &Path) {
    let git = init_repo(dir);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "fixture"]);
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
fn call_graph_section_reports_rate_and_hubs() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("src/tax.ts"),
        "export function computeTax(x: number): number { return x * 1.2; }\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/order.ts"),
        "import { computeTax } from './tax';\n\
         function round(x: number): number { return x; }\n\
         export function processOrder(o: any) {\n\
           const t = computeTax(o.amount);\n\
           const u = computeTax(t);\n\
           o.save();\n\
           return round(u);\n\
         }\n",
    )
    .unwrap();
    commit_all(dir.path());

    let report = analyze_json(dir.path());
    let cg = &report["call_graph"];
    // 3 edges: 1 resolved + 1 same-file + 1 unresolved → rate 2/3 (D7:
    // same-file counts as resolved).
    assert_eq!(cg["edges_resolved"], 1);
    assert_eq!(cg["edges_same_file"], 1);
    assert_eq!(cg["edges_unresolved"], 1);
    let rate = cg["resolution_rate"].as_f64().expect("rate");
    assert!((rate - 2.0 / 3.0).abs() < 1e-9, "rate was {rate}");
    // Both targets have in-degree 1 — deterministic path/name order.
    assert_eq!(
        cg["function_hubs"],
        serde_json::json!([
            { "path": "src/order.ts", "name": "round", "resolved_in_degree": 1 },
            { "path": "src/tax.ts", "name": "computeTax", "resolved_in_degree": 1 },
        ])
    );
}

#[test]
fn call_graph_hubs_suppressed_below_trust_floor() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    // 1 resolvable call vs 3 method calls → rate 0.25 < default floor 0.5.
    std::fs::write(
        dir.path().join("src/a.ts"),
        "function local(x: any) { return x; }\n\
         export function run(o: any) {\n\
           o.load();\n\
           o.save();\n\
           o.close();\n\
           return local(o);\n\
         }\n",
    )
    .unwrap();
    commit_all(dir.path());

    let report = analyze_json(dir.path());
    let cg = &report["call_graph"];
    assert_eq!(cg["edges_same_file"], 1);
    assert_eq!(cg["edges_unresolved"], 3);
    let rate = cg["resolution_rate"].as_f64().expect("rate");
    assert!((rate - 0.25).abs() < 1e-9, "rate was {rate}");
    assert_eq!(
        cg["function_hubs"],
        serde_json::json!([]),
        "hubs must be suppressed below the trust floor"
    );
}
