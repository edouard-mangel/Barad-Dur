mod community;
mod inheritance;
mod test_safety_net;
pub(crate) use inheritance::inheritance_findings;
pub(crate) use test_safety_net::test_safety_net;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::config::CouplingThresholds;
use crate::metrics::file_role::{classify, FileRole};
use crate::metrics::{median, score_prevalence, CategoryResult, MetricValue, RawValue};
use crate::scorer::CouplingFindingCounts;
use crate::snapshot::{CouplingFinding, CouplingKind, RepoSnapshot};

/// path → (first-half partners, second-half partners) for files whose
/// co-change reach grew — computed once per run by the caller (the
/// `flagged_god_objects` precedent) and shared between the Coupling
/// category and the hotspot rows.
pub type CouplingReach = std::collections::BTreeMap<PathBuf, (usize, usize)>;

pub fn compute_coupling(
    snapshot: &RepoSnapshot,
    thresholds: &CouplingThresholds,
    reach: &CouplingReach,
) -> CategoryResult {
    let barrel = gated_barrel_findings(snapshot, thresholds);
    let inh = inheritance_findings(snapshot, thresholds.inheritance_min_depth);
    let corr = corroboration_degree(snapshot, thresholds);
    let weight = thresholds.corroboration_weight;
    let metrics = vec![
        afferent_coupling(snapshot),
        efferent_coupling(snapshot),
        circular_dependencies(snapshot),
        change_coupling_smells(snapshot, thresholds),
        pressman_metric(snapshot, CouplingKind::Content, barrel, &corr, weight),
        pressman_metric(snapshot, CouplingKind::Common, Vec::new(), &corr, weight),
        pressman_metric(snapshot, CouplingKind::Inheritance, inh, &corr, weight),
        pressman_metric(snapshot, CouplingKind::Control, Vec::new(), &corr, weight),
        coupling_reach_trend(reach),
        test_safety_net(snapshot, thresholds),
    ];
    apply_severity_cap(
        CategoryResult {
            name: "Coupling".to_string(),
            score: 0,
            metrics,
        }
        .compute_score(),
    )
}

/// Extract the first `depth` path components joined by "/".
/// Falls back gracefully if path has fewer components than `depth`.
pub(crate) fn extract_component(path: &Path, depth: usize) -> String {
    path.components()
        .take(depth)
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Unscored evidence row for the growing-reach decay signal (Ch. 8,
/// trends M3) — never folded into the category score (the
/// M5-corroboration precedent), and independent of the import graph, so
/// it survives snapshots with no parsed imports. Lists the widest-reach
/// offenders so the aggregate count is actionable.
fn coupling_reach_trend(reach: &CouplingReach) -> MetricValue {
    const TOP_N: usize = 3;
    let description = if reach.is_empty() {
        "No files with growing co-change reach".to_string()
    } else {
        let mut worst: Vec<(&PathBuf, &(usize, usize))> = reach.iter().collect();
        worst.sort_by_key(|(path, (_, second))| (std::cmp::Reverse(*second), (*path).clone()));
        let top = worst
            .iter()
            .take(TOP_N)
            .map(|(path, (first, second))| format!("{} ({first} → {second})", path.display()))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{} file(s) whose co-change partner count at least doubled half-over-half — {top}",
            reach.len()
        )
    };
    MetricValue {
        name: "Co-change reach trend".to_string(),
        description,
        raw_value: RawValue::Count(reach.len()),
        score: None,
    }
}

/// Growth factor a file's distinct co-change partner count must reach,
/// half-over-half, to be flagged as growing reach (Ch. 8 decay, trends M3).
const DECAY_GROWTH_FACTOR: f64 = 2.0;

/// Commits touching more than this many known files are bulk operations
/// (sprints, reformat sweeps) and form no partner pairs — code-maat's
/// `--max-changeset-size` practice (its default is 30). Without it, one
/// wide commit flags a third of the tree as "growing reach" (dogfood).
const MAX_CHANGESET_SIZE: usize = 30;

/// Files whose distinct co-change partner count at least doubled from the
/// first to the second half of the window AND reaches `min_partners`
/// (`thresholds.coupling.decay_min_partners`) in the second half —
/// Tornhill's "architectural decay in progress" (Ch. 8, trends M3).
/// Returns path → (first-half partners, second-half partners),
/// deterministic. Merge commits are excluded and the split is computed at
/// metric time — `snapshot.file_change_pairs` (whole-window, exact-commit)
/// is untouched. Empty when either half forms no pairs: a one-burst
/// history has no halves to compare, and claiming decay against a
/// pair-less baseline would fabricate signal. Files with zero first-half
/// partners are never flagged — new reach is not decay.
///
/// The halves split on the wall-clock midpoint of observed activity, not
/// commit volume, so a lopsided history (one early commit, then a sprint)
/// yields a thin baseline half — a calibration candidate (median-commit
/// split) if that proves noisy in practice.
///
/// Calibration note (dogfood 2026-08-20): on a window whose second half is
/// an active feature sprint, many files legitimately grow reach at once
/// (57 flags on this repo even with the changeset cap). The annotation is
/// factual about window shape and carries no score weight; a
/// median-relative outlier gate (the `is_structural_hub` pattern) is the
/// candidate calibration if per-file decay alarms are wanted later.
pub fn growing_coupling_reach(snapshot: &RepoSnapshot, min_partners: usize) -> CouplingReach {
    let known = snapshot.known_paths();
    let commits: Vec<&crate::snapshot::Commit> = snapshot
        .commits
        .iter()
        .filter(|c| snapshot.time_window.contains(&c.timestamp) && !c.is_merge)
        .collect();
    let (Some(min_ts), Some(max_ts)) = (
        commits.iter().map(|c| c.timestamp).min(),
        commits.iter().map(|c| c.timestamp).max(),
    ) else {
        return std::collections::BTreeMap::new();
    };
    let midpoint = min_ts + (max_ts - min_ts) / 2;
    let mut partners: [HashMap<&PathBuf, HashSet<&PathBuf>>; 2] = [HashMap::new(), HashMap::new()];
    for c in &commits {
        let half = usize::from(c.timestamp >= midpoint);
        let files: Vec<&PathBuf> = c
            .files_changed
            .iter()
            .filter_map(|fc| known.get(&fc.path).copied())
            .collect();
        if files.len() > MAX_CHANGESET_SIZE {
            continue;
        }
        for i in 0..files.len() {
            for j in i + 1..files.len() {
                partners[half].entry(files[i]).or_default().insert(files[j]);
                partners[half].entry(files[j]).or_default().insert(files[i]);
            }
        }
    }
    // A half without pair-forming activity is no baseline: raw commit
    // counts don't qualify (a half of only bulk sweeps or excluded-path
    // commits would fabricate a tree-wide 0 → N signal).
    if partners[0].is_empty() || partners[1].is_empty() {
        return std::collections::BTreeMap::new();
    }
    partners[1]
        .iter()
        .filter_map(|(path, second_set)| {
            let first = partners[0].get(*path).map_or(0, |set| set.len());
            let second = second_set.len();
            // `first >= 1`: a file with no first-half partners has no
            // baseline to have decayed from — new reach is not decay.
            // With that, `second >= first * 2` alone implies growth.
            (first >= 1
                && second >= min_partners
                && second as f64 >= first as f64 * DECAY_GROWTH_FACTOR)
                .then(|| ((*path).clone(), (first, second)))
        })
        .collect()
}

/// Afferent coupling (Ca): how many files depend on each file (incoming imports).
///
/// Scored on the median Ca rather than the max. A single hub (core data model)
/// is normal — what matters is whether *most* files have excessive incoming deps.
fn afferent_coupling(snapshot: &RepoSnapshot) -> MetricValue {
    let incoming = crate::metrics::incoming_import_counts(&snapshot.import_graph);

    if incoming.is_empty() {
        return MetricValue {
            name: "Afferent coupling".to_string(),
            description: "No import dependencies detected".to_string(),
            raw_value: RawValue::Count(0),
            score: None,
        };
    }

    // Include all files in the distribution (files with zero incoming deps too),
    // so a single hub doesn't skew the median.
    let ca_values: Vec<usize> = snapshot
        .files
        .iter()
        .map(|f| incoming.get(f.path.as_path()).copied().unwrap_or(0))
        .collect();

    let max_ca = ca_values.iter().copied().max().unwrap_or(0);
    let mean_ca = ca_values.iter().sum::<usize>() as f64 / ca_values.len() as f64;
    let median_ca = median(&ca_values);

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
            "Incoming deps — median: {median_ca:.1}, mean: {mean_ca:.1}, max: {max_ca}"
        ),
        raw_value: RawValue::Float(median_ca),
        score: Some(score),
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
            score: None,
        };
    }

    // Include all files in the distribution (files with zero outgoing imports too).
    let ce_values: Vec<usize> = snapshot
        .files
        .iter()
        .map(|f| crate::metrics::outgoing_degree(&snapshot.import_graph, &f.path))
        .collect();

    let max_ce = ce_values.iter().copied().max().unwrap_or(0);
    let mean_ce = ce_values.iter().sum::<usize>() as f64 / ce_values.len() as f64;
    let median_ce = median(&ce_values);

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
            "Outgoing deps — median: {median_ce:.1}, mean: {mean_ce:.1}, max: {max_ce}"
        ),
        raw_value: RawValue::Float(median_ce),
        score: Some(score),
    }
}

/// Cross-boundary co-change pairs that qualify as change-coupling smells:
/// different components, both files have commit history, and their
/// co-change ratio meets the configured threshold. Single source of truth
/// for "a meaningful co-change" — consumed by both the smell metric and
/// corroboration (M5).
fn qualifying_smell_pairs<'a>(
    snapshot: &'a RepoSnapshot,
    thresholds: &'a CouplingThresholds,
) -> impl Iterator<Item = (&'a PathBuf, &'a PathBuf)> + 'a {
    snapshot
        .file_change_pairs
        .iter()
        .filter_map(move |(path_a, path_b, co_changes)| {
            let comp_a = extract_component(path_a, thresholds.component_depth);
            let comp_b = extract_component(path_b, thresholds.component_depth);
            if comp_a == comp_b {
                return None;
            }
            let commits_a = snapshot.commits_by_file.get(path_a).map_or(0, |v| v.len());
            let commits_b = snapshot.commits_by_file.get(path_b).map_or(0, |v| v.len());
            crate::metrics::meets_coupling_ratio(
                *co_changes,
                commits_a,
                commits_b,
                thresholds.change_coupling_min_ratio,
            )
            .then_some((path_a, path_b))
        })
}

/// Files that participate in a qualifying cross-boundary co-change, mapped
/// to their count of *distinct* qualifying partners. Presence of a finding's
/// path in this map is what marks the finding "corroborated" (M5).
pub(crate) fn corroboration_degree(
    snapshot: &RepoSnapshot,
    thresholds: &CouplingThresholds,
) -> HashMap<PathBuf, usize> {
    let mut partners: HashMap<PathBuf, HashSet<PathBuf>> = HashMap::new();
    for (a, b) in qualifying_smell_pairs(snapshot, thresholds) {
        partners.entry(a.clone()).or_default().insert(b.clone());
        partners.entry(b.clone()).or_default().insert(a.clone());
    }
    partners.into_iter().map(|(k, v)| (k, v.len())).collect()
}

/// Change coupling smells: cross-boundary file pairs that co-change at or above
/// the configured ratio threshold.
///
/// Scored on smell count: 0 → 100, 1–2 → 75, 3–5 → 50, >5 → 25
fn change_coupling_smells(snapshot: &RepoSnapshot, thresholds: &CouplingThresholds) -> MetricValue {
    let smell_pairs: Vec<(&PathBuf, &PathBuf)> =
        qualifying_smell_pairs(snapshot, thresholds).collect();
    let smell_count = smell_pairs.len();

    let communities = community::detect_communities(&snapshot.import_graph);
    let corroborated: Vec<_> = smell_pairs
        .iter()
        .filter(|(a, b)| match (communities.get(*a), communities.get(*b)) {
            (Some(ca), Some(cb)) => ca != cb,
            _ => false,
        })
        .collect();
    let scored_pairs: Vec<_> = if thresholds.community_corroboration {
        corroborated.clone()
    } else {
        smell_pairs.iter().collect()
    };
    let affected: HashSet<&PathBuf> = scored_pairs.iter().flat_map(|(a, b)| [*a, *b]).collect();
    let source_total = source_population(snapshot).len();
    let score = score_prevalence(affected.len(), source_total);

    let community_note = if thresholds.community_corroboration && smell_count > 0 {
        format!(", {} also cross-community", corroborated.len())
    } else {
        String::new()
    };

    let basis = if smell_count == 0 {
        String::new()
    } else if thresholds.community_corroboration {
        format!(
            "; score based on {}/{} source files with cross-community corroboration",
            affected.len(),
            source_total
        )
    } else {
        format!(
            "; score based on {}/{} affected source files",
            affected.len(),
            source_total
        )
    };
    MetricValue {
        name: "Change coupling smells".to_string(),
        description: format!(
            "{} cross-boundary co-change pair(s) above {:.0}% ratio threshold{}{}",
            smell_count,
            thresholds.change_coupling_min_ratio * 100.0,
            community_note,
            basis,
        ),
        raw_value: RawValue::Count(smell_count),
        score: Some(score),
    }
}

/// Production-source population used to turn absolute coupling findings into
/// repository-size-aware prevalence scores. Fall back through the available
/// snapshot collections so focused fixtures remain representative.
fn source_population(snapshot: &RepoSnapshot) -> HashSet<&PathBuf> {
    let mut paths: HashSet<&PathBuf> = snapshot
        .file_metrics
        .keys()
        .filter(|path| classify(path) == FileRole::Source)
        .collect();
    paths.extend(
        snapshot
            .files
            .iter()
            .map(|file| &file.path)
            .filter(|path| classify(path) == FileRole::Source),
    );
    for (source, targets) in &snapshot.import_graph {
        if classify(source) == FileRole::Source {
            paths.insert(source);
        }
        paths.extend(
            targets
                .iter()
                .filter(|path| classify(path) == FileRole::Source),
        );
    }
    for (a, b, _) in &snapshot.file_change_pairs {
        if classify(a) == FileRole::Source {
            paths.insert(a);
        }
        if classify(b) == FileRole::Source {
            paths.insert(b);
        }
    }
    paths
}

/// Whether the import graph has an edge `from` → `to`.
fn has_edge(graph: &HashMap<PathBuf, Vec<PathBuf>>, from: &PathBuf, to: &PathBuf) -> bool {
    graph.get(from).is_some_and(|targets| targets.contains(to))
}

/// Dedup key for a direct A↔B cycle: the pair in lexicographic order.
fn pair_key(a: &Path, b: &Path) -> (PathBuf, PathBuf) {
    let mut pair = [a.to_path_buf(), b.to_path_buf()];
    pair.sort();
    let [first, second] = pair;
    (first, second)
}

/// Dedup key for an A→B→C→A cycle: its two smallest members, so the same
/// loop found from any entry point collapses to one entry.
fn trio_key(a: &Path, b: &Path, c: &Path) -> (PathBuf, PathBuf) {
    let mut trio = [a.to_path_buf(), b.to_path_buf(), c.to_path_buf()];
    trio.sort();
    let [first, second, _] = trio;
    (first, second)
}

/// Circular dependencies: file pairs where A→B and B→A (depth 1) or
/// A→B→C→A (depth 2).
fn circular_dependencies(snapshot: &RepoSnapshot) -> MetricValue {
    let graph = &snapshot.import_graph;
    let edges = graph
        .iter()
        .flat_map(|(a, targets)| targets.iter().map(move |b| (a, b)))
        .filter(|(a, b)| a != b);

    let cycles: HashSet<(PathBuf, PathBuf)> = edges
        .flat_map(|(a, b)| {
            let direct = has_edge(graph, b, a).then(|| pair_key(a, b));
            let depth_two = graph
                .get(b)
                .into_iter()
                .flatten()
                .filter(move |c| *c != a && *c != b && has_edge(graph, c, a))
                .map(move |c| trio_key(a, b, c));
            direct.into_iter().chain(depth_two)
        })
        .collect();

    let count = cycles.len();
    let affected: HashSet<&PathBuf> = cycles.iter().flat_map(|(a, b)| [a, b]).collect();
    let source_total = source_population(snapshot).len();
    let score = score_prevalence(affected.len(), source_total);

    let cycle_list: Vec<String> = cycles
        .iter()
        .take(10)
        .map(|(a, b)| format!("{} <-> {}", a.display(), b.display()))
        .collect();

    MetricValue {
        name: "Circular dependencies".to_string(),
        description: format!(
            "{count} circular dependency pair(s), affecting {}/{} source files",
            affected.len(),
            source_total
        ),
        raw_value: RawValue::List(cycle_list),
        score: Some(score),
    }
}

const BARREL_NAMES: &[&str] = &["index.ts", "index.tsx", "index.js", "index.jsx"];
const JS_EXTS: &[&str] = &["ts", "tsx", "js", "jsx", "mjs", "cjs"];
const DETECTABLE_EXTS: &[&str] = &["rs", "ts", "tsx", "js", "jsx", "mjs", "cjs"];

/// True when the collector's AST pass actually ran on this snapshot.
/// `collect_snapshot_at` (ADR-005, backfill) skips it, leaving
/// `file_metrics` empty — an empty findings list there means "not
/// collected", never "clean".
pub(crate) fn detection_ran(snapshot: &RepoSnapshot) -> bool {
    !snapshot.file_metrics.is_empty()
}

/// Single source of truth for per-kind finding counts. Must equal what the
/// three Pressman metrics report (Content includes barrel-bypass findings
/// when the rule is enabled). `None` when detection did not run or no
/// detectable-language files exist.
pub(crate) fn pressman_finding_counts(
    snapshot: &RepoSnapshot,
    thresholds: &CouplingThresholds,
) -> Option<CouplingFindingCounts> {
    if !detection_ran(snapshot) || !has_detectable_files(snapshot) {
        return None;
    }
    let findings = all_coupling_findings(snapshot, thresholds);
    let count_kind = |kind: CouplingKind| findings.iter().filter(|f| f.kind == kind).count();
    Some(CouplingFindingCounts {
        content: count_kind(CouplingKind::Content),
        common: count_kind(CouplingKind::Common),
        inheritance: count_kind(CouplingKind::Inheritance),
        control: count_kind(CouplingKind::Control),
    })
}

fn has_detectable_files(snapshot: &RepoSnapshot) -> bool {
    snapshot.files.iter().any(|f| {
        f.path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| DETECTABLE_EXTS.contains(&e))
    })
}

/// Severity-banded score for a Pressman finding count.
///
/// MAINTAINER-AUTHORED BANDS. Invariants the rest of the system relies on:
/// - count 0 → 100 for every kind
/// - Content: any count ≥ 1 must score ≤ 50 (triggers the category cap)
/// - Common: large counts must reach ≤ 25 (triggers the category cap)
/// - Control is the mildest rung: keep bands lenient
pub(crate) const fn score_pressman(kind: CouplingKind, count: usize) -> u32 {
    match kind {
        CouplingKind::Content => match count {
            0 => 100,
            1 => 50,
            2..=3 => 35,
            _ => 25,
        },
        CouplingKind::Common => match count {
            // Maintainer decision (2026-07-02): harsher than the draft —
            // one mutable global already stings, four trigger the category cap.
            0 => 100,
            1 => 60,
            2..=3 => 40,
            _ => 25,
        },
        CouplingKind::Inheritance => match count {
            // Maintainer decision (2026-07-15): between Common's harshness and
            // Control's leniency; the floor never triggers the ≤25 category cap.
            0 => 100,
            1..=2 => 70,
            3..=6 => 55,
            _ => 40,
        },
        CouplingKind::Control => match count {
            0 => 100,
            1..=5 => 85,
            6..=15 => 70,
            _ => 50,
        },
    }
}

fn pressman_metric(
    snapshot: &RepoSnapshot,
    kind: CouplingKind,
    extra: Vec<CouplingFinding>,
    corr: &HashMap<PathBuf, usize>,
    weight: f64,
) -> MetricValue {
    let (name, rung) = match kind {
        CouplingKind::Content => (
            "Content coupling",
            "worst rung: another module's internals reached",
        ),
        CouplingKind::Common => ("Common coupling", "shared mutable global state"),
        CouplingKind::Inheritance => ("Inheritance coupling", "deep class inheritance chains"),
        CouplingKind::Control => ("Control coupling", "flag parameters steering callee logic"),
    };
    if !detection_ran(snapshot) {
        return MetricValue {
            name: name.to_string(),
            description: "Coupling detection did not run (no parsed files)".to_string(),
            raw_value: RawValue::Count(0),
            score: None,
        };
    }
    if !has_detectable_files(snapshot) {
        return MetricValue {
            name: name.to_string(),
            description: "No files in detectable languages (Rust, TS/JS)".to_string(),
            raw_value: RawValue::Count(0),
            score: None,
        };
    }
    let findings: Vec<CouplingFinding> = snapshot
        .coupling_findings
        .iter()
        .filter(|f| f.kind == kind && classify(&f.path) == FileRole::Source)
        .cloned()
        .chain(
            extra
                .into_iter()
                .filter(|f| classify(&f.path) == FileRole::Source),
        )
        .collect();
    let count = findings.len();

    // Weighted effective count: corroborated findings (their file co-changes
    // cross-boundary) weigh `weight`, dormant ones weigh 1. Only the *scored*
    // count is weighted — the displayed count stays truthful. weight == 1.0
    // reproduces pre-M5 scores exactly.
    let corroborated_count = findings
        .iter()
        .filter(|f| corr.contains_key(&f.path))
        .count();
    let dormant_count = count - corroborated_count;
    let effective = (dormant_count as f64 + corroborated_count as f64 * weight).round() as usize;

    let list: Vec<String> = findings
        .iter()
        .take(10)
        .map(|f| {
            let base = match f.line {
                Some(l) => format!("{}:{} — {}", f.path.display(), l, f.evidence),
                None => format!("{} — {}", f.path.display(), f.evidence),
            };
            match corr.get(&f.path) {
                Some(n) => format!("{base} — corroborated (co-changes with {n} file(s))"),
                None => base,
            }
        })
        .collect();

    let corr_note = if corroborated_count > 0 {
        format!(" ({corroborated_count} corroborated by change history)")
    } else {
        String::new()
    };

    MetricValue {
        name: name.to_string(),
        description: format!("{count} finding(s){corr_note} — {rung}"),
        raw_value: RawValue::List(list),
        score: Some(score_pressman(kind, effective)),
    }
}

/// The config-gated barrel-bypass content findings: `barrel_bypass_findings`
/// when `content_barrel_rule` is on, empty otherwise. Single definition of
/// the barrel-gating that four call sites previously duplicated.
pub(crate) fn gated_barrel_findings(
    snapshot: &RepoSnapshot,
    thresholds: &CouplingThresholds,
) -> Vec<CouplingFinding> {
    if thresholds.content_barrel_rule {
        barrel_bypass_findings(snapshot, thresholds.component_depth)
    } else {
        Vec::new()
    }
}

/// Every coupling finding for a snapshot: the AST findings in
/// `snapshot.coupling_findings` plus the gated barrel-bypass content findings
/// plus the metric-time inheritance-depth findings. The complete set the
/// metric, counts, hotspots, gate ratchet, and M6 actions all consume, so
/// none can disagree about what was found.
pub(crate) fn all_coupling_findings(
    snapshot: &RepoSnapshot,
    thresholds: &CouplingThresholds,
) -> Vec<CouplingFinding> {
    snapshot
        .coupling_findings
        .iter()
        .filter(|finding| classify(&finding.path) == FileRole::Source)
        .cloned()
        .chain(gated_barrel_findings(snapshot, thresholds))
        .chain(inheritance_findings(
            snapshot,
            thresholds.inheritance_min_depth,
        ))
        .collect()
}

/// Content coupling via barrel bypass: a cross-component relative import
/// that resolves to a non-index file in a directory that has a barrel.
/// Line info is unavailable (graph-derived), so `line: None`.
pub(crate) fn barrel_bypass_findings(
    snapshot: &RepoSnapshot,
    component_depth: usize,
) -> Vec<CouplingFinding> {
    let barrel_dirs: HashSet<PathBuf> = snapshot
        .files
        .iter()
        .filter(|f| {
            f.path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| BARREL_NAMES.contains(&n))
        })
        .filter_map(|f| f.path.parent().map(|p| p.to_owned()))
        .collect();

    let mut findings: Vec<CouplingFinding> = snapshot
        .import_graph
        .iter()
        .flat_map(|(source, targets)| {
            let barrel_dirs = &barrel_dirs;
            targets.iter().filter_map(move |target| {
                let is_js = target
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| JS_EXTS.contains(&e));
                let is_barrel_file = target
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| BARREL_NAMES.contains(&n));
                let target_dir = target.parent()?;
                let bypass = is_js
                    && !is_barrel_file
                    && barrel_dirs.contains(target_dir)
                    && source.parent() != Some(target_dir)
                    && extract_component(source, component_depth)
                        != extract_component(target, component_depth);
                bypass.then(|| CouplingFinding {
                    path: source.clone(),
                    line: None,
                    kind: CouplingKind::Content,
                    evidence: format!(
                        "imports {} directly — barrel {}/index.* exists",
                        target.display(),
                        target_dir.display()
                    ),
                })
            })
        })
        .collect();
    // `snapshot.import_graph` is a HashMap, so iteration order (and thus
    // finding order) is otherwise unspecified across runs. Sort so the
    // Content metric's top-10 evidence list — and M3's gate-delta output,
    // which diffs this list run-to-run — is deterministic.
    findings.sort_by(|a, b| (&a.path, &a.evidence).cmp(&(&b.path, &b.evidence)));
    findings
}

/// Category score ceiling applied when a critical/major Pressman finding is
/// present. Derived from `SCORE_GOOD_MIN` (scorer/types.rs — the single
/// source of truth for score-band thresholds; the CLAUDE.md project rule is
/// that band thresholds are never re-hardcoded) rather than a bare literal,
/// so the cap tracks the "good" band boundary if it ever moves.
const SEVERITY_CAP: u32 = crate::scorer::SCORE_GOOD_MIN - 1;

/// Cap triggers derive from the band table itself: Content triggers on any
/// finding (its count-1 band), Common on its 4+ band. Re-tuning
/// `score_pressman` moves the cap automatically — no second edit to forget.
const CONTENT_CAP_TRIGGER: u32 = score_pressman(CouplingKind::Content, 1);
const COMMON_CAP_TRIGGER: u32 = score_pressman(CouplingKind::Common, 4);

/// Pressman's scale is ordinal by severity: the worst rung present bounds
/// how healthy the category can be called. A flat average would hide one
/// catastrophic finding behind six healthy metrics (see spec, resolved
/// question 5).
fn apply_severity_cap(mut cat: CategoryResult) -> CategoryResult {
    let triggers: Vec<String> = cat
        .metrics
        .iter()
        .filter(|m| {
            let limit = match m.name.as_str() {
                "Content coupling" => CONTENT_CAP_TRIGGER,
                "Common coupling" => COMMON_CAP_TRIGGER,
                _ => return false,
            };
            m.score.is_some_and(|s| s <= limit)
        })
        .map(|m| m.name.clone())
        .collect();
    if cat.score > SEVERITY_CAP && !triggers.is_empty() {
        cat.score = SEVERITY_CAP;
        for m in cat
            .metrics
            .iter_mut()
            .filter(|m| triggers.contains(&m.name))
        {
            m.description.push_str(&format!(
                " — category score capped at {SEVERITY_CAP} (severity cap)"
            ));
        }
    }
    cat
}

#[cfg(test)]
mod tests;
