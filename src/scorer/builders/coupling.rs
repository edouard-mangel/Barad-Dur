//! Per-file coupling views: co-change pairs (test-pair aware), afferent /
//! efferent metrics, and the import graph's edges and cycles.

use std::collections::HashSet;

use crate::metrics::coupling::extract_component;
use crate::snapshot::RepoSnapshot;

use super::super::types::{CouplingPair, FileCouplingMetrics, ImportEdge};

fn file_stem(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    // Strip only the last extension so compound extensions like .test.ts are preserved
    match name.rfind('.') {
        Some(pos) => name[..pos].to_string(),
        None => name.to_string(),
    }
}

fn is_test_of(prod: &str, test: &str) -> bool {
    test == format!("{}test", prod)
        || test == format!("{}tests", prod)
        || test == format!("{}.test", prod)
        || test == format!("{}.spec", prod)
        || test == format!("{}_test", prod)
        || test == format!("{}_spec", prod)
        || test == format!("test_{}", prod)
}

fn is_test_pair(a: &str, b: &str) -> bool {
    let sa = file_stem(a).to_lowercase();
    let sb = file_stem(b).to_lowercase();
    is_test_of(&sa, &sb) || is_test_of(&sb, &sa)
}

/// Net in-window (added − deleted) lines per file across non-merge
/// commits — Ch. 14's "which coupled member actually grew" (trends M1).
fn net_growth_by_file(
    snapshot: &RepoSnapshot,
) -> std::collections::HashMap<&std::path::PathBuf, i64> {
    let known = snapshot.known_paths();
    snapshot
        .commits
        .iter()
        .filter(|c| !c.is_merge)
        .flat_map(|c| c.files_changed.iter())
        // Same membership universe as the churn timeline — excluded paths
        // never enter the map (they could not be looked up anyway).
        .filter(move |fc| known.contains(&fc.path))
        .fold(std::collections::HashMap::new(), |mut m, fc| {
            *m.entry(&fc.path).or_insert(0) += i64::from(fc.additions) - i64::from(fc.deletions);
            m
        })
}

pub(crate) fn build_coupling_pairs(
    snapshot: &RepoSnapshot,
    component_depth: usize,
) -> Vec<CouplingPair> {
    let growth = net_growth_by_file(snapshot);
    snapshot
        .file_change_pairs
        .iter()
        .map(|(a, b, co)| {
            let a_changes = snapshot
                .commits_by_file
                .get(a)
                .map(|v| v.len())
                .unwrap_or(0);
            let b_changes = snapshot
                .commits_by_file
                .get(b)
                .map(|v| v.len())
                .unwrap_or(0);
            let min_changes = a_changes.min(b_changes).max(1);
            let coupling_pct = (*co as f64 / min_changes as f64 * 100.0).min(100.0);
            let cross_boundary =
                extract_component(a, component_depth) != extract_component(b, component_depth);
            CouplingPair {
                file_a: a.to_string_lossy().to_string(),
                file_b: b.to_string_lossy().to_string(),
                co_changes: *co,
                coupling_pct,
                cross_boundary,
                is_test_pair: is_test_pair(&a.to_string_lossy(), &b.to_string_lossy()),
                growth_a: growth.get(a).copied().unwrap_or(0),
                growth_b: growth.get(b).copied().unwrap_or(0),
            }
        })
        .collect()
}

pub(crate) fn build_per_file_coupling(snapshot: &RepoSnapshot) -> Vec<FileCouplingMetrics> {
    let mut metrics: Vec<FileCouplingMetrics> = snapshot
        .files
        .iter()
        .map(|file| {
            let path_str = file.path.to_string_lossy().to_string();
            let ce = snapshot
                .import_graph
                .get(&file.path)
                .map(|imports| imports.len())
                .unwrap_or(0);
            let ca = snapshot
                .import_graph
                .values()
                .filter(|imports| imports.contains(&file.path))
                .count();
            let instability = if ca + ce == 0 {
                0.0
            } else {
                ce as f64 / (ca + ce) as f64
            };
            FileCouplingMetrics {
                path: path_str,
                ca,
                ce,
                instability,
            }
        })
        .collect();

    metrics.sort_by(|a, b| {
        b.instability
            .partial_cmp(&a.instability)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.path.cmp(&b.path))
    });
    metrics
}

pub(crate) fn build_import_edges(snapshot: &RepoSnapshot) -> Vec<ImportEdge> {
    let mut edges: Vec<ImportEdge> = snapshot
        .import_graph
        .iter()
        .flat_map(|(from, imports)| {
            imports.iter().map(|to| ImportEdge {
                from: from.to_string_lossy().to_string(),
                to: to.to_string_lossy().to_string(),
            })
        })
        .collect();

    edges.sort_by(|a, b| a.from.cmp(&b.from).then_with(|| a.to.cmp(&b.to)));
    edges
}

/// Detect import cycles as sorted node lists: A↔B (depth 1) and
/// A→B→C→A (depth 2). Same semantics as the circular-dependencies
/// metric, but keeps the member files so renderers can highlight the
/// offending edges instead of only counting them.
pub(crate) fn build_import_cycles(snapshot: &RepoSnapshot) -> Vec<Vec<String>> {
    let graph = &snapshot.import_graph;
    let mut cycles: HashSet<Vec<String>> = HashSet::new();

    for (a, targets_a) in graph {
        for b in targets_a {
            let Some(targets_b) = graph.get(b) else {
                continue;
            };
            if targets_b.contains(a) {
                let mut pair = vec![
                    a.to_string_lossy().to_string(),
                    b.to_string_lossy().to_string(),
                ];
                pair.sort();
                cycles.insert(pair);
            }
            for c in targets_b {
                if c != a && c != b && graph.get(c).map(|t| t.contains(a)).unwrap_or(false) {
                    let mut trio = vec![
                        a.to_string_lossy().to_string(),
                        b.to_string_lossy().to_string(),
                        c.to_string_lossy().to_string(),
                    ];
                    trio.sort();
                    cycles.insert(trio);
                }
            }
        }
    }

    let mut result: Vec<Vec<String>> = cycles.into_iter().collect();
    result.sort();
    result
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::snapshot::{CommitId, TimeWindow};
    use std::path::PathBuf;

    #[test]
    fn coupling_pairs_carry_net_growth_per_side() {
        use crate::snapshot::{ChangeType, Commit, FileChange};
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.files = vec![make_file_entry("a.rs"), make_file_entry("b.rs")];
        snapshot.file_change_pairs = vec![(PathBuf::from("a.rs"), PathBuf::from("b.rs"), 3)];
        let change = |p: &str, add, del| FileChange {
            path: p.into(),
            additions: add,
            deletions: del,
            change_type: ChangeType::Modified,
        };
        let commit = |id, files: Vec<FileChange>, is_merge| Commit {
            id: CommitId(id),
            author: 0,
            timestamp: chrono::Utc::now(),
            message: String::new(),
            files_changed: files,
            is_merge,
            parent_count: if is_merge { 2 } else { 1 },
        };
        snapshot.commits = vec![
            // a.rs grows (+40 −10 = +30); b.rs shrinks (+5 −25 = −20).
            commit(
                0,
                vec![change("a.rs", 40, 10), change("b.rs", 5, 25)],
                false,
            ),
            // Merge churn must not count toward growth.
            commit(
                1,
                vec![change("a.rs", 900, 0), change("b.rs", 900, 0)],
                true,
            ),
        ];
        let pairs = build_coupling_pairs(&snapshot, 2);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].growth_a, 30, "a.rs net growth");
        assert_eq!(pairs[0].growth_b, -20, "b.rs net shrink");
    }

    fn make_snapshot_with_imports(
        files: Vec<&str>,
        import_graph: Vec<(&str, Vec<&str>)>,
    ) -> RepoSnapshot {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.files = files.into_iter().map(make_file_entry).collect();
        snapshot.import_graph = import_graph
            .into_iter()
            .map(|(from, imports)| {
                (
                    PathBuf::from(from),
                    imports.into_iter().map(PathBuf::from).collect(),
                )
            })
            .collect();
        snapshot
    }

    #[test]
    fn is_test_pair_detects_suffix_test() {
        assert!(is_test_pair(
            "src/UserService.java",
            "tests/UserServiceTest.java"
        ));
        assert!(is_test_pair(
            "src/UserService.java",
            "tests/UserServiceTests.java"
        ));
        assert!(is_test_pair(
            "tests/UserServiceTest.java",
            "src/UserService.java"
        )); // symmetric
    }

    #[test]
    fn is_test_pair_detects_dot_test_spec() {
        assert!(is_test_pair("src/parser.ts", "src/parser.test.ts"));
        assert!(is_test_pair("src/parser.ts", "src/parser.spec.ts"));
        assert!(is_test_pair("src/parser.test.ts", "src/parser.ts"));
    }

    #[test]
    fn is_test_pair_detects_underscore_test_spec() {
        assert!(is_test_pair("user.go", "user_test.go"));
        assert!(is_test_pair("user.go", "user_spec.go"));
        assert!(is_test_pair("user_test.go", "user.go"));
    }

    #[test]
    fn is_test_pair_detects_test_prefix() {
        assert!(is_test_pair("user.py", "test_user.py"));
        assert!(is_test_pair("test_user.py", "user.py"));
    }

    #[test]
    fn is_test_pair_case_insensitive() {
        assert!(is_test_pair("UserService.cs", "USERSERVICETEST.cs"));
    }

    #[test]
    fn is_test_pair_rejects_unrelated_pairs() {
        assert!(!is_test_pair("src/user.rs", "src/order.rs"));
        assert!(!is_test_pair("src/user.rs", "src/user_handler.rs"));
    }

    #[test]
    fn coupling_pair_non_test_pair_is_false() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.files = vec![make_file_entry("src/foo.rs"), make_file_entry("src/bar.rs")];
        let a = PathBuf::from("src/foo.rs");
        let b = PathBuf::from("src/bar.rs");
        snapshot.file_change_pairs = vec![(a.clone(), b.clone(), 3)];
        snapshot
            .commits_by_file
            .insert(a, vec![CommitId(0), CommitId(1), CommitId(2)]);
        snapshot
            .commits_by_file
            .insert(b, vec![CommitId(0), CommitId(1), CommitId(2)]);

        let pairs = build_coupling_pairs(&snapshot, 1);
        assert_eq!(pairs.len(), 1);
        assert!(!pairs[0].is_test_pair);
    }

    #[test]
    fn coupling_pair_test_file_is_flagged() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.files = vec![
            make_file_entry("src/user.go"),
            make_file_entry("src/user_test.go"),
        ];
        let a = PathBuf::from("src/user.go");
        let b = PathBuf::from("src/user_test.go");
        snapshot.file_change_pairs = vec![(a.clone(), b.clone(), 3)];
        snapshot
            .commits_by_file
            .insert(a, vec![CommitId(0), CommitId(1), CommitId(2)]);
        snapshot
            .commits_by_file
            .insert(b, vec![CommitId(0), CommitId(1), CommitId(2)]);

        let pairs = build_coupling_pairs(&snapshot, 1);
        assert_eq!(pairs.len(), 1);
        assert!(
            pairs[0].is_test_pair,
            "user.go ↔ user_test.go must be flagged as a test pair"
        );
    }

    #[test]
    fn import_edges_empty_graph() {
        let snapshot = make_snapshot_with_imports(vec!["src/isolated.rs"], vec![]);

        assert!(build_import_edges(&snapshot).is_empty());
    }

    #[test]
    fn import_edges_flattens_and_sorts_graph() {
        // HashMap iteration order is arbitrary — edges must come out
        // sorted by (from, to) so report output stays deterministic.
        let snapshot = make_snapshot_with_imports(
            vec!["src/a.rs", "src/b.rs", "src/core.rs", "src/dep.rs"],
            vec![
                ("src/b.rs", vec!["src/core.rs"]),
                ("src/a.rs", vec!["src/dep.rs", "src/core.rs"]),
            ],
        );

        let edges = build_import_edges(&snapshot);

        let as_pairs: Vec<(&str, &str)> = edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect();
        assert_eq!(
            as_pairs,
            vec![
                ("src/a.rs", "src/core.rs"),
                ("src/a.rs", "src/dep.rs"),
                ("src/b.rs", "src/core.rs"),
            ]
        );
    }

    #[test]
    fn import_cycles_empty_graph() {
        let snapshot = make_snapshot_with_imports(vec!["src/a.rs"], vec![]);

        assert!(build_import_cycles(&snapshot).is_empty());
    }

    #[test]
    fn import_cycles_chain_without_cycle_is_empty() {
        let snapshot = make_snapshot_with_imports(
            vec!["src/a.rs", "src/b.rs", "src/c.rs"],
            vec![
                ("src/a.rs", vec!["src/b.rs"]),
                ("src/b.rs", vec!["src/c.rs"]),
            ],
        );

        assert!(build_import_cycles(&snapshot).is_empty());
    }

    #[test]
    fn import_cycles_detects_direct_cycle() {
        let snapshot = make_snapshot_with_imports(
            vec!["src/a.rs", "src/b.rs"],
            vec![
                ("src/a.rs", vec!["src/b.rs"]),
                ("src/b.rs", vec!["src/a.rs"]),
            ],
        );

        let cycles = build_import_cycles(&snapshot);

        assert_eq!(cycles, vec![vec!["src/a.rs", "src/b.rs"]]);
    }

    #[test]
    fn import_cycles_detects_depth_two_cycle_once() {
        // a→b→c→a is reachable from all three starting points but must
        // be reported as a single cycle.
        let snapshot = make_snapshot_with_imports(
            vec!["src/a.rs", "src/b.rs", "src/c.rs"],
            vec![
                ("src/a.rs", vec!["src/b.rs"]),
                ("src/b.rs", vec!["src/c.rs"]),
                ("src/c.rs", vec!["src/a.rs"]),
            ],
        );

        let cycles = build_import_cycles(&snapshot);

        assert_eq!(cycles, vec![vec!["src/a.rs", "src/b.rs", "src/c.rs"]]);
    }

    #[test]
    fn per_file_coupling_no_deps() {
        // file with no imports and no dependents → ca=0, ce=0, instability=0.0
        let snapshot = make_snapshot_with_imports(vec!["src/isolated.rs"], vec![]);

        let result = build_per_file_coupling(&snapshot);

        assert_eq!(result.len(), 1);
        let m = &result[0];
        assert_eq!(m.path, "src/isolated.rs");
        assert_eq!(m.ca, 0);
        assert_eq!(m.ce, 0);
        assert!((m.instability - 0.0_f64).abs() < f64::EPSILON);
    }

    #[test]
    fn per_file_coupling_mixed_deps() {
        // file imported by 3 others and importing 1 → ca=3, ce=1, instability=0.25
        let snapshot = make_snapshot_with_imports(
            vec![
                "src/core.rs",
                "src/a.rs",
                "src/b.rs",
                "src/c.rs",
                "src/dep.rs",
            ],
            vec![
                ("src/a.rs", vec!["src/core.rs"]),
                ("src/b.rs", vec!["src/core.rs"]),
                ("src/c.rs", vec!["src/core.rs"]),
                ("src/core.rs", vec!["src/dep.rs"]),
            ],
        );

        let result = build_per_file_coupling(&snapshot);

        let core = result.iter().find(|m| m.path == "src/core.rs").unwrap();
        assert_eq!(core.ca, 3, "ca should be 3 (imported by a, b, c)");
        assert_eq!(core.ce, 1, "ce should be 1 (imports dep)");
        assert!(
            (core.instability - 0.25_f64).abs() < 1e-10,
            "instability should be 0.25, got {}",
            core.instability
        );
    }

    #[test]
    fn per_file_coupling_pure_efferent() {
        // file with ce=5, ca=0 → instability=1.0
        let snapshot = make_snapshot_with_imports(
            vec![
                "src/leaf.rs",
                "src/dep1.rs",
                "src/dep2.rs",
                "src/dep3.rs",
                "src/dep4.rs",
                "src/dep5.rs",
            ],
            vec![(
                "src/leaf.rs",
                vec![
                    "src/dep1.rs",
                    "src/dep2.rs",
                    "src/dep3.rs",
                    "src/dep4.rs",
                    "src/dep5.rs",
                ],
            )],
        );

        let result = build_per_file_coupling(&snapshot);

        let leaf = result.iter().find(|m| m.path == "src/leaf.rs").unwrap();
        assert_eq!(leaf.ca, 0);
        assert_eq!(leaf.ce, 5);
        assert!(
            (leaf.instability - 1.0_f64).abs() < f64::EPSILON,
            "instability should be 1.0, got {}",
            leaf.instability
        );
    }

    #[test]
    fn per_file_coupling_pure_afferent() {
        // file with ca=5, ce=0 → instability=0.0
        let snapshot = make_snapshot_with_imports(
            vec![
                "src/stable.rs",
                "src/user1.rs",
                "src/user2.rs",
                "src/user3.rs",
                "src/user4.rs",
                "src/user5.rs",
            ],
            vec![
                ("src/user1.rs", vec!["src/stable.rs"]),
                ("src/user2.rs", vec!["src/stable.rs"]),
                ("src/user3.rs", vec!["src/stable.rs"]),
                ("src/user4.rs", vec!["src/stable.rs"]),
                ("src/user5.rs", vec!["src/stable.rs"]),
            ],
        );

        let result = build_per_file_coupling(&snapshot);

        let stable = result.iter().find(|m| m.path == "src/stable.rs").unwrap();
        assert_eq!(stable.ca, 5);
        assert_eq!(stable.ce, 0);
        assert!(
            (stable.instability - 0.0_f64).abs() < f64::EPSILON,
            "instability should be 0.0, got {}",
            stable.instability
        );
    }
}
