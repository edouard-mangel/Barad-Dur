use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::metrics::{CategoryResult, MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

pub fn compute_coupling(snapshot: &RepoSnapshot) -> CategoryResult {
    let metrics = vec![
        afferent_coupling(snapshot),
        efferent_coupling(snapshot),
        circular_dependencies(snapshot),
    ];
    CategoryResult {
        name: "Coupling".to_string(),
        score: 0,
        metrics,
    }
    .compute_score()
}

/// Compute median of a non-empty slice (sorts in place).
fn median(values: &mut [usize]) -> f64 {
    values.sort_unstable();
    let len = values.len();
    #[allow(clippy::manual_is_multiple_of)] // is_multiple_of is unstable on CI's stable Rust
    if len % 2 == 0 {
        (values[len / 2 - 1] + values[len / 2]) as f64 / 2.0
    } else {
        values[len / 2] as f64
    }
}

/// Afferent coupling (Ca): how many files depend on each file (incoming imports).
///
/// Scored on the median Ca rather than the max. A single hub (core data model)
/// is normal — what matters is whether *most* files have excessive incoming deps.
fn afferent_coupling(snapshot: &RepoSnapshot) -> MetricValue {
    let mut incoming: HashMap<&PathBuf, usize> = HashMap::new();
    for targets in snapshot.import_graph.values() {
        for target in targets {
            *incoming.entry(target).or_insert(0) += 1;
        }
    }

    if incoming.is_empty() {
        return MetricValue {
            name: "Afferent coupling".to_string(),
            description: "No import dependencies detected".to_string(),
            raw_value: RawValue::Count(0),
            score: 100,
        };
    }

    // Include all files in the distribution (files with zero incoming deps too),
    // so a single hub doesn't skew the median.
    let mut ca_values: Vec<usize> = snapshot
        .files
        .iter()
        .map(|f| incoming.get(&f.path).copied().unwrap_or(0))
        .collect();

    let max_ca = ca_values.iter().copied().max().unwrap_or(0);
    let mean_ca = ca_values.iter().sum::<usize>() as f64 / ca_values.len() as f64;
    let median_ca = median(&mut ca_values);

    // Score on median: most files having few dependents is healthy
    let score = if median_ca <= 2.0 {
        100
    } else if median_ca <= 5.0 {
        75
    } else if median_ca <= 10.0 {
        50
    } else {
        25
    };

    MetricValue {
        name: "Afferent coupling".to_string(),
        description: format!(
            "Incoming deps — median: {:.1}, mean: {:.1}, max: {}",
            median_ca, mean_ca, max_ca
        ),
        raw_value: RawValue::Float(median_ca),
        score,
    }
}

/// Efferent coupling (Ce): how many files each file imports (outgoing).
///
/// Scored on the median Ce. A few orchestrator files with many imports are
/// expected — the score reflects whether the typical file is well-scoped.
fn efferent_coupling(snapshot: &RepoSnapshot) -> MetricValue {
    if snapshot.import_graph.is_empty() {
        return MetricValue {
            name: "Efferent coupling".to_string(),
            description: "No import dependencies detected".to_string(),
            raw_value: RawValue::Count(0),
            score: 100,
        };
    }

    // Include all files in the distribution (files with zero outgoing imports too).
    let mut ce_values: Vec<usize> = snapshot
        .files
        .iter()
        .map(|f| {
            snapshot
                .import_graph
                .get(&f.path)
                .map(|v| v.len())
                .unwrap_or(0)
        })
        .collect();

    let max_ce = ce_values.iter().copied().max().unwrap_or(0);
    let mean_ce = ce_values.iter().sum::<usize>() as f64 / ce_values.len() as f64;
    let median_ce = median(&mut ce_values);

    // Score on median: most files importing few deps is healthy
    let score = if median_ce <= 3.0 {
        100
    } else if median_ce <= 6.0 {
        75
    } else if median_ce <= 12.0 {
        50
    } else {
        25
    };

    MetricValue {
        name: "Efferent coupling".to_string(),
        description: format!(
            "Outgoing deps — median: {:.1}, mean: {:.1}, max: {}",
            median_ce, mean_ce, max_ce
        ),
        raw_value: RawValue::Float(median_ce),
        score,
    }
}

/// Circular dependencies: file pairs where A→B and B→A (depth 1) or
/// A→B→C→A (depth 2).
fn circular_dependencies(snapshot: &RepoSnapshot) -> MetricValue {
    let mut cycles: HashSet<(PathBuf, PathBuf)> = HashSet::new();

    for (a, targets_a) in &snapshot.import_graph {
        for b in targets_a {
            // Direct cycle: A→B and B→A
            if let Some(targets_b) = snapshot.import_graph.get(b) {
                if targets_b.contains(a) {
                    let pair = if a < b {
                        (a.clone(), b.clone())
                    } else {
                        (b.clone(), a.clone())
                    };
                    cycles.insert(pair);
                }
                // Depth-2 cycle: A→B→C→A
                for c in targets_b {
                    if c != a && c != b {
                        if let Some(targets_c) = snapshot.import_graph.get(c) {
                            if targets_c.contains(a) {
                                let mut trio = [a.clone(), b.clone(), c.clone()];
                                trio.sort();
                                cycles.insert((trio[0].clone(), trio[1].clone()));
                            }
                        }
                    }
                }
            }
        }
    }

    let count = cycles.len();
    let score = match count {
        0 => 100,
        1..=2 => 75,
        3..=5 => 50,
        _ => 25,
    };

    let cycle_list: Vec<String> = cycles
        .iter()
        .take(10)
        .map(|(a, b)| format!("{} <-> {}", a.display(), b.display()))
        .collect();

    MetricValue {
        name: "Circular dependencies".to_string(),
        description: format!("{} circular dependency pairs detected", count),
        raw_value: RawValue::List(cycle_list),
        score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::*;
    use std::path::PathBuf;

    fn empty_snapshot() -> RepoSnapshot {
        RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        )
    }

    fn make_file(name: &str) -> FileEntry {
        FileEntry {
            path: PathBuf::from(name),
            size_bytes: 100,
            is_binary: false,
            depth: 1,
            blob_oid: String::new(),
        }
    }

    #[test]
    fn afferent_coupling_empty_graph() {
        let snapshot = empty_snapshot();
        let result = afferent_coupling(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn afferent_coupling_single_hub_scores_well() {
        let mut snapshot = empty_snapshot();
        // 20 source files + 1 hub → 21 files total, only core.rs has Ca=20
        // Median Ca across all 21 files = 0 (most have zero dependents) → score 100
        snapshot.files.push(make_file("core.rs"));
        for i in 0..20 {
            let name = format!("f{}.rs", i);
            snapshot.files.push(make_file(&name));
            snapshot
                .import_graph
                .insert(PathBuf::from(&name), vec![PathBuf::from("core.rs")]);
        }
        let result = afferent_coupling(&snapshot);
        assert_eq!(result.score, 100); // median Ca=0, single hub is fine
    }

    #[test]
    fn afferent_coupling_widespread_deps_scores_lower() {
        let mut snapshot = empty_snapshot();
        // 6 hub files, each depended on by 30 source files
        // All files in the repo: 6 hubs + 30 sources = 36
        // Ca distribution: 6 files with Ca=30, 30 files with Ca=0
        // Median Ca = 0 (majority are sources) → still 100
        // But if we flip it: 30 files each depending on all 6 hubs,
        // and the 6 hubs also depend on each other...
        // Let's make a scenario where median is actually high:
        // 10 files, each one is depended upon by all 9 others → Ca=9 each
        for i in 0..10 {
            snapshot.files.push(make_file(&format!("f{}.rs", i)));
            let targets: Vec<PathBuf> = (0..10)
                .filter(|&j| j != i)
                .map(|j| PathBuf::from(format!("f{}.rs", j)))
                .collect();
            snapshot
                .import_graph
                .insert(PathBuf::from(format!("f{}.rs", i)), targets);
        }
        let result = afferent_coupling(&snapshot);
        assert!(result.score <= 50, "score={}, expected <=50", result.score);
    }

    #[test]
    fn afferent_coupling_description_shows_distribution() {
        let mut snapshot = empty_snapshot();
        snapshot.files.push(make_file("core.rs"));
        for i in 0..5 {
            let name = format!("f{}.rs", i);
            snapshot.files.push(make_file(&name));
            snapshot
                .import_graph
                .insert(PathBuf::from(&name), vec![PathBuf::from("core.rs")]);
        }
        let result = afferent_coupling(&snapshot);
        assert!(result.description.contains("median:"));
        assert!(result.description.contains("mean:"));
        assert!(result.description.contains("max:"));
    }

    #[test]
    fn efferent_coupling_empty_graph() {
        let snapshot = empty_snapshot();
        let result = efferent_coupling(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn efferent_coupling_single_heavy_file_scores_well() {
        let mut snapshot = empty_snapshot();
        // 36 files total: 1 heavy (25 imports), 10 light (1 import), 25 deps
        // Median Ce across 36 files: mostly 0 → score 100
        snapshot.files.push(make_file("main.rs"));
        snapshot.import_graph.insert(
            PathBuf::from("main.rs"),
            (0..25)
                .map(|i| PathBuf::from(format!("dep{}.rs", i)))
                .collect(),
        );
        for i in 0..25 {
            snapshot.files.push(make_file(&format!("dep{}.rs", i)));
        }
        for i in 0..10 {
            let name = format!("small{}.rs", i);
            snapshot.files.push(make_file(&name));
            snapshot
                .import_graph
                .insert(PathBuf::from(&name), vec![PathBuf::from("util.rs")]);
        }
        let result = efferent_coupling(&snapshot);
        assert_eq!(result.score, 100); // median Ce ≈ 0
    }

    #[test]
    fn efferent_coupling_all_heavy_scores_low() {
        let mut snapshot = empty_snapshot();
        // 10 files, each imports 15 unique deps (all files in the repo)
        // All 10 files have Ce=15, no other files → median Ce=15 → score 25
        for i in 0..10 {
            let name = format!("f{}.rs", i);
            snapshot.files.push(make_file(&name));
            snapshot.import_graph.insert(
                PathBuf::from(&name),
                (0..15)
                    .map(|j| PathBuf::from(format!("dep{}_{}.rs", i, j)))
                    .collect(),
            );
        }
        let result = efferent_coupling(&snapshot);
        assert_eq!(result.score, 25);
    }

    #[test]
    fn efferent_coupling_description_shows_distribution() {
        let mut snapshot = empty_snapshot();
        snapshot.files.push(make_file("a.rs"));
        snapshot.files.push(make_file("b.rs"));
        snapshot.files.push(make_file("c.rs"));
        snapshot.import_graph.insert(
            PathBuf::from("a.rs"),
            vec![PathBuf::from("b.rs"), PathBuf::from("c.rs")],
        );
        let result = efferent_coupling(&snapshot);
        assert!(result.description.contains("median:"));
        assert!(result.description.contains("mean:"));
        assert!(result.description.contains("max:"));
    }

    #[test]
    fn circular_deps_none() {
        let mut snapshot = empty_snapshot();
        snapshot
            .import_graph
            .insert(PathBuf::from("a.rs"), vec![PathBuf::from("b.rs")]);
        // b does not import a → no cycle
        let result = circular_dependencies(&snapshot);
        assert_eq!(result.score, 100);
    }

    #[test]
    fn circular_deps_direct() {
        let mut snapshot = empty_snapshot();
        snapshot
            .import_graph
            .insert(PathBuf::from("a.rs"), vec![PathBuf::from("b.rs")]);
        snapshot
            .import_graph
            .insert(PathBuf::from("b.rs"), vec![PathBuf::from("a.rs")]);
        let result = circular_dependencies(&snapshot);
        assert_eq!(result.score, 75); // 1 cycle
    }

    #[test]
    fn circular_deps_transitive_depth2() {
        let mut snapshot = empty_snapshot();
        // A→B→C→A
        snapshot
            .import_graph
            .insert(PathBuf::from("a.rs"), vec![PathBuf::from("b.rs")]);
        snapshot
            .import_graph
            .insert(PathBuf::from("b.rs"), vec![PathBuf::from("c.rs")]);
        snapshot
            .import_graph
            .insert(PathBuf::from("c.rs"), vec![PathBuf::from("a.rs")]);
        let result = circular_dependencies(&snapshot);
        assert!(result.score < 100, "should detect depth-2 cycle");
    }

    #[test]
    fn circular_deps_many() {
        let mut snapshot = empty_snapshot();
        // 6 direct cycles → score 25
        for i in 0..6 {
            let a = PathBuf::from(format!("a{}.rs", i));
            let b = PathBuf::from(format!("b{}.rs", i));
            snapshot.import_graph.insert(a.clone(), vec![b.clone()]);
            snapshot.import_graph.insert(b, vec![a]);
        }
        let result = circular_dependencies(&snapshot);
        assert_eq!(result.score, 25);
    }

    #[test]
    fn compute_coupling_returns_three_metrics() {
        let snapshot = empty_snapshot();
        let result = compute_coupling(&snapshot);
        assert_eq!(result.metrics.len(), 3);
        assert_eq!(result.name, "Coupling");
    }
}
