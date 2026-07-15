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
}
