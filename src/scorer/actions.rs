use std::collections::HashMap;
use std::path::Path;

use crate::metrics::CategoryResult;

use super::types::ActionItem;

pub fn compute_overall_score_with_weights(
    categories: &[CategoryResult],
    weights: &[(&str, f64)],
) -> u32 {
    if categories.is_empty() {
        return 0;
    }

    let mut weighted_sum = 0.0;
    let mut total_weight = 0.0;

    for cat in categories {
        let weight = weights
            .iter()
            .find(|(name, _)| *name == cat.name)
            .map(|(_, w)| *w)
            .unwrap_or(0.25);

        weighted_sum += cat.score as f64 * weight;
        total_weight += weight;
    }

    if total_weight > 0.0 {
        (weighted_sum / total_weight).round() as u32
    } else {
        0
    }
}

pub(super) fn generate_top_actions(categories: &[CategoryResult]) -> Vec<ActionItem> {
    let mut low_metrics: Vec<(&str, &str, u32)> = Vec::new();

    for cat in categories {
        for metric in &cat.metrics {
            // Unscored metrics (insufficient data) cannot drive suggestions.
            if let Some(score) = metric.score {
                low_metrics.push((&cat.name, &metric.name, score));
            }
        }
    }

    low_metrics.sort_by_key(|m| m.2);

    low_metrics
        .iter()
        .take(3)
        .filter(|m| m.2 < 80)
        .map(|(cat, metric, score)| {
            let (target_tab, sort_by) = target_tab_for_metric(metric);
            ActionItem {
                text: format!(
                    "[{}] {} (score: {}) — {}",
                    cat,
                    metric,
                    score,
                    suggest_action(metric)
                ),
                target_tab: target_tab.map(String::from),
                sort_by: sort_by.map(String::from),
            }
        })
        .collect()
}

const CONTENT_ADVICE: &str =
    "Reaches into another module's internals — import through the module's public interface instead.";
const COMMON_ADVICE: &str =
    "Shared mutable global state — replace it with explicitly passed or injected state.";
const CONTROL_ADVICE: &str =
    "A flag parameter steers this function's control flow — split it into two intent-revealing functions.";
const INHERITANCE_ADVICE: &str =
    "Deep inheritance chain — favor composition over inheritance, or flatten the hierarchy.";

/// Per-file coupling refactoring suggestions, ranked worst-rung-first
/// (Content≻Common≻Inheritance≻Control), corroborated-before-dormant within a rung, then
/// higher finding-count first, capped at 10. A file's action speaks to its
/// most severe rung. Empty when detection did not run.
pub(super) fn generate_coupling_actions(
    snapshot: &crate::snapshot::RepoSnapshot,
    thresholds: &crate::config::CouplingThresholds,
) -> Vec<ActionItem> {
    use crate::metrics::coupling::{all_coupling_findings, corroboration_degree, detection_ran};
    use crate::snapshot::CouplingKind;

    if !detection_ran(snapshot) {
        return Vec::new();
    }
    let findings = all_coupling_findings(snapshot, thresholds);
    if findings.is_empty() {
        return Vec::new();
    }
    let corr = corroboration_degree(snapshot, thresholds);

    // severity index: lower = worse. Group by file, tracking worst rung + count.
    let mut by_file: HashMap<&Path, (u8, usize)> = HashMap::new();
    for f in &findings {
        let sev = match f.kind {
            CouplingKind::Content => 0u8,
            CouplingKind::Common => 1,
            CouplingKind::Inheritance => 2,
            CouplingKind::Control => 3,
        };
        let entry = by_file.entry(f.path.as_path()).or_insert((sev, 0));
        entry.0 = entry.0.min(sev);
        entry.1 += 1;
    }

    let mut rows: Vec<(&Path, u8, bool, usize)> = by_file
        .into_iter()
        .map(|(path, (sev, count))| (path, sev, corr.contains_key(path), count))
        .collect();
    // worst rung asc → corroborated first → count desc → path asc.
    rows.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then(b.2.cmp(&a.2))
            .then(b.3.cmp(&a.3))
            .then(a.0.cmp(b.0))
    });

    rows.into_iter()
        .take(10)
        .map(|(path, sev, corroborated, count)| {
            let (kind_label, advice) = match sev {
                0 => ("content", CONTENT_ADVICE),
                1 => ("common", COMMON_ADVICE),
                2 => ("inheritance", INHERITANCE_ADVICE),
                _ => ("control", CONTROL_ADVICE),
            };
            let corr_note = if corroborated {
                ", corroborated by change history"
            } else {
                ""
            };
            ActionItem {
                text: format!(
                    "[Coupling] {} — {} finding(s) (worst: {}){} — {}",
                    path.display(),
                    count,
                    kind_label,
                    corr_note,
                    advice
                ),
                target_tab: Some("coupling".to_string()),
                sort_by: None,
            }
        })
        .collect()
}

// No entry may be a prefix of another — `.find()` returns the first match,
// so overlapping entries would make grouping order-dependent.
const GROUPING_PREFIXES: &[&str] = &[
    "get_",
    "set_",
    "handle_",
    "validate_",
    "build_",
    "compute_",
    "parse_",
    "render_",
    "is_",
    "has_",
];

/// True if `name` starts with `prefix_with_underscore`'s verb at a real word
/// boundary — either the snake_case `_` itself (`get_user`) or a camelCase
/// capital letter (`getUserData`), so both conventions cluster identically
/// across this collector's 8 supported languages. Lookalikes with no real
/// boundary ("getter", "geta") never match.
fn matches_group_prefix(name: &str, prefix_with_underscore: &str) -> bool {
    let verb = &prefix_with_underscore[..prefix_with_underscore.len() - 1];
    // Case-insensitive on the verb itself — Go/C# capitalize exported
    // methods (`GetUserData`, not `getUserData`), so the verb has to match
    // regardless of case; only the boundary check below cares about case.
    if name.len() < verb.len() || !name[..verb.len()].eq_ignore_ascii_case(verb) {
        return false;
    }
    match name.as_bytes().get(verb.len()) {
        Some(b'_') => true,
        Some(b) => b.is_ascii_uppercase(),
        None => false,
    }
}

/// Group a file's function names by a known verb prefix — a cheap split-
/// boundary suggestion for a god-object file (Appendix 1). Only groups with
/// ≥2 members are returned; a lone `handle_x` isn't a split boundary.
fn group_methods_by_prefix(
    functions: &[crate::snapshot::FunctionMetrics],
) -> Vec<(&'static str, Vec<&str>)> {
    let groups: HashMap<&'static str, Vec<&str>> =
        functions.iter().fold(HashMap::new(), |mut groups, f| {
            if let Some(prefix) = GROUPING_PREFIXES
                .iter()
                .find(|p| matches_group_prefix(&f.name, p))
            {
                groups.entry(prefix).or_default().push(f.name.as_str());
            }
            groups
        });

    let mut result: Vec<(&'static str, Vec<&str>)> = groups
        .into_iter()
        .filter(|(_, names)| names.len() >= 2)
        .map(|(prefix, mut names)| {
            names.sort();
            (prefix, names)
        })
        .collect();
    result.sort_by_key(|(prefix, _)| *prefix);
    result
}

/// A file's clustering groups: (shared verb prefix, sorted matching function names).
type MethodGroups<'a> = Vec<(&'static str, Vec<&'a str>)>;
/// A god-object file paired with its reason and its clustering groups —
/// the raw material `generate_refactoring_actions` ranks and formats.
type RefactorCandidate<'a> = (std::path::PathBuf, String, MethodGroups<'a>);

/// Per-file method-grouping refactor suggestions for god-object files
/// (Appendix 1) — groups function names by shared verb prefix to hint at a
/// split boundary that already exists in the code, and folds in the
/// god-object's own reason (LOC, hub, name-smell) so the action reads as one
/// coherent finding instead of a bare grouping with no explanation. Advisory
/// only: files with no qualifying group get no action. Ranked by total
/// clustering-method count descending (more clustered methods means a
/// clearer, larger split), path ascending as the tiebreak for determinism,
/// then capped at 5 — this is an advisory list layered onto `top_actions`,
/// not its own report section.
pub(super) fn generate_refactoring_actions(
    snapshot: &crate::snapshot::RepoSnapshot,
    flagged_god_objects: &[(std::path::PathBuf, String)],
) -> Vec<ActionItem> {
    // Decorate each candidate with its total clustering-method count once,
    // rather than recomputing it inside the sort comparator on every
    // comparison.
    let mut candidates: Vec<(usize, RefactorCandidate)> = flagged_god_objects
        .iter()
        .filter_map(|(path, reason)| {
            let functions = &snapshot.file_metrics.get(path)?.functions;
            let groups = group_methods_by_prefix(functions);
            if groups.is_empty() {
                return None;
            }
            let count: usize = groups.iter().map(|(_, names)| names.len()).sum();
            Some((count, (path.clone(), reason.clone(), groups)))
        })
        .collect();

    // Tie-break by display string, not PathBuf component ordering — matches
    // god_object_files' own sort (PathBuf::cmp compares path components,
    // which can disagree with plain byte-string comparison, e.g. for
    // "src-utils.rs" vs "src/utils.rs").
    candidates.sort_by(|(a_count, (a_path, ..)), (b_count, (b_path, ..))| {
        b_count
            .cmp(a_count)
            .then_with(|| a_path.to_string_lossy().cmp(&b_path.to_string_lossy()))
    });

    candidates
        .into_iter()
        .take(5)
        .map(|(_, (path, reason, groups))| {
            let groups_text = groups
                .iter()
                .map(|(prefix, names)| format!("{prefix}* ({})", names.len()))
                .collect::<Vec<_>>()
                .join(", ");
            ActionItem {
                text: format!(
                    "[Health] {} — {} — consider splitting by responsibility: {}",
                    path.display(),
                    reason,
                    groups_text
                ),
                target_tab: Some("hotspots".to_string()),
                sort_by: Some("complexity".to_string()),
            }
        })
        .collect()
}

fn target_tab_for_metric(metric_name: &str) -> (Option<&'static str>, Option<&'static str>) {
    match metric_name {
        "Bus factor" => (Some("ownership"), Some("authors")),
        "God objects" => (Some("hotspots"), Some("complexity")),
        "Complex hotspots" => (Some("hotspots"), Some("complexity")),
        "Long methods" => (Some("hotspots"), Some("complexity")),
        "Code biomarkers" => (Some("hotspots"), Some("complexity")),
        "Afferent coupling" => (Some("coupling"), None),
        "Efferent coupling" => (Some("coupling"), None),
        "Circular dependencies" => (Some("coupling"), None),
        "Change coupling smells" => (Some("coupling"), None),
        "Knowledge distribution" => (Some("ownership"), None),
        "Ownership clarity" => (Some("ownership"), None),
        "Collaboration patterns" => (Some("ownership"), None),
        "Code/test growth balance" => (Some("trends"), None),
        "Cross-team coupling" => (Some("ownership"), None),
        "Knowledge loss" => (Some("ownership"), None),
        "Code age" => (Some("age"), None),
        "Growth trend" => (Some("trends"), None),
        "Refactoring ratio" => (Some("hotspots"), None),
        "Commit cadence" => (Some("trends"), None),
        _ => (None, None),
    }
}

fn suggest_action(metric_name: &str) -> &'static str {
    match metric_name {
        "Bus factor" => "Increase code review coverage and pair programming to spread knowledge",
        "God objects" => {
            "Break down large files by extracting responsibilities into smaller modules"
        }
        "Complex hotspots" => {
            "Prioritize refactoring files with both high complexity and high churn"
        }
        "Long methods" => {
            "Extract smaller functions from the longest methods to improve readability"
        }
        "Code biomarkers" => "Reduce nesting depth by applying early returns and guard clauses",
        "Afferent coupling" => {
            "Reduce dependents on high-Ca files by introducing abstractions or splitting modules"
        }
        "Efferent coupling" => "Reduce imports by extracting shared interfaces or facades",
        "Circular dependencies" => {
            "Break circular imports by extracting shared types into a separate module"
        }
        "Change coupling smells" => {
            "Decouple cross-boundary co-changing files by introducing interfaces or shared abstractions"
        }
        "Knowledge distribution" => "Encourage cross-team contributions and rotate ownership",
        "Contributor activity" => "Onboard more active contributors or check team health",
        "Ownership clarity" => "Assign clear code owners via CODEOWNERS file",
        "Collaboration patterns" => "Break directory silos through cross-functional reviews",
        "Code/test growth balance" => {
            "Pair recent source growth with tests — start with the listed untested second-half files"
        }
        "Cross-team coupling" => {
            "Align ownership with change patterns — co-owning coupled files or splitting them along owner boundaries"
        }
        "Knowledge loss" => {
            "Schedule knowledge-transfer or documentation passes over the most unattributed files"
        }
        "Merge patterns" => "Review branching strategy for healthier merge patterns",
        "Growth trend" => "Monitor growth rate and plan for sustainable development",
        "Refactoring ratio" => "Balance new feature work with refactoring of existing code",
        "Code age" => "Plan modernization of oldest code sections",
        "Commit cadence" => "Establish regular commit patterns and avoid large batches",
        "Commit message quality" => "Adopt conventional commits or enforce message guidelines",
        "History cleanliness" => {
            "Clean up merge strategy and enforce linear history where possible"
        }
        "Gitignore coverage" => "Add suspicious files to .gitignore and remove from tracking",
        _ => "Review and improve this metric",
    }
}

pub(super) fn score_commit_message(msg: &str) -> f64 {
    let trimmed = msg.trim();
    let len = trimmed.len();

    let mut score: f64 = 10.0; // base points for having any message

    // Length score: 0-40 points
    score += match len {
        0..=3 => 0.0,
        4..=10 => 10.0,
        11..=50 => 30.0,
        _ => 40.0,
    };

    // Conventional commit prefix: +30 points
    let prefixes = [
        "feat:",
        "fix:",
        "docs:",
        "style:",
        "refactor:",
        "perf:",
        "test:",
        "chore:",
        "ci:",
        "build:",
    ];
    if prefixes.iter().any(|p| trimmed.starts_with(p)) {
        score += 30.0;
    }

    // Descriptive (>20 chars or has body): +20 points
    if trimmed.contains('\n') || len > 20 {
        score += 20.0;
    }

    // Penalty for low-effort messages
    let lower = trimmed.to_lowercase();
    if lower == "wip" || lower == "fix" || lower == "update" || lower == "." {
        score = score.min(10.0);
    }

    score.min(100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn growth_balance_action_arms_are_pinned() {
        assert_eq!(
            target_tab_for_metric("Code/test growth balance"),
            (Some("trends"), None)
        );
        assert_eq!(
            suggest_action("Code/test growth balance"),
            "Pair recent source growth with tests — start with the listed untested second-half files"
        );
    }

    #[test]
    fn target_tab_for_metric_pins_representative_arms() {
        // These two functions had no direct tests — every whole-function
        // mutant survived the MR gate. Pin one arm per behavior class.
        assert_eq!(
            target_tab_for_metric("Cross-team coupling"),
            (Some("ownership"), None)
        );
        assert_eq!(
            target_tab_for_metric("Code biomarkers"),
            (Some("hotspots"), Some("complexity"))
        );
        assert_eq!(
            target_tab_for_metric("Afferent coupling"),
            (Some("coupling"), None)
        );
        assert_eq!(target_tab_for_metric("not a metric"), (None, None));
        assert_eq!(
            target_tab_for_metric("Knowledge loss"),
            (Some("ownership"), None)
        );
    }

    #[test]
    fn suggest_action_pins_representative_arms() {
        assert_eq!(
            suggest_action("Cross-team coupling"),
            "Align ownership with change patterns — co-owning coupled files or splitting them along owner boundaries"
        );
        assert_eq!(
            suggest_action("Bus factor"),
            "Increase code review coverage and pair programming to spread knowledge"
        );
        assert!(
            !suggest_action("not a metric").is_empty(),
            "the fallback suggestion must be non-empty"
        );
        assert_eq!(
            suggest_action("Knowledge loss"),
            "Schedule knowledge-transfer or documentation passes over the most unattributed files"
        );
    }
    use crate::metrics::{CategoryResult, MetricValue, RawValue};

    const WEIGHTS: &[(&str, f64)] = &[
        ("Health", 0.25),
        ("Team", 0.10),
        ("Evolution", 0.25),
        ("Git Hygiene", 0.20),
        ("Coupling", 0.20),
    ];

    fn make_category(name: &str, score: u32) -> CategoryResult {
        CategoryResult {
            name: name.to_string(),
            score,
            metrics: vec![MetricValue {
                name: format!("{} metric", name),
                description: "test".to_string(),
                raw_value: RawValue::Integer(0),
                score: Some(score),
            }],
        }
    }

    #[test]
    fn overall_score_weighted_average() {
        let categories = vec![
            make_category("Health", 80),
            make_category("Team", 60),
            make_category("Evolution", 70),
            make_category("Git Hygiene", 50),
            make_category("Coupling", 60),
        ];
        let score = compute_overall_score_with_weights(&categories, WEIGHTS);
        // 80*0.25 + 60*0.10 + 70*0.25 + 50*0.20 + 60*0.20 = 20+6+17.5+10+12 = 65.5 → 66
        assert_eq!(score, 66);
    }

    #[test]
    fn overall_score_single_category() {
        let categories = vec![make_category("Health", 75)];
        let score = compute_overall_score_with_weights(&categories, WEIGHTS);
        assert_eq!(score, 75);
    }

    #[test]
    fn overall_score_empty() {
        let score = compute_overall_score_with_weights(&[], WEIGHTS);
        assert_eq!(score, 0);
    }

    #[test]
    fn overall_score_custom_weights() {
        let categories = vec![
            make_category("Health", 100),
            make_category("Team", 0),
            make_category("Evolution", 0),
            make_category("Git Hygiene", 0),
        ];
        let weights = vec![
            ("Health", 1.0),
            ("Team", 0.0),
            ("Evolution", 0.0),
            ("Git Hygiene", 0.0),
        ];
        let score = compute_overall_score_with_weights(&categories, &weights);
        assert_eq!(score, 100);
    }

    #[test]
    fn top_actions_picks_worst() {
        let categories = vec![
            CategoryResult {
                name: "Health".to_string(),
                score: 50,
                metrics: vec![
                    MetricValue {
                        name: "Bus factor".to_string(),
                        description: "bad".to_string(),
                        raw_value: RawValue::Integer(1),
                        score: Some(20),
                    },
                    MetricValue {
                        name: "Churn hotspots".to_string(),
                        description: "ok".to_string(),
                        raw_value: RawValue::Count(0),
                        score: Some(90),
                    },
                ],
            },
            CategoryResult {
                name: "Team".to_string(),
                score: 40,
                metrics: vec![MetricValue {
                    name: "Knowledge distribution".to_string(),
                    description: "bad".to_string(),
                    raw_value: RawValue::Float(0.8),
                    score: Some(15),
                }],
            },
        ];

        let actions = generate_top_actions(&categories);
        assert!(!actions.is_empty());
        assert!(actions[0].text.contains("Knowledge distribution"));
    }

    #[test]
    fn score_commit_message_quality() {
        assert!(score_commit_message("feat: add login flow with validation") > 80.0);
        assert!(score_commit_message("fix: typo") > 40.0);
        assert!(score_commit_message("wip") < 20.0);
        assert!(score_commit_message("") < 15.0);
    }

    use crate::config::CouplingThresholds;
    use crate::snapshot::{CouplingFinding, CouplingKind, RepoSnapshot};
    use std::path::PathBuf;

    fn snap_with(findings: Vec<CouplingFinding>) -> RepoSnapshot {
        let mut s = crate::metrics::testutil::make_snapshot();
        s.files = vec![crate::metrics::testutil::make_file("src/a.rs")];
        s.file_metrics.insert(
            PathBuf::from("src/a.rs"),
            crate::snapshot::FileComplexity::default(),
        );
        s.coupling_findings = findings;
        s
    }
    fn finding(path: &str, kind: CouplingKind) -> CouplingFinding {
        CouplingFinding {
            path: PathBuf::from(path),
            line: Some(1),
            kind,
            evidence: "e".into(),
        }
    }

    #[test]
    fn coupling_actions_empty_when_no_findings() {
        let s = snap_with(vec![]);
        assert!(generate_coupling_actions(&s, &CouplingThresholds::default()).is_empty());
    }

    #[test]
    fn coupling_actions_order_content_common_control() {
        let s = snap_with(vec![
            finding("src/ctrl.rs", CouplingKind::Control),
            finding("src/glob.rs", CouplingKind::Common),
            finding("src/int.rs", CouplingKind::Content),
        ]);
        let acts = generate_coupling_actions(&s, &CouplingThresholds::default());
        let files: Vec<&str> = acts.iter().map(|a| a.text.as_str()).collect();
        assert!(files[0].contains("src/int.rs") && files[0].contains("worst: content"));
        assert!(files[1].contains("src/glob.rs") && files[1].contains("worst: common"));
        assert!(files[2].contains("src/ctrl.rs") && files[2].contains("worst: control"));
        assert_eq!(acts[0].target_tab.as_deref(), Some("coupling"));
        assert!(acts[0].sort_by.is_none());
    }

    #[test]
    fn coupling_actions_worst_rung_wins_for_mixed_file() {
        // Two mixed files with findings inserted in OPPOSITE severity orders.
        // Both must resolve to their most-severe kind (common) regardless of
        // insertion order — `src/mix2.rs` (Common then Control) is the case a
        // dropped `.min()` / last-write-wins would get wrong.
        let s = snap_with(vec![
            finding("src/mix1.rs", CouplingKind::Control),
            finding("src/mix1.rs", CouplingKind::Common),
            finding("src/mix2.rs", CouplingKind::Common),
            finding("src/mix2.rs", CouplingKind::Control),
        ]);
        let acts = generate_coupling_actions(&s, &CouplingThresholds::default());
        assert_eq!(acts.len(), 2);
        for a in &acts {
            assert!(a.text.contains("worst: common"), "{}", a.text);
            assert!(a.text.contains("2 finding(s)"), "{}", a.text);
        }
    }

    #[test]
    fn coupling_actions_corroborated_first_within_rung() {
        // Two Common files, one corroborated (co-changes cross-boundary).
        let mut s = snap_with(vec![
            finding("src/dormant.rs", CouplingKind::Common),
            finding("src/live.rs", CouplingKind::Common),
        ]);
        s.files
            .push(crate::metrics::testutil::make_file("src/live.rs"));
        s.files
            .push(crate::metrics::testutil::make_file("src/dormant.rs"));
        s.file_change_pairs
            .push((PathBuf::from("src/live.rs"), PathBuf::from("tests/x.rs"), 5));
        for f in ["src/live.rs", "tests/x.rs"] {
            s.commits_by_file.insert(
                PathBuf::from(f),
                (0u32..10).map(crate::snapshot::CommitId).collect(),
            );
        }
        let acts = generate_coupling_actions(&s, &CouplingThresholds::default());
        assert!(acts[0].text.contains("src/live.rs"));
        assert!(acts[0].text.contains("corroborated by change history"));
        assert!(acts[1].text.contains("src/dormant.rs"));
        assert!(!acts[1].text.contains("corroborated"));
    }

    #[test]
    fn coupling_actions_capped_at_ten_keeps_path_sorted_prefix() {
        // 15 same-rung, same-count, dormant Control findings on distinct files.
        // The only tiebreak is path asc, so the surviving 10 must be the
        // lexicographically smallest 10 paths — catches a cap-before-sort bug.
        let paths: Vec<String> = (0..15).map(|i| format!("src/f{i:02}.rs")).collect();
        let findings = paths
            .iter()
            .map(|p| finding(p, CouplingKind::Control))
            .collect();
        let s = snap_with(findings);
        let acts = generate_coupling_actions(&s, &CouplingThresholds::default());
        assert_eq!(acts.len(), 10);
        let mut expected = paths.clone();
        expected.sort();
        for (act, want) in acts.iter().zip(expected.iter().take(10)) {
            assert!(
                act.text.contains(want.as_str()),
                "expected {want} in {}",
                act.text
            );
        }
    }

    #[test]
    fn coupling_actions_empty_when_detection_did_not_run() {
        // Findings present but no file_metrics (AST pass didn't run — ADR-005
        // backfill). Must return empty, never fabricated actions.
        let mut s = snap_with(vec![finding("src/a.rs", CouplingKind::Common)]);
        s.file_metrics.clear();
        assert!(generate_coupling_actions(&s, &CouplingThresholds::default()).is_empty());
    }

    #[test]
    fn coupling_actions_advice_is_kind_specific() {
        for (kind, needle) in [
            (CouplingKind::Content, "public interface"),
            (CouplingKind::Common, "injected state"),
            (CouplingKind::Inheritance, "composition"),
            (CouplingKind::Control, "intent-revealing"),
        ] {
            let s = snap_with(vec![finding("src/a.rs", kind)]);
            let acts = generate_coupling_actions(&s, &CouplingThresholds::default());
            assert!(
                acts[0].text.contains(needle),
                "kind {kind:?}: {}",
                acts[0].text
            );
        }
    }

    #[test]
    fn inheritance_ranks_between_common_and_control() {
        let s = snap_with(vec![
            finding("src/ctrl.rs", CouplingKind::Control),
            finding("src/deep.ts", CouplingKind::Inheritance),
            finding("src/glob.rs", CouplingKind::Common),
        ]);
        let actions = generate_coupling_actions(&s, &crate::config::CouplingThresholds::default());
        let texts: Vec<&str> = actions.iter().map(|a| a.text.as_str()).collect();
        assert!(texts[0].contains("src/glob.rs") && texts[0].contains("worst: common"));
        assert!(texts[1].contains("src/deep.ts") && texts[1].contains("worst: inheritance"));
        assert!(texts[2].contains("src/ctrl.rs") && texts[2].contains("worst: control"));
    }

    fn fm(name: &str) -> crate::snapshot::FunctionMetrics {
        crate::snapshot::FunctionMetrics {
            name: name.to_string(),
            loc: 10,
            cyclomatic_complexity: 2,
            max_nesting_depth: 1,
        }
    }

    #[test]
    fn group_methods_by_prefix_groups_shared_verbs() {
        let functions = vec![
            fm("handle_a"),
            fm("handle_b"),
            fm("handle_c"),
            fm("validate_x"),
            fm("validate_y"),
            fm("parse_one"),
        ];
        let groups = group_methods_by_prefix(&functions);
        assert_eq!(
            groups,
            vec![
                ("handle_", vec!["handle_a", "handle_b", "handle_c"]),
                ("validate_", vec!["validate_x", "validate_y"]),
            ]
        );
    }

    #[test]
    fn group_methods_by_prefix_excludes_singleton_groups() {
        let functions = vec![fm("parse_only_one"), fm("main")];
        assert!(group_methods_by_prefix(&functions).is_empty());
    }

    #[test]
    fn group_methods_by_prefix_returns_empty_for_no_matches() {
        let functions = vec![fm("run")];
        assert!(group_methods_by_prefix(&functions).is_empty());
    }

    #[test]
    fn group_methods_by_prefix_matches_camel_case_boundaries() {
        // The collector supports 8 languages via tree-sitter; camelCase
        // methods (Java/C#/JS/TS/Kotlin/Swift) must cluster exactly like
        // their snake_case equivalents.
        let functions = vec![fm("getUserData"), fm("getUserProfile"), fm("handleClick")];
        let groups = group_methods_by_prefix(&functions);
        assert_eq!(
            groups,
            vec![("get_", vec!["getUserData", "getUserProfile"])]
        );
    }

    #[test]
    fn group_methods_by_prefix_camel_case_boundary_excludes_lookalikes() {
        // "getter" and "geta" share the "get" letters but not a real word
        // boundary (next char is lowercase, neither '_' nor uppercase) —
        // must not match, same discipline as the snake_case case.
        let functions = vec![fm("getter"), fm("geta")];
        assert!(group_methods_by_prefix(&functions).is_empty());
    }

    #[test]
    fn group_methods_by_prefix_mixed_snake_and_camel_case_share_a_group() {
        let functions = vec![fm("handle_click"), fm("handleSubmit")];
        let groups = group_methods_by_prefix(&functions);
        assert_eq!(
            groups,
            vec![("handle_", vec!["handleSubmit", "handle_click"])]
        );
    }

    #[test]
    fn group_methods_by_prefix_matches_pascal_case_exported_methods() {
        // Go and C# capitalize exported/public method names (GetUserData,
        // not getUserData) — the verb itself must match case-insensitively,
        // with the boundary check (next char uppercase) unchanged.
        let functions = vec![fm("GetUserData"), fm("GetUserProfile"), fm("HandleClick")];
        let groups = group_methods_by_prefix(&functions);
        assert_eq!(
            groups,
            vec![("get_", vec!["GetUserData", "GetUserProfile"])]
        );
    }

    #[test]
    fn generate_refactoring_actions_emits_action_for_clustering_god_object() {
        let mut snapshot = crate::snapshot::RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            crate::snapshot::TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            std::path::PathBuf::from("god.rs"),
            crate::snapshot::FileComplexity {
                total_lines: 600,
                loc: 520,
                cyclomatic_complexity: 10,
                public_methods: 5,
                properties: 2,
                functions: vec![fm("handle_a"), fm("handle_b"), fm("main")],
                ..Default::default()
            },
        );
        let thresholds = crate::config::HealthThresholds::default();
        let actions = generate_refactoring_actions(
            &snapshot,
            &crate::metrics::health::god_object_files(&snapshot, &thresholds),
        );
        assert_eq!(actions.len(), 1);
        assert!(actions[0].text.contains("god.rs"));
        assert!(actions[0].text.contains("520 loc"));
        assert!(actions[0].text.contains("handle_* (2)"));
        assert_eq!(actions[0].target_tab, Some("hotspots".to_string()));
    }

    #[test]
    fn generate_refactoring_actions_ranks_by_group_size_and_caps_at_five() {
        let mut snapshot = crate::snapshot::RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            crate::snapshot::TimeWindow::default(),
        );
        // 6 god-object files, each with a distinct total clustering-method
        // count (sum of group sizes): f0=2 (smallest), f5=7 (largest). Only
        // the top 5 by count should survive, in descending order; f0 (count
        // 2, the smallest) must be dropped.
        let counts = [
            ("f0.rs", 2),
            ("f1.rs", 3),
            ("f2.rs", 4),
            ("f3.rs", 5),
            ("f4.rs", 6),
            ("f5.rs", 7),
        ];
        for (name, count) in counts {
            let functions: Vec<_> = (0..count).map(|i| fm(&format!("handle_{i}"))).collect();
            snapshot.file_metrics.insert(
                std::path::PathBuf::from(name),
                crate::snapshot::FileComplexity {
                    total_lines: 600,
                    loc: 520,
                    cyclomatic_complexity: 10,
                    public_methods: 5,
                    properties: 2,
                    functions,
                    ..Default::default()
                },
            );
        }
        let thresholds = crate::config::HealthThresholds::default();
        let actions = generate_refactoring_actions(
            &snapshot,
            &crate::metrics::health::god_object_files(&snapshot, &thresholds),
        );
        assert_eq!(actions.len(), 5, "must be capped at 5: {actions:#?}");
        let expected_order = ["f5.rs", "f4.rs", "f3.rs", "f2.rs", "f1.rs"];
        for (action, expected) in actions.iter().zip(expected_order.iter()) {
            assert!(
                action.text.contains(expected),
                "expected {expected} next, got: {}",
                action.text
            );
        }
        assert!(
            actions.iter().all(|a| !a.text.contains("f0.rs")),
            "smallest group (f0.rs) must be dropped by the cap"
        );
    }

    #[test]
    fn generate_refactoring_actions_tie_break_sorts_by_display_string_not_pathbuf() {
        // Regression guard mirroring god_object_files' own fix: PathBuf::cmp
        // compares path COMPONENTS, which disagrees with plain byte-string
        // comparison for a pair like "src-utils.rs" (one component) vs
        // "src/utils.rs" (two components) — '-' (0x2D) < '/' (0x2F) as
        // bytes, but "src" < "src-utils.rs" as path components. Both
        // candidates here have the same clustering-method count (2), so the
        // tie-break alone decides the order.
        let mut snapshot = crate::snapshot::RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            crate::snapshot::TimeWindow::default(),
        );
        for name in ["src/utils.rs", "src-utils.rs"] {
            snapshot.file_metrics.insert(
                std::path::PathBuf::from(name),
                crate::snapshot::FileComplexity {
                    total_lines: 600,
                    loc: 520,
                    cyclomatic_complexity: 10,
                    public_methods: 5,
                    properties: 2,
                    functions: vec![fm("handle_a"), fm("handle_b")],
                    ..Default::default()
                },
            );
        }
        let thresholds = crate::config::HealthThresholds::default();
        let actions = generate_refactoring_actions(
            &snapshot,
            &crate::metrics::health::god_object_files(&snapshot, &thresholds),
        );
        assert_eq!(actions.len(), 2);
        assert!(
            actions[0].text.contains("src-utils.rs"),
            "tie-break must sort by display string ('-' < '/'), got: {:#?}",
            actions
        );
        assert!(actions[1].text.contains("src/utils.rs"));
    }

    #[test]
    fn generate_refactoring_actions_skips_god_object_with_no_clustering() {
        let mut snapshot = crate::snapshot::RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            crate::snapshot::TimeWindow::default(),
        );
        snapshot.file_metrics.insert(
            std::path::PathBuf::from("god.rs"),
            crate::snapshot::FileComplexity {
                total_lines: 600,
                loc: 520,
                cyclomatic_complexity: 10,
                public_methods: 5,
                properties: 2,
                functions: vec![fm("run")],
                ..Default::default()
            },
        );
        let thresholds = crate::config::HealthThresholds::default();
        assert!(generate_refactoring_actions(
            &snapshot,
            &crate::metrics::health::god_object_files(&snapshot, &thresholds)
        )
        .is_empty());
    }

    #[test]
    fn generate_refactoring_actions_skips_non_god_object_with_clustering_names() {
        let mut snapshot = crate::snapshot::RepoSnapshot::new(
            std::path::PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            crate::snapshot::TimeWindow::default(),
        );
        // Small file (not flagged as a god object) with clustering method names —
        // proves the shared-selection-function gate (decision 6 in the spec).
        snapshot.file_metrics.insert(
            std::path::PathBuf::from("small.rs"),
            crate::snapshot::FileComplexity {
                total_lines: 50,
                loc: 40,
                cyclomatic_complexity: 3,
                public_methods: 2,
                properties: 1,
                functions: vec![fm("handle_a"), fm("handle_b")],
                ..Default::default()
            },
        );
        let thresholds = crate::config::HealthThresholds::default();
        assert!(generate_refactoring_actions(
            &snapshot,
            &crate::metrics::health::god_object_files(&snapshot, &thresholds)
        )
        .is_empty());
    }
}
