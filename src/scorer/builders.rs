use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::config::CouplingThresholds;
use crate::metrics::coupling::{barrel_bypass_findings, extract_component};
use crate::snapshot::{CouplingKind, RepoSnapshot};

use super::actions::score_commit_message;
use super::types::{
    AuthorCard, AuthorShare, CouplingPair, FileAge, FileCouplingMetrics, FileOwnership,
    HotspotFile, ImportEdge,
};

const BUG_KEYWORDS: &[&str] = &["fix", "bug", "broken", "crash", "regression"];

/// Number of equal time slices the analysis window is split into for the
/// per-file churn sparkline. All files share the same axis so timelines
/// are comparable across rows.
const CHURN_TIMELINE_BUCKETS: usize = 12;

fn churn_timeline(
    commit_ids: Option<&Vec<crate::snapshot::CommitId>>,
    ts_by_id: &HashMap<crate::snapshot::CommitId, i64>,
    min_ts: i64,
    max_ts: i64,
) -> Vec<u32> {
    if min_ts > max_ts {
        return vec![0; CHURN_TIMELINE_BUCKETS]; // snapshot has no commits
    }
    let span = (max_ts - min_ts + 1) as i128;
    commit_ids
        .into_iter()
        .flatten()
        .filter_map(|id| ts_by_id.get(id))
        .fold(vec![0u32; CHURN_TIMELINE_BUCKETS], |mut acc, ts| {
            let idx = ((*ts - min_ts) as i128 * CHURN_TIMELINE_BUCKETS as i128 / span) as usize;
            acc[idx.min(CHURN_TIMELINE_BUCKETS - 1)] += 1;
            acc
        })
}

pub(super) fn build_hotspots(
    snapshot: &RepoSnapshot,
    coupling: &CouplingThresholds,
) -> Vec<HotspotFile> {
    // Pre-classify bug-fix commits by ID to avoid O(files × commits) message scanning.
    let bug_commit_ids: HashSet<crate::snapshot::CommitId> = snapshot
        .commits
        .iter()
        .filter(|c| {
            let msg = c.message.to_lowercase();
            BUG_KEYWORDS.iter().any(|kw| msg.contains(kw))
        })
        .map(|c| c.id)
        .collect();

    let ts_by_id: HashMap<crate::snapshot::CommitId, i64> = snapshot
        .commits
        .iter()
        .map(|c| (c.id, c.timestamp.timestamp()))
        .collect();
    let (min_ts, max_ts) = ts_by_id
        .values()
        .fold((i64::MAX, i64::MIN), |(lo, hi), &t| (lo.min(t), hi.max(t)));

    let barrel = if coupling.content_barrel_rule {
        barrel_bypass_findings(snapshot, coupling.component_depth)
    } else {
        Vec::new()
    };
    let finding_counts: HashMap<&Path, (usize, usize, usize)> = snapshot
        .coupling_findings
        .iter()
        .chain(barrel.iter())
        .fold(HashMap::new(), |mut acc, f| {
            let entry = acc.entry(f.path.as_path()).or_default();
            match f.kind {
                CouplingKind::Content => entry.0 += 1,
                CouplingKind::Common => entry.1 += 1,
                CouplingKind::Control => entry.2 += 1,
            }
            acc
        });

    let mut files: Vec<HotspotFile> = snapshot
        .files
        .iter()
        .filter(|f| !f.is_binary)
        .map(|f| {
            let commit_ids = snapshot.commits_by_file.get(&f.path);
            let churn = commit_ids.map(|v| v.len()).unwrap_or(0);
            let bug_commit_count = commit_ids
                .map(|ids| ids.iter().filter(|id| bug_commit_ids.contains(id)).count())
                .unwrap_or(0);
            let metrics = snapshot
                .file_metrics
                .get(&f.path)
                .cloned()
                .unwrap_or_default();
            let (content_findings, common_findings, control_findings) = finding_counts
                .get(f.path.as_path())
                .copied()
                .unwrap_or((0, 0, 0));
            HotspotFile {
                path: f.path.to_string_lossy().to_string(),
                churn_count: churn,
                bug_commit_count,
                loc: metrics.loc,
                total_lines: metrics.total_lines,
                cyclomatic_complexity: metrics.cyclomatic_complexity,
                public_methods: metrics.public_methods,
                properties: metrics.properties,
                hotspot_score: 0.0,
                content_findings,
                common_findings,
                control_findings,
                churn_timeline: churn_timeline(commit_ids, &ts_by_id, min_ts, max_ts),
            }
        })
        .collect();

    if files.is_empty() {
        return files;
    }

    let max_churn = files
        .iter()
        .map(|f| f.churn_count)
        .max()
        .unwrap_or(1)
        .max(1);
    let max_cc = files
        .iter()
        .map(|f| f.cyclomatic_complexity as usize)
        .max()
        .unwrap_or(1)
        .max(1);
    let max_loc = files.iter().map(|f| f.loc).max().unwrap_or(1).max(1);

    for f in &mut files {
        let churn_norm = f.churn_count as f64 / max_churn as f64;
        let cc_norm = f.cyclomatic_complexity as f64 / max_cc as f64;
        let loc_norm = f.loc as f64 / max_loc as f64;
        let base = (churn_norm * 0.5 + cc_norm * 0.3 + loc_norm * 0.2) * 100.0;
        // Content/Common findings multiply risk (severity × change
        // frequency); capped because every consumer assumes 0–100.
        f.hotspot_score = if f.content_findings + f.common_findings > 0 {
            (base * coupling.hotspot_multiplier).min(100.0)
        } else {
            base
        };
    }

    files.sort_by(|a, b| b.hotspot_score.partial_cmp(&a.hotspot_score).unwrap());
    files
}

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

pub(super) fn build_coupling_pairs(
    snapshot: &RepoSnapshot,
    component_depth: usize,
) -> Vec<CouplingPair> {
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
            }
        })
        .collect()
}

pub(super) fn build_author_ownership(snapshot: &RepoSnapshot) -> Vec<FileOwnership> {
    snapshot
        .blame_map
        .iter()
        .map(|(path, lines)| {
            let mut author_counts: HashMap<usize, usize> = HashMap::new();
            for line in lines {
                *author_counts.entry(line.author_id).or_insert(0) += line.line_count;
            }
            let total: usize = lines.iter().map(|l| l.line_count).sum::<usize>().max(1);
            let mut authors: Vec<AuthorShare> = author_counts
                .into_iter()
                .map(|(id, count)| {
                    let name = snapshot
                        .authors
                        .get(id)
                        .map(|a| a.name.clone())
                        .unwrap_or_else(|| format!("author-{}", id));
                    AuthorShare {
                        name,
                        pct: count as f64 / total as f64 * 100.0,
                    }
                })
                .collect();
            authors.sort_by(|a, b| b.pct.partial_cmp(&a.pct).unwrap());
            FileOwnership {
                path: path.to_string_lossy().to_string(),
                authors,
            }
        })
        .collect()
}

pub(super) fn build_file_ages(snapshot: &RepoSnapshot) -> Vec<FileAge> {
    let now = chrono::Utc::now();
    let fallback = snapshot.created_at - chrono::Duration::days(365 * 5);
    let mut ages: Vec<FileAge> = snapshot
        .files
        .iter()
        .filter(|f| !f.is_binary)
        .map(|f| {
            let last_modified = snapshot
                .commits_by_file
                .get(&f.path)
                .and_then(|commit_ids| {
                    commit_ids
                        .iter()
                        .filter_map(|cid| snapshot.commits.iter().find(|c| c.id == *cid))
                        .map(|c| c.timestamp)
                        .max()
                })
                .unwrap_or(fallback);
            let days = (now - last_modified).num_days().max(0);
            FileAge {
                path: f.path.to_string_lossy().to_string(),
                last_modified,
                days_since_modified: days,
            }
        })
        .collect();
    ages.sort_by_key(|a| std::cmp::Reverse(a.days_since_modified));
    ages
}

pub(super) fn build_author_cards(snapshot: &RepoSnapshot) -> Vec<AuthorCard> {
    let now = chrono::Utc::now();

    // Pre-compute per-author blame lines across all files
    let mut author_lines: HashMap<usize, usize> = HashMap::new();
    let mut author_file_pcts: HashMap<usize, Vec<(String, f64)>> = HashMap::new();
    let mut author_files_owned: HashMap<usize, usize> = HashMap::new();

    for (path, blame_lines) in &snapshot.blame_map {
        let total: usize = blame_lines
            .iter()
            .map(|b| b.line_count)
            .sum::<usize>()
            .max(1);
        let mut counts: HashMap<usize, usize> = HashMap::new();
        for bl in blame_lines {
            *counts.entry(bl.author_id).or_insert(0) += bl.line_count;
        }
        for (&author_id, &count) in &counts {
            *author_lines.entry(author_id).or_insert(0) += count;
            let pct = count as f64 / total as f64 * 100.0;
            author_file_pcts
                .entry(author_id)
                .or_default()
                .push((path.to_string_lossy().to_string(), pct));
            if pct > 50.0 {
                *author_files_owned.entry(author_id).or_insert(0) += 1;
            }
        }
    }

    let mut cards: Vec<AuthorCard> = snapshot
        .authors
        .iter()
        .map(|author| {
            let commit_ids = snapshot
                .commits_by_author
                .get(&author.id)
                .cloned()
                .unwrap_or_default();

            let author_commits: Vec<&crate::snapshot::Commit> = commit_ids
                .iter()
                .filter_map(|cid| snapshot.commits.iter().find(|c| c.id == *cid))
                .collect();

            let commit_count = author_commits.len();

            let last_active = author_commits
                .iter()
                .map(|c| c.timestamp)
                .max()
                .unwrap_or(snapshot.created_at);
            let days_since_active = (now - last_active).num_days().max(0);

            let avg_commit_quality = if author_commits.is_empty() {
                0.0
            } else {
                let total_q: f64 = author_commits
                    .iter()
                    .map(|c| score_commit_message(&c.message))
                    .sum();
                total_q / author_commits.len() as f64
            };

            let mut file_pcts = author_file_pcts
                .get(&author.id)
                .cloned()
                .unwrap_or_default();
            file_pcts.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let top_files: Vec<String> = file_pcts.iter().take(5).map(|(p, _)| p.clone()).collect();

            let mut dirs = std::collections::HashSet::new();
            for commit in &author_commits {
                for fc in &commit.files_changed {
                    if let Some(parent) = fc.path.parent() {
                        dirs.insert(parent.to_string_lossy().to_string());
                    }
                }
            }

            AuthorCard {
                name: author.name.clone(),
                email: author.email.clone(),
                commit_count,
                files_owned: *author_files_owned.get(&author.id).unwrap_or(&0),
                lines_owned: *author_lines.get(&author.id).unwrap_or(&0),
                avg_commit_quality,
                top_files,
                last_active,
                days_since_active,
                directories_touched: dirs.len(),
            }
        })
        .collect();

    cards.sort_by_key(|c| std::cmp::Reverse(c.commit_count));
    cards
}

pub(super) fn build_per_file_coupling(snapshot: &RepoSnapshot) -> Vec<FileCouplingMetrics> {
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

pub(super) fn build_import_edges(snapshot: &RepoSnapshot) -> Vec<ImportEdge> {
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
pub(super) fn build_import_cycles(snapshot: &RepoSnapshot) -> Vec<Vec<String>> {
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
    use super::*;
    use crate::snapshot::{Author, BlameLine, Commit, CommitId, FileEntry, TimeWindow};
    use chrono::Utc;
    use std::path::PathBuf;

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

    fn make_commit(id: u32, message: &str) -> Commit {
        Commit {
            id: CommitId(id),
            author: 0,
            timestamp: Utc::now(),
            message: message.to_string(),
            files_changed: vec![],
            is_merge: false,
            parent_count: 1,
        }
    }

    fn make_file_entry(path: &str) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            size_bytes: 100,
            is_binary: false,
            depth: 1,
            blob_oid: String::new(),
        }
    }

    fn make_commit_at(id: u32, ts: chrono::DateTime<Utc>) -> Commit {
        Commit {
            id: CommitId(id),
            author: 0,
            timestamp: ts,
            message: "chore: touch".to_string(),
            files_changed: vec![],
            is_merge: false,
            parent_count: 1,
        }
    }

    #[test]
    fn churn_timeline_buckets_commits_across_window() {
        use chrono::TimeZone;
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let t0 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let t_end = Utc.with_ymd_and_hms(2026, 12, 1, 0, 0, 0).unwrap();
        snapshot.files = vec![make_file_entry("src/lib.rs")];
        snapshot.commits = vec![make_commit_at(0, t0), make_commit_at(1, t_end)];
        snapshot
            .commits_by_file
            .insert(PathBuf::from("src/lib.rs"), vec![CommitId(0), CommitId(1)]);

        let hotspots = build_hotspots(&snapshot, &crate::config::CouplingThresholds::default());
        let timeline = &hotspots[0].churn_timeline;
        assert_eq!(timeline.len(), 12);
        assert_eq!(timeline[0], 1, "oldest commit lands in the first bucket");
        assert_eq!(timeline[11], 1, "newest commit lands in the last bucket");
        assert_eq!(timeline.iter().sum::<u32>(), 2);
    }

    #[test]
    fn churn_timeline_single_commit_window() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.files = vec![make_file_entry("src/lib.rs")];
        snapshot.commits = vec![make_commit(0, "feat: only commit")];
        snapshot
            .commits_by_file
            .insert(PathBuf::from("src/lib.rs"), vec![CommitId(0)]);

        let hotspots = build_hotspots(&snapshot, &crate::config::CouplingThresholds::default());
        let timeline = &hotspots[0].churn_timeline;
        assert_eq!(timeline.len(), 12);
        assert_eq!(
            timeline.iter().sum::<u32>(),
            1,
            "zero-span window must not panic or drop the commit"
        );
    }

    #[test]
    fn churn_timeline_is_all_zeros_for_untouched_file() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.files = vec![make_file_entry("src/lib.rs")];
        snapshot.commits = vec![make_commit(0, "feat: touches nothing tracked")];

        let hotspots = build_hotspots(&snapshot, &crate::config::CouplingThresholds::default());
        let timeline = &hotspots[0].churn_timeline;
        assert_eq!(timeline.len(), 12);
        assert!(timeline.iter().all(|&v| v == 0));
    }

    #[test]
    fn bug_commit_count_is_zero_when_no_bug_commits() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let path = PathBuf::from("src/lib.rs");
        snapshot.files = vec![make_file_entry("src/lib.rs")];
        snapshot.commits = vec![
            make_commit(0, "feat: add new endpoint"),
            make_commit(1, "refactor: extract helper"),
        ];
        snapshot
            .commits_by_file
            .insert(path, vec![CommitId(0), CommitId(1)]);

        let hotspots = build_hotspots(&snapshot, &crate::config::CouplingThresholds::default());
        assert_eq!(hotspots.len(), 1);
        assert_eq!(hotspots[0].bug_commit_count, 0);
    }

    #[test]
    fn bug_commit_count_detects_all_keywords() {
        for (keyword, label) in &[
            ("fix: broken auth", "fix"),
            ("bug in parser found", "bug"),
            ("broken after merge", "broken"),
            ("crash on startup", "crash"),
            ("regression in login", "regression"),
        ] {
            let mut snapshot = RepoSnapshot::new(
                PathBuf::from("/tmp/test"),
                "test".into(),
                "main".into(),
                TimeWindow::default(),
            );
            let path = PathBuf::from("src/lib.rs");
            snapshot.files = vec![make_file_entry("src/lib.rs")];
            snapshot.commits = vec![make_commit(0, keyword)];
            snapshot.commits_by_file.insert(path, vec![CommitId(0)]);

            let hotspots = build_hotspots(&snapshot, &crate::config::CouplingThresholds::default());
            assert_eq!(
                hotspots[0].bug_commit_count, 1,
                "keyword '{}' should be detected",
                label
            );
        }
    }

    #[test]
    fn bug_commit_count_is_case_insensitive() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        let path = PathBuf::from("src/lib.rs");
        snapshot.files = vec![make_file_entry("src/lib.rs")];
        snapshot.commits = vec![make_commit(0, "FIX: uppercase message")];
        snapshot.commits_by_file.insert(path, vec![CommitId(0)]);

        let hotspots = build_hotspots(&snapshot, &crate::config::CouplingThresholds::default());
        assert_eq!(hotspots[0].bug_commit_count, 1);
    }

    #[test]
    fn bug_commit_count_only_counts_commits_touching_that_file() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.files = vec![make_file_entry("src/a.rs"), make_file_entry("src/b.rs")];
        snapshot.commits = vec![
            make_commit(0, "fix: broken in a"), // bug commit touching a only
            make_commit(1, "feat: add to b"),   // normal commit touching b only
        ];
        snapshot
            .commits_by_file
            .insert(PathBuf::from("src/a.rs"), vec![CommitId(0)]);
        snapshot
            .commits_by_file
            .insert(PathBuf::from("src/b.rs"), vec![CommitId(1)]);

        let hotspots = build_hotspots(&snapshot, &crate::config::CouplingThresholds::default());
        let a = hotspots.iter().find(|f| f.path == "src/a.rs").unwrap();
        let b = hotspots.iter().find(|f| f.path == "src/b.rs").unwrap();
        assert_eq!(a.bug_commit_count, 1, "a.rs should have 1 bug commit");
        assert_eq!(b.bug_commit_count, 0, "b.rs should have 0 bug commits");
    }

    #[test]
    fn bug_commit_count_zero_for_file_not_in_commits_by_file() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.files = vec![make_file_entry("src/new.rs")];
        snapshot.commits = vec![make_commit(0, "fix: something")];
        // commits_by_file intentionally left empty — file not linked to any commit

        let hotspots = build_hotspots(&snapshot, &crate::config::CouplingThresholds::default());
        assert_eq!(hotspots[0].bug_commit_count, 0);
    }

    fn make_test_snapshot_with_blame(
        authors: Vec<(&str, &str)>,
        blame_entries: Vec<(&str, Vec<BlameLine>)>,
    ) -> RepoSnapshot {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp/test"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );
        snapshot.authors = authors
            .into_iter()
            .enumerate()
            .map(|(i, (name, email))| Author {
                id: i,
                name: name.to_string(),
                email: email.to_string(),
            })
            .collect();
        snapshot.blame_map = blame_entries
            .into_iter()
            .map(|(path, lines)| (PathBuf::from(path), lines))
            .collect();
        snapshot
    }

    fn blame(author_id: usize, line_count: usize) -> BlameLine {
        BlameLine {
            author_id,
            timestamp: Utc::now(),
            line_count,
        }
    }

    #[test]
    fn ownership_single_author_uncompressed() {
        let snapshot = make_test_snapshot_with_blame(
            vec![("Alice", "alice@x.com")],
            vec![("main.rs", vec![blame(0, 1), blame(0, 1), blame(0, 1)])],
        );

        let ownership = build_author_ownership(&snapshot);
        assert_eq!(ownership.len(), 1);
        assert_eq!(ownership[0].authors.len(), 1);
        assert!((ownership[0].authors[0].pct - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ownership_single_author_rle_compressed() {
        let snapshot = make_test_snapshot_with_blame(
            vec![("Alice", "alice@x.com")],
            vec![("main.rs", vec![blame(0, 50)])],
        );

        let ownership = build_author_ownership(&snapshot);
        assert_eq!(ownership[0].authors[0].pct, 100.0);
    }

    #[test]
    fn ownership_two_authors_uncompressed() {
        let snapshot = make_test_snapshot_with_blame(
            vec![("Alice", "alice@x.com"), ("Bob", "bob@x.com")],
            vec![(
                "main.rs",
                vec![blame(0, 1), blame(0, 1), blame(0, 1), blame(1, 1)],
            )],
        );

        let ownership = build_author_ownership(&snapshot);
        let file = &ownership[0];
        // Alice: 3/4 = 75%, Bob: 1/4 = 25%
        assert_eq!(file.authors[0].name, "Alice");
        assert!((file.authors[0].pct - 75.0).abs() < f64::EPSILON);
        assert_eq!(file.authors[1].name, "Bob");
        assert!((file.authors[1].pct - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ownership_two_authors_rle_gives_same_result_as_uncompressed() {
        // RLE: Alice owns 30 lines, Bob owns 10 lines → 75% / 25%
        let snapshot_rle = make_test_snapshot_with_blame(
            vec![("Alice", "alice@x.com"), ("Bob", "bob@x.com")],
            vec![("main.rs", vec![blame(0, 30), blame(1, 10)])],
        );
        // Uncompressed equivalent: same 40 lines, one entry per line
        let mut uncompressed_lines = vec![blame(0, 1); 30];
        uncompressed_lines.extend(vec![blame(1, 1); 10]);
        let snapshot_flat = make_test_snapshot_with_blame(
            vec![("Alice", "alice@x.com"), ("Bob", "bob@x.com")],
            vec![("main.rs", uncompressed_lines)],
        );

        let own_rle = build_author_ownership(&snapshot_rle);
        let own_flat = build_author_ownership(&snapshot_flat);

        for (r, f) in own_rle[0].authors.iter().zip(own_flat[0].authors.iter()) {
            assert_eq!(r.name, f.name);
            assert!((r.pct - f.pct).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn ownership_empty_blame_map_returns_empty() {
        let snapshot = make_test_snapshot_with_blame(vec![("Alice", "alice@x.com")], vec![]);
        let ownership = build_author_ownership(&snapshot);
        assert!(ownership.is_empty());
    }

    #[test]
    fn hotspot_rows_carry_per_kind_finding_counts() {
        use crate::snapshot::{CouplingFinding, CouplingKind};
        let mut snapshot = crate::metrics::testutil::make_snapshot();
        snapshot.files = vec![
            crate::metrics::testutil::make_file("src/dirty.rs"),
            crate::metrics::testutil::make_file("src/clean.rs"),
        ];
        snapshot.coupling_findings = vec![
            CouplingFinding {
                path: "src/dirty.rs".into(),
                line: Some(1),
                kind: CouplingKind::Common,
                evidence: "static mut CACHE: usize = 0;".into(),
            },
            CouplingFinding {
                path: "src/dirty.rs".into(),
                line: Some(9),
                kind: CouplingKind::Control,
                evidence: "pub fn go(fast: bool)".into(),
            },
        ];
        let cfg = crate::config::CouplingThresholds::default();
        let hotspots = build_hotspots(&snapshot, &cfg);
        let dirty = hotspots.iter().find(|h| h.path == "src/dirty.rs").unwrap();
        assert_eq!(
            (
                dirty.content_findings,
                dirty.common_findings,
                dirty.control_findings
            ),
            (0, 1, 1)
        );
        let clean = hotspots.iter().find(|h| h.path == "src/clean.rs").unwrap();
        assert_eq!(
            (
                clean.content_findings,
                clean.common_findings,
                clean.control_findings
            ),
            (0, 0, 0)
        );
    }

    #[test]
    fn hotspot_content_counts_include_barrel_findings_only_when_toggle_on() {
        // Cross-component import bypassing src/a's barrel — the same shape
        // the gate's ratchet_finding_sets tests use.
        let mut snapshot = crate::metrics::testutil::make_snapshot();
        snapshot.files = vec![
            crate::metrics::testutil::make_file("src/a/index.ts"),
            crate::metrics::testutil::make_file("src/a/impl.ts"),
            crate::metrics::testutil::make_file("src/b/user.ts"),
        ];
        snapshot
            .import_graph
            .insert("src/b/user.ts".into(), vec!["src/a/impl.ts".into()]);
        let cfg = crate::config::CouplingThresholds::default();
        assert!(cfg.content_barrel_rule, "default toggle must be on");
        let on = build_hotspots(&snapshot, &cfg);
        let user_on = on.iter().find(|h| h.path == "src/b/user.ts").unwrap();
        assert_eq!(
            user_on.content_findings, 1,
            "barrel bypass joins the importing file's content count"
        );

        let cfg_off = crate::config::CouplingThresholds {
            content_barrel_rule: false,
            ..crate::config::CouplingThresholds::default()
        };
        let off = build_hotspots(&snapshot, &cfg_off);
        let user_off = off.iter().find(|h| h.path == "src/b/user.ts").unwrap();
        assert_eq!(
            user_off.content_findings, 0,
            "toggle off must mirror pressman_finding_counts' gating"
        );
    }

    /// Two files identical in churn/CC/LOC; one carries a Common finding.
    /// Base score for both: cc_norm=1, loc_norm=1, churn=0 →
    /// (0.3 + 0.2) × 100 = 50. Flagged file: 50 × 1.25 = 62.5.
    fn twin_snapshot(kind: crate::snapshot::CouplingKind) -> crate::snapshot::RepoSnapshot {
        use crate::snapshot::{CouplingFinding, FileComplexity};
        let mut snapshot = crate::metrics::testutil::make_snapshot();
        snapshot.files = vec![
            crate::metrics::testutil::make_file("src/flagged.rs"),
            crate::metrics::testutil::make_file("src/plain.rs"),
        ];
        for p in ["src/flagged.rs", "src/plain.rs"] {
            snapshot.file_metrics.insert(
                p.into(),
                FileComplexity {
                    loc: 100,
                    cyclomatic_complexity: 10,
                    ..Default::default()
                },
            );
        }
        snapshot.coupling_findings = vec![CouplingFinding {
            path: "src/flagged.rs".into(),
            line: Some(1),
            kind,
            evidence: "evidence".into(),
        }];
        snapshot
    }

    #[test]
    fn common_finding_multiplies_hotspot_score() {
        let snapshot = twin_snapshot(crate::snapshot::CouplingKind::Common);
        let cfg = crate::config::CouplingThresholds::default();
        let hotspots = build_hotspots(&snapshot, &cfg);
        let flagged = hotspots
            .iter()
            .find(|h| h.path == "src/flagged.rs")
            .unwrap();
        let plain = hotspots.iter().find(|h| h.path == "src/plain.rs").unwrap();
        assert!((plain.hotspot_score - 50.0).abs() < 1e-9);
        assert!(
            (flagged.hotspot_score - 62.5).abs() < 1e-9,
            "50 × default 1.25 = 62.5, got {}",
            flagged.hotspot_score
        );
    }

    #[test]
    fn control_finding_does_not_multiply_hotspot_score() {
        let snapshot = twin_snapshot(crate::snapshot::CouplingKind::Control);
        let cfg = crate::config::CouplingThresholds::default();
        let hotspots = build_hotspots(&snapshot, &cfg);
        let flagged = hotspots
            .iter()
            .find(|h| h.path == "src/flagged.rs")
            .unwrap();
        assert!(
            (flagged.hotspot_score - 50.0).abs() < 1e-9,
            "control is the least severe rung — no multiplier (spec)"
        );
    }

    #[test]
    fn multiplied_hotspot_score_caps_at_100() {
        // Base score here is 50 (cc_norm=1, loc_norm=1, churn=0); a large
        // multiplier would push it to 500 — the cap must clamp to 100
        // because every consumer assumes the 0–100 domain.
        let snapshot = twin_snapshot(crate::snapshot::CouplingKind::Common);
        let cfg = crate::config::CouplingThresholds {
            hotspot_multiplier: 10.0,
            ..crate::config::CouplingThresholds::default()
        };
        let hotspots = build_hotspots(&snapshot, &cfg);
        let flagged = hotspots
            .iter()
            .find(|h| h.path == "src/flagged.rs")
            .unwrap();
        assert!(
            (flagged.hotspot_score - 100.0).abs() < 1e-9,
            "consumers assume 0–100; got {}",
            flagged.hotspot_score
        );
    }
}
