//! Call-graph walking skeleton (M1): a TS fixture repo flows through the
//! full collector — extraction, aggregation, specifier resolution — into
//! `RepoSnapshot::call_records`, and the records survive a cache
//! round-trip. Function-level: who calls whom, with counts, honest about
//! what static analysis cannot resolve.

use std::process::{Command, Output};

use barad_dur::collector::Collector;
use barad_dur::snapshot::{CallRecord, CalleeRef, TimeWindow};

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
        dir.path().join("src/tax.ts"),
        "export function computeTax(x: number): number { return x * 1.2; }\n",
    )
    .unwrap();
    // One resolved cross-file edge (called twice), one same-file edge,
    // one honest-unresolved method call.
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
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "fixture"]);
    dir
}

fn expected_records() -> Vec<CallRecord> {
    // Sorted by (path, caller, callee variant rank): SameFile, Resolved,
    // Unresolved — all within caller `processOrder` of src/order.ts.
    vec![
        CallRecord {
            path: "src/order.ts".into(),
            caller: "processOrder".into(),
            callee: CalleeRef::SameFile("round".into()),
            count: 1,
        },
        CallRecord {
            path: "src/order.ts".into(),
            caller: "processOrder".into(),
            callee: CalleeRef::Resolved {
                path: "src/tax.ts".into(),
                name: "computeTax".into(),
            },
            count: 2,
        },
        CallRecord {
            path: "src/order.ts".into(),
            caller: "processOrder".into(),
            callee: CalleeRef::Unresolved {
                name: "save".into(),
            },
            count: 1,
        },
    ]
}

#[test]
fn collector_populates_resolved_call_records_end_to_end() {
    let dir = fixture_repo();
    let collector = Collector::open(dir.path(), TimeWindow::default()).expect("open");
    let snapshot = collector.collect_snapshot().expect("collect");
    assert_eq!(snapshot.call_records, expected_records());
}

#[test]
fn call_records_survive_a_cache_round_trip() {
    let dir = fixture_repo();
    let collector = Collector::open(dir.path(), TimeWindow::default()).expect("open");
    let snapshot = collector.collect_snapshot().expect("collect");
    barad_dur::cache::storage::save(&snapshot, dir.path()).expect("save");
    let loaded = barad_dur::cache::storage::load(dir.path())
        .expect("load")
        .expect("cache hit");
    assert_eq!(loaded.call_records, expected_records());
}
