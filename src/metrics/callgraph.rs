//! Call-graph report section (call-graph M2): pure `(snapshot) → value`
//! summary of the snapshot's call records — resolution-rate accounting
//! (D7: same-file counts as resolved) and barrel-chased function hubs,
//! suppressed below the configured trust floor (design §4).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::config::HealthThresholds;
use crate::metrics::reexport::{chase_named, reexport_index, resolve_symbol};
use crate::scorer::{CallGraphReport, FunctionHub};
use crate::snapshot::{CalleeRef, RepoSnapshot};

/// Build the report's `call_graph` section. `None` means "no call data" —
/// either the AST pass did not run (ADR-005 backfill snapshot) or no file
/// in a supported language produced call edges — never "zero calls".
pub(crate) fn call_graph_report(
    snapshot: &RepoSnapshot,
    thresholds: &HealthThresholds,
) -> Option<CallGraphReport> {
    if snapshot.file_metrics.is_empty() || snapshot.call_records.is_empty() {
        return None;
    }
    let records = &snapshot.call_records;
    // One exhaustive pass: a future CalleeRef variant is a compile error
    // here, not a silently uncounted bucket (review C10).
    let (edges_resolved, edges_same_file, edges_unresolved) =
        records
            .iter()
            .fold((0, 0, 0), |(r, s, u), rec| match rec.callee {
                CalleeRef::Resolved { .. } => (r + 1, s, u),
                CalleeRef::SameFile(_) => (r, s + 1, u),
                CalleeRef::Unresolved { .. } => (r, s, u + 1),
            });
    // D7: a same-file callee is a named, located target — it counts as
    // resolved. Rate over edge records, not call counts.
    let resolution_rate = (edges_resolved + edges_same_file) as f64 / records.len() as f64;
    let function_hubs = if resolution_rate < thresholds.call_resolution_floor {
        // Trust floor (design §4): don't showcase hubs built on
        // mostly-unresolved data; the honest counts above still tell why.
        Vec::new()
    } else {
        top_hubs(snapshot)
    };
    Some(CallGraphReport {
        resolution_rate,
        edges_resolved,
        edges_same_file,
        edges_unresolved,
        function_hubs,
    })
}

/// Top-10 call targets by distinct-caller in-degree over resolved and
/// same-file edges, barrel-chased to the declaring file where possible.
fn top_hubs(snapshot: &RepoSnapshot) -> Vec<FunctionHub> {
    let rx = reexport_index(&snapshot.reexports);
    let declares = |key: (&PathBuf, &str)| {
        snapshot
            .file_metrics
            .get(key.0)
            .is_some_and(|m| m.functions.iter().any(|f| f.name == key.1))
    };
    let callers_by_target: HashMap<(&PathBuf, &str), HashSet<(&PathBuf, &str)>> = snapshot
        .call_records
        .iter()
        .filter_map(|r| {
            let target = match &r.callee {
                CalleeRef::SameFile(name) => (&r.path, name.as_str()),
                CalleeRef::Resolved { path, name } => {
                    let key = (path, name.as_str());
                    // A target resolved to a barrel has no declaration
                    // there — chase to the declaring file; when nothing
                    // declares the symbol (arrow-const exports, classes),
                    // fall back to following explicit named re-exports to
                    // their terminal file (review F4) so barrel and
                    // direct importers land on one hub key.
                    resolve_symbol(key, &declares, &rx, &mut Vec::new())
                        .unwrap_or_else(|| chase_named(key, &rx))
                }
                CalleeRef::Unresolved { .. } => return None,
            };
            Some((target, (&r.path, r.caller.as_str())))
        })
        // Self-recursion is not an incoming caller (review F3).
        .filter(|(target, caller)| target != caller)
        .fold(HashMap::new(), |mut m, (target, caller)| {
            m.entry(target).or_default().insert(caller);
            m
        });
    let mut hubs: Vec<FunctionHub> = callers_by_target
        .into_iter()
        .map(|((path, name), callers)| FunctionHub {
            path: path.display().to_string(),
            name: name.to_string(),
            resolved_in_degree: callers.len(),
        })
        .collect();
    hubs.sort_by(|a, b| {
        (std::cmp::Reverse(a.resolved_in_degree), &a.path, &a.name).cmp(&(
            std::cmp::Reverse(b.resolved_in_degree),
            &b.path,
            &b.name,
        ))
    });
    hubs.truncate(10);
    hubs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::testutil::{make_snapshot, normal_function};
    use crate::snapshot::{CallRecord, CalleeRef, FileComplexity, ReExportKind, ReExportRecord};

    fn record(path: &str, caller: &str, callee: CalleeRef, count: u32) -> CallRecord {
        CallRecord {
            path: path.into(),
            caller: caller.into(),
            callee,
            count,
        }
    }

    fn resolved(path: &str, name: &str) -> CalleeRef {
        CalleeRef::Resolved {
            path: path.into(),
            name: name.into(),
        }
    }

    fn unresolved(name: &str) -> CalleeRef {
        CalleeRef::Unresolved { name: name.into() }
    }

    /// Snapshot with the AST-pass marker set (non-empty `file_metrics`)
    /// and the given records; `declared` lists (file, function) pairs.
    fn snap(records: Vec<CallRecord>, declared: &[(&str, &str)]) -> RepoSnapshot {
        let mut s = make_snapshot();
        s.file_metrics
            .insert("marker.rs".into(), FileComplexity::default());
        for (file, func) in declared {
            s.file_metrics
                .entry(PathBuf::from(file))
                .or_default()
                .functions
                .push(normal_function(func));
        }
        s.call_records = records;
        s
    }

    #[test]
    fn none_when_detection_did_not_run() {
        // ADR-005 backfill shape: records without file_metrics can't occur
        // in practice, but the guard must key off file_metrics regardless.
        let mut s = make_snapshot();
        s.call_records = vec![record("a.ts", "f", unresolved("x"), 1)];
        assert_eq!(call_graph_report(&s, &HealthThresholds::default()), None);
    }

    #[test]
    fn none_when_no_call_records() {
        let s = snap(Vec::new(), &[]);
        assert_eq!(
            call_graph_report(&s, &HealthThresholds::default()),
            None,
            "no call data must be None, never a fake zero-rate report"
        );
    }

    #[test]
    fn rate_counts_same_file_as_resolved_and_keeps_hubs_at_the_floor() {
        // D7: rate = (resolved + same_file) / total = (1 + 1) / 4 = 0.5 —
        // exactly the default floor, so hubs must be PRESENT (suppression
        // is strictly-below).
        let s = snap(
            vec![
                record("a.ts", "f", resolved("lib.ts", "helper"), 1),
                record("a.ts", "f", CalleeRef::SameFile("local".into()), 1),
                record("a.ts", "f", unresolved("save"), 1),
                record("a.ts", "g", unresolved("load"), 1),
            ],
            &[("lib.ts", "helper")],
        );
        let r = call_graph_report(&s, &HealthThresholds::default()).expect("report");
        assert!((r.resolution_rate - 0.5).abs() < f64::EPSILON);
        assert_eq!(r.edges_resolved, 1);
        assert_eq!(r.edges_same_file, 1);
        assert_eq!(r.edges_unresolved, 2);
        assert_eq!(
            r.function_hubs,
            vec![
                FunctionHub {
                    path: "a.ts".into(),
                    name: "local".into(),
                    resolved_in_degree: 1,
                },
                FunctionHub {
                    path: "lib.ts".into(),
                    name: "helper".into(),
                    resolved_in_degree: 1,
                },
            ]
        );
    }

    #[test]
    fn hubs_suppressed_strictly_below_floor_but_counts_stay() {
        // rate = 1/3 < 0.5: honest counts, no hub showcase.
        let s = snap(
            vec![
                record("a.ts", "f", resolved("lib.ts", "helper"), 1),
                record("a.ts", "f", unresolved("save"), 1),
                record("a.ts", "g", unresolved("load"), 1),
            ],
            &[("lib.ts", "helper")],
        );
        let r = call_graph_report(&s, &HealthThresholds::default()).expect("report");
        assert_eq!(r.edges_resolved, 1);
        assert_eq!(r.edges_unresolved, 2);
        assert!(r.function_hubs.is_empty(), "hubs must be suppressed: {r:?}");
    }

    #[test]
    fn all_resolved_rate_is_one() {
        let s = snap(
            vec![record("a.ts", "f", resolved("lib.ts", "helper"), 2)],
            &[("lib.ts", "helper")],
        );
        let r = call_graph_report(&s, &HealthThresholds::default()).expect("report");
        assert!((r.resolution_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn hub_in_degree_counts_distinct_callers_not_edge_counts() {
        // Two callers (one per file) each call helper many times: the
        // in-degree is 2 distinct callers, not the summed call counts.
        let s = snap(
            vec![
                record("a.ts", "f", resolved("lib.ts", "helper"), 5),
                record("b.ts", "g", resolved("lib.ts", "helper"), 7),
            ],
            &[("lib.ts", "helper")],
        );
        let r = call_graph_report(&s, &HealthThresholds::default()).expect("report");
        assert_eq!(
            r.function_hubs,
            vec![FunctionHub {
                path: "lib.ts".into(),
                name: "helper".into(),
                resolved_in_degree: 2,
            }]
        );
    }

    #[test]
    fn self_recursion_does_not_count_as_a_caller() {
        // Review F3: `walk` calling itself must not add itself to its own
        // distinct-caller in-degree.
        let s = snap(
            vec![
                record("lib.ts", "walk", CalleeRef::SameFile("walk".into()), 1),
                record("a.ts", "run", resolved("lib.ts", "walk"), 1),
            ],
            &[("lib.ts", "walk")],
        );
        let r = call_graph_report(&s, &HealthThresholds::default()).expect("report");
        assert_eq!(
            r.function_hubs,
            vec![FunctionHub {
                path: "lib.ts".into(),
                name: "walk".into(),
                resolved_in_degree: 1,
            }],
            "in-degree must count only the external caller"
        );
    }

    #[test]
    fn barrel_resolved_target_is_chased_to_declaring_file() {
        let mut s = snap(
            vec![record("a.ts", "f", resolved("src/index.ts", "B"), 1)],
            &[("src/b.ts", "B")],
        );
        s.reexports = vec![ReExportRecord {
            path: "src/index.ts".into(),
            target: "src/b.ts".into(),
            kind: ReExportKind::Named {
                exported: "B".into(),
                source: "B".into(),
            },
        }];
        let r = call_graph_report(&s, &HealthThresholds::default()).expect("report");
        assert_eq!(
            r.function_hubs,
            vec![FunctionHub {
                path: "src/b.ts".into(),
                name: "B".into(),
                resolved_in_degree: 1,
            }],
            "in-degree must land on the declaring file, not the barrel"
        );
    }

    #[test]
    fn barrel_and_direct_importers_of_an_undeclared_symbol_share_one_hub() {
        // Review F4: `export const useFetch = () => …` has no
        // FunctionMetrics entry, so the declares-chase dead-ends — the
        // named-only chase must still unify barrel importers with direct
        // importers on the terminal file.
        let mut s = snap(
            vec![
                record("a.ts", "f", resolved("src/index.ts", "useFetch"), 1),
                record("b.ts", "g", resolved("src/use_fetch.ts", "useFetch"), 1),
            ],
            &[],
        );
        s.reexports = vec![ReExportRecord {
            path: "src/index.ts".into(),
            target: "src/use_fetch.ts".into(),
            kind: ReExportKind::Named {
                exported: "useFetch".into(),
                source: "useFetch".into(),
            },
        }];
        let r = call_graph_report(&s, &HealthThresholds::default()).expect("report");
        assert_eq!(
            r.function_hubs,
            vec![FunctionHub {
                path: "src/use_fetch.ts".into(),
                name: "useFetch".into(),
                resolved_in_degree: 2,
            }],
            "one symbol must not split into a barrel row and a direct row"
        );
    }

    #[test]
    fn unchased_target_keeps_its_original_key() {
        // Target file declares no such function and is no barrel — the
        // hub keys on the resolved target as-is (constructor edges, D3).
        let s = snap(
            vec![record("a.ts", "f", resolved("lib.ts", "Widget"), 1)],
            &[],
        );
        let r = call_graph_report(&s, &HealthThresholds::default()).expect("report");
        assert_eq!(
            r.function_hubs,
            vec![FunctionHub {
                path: "lib.ts".into(),
                name: "Widget".into(),
                resolved_in_degree: 1,
            }]
        );
    }

    #[test]
    fn hubs_sorted_by_degree_desc_then_path_name_and_capped_at_ten() {
        // 12 targets: "top" with in-degree 2, eleven with degree 1 —
        // only 10 hubs total survive, "top" first, rest lexicographic.
        let mut records = vec![
            record("x1.ts", "f", resolved("lib.ts", "top"), 1),
            record("x2.ts", "g", resolved("lib.ts", "top"), 1),
        ];
        let names = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"];
        for n in names {
            records.push(record("x1.ts", "f", resolved("lib.ts", n), 1));
        }
        let declared: Vec<(&str, &str)> = names
            .iter()
            .map(|n| ("lib.ts", *n))
            .chain([("lib.ts", "top")])
            .collect();
        let s = snap(records, &declared);
        let r = call_graph_report(&s, &HealthThresholds::default()).expect("report");
        assert_eq!(r.function_hubs.len(), 10, "top-10 cap");
        assert_eq!(r.function_hubs[0].name, "top");
        assert_eq!(r.function_hubs[0].resolved_in_degree, 2);
        let rest: Vec<&str> = r.function_hubs[1..]
            .iter()
            .map(|h| h.name.as_str())
            .collect();
        assert_eq!(rest, vec!["a", "b", "c", "d", "e", "f", "g", "h", "i"]);
    }
}
