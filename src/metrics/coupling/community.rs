//! Single-level greedy modularity community detection (the local-moving
//! phase of Louvain, without graph coarsening) over the import graph.
//! Used only as additive structural evidence for change-coupling smells —
//! never scored on its own.

use std::collections::HashMap;
use std::path::PathBuf;

/// Adjacency list per node: `(neighbor index, edge weight)`.
type AdjacencyList = Vec<Vec<(usize, f64)>>;
/// Sorted node paths, adjacency list, per-node total degree, total edge weight (`m`).
type WeightedGraph = (Vec<PathBuf>, AdjacencyList, Vec<f64>, f64);

/// Partitions files into communities by modularity-optimizing over the
/// import graph, treated as unweighted-undirected (an edge A→B counts the
/// same as B→A; multiplicity sums into edge weight). Deterministic: nodes
/// are visited in a fixed sorted order every pass. Isolated files (no
/// import edges at all) each get their own singleton community.
pub(crate) fn detect_communities(
    import_graph: &HashMap<PathBuf, Vec<PathBuf>>,
) -> HashMap<PathBuf, usize> {
    let (nodes, neighbors, degree, total_weight) = build_graph(import_graph);
    let community = optimize_modularity(&neighbors, &degree, total_weight);
    nodes.into_iter().zip(community).collect()
}

/// Every distinct path referenced as an import source or target, sorted for
/// deterministic node indexing; a per-node adjacency list of (neighbor
/// index, edge weight), aggregating direction and multiplicity; each node's
/// total incident weight; and the graph's total edge weight (`m`).
fn build_graph(import_graph: &HashMap<PathBuf, Vec<PathBuf>>) -> WeightedGraph {
    let mut nodes: Vec<PathBuf> = import_graph
        .iter()
        .flat_map(|(source, targets)| std::iter::once(source).chain(targets.iter()))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    nodes.sort();
    let index: HashMap<&PathBuf, usize> = nodes.iter().enumerate().map(|(i, p)| (p, i)).collect();

    let mut weights: HashMap<(usize, usize), f64> = HashMap::new();
    for (source, targets) in import_graph {
        let a = index[source];
        for target in targets {
            let b = index[target];
            if a == b {
                continue; // self-loops don't affect community structure
            }
            let key = (a.min(b), a.max(b));
            *weights.entry(key).or_insert(0.0) += 1.0;
        }
    }

    let mut neighbors: Vec<Vec<(usize, f64)>> = vec![Vec::new(); nodes.len()];
    let mut degree = vec![0.0; nodes.len()];
    let mut total_weight = 0.0;
    for (&(a, b), &w) in &weights {
        neighbors[a].push((b, w));
        neighbors[b].push((a, w));
        degree[a] += w;
        degree[b] += w;
        total_weight += w;
    }

    (nodes, neighbors, degree, total_weight)
}

/// Louvain's local-moving phase: repeatedly move each node into whichever
/// neighboring community maximizes modularity gain, until a full pass makes
/// no moves. No graph coarsening — this is the single-level variant.
fn optimize_modularity(neighbors: &[Vec<(usize, f64)>], degree: &[f64], m: f64) -> Vec<usize> {
    let n = neighbors.len();
    let mut community: Vec<usize> = (0..n).collect();
    if n == 0 || m == 0.0 {
        return community;
    }
    let mut sigma_tot: Vec<f64> = degree.to_vec();
    let two_m = 2.0 * m;
    const MAX_PASSES: usize = 50;
    const EPSILON: f64 = 1e-9;

    for _ in 0..MAX_PASSES {
        let mut moved_any = false;
        for i in 0..n {
            let c_i = community[i];
            let k_i = degree[i];
            sigma_tot[c_i] -= k_i;

            let mut k_in: HashMap<usize, f64> = HashMap::new();
            for &(j, w) in &neighbors[i] {
                *k_in.entry(community[j]).or_insert(0.0) += w;
            }

            let mut best_c = c_i;
            let mut best_gain =
                k_in.get(&c_i).copied().unwrap_or(0.0) - sigma_tot[c_i] * k_i / two_m;
            for (&c, &k_in_c) in &k_in {
                if c == c_i {
                    continue;
                }
                let gain = k_in_c - sigma_tot[c] * k_i / two_m;
                if gain > best_gain + EPSILON {
                    best_gain = gain;
                    best_c = c;
                }
            }

            sigma_tot[best_c] += k_i;
            if best_c != c_i {
                community[i] = best_c;
                moved_any = true;
            }
        }
        if !moved_any {
            break;
        }
    }

    community
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(edges: &[(&str, &str)]) -> HashMap<PathBuf, Vec<PathBuf>> {
        let mut g: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        for (a, b) in edges {
            g.entry(PathBuf::from(a))
                .or_default()
                .push(PathBuf::from(b));
        }
        g
    }

    #[test]
    fn empty_graph_returns_empty_map() {
        let communities = detect_communities(&HashMap::new());
        assert!(communities.is_empty());
    }

    #[test]
    fn two_disconnected_triangles_form_two_communities() {
        let g = graph(&[
            ("a", "b"),
            ("b", "c"),
            ("c", "a"),
            ("x", "y"),
            ("y", "z"),
            ("z", "x"),
        ]);
        let communities = detect_communities(&g);
        let ca = communities[&PathBuf::from("a")];
        let cx = communities[&PathBuf::from("x")];
        assert_eq!(ca, communities[&PathBuf::from("b")]);
        assert_eq!(ca, communities[&PathBuf::from("c")]);
        assert_eq!(cx, communities[&PathBuf::from("y")]);
        assert_eq!(cx, communities[&PathBuf::from("z")]);
        assert_ne!(ca, cx);
    }

    #[test]
    fn single_clique_is_one_community() {
        let g = graph(&[
            ("a", "b"),
            ("a", "c"),
            ("a", "d"),
            ("b", "c"),
            ("b", "d"),
            ("c", "d"),
        ]);
        let communities = detect_communities(&g);
        let ca = communities[&PathBuf::from("a")];
        for n in ["b", "c", "d"] {
            assert_eq!(
                communities[&PathBuf::from(n)],
                ca,
                "node {n} should share community with a"
            );
        }
    }

    #[test]
    fn dumbbell_graph_splits_at_the_bridge() {
        // Two triangles joined by a single bridge edge c-x: the bridge is too
        // weak relative to within-cluster density to pull the clusters together.
        let g = graph(&[
            ("a", "b"),
            ("b", "c"),
            ("c", "a"),
            ("x", "y"),
            ("y", "z"),
            ("z", "x"),
            ("c", "x"),
        ]);
        let communities = detect_communities(&g);
        let ca = communities[&PathBuf::from("a")];
        let cx = communities[&PathBuf::from("x")];
        assert_eq!(ca, communities[&PathBuf::from("b")]);
        assert_eq!(ca, communities[&PathBuf::from("c")]);
        assert_eq!(cx, communities[&PathBuf::from("y")]);
        assert_eq!(cx, communities[&PathBuf::from("z")]);
        assert_ne!(ca, cx);
    }

    #[test]
    fn isolated_node_gets_its_own_community() {
        let mut g = graph(&[("a", "b"), ("b", "a")]);
        g.insert(PathBuf::from("solo"), Vec::new());
        let communities = detect_communities(&g);
        let ca = communities[&PathBuf::from("a")];
        assert_eq!(ca, communities[&PathBuf::from("b")]);
        assert_ne!(ca, communities[&PathBuf::from("solo")]);
    }

    #[test]
    fn self_loop_does_not_panic() {
        let g = graph(&[("a", "a")]);
        let communities = detect_communities(&g);
        assert!(communities.contains_key(&PathBuf::from("a")));
    }
}
