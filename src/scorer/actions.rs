use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use crate::metrics::CategoryResult;

use super::types::ActionItem;

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

    let scored = low_metrics
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
        });

    // Advisory metrics don't score, but ones carrying concrete findings
    // still deserve their curated advice — appended after the scored
    // actions so they never displace a real score problem.
    let advisory = categories
        .iter()
        .flat_map(|cat| cat.metrics.iter().map(move |metric| (cat, metric)))
        .filter(|(_, metric)| metric.score.is_none() && advisory_has_findings(&metric.raw_value))
        .take(2)
        .map(|(cat, metric)| {
            let (target_tab, sort_by) = target_tab_for_metric(&metric.name);
            ActionItem {
                text: format!(
                    "[{}] {} (advisory) — {}",
                    cat.name,
                    metric.name,
                    suggest_action(&metric.name)
                ),
                target_tab: target_tab.map(String::from),
                sort_by: sort_by.map(String::from),
            }
        });

    scored.chain(advisory).collect()
}

/// Whether an unscored metric's raw value carries concrete findings worth an
/// advisory action. Annotations (Integer/Float/Percentage) and N/A text are
/// not findings.
fn advisory_has_findings(raw_value: &crate::metrics::RawValue) -> bool {
    match raw_value {
        crate::metrics::RawValue::List(items) => !items.is_empty(),
        crate::metrics::RawValue::Count(count) => *count > 0,
        _ => false,
    }
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
    evidence: &crate::metrics::coupling::CouplingEvidence,
) -> Vec<ActionItem> {
    use crate::snapshot::CouplingKind;

    if !evidence.detection_ran {
        return Vec::new();
    }
    let findings = &evidence.findings;
    if findings.is_empty() {
        return Vec::new();
    }

    // severity index: lower = worse. Group by file, tracking worst rung + count.
    let mut by_file: HashMap<&Path, (u8, usize)> = HashMap::new();
    for f in findings {
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
        .map(|(path, (sev, count))| (path, sev, evidence.is_corroborated(path), count))
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
    if !name
        .get(..verb.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(verb))
    {
        return false;
    }
    match name.as_bytes().get(verb.len()) {
        Some(b'_') => true,
        Some(b) => b.is_ascii_uppercase(),
        None => false,
    }
}

/// One owner-local group. Predicates additionally name the direct dependency
/// shared by every member; matching names alone do not establish that evidence.
#[derive(Debug, PartialEq, Eq)]
struct MethodGroup<'a> {
    owner_id: &'a str,
    owner_label: &'a str,
    prefix: &'static str,
    names: Vec<&'a str>,
    dependency: Option<&'a crate::snapshot::ResponsibilityDependency>,
}

type MethodGroups<'a> = Vec<MethodGroup<'a>>;

/// One owner-local member name with the evidence of every declaration carrying it.
type GroupMember<'a> = (
    &'a str,
    BTreeSet<&'a crate::snapshot::ResponsibilityDependency>,
);

/// Partition every prefix by declaration identity before considering evidence.
/// Unknown owners and recognized tests cannot participate in advice. Accessor
/// pairs and overloads share one member name, so each name counts once.
fn group_methods_by_prefix(functions: &[crate::snapshot::FunctionMetrics]) -> MethodGroups<'_> {
    let groups = functions
        .iter()
        .filter(|function| !function.is_test)
        .filter_map(|function| {
            let owner = function.responsibility.as_ref()?;
            let prefix = GROUPING_PREFIXES
                .iter()
                .copied()
                .find(|prefix| matches_group_prefix(&function.name, prefix))?;
            Some((
                (owner.owner_id.as_str(), prefix),
                (function.name.as_str(), owner),
            ))
        })
        .fold(
            BTreeMap::<_, (&str, BTreeMap<&str, BTreeSet<_>>)>::new(),
            |mut groups, (key, (name, owner))| {
                let (_, members) = groups
                    .entry(key)
                    .or_insert_with(|| (owner.owner_label.as_str(), BTreeMap::new()));
                members
                    .entry(name)
                    .or_default()
                    .extend(owner.dependencies.iter());
                groups
            },
        );

    // Functions arrive in extraction order, so an owner's first member marks
    // its source position; byte offsets inside owner ids do not sort as text.
    let source_order = functions
        .iter()
        .filter_map(|function| function.responsibility.as_ref())
        .enumerate()
        .fold(HashMap::new(), |mut order, (index, owner)| {
            order.entry(owner.owner_id.as_str()).or_insert(index);
            order
        });
    let mut result: MethodGroups<'_> = groups
        .into_iter()
        .filter(|(_, (_, members))| members.len() >= 2)
        .flat_map(|((owner_id, prefix), (owner_label, members))| {
            let members: Vec<GroupMember<'_>> = members.into_iter().collect();
            if matches!(prefix, "has_" | "is_") {
                predicate_groups(owner_id, prefix, owner_label, &members)
            } else {
                vec![MethodGroup {
                    owner_id,
                    owner_label,
                    prefix,
                    names: members.iter().map(|(name, _)| *name).collect(),
                    dependency: None,
                }]
            }
        })
        .collect();
    result.sort_by_key(|group| {
        (
            source_order.get(group.owner_id).copied(),
            group.prefix,
            group.dependency.map(dependency_key),
        )
    });
    result
}

/// Evidence identity: fields before callees, then the file-local identity.
fn dependency_key(dependency: &crate::snapshot::ResponsibilityDependency) -> (u8, &str) {
    use crate::snapshot::ResponsibilityDependency;
    match dependency {
        ResponsibilityDependency::Field { identity, .. } => (0, identity.as_str()),
        ResponsibilityDependency::Callee { identity, .. } => (1, identity.as_str()),
    }
}

/// Select the largest remaining group sharing one identical dependency. Sorting
/// by kind then source identity makes overlapping evidence deterministic; removing
/// assigned members prevents transitive chains and counts each function once.
fn predicate_groups<'a>(
    owner_id: &'a str,
    prefix: &'static str,
    owner_label: &'a str,
    members: &[GroupMember<'a>],
) -> MethodGroups<'a> {
    let mut candidates = BTreeMap::new();
    for (index, (_, dependencies)) in members.iter().enumerate() {
        for &dependency in dependencies {
            let key = dependency_key(dependency);
            let (_, indexes) = candidates
                .entry(key)
                .or_insert_with(|| (dependency, BTreeSet::new()));
            indexes.insert(index);
        }
    }
    let mut groups = Vec::new();
    loop {
        candidates.retain(|_, (_, indexes)| indexes.len() >= 2);
        let selected = candidates
            .iter()
            .min_by(|(a_key, (_, a)), (b_key, (_, b))| {
                b.len().cmp(&a.len()).then_with(|| a_key.cmp(b_key))
            })
            .map(|(key, _)| *key);
        let Some(key) = selected else { break };
        let (dependency, assigned) = candidates.remove(&key).expect("selected dependency exists");
        let mut names: Vec<_> = assigned.iter().map(|index| members[*index].0).collect();
        names.sort_unstable();
        groups.push(MethodGroup {
            owner_id,
            owner_label,
            prefix,
            names,
            dependency: Some(dependency),
        });
        for (_, indexes) in candidates.values_mut() {
            indexes.retain(|index| !assigned.contains(index));
        }
    }
    groups
}

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
            let count: usize = groups.iter().map(|group| group.names.len()).sum();
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
            let groups_text = responsibility_text(&groups);
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

/// Owners shown in one refactoring action; the rest are summarised as a count.
const MAX_ADVICE_OWNERS: usize = 5;

/// One segment per owner, each listing that owner's groups: the
/// `MAX_ADVICE_OWNERS` owners with the most grouped names, shown in source
/// order, then a count of the groups left out.
fn responsibility_text(groups: &[MethodGroup<'_>]) -> String {
    use crate::snapshot::ResponsibilityDependency;

    let owners = groups
        .iter()
        .fold(Vec::<Vec<&MethodGroup<'_>>>::new(), |mut owners, group| {
            match owners
                .last_mut()
                .filter(|owner| owner[0].owner_id == group.owner_id)
            {
                Some(owner) => owner.push(group),
                None => owners.push(vec![group]),
            }
            owners
        });
    // Keep the owners with the most grouped names (earlier owner on a tie), then
    // render the kept ones in source order.
    let kept: BTreeSet<usize> = owners
        .iter()
        .enumerate()
        .map(|(index, owner)| {
            let names: usize = owner.iter().map(|group| group.names.len()).sum();
            (std::cmp::Reverse(names), index)
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(MAX_ADVICE_OWNERS)
        .map(|(_, index)| index)
        .collect();
    let (shown, hidden_owners): (Vec<_>, Vec<_>) = owners
        .iter()
        .enumerate()
        .partition(|(index, _)| kept.contains(index));
    let segments = shown.into_iter().map(|(_, owner)| {
        let listed = owner
            .iter()
            .map(|group| {
                let evidence = match group.dependency {
                    Some(ResponsibilityDependency::Field { label, .. }) => {
                        format!(" shared field {label}")
                    }
                    Some(ResponsibilityDependency::Callee { label, .. }) => {
                        format!(" shared callee {label}")
                    }
                    None => String::new(),
                };
                format!("{}* ({}){evidence}", group.prefix, group.names.len())
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("{}: {listed}", owner[0].owner_label)
    });
    let hidden: usize = hidden_owners.iter().map(|(_, owner)| owner.len()).sum();
    segments
        .chain((hidden > 0).then(|| {
            format!(
                "+{hidden} more {}",
                if hidden == 1 { "group" } else { "groups" }
            )
        }))
        .collect::<Vec<_>>()
        .join(" | ")
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
        "Test safety net" => (Some("coupling"), None),
        "Knowledge distribution" => (Some("ownership"), None),
        "Churn-ownership risk" => (Some("ownership"), None),
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
        "Test safety net" => {
            "Revive the paired tests of recently-changed source files — start with the lowest co-change pairs"
        }
        "Knowledge distribution" => "Encourage cross-team contributions and rotate ownership",
        "Churn-ownership risk" => {
            "Pair a second maintainer on the flagged high-churn single-owner files"
        }
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
        "Gitignore coverage" => {
            "Review suspicious tracked files; ignore and untrack only confirmed credentials, local files, or generated artifacts"
        }
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
        assert_eq!(
            target_tab_for_metric("Test safety net"),
            (Some("coupling"), None)
        );
    }

    #[test]
    fn churn_ownership_risk_action_arms_are_pinned() {
        // Advisory metric with a List of findings — must carry curated
        // advice, not the generic fallback.
        assert_eq!(
            target_tab_for_metric("Churn-ownership risk"),
            (Some("ownership"), None)
        );
        assert_eq!(
            suggest_action("Churn-ownership risk"),
            "Pair a second maintainer on the flagged high-churn single-owner files"
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
        assert_eq!(
            suggest_action("Test safety net"),
            "Revive the paired tests of recently-changed source files — start with the lowest co-change pairs"
        );
    }

    #[test]
    fn gitignore_action_requires_review_and_confirmation() {
        let action = suggest_action("Gitignore coverage");
        assert_eq!(
            action,
            "Review suspicious tracked files; ignore and untrack only confirmed credentials, local files, or generated artifacts"
        );
        assert!(!action.contains("remove from tracking"));
    }
    use crate::metrics::{CategoryResult, MetricValue, RawValue};

    #[test]
    fn top_actions_picks_worst() {
        let categories = vec![
            CategoryResult {
                name: "Health".to_string(),
                score: Some(50),
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
                score: Some(40),
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
    fn top_actions_includes_advisory_metrics_with_findings() {
        // Metrics the recalibration made advisory (score: None) still carry
        // real findings; their curated advice must stay reachable.
        let categories = vec![CategoryResult {
            name: "Team".to_string(),
            score: Some(100),
            metrics: vec![
                MetricValue {
                    name: "Collaboration patterns".to_string(),
                    description: "8/9 top-level directories...".to_string(),
                    raw_value: RawValue::Count(8),
                    score: None,
                },
                MetricValue {
                    name: "Cross-team coupling".to_string(),
                    description: "1 cross-team pair".to_string(),
                    raw_value: RawValue::List(vec!["a.rs <-> b.rs".to_string()]),
                    score: None,
                },
            ],
        }];
        let actions = generate_top_actions(&categories);
        let silo = actions
            .iter()
            .find(|a| a.text.contains("Break directory silos"))
            .expect("collaboration-patterns advice must be reachable");
        assert!(silo.text.contains("advisory"), "{}", silo.text);
        assert_eq!(silo.target_tab.as_deref(), Some("ownership"));
        assert!(actions
            .iter()
            .any(|a| a.text.contains("Align ownership with change patterns")));
    }

    #[test]
    fn top_actions_skips_advisory_metrics_without_findings() {
        // N/A metrics, zero counts, and empty evidence lists are not
        // findings — no advisory action.
        let metric = |name: &str, raw_value| MetricValue {
            name: name.to_string(),
            description: "d".to_string(),
            raw_value,
            score: None,
        };
        let categories = vec![CategoryResult {
            name: "Team".to_string(),
            score: Some(100),
            metrics: vec![
                metric("Bus factor", RawValue::Text("N/A".to_string())),
                metric("Collaboration patterns", RawValue::Count(0)),
                metric("Cross-team coupling", RawValue::List(vec![])),
                metric("Growth trend", RawValue::Integer(42)),
            ],
        }];
        assert!(generate_top_actions(&categories).is_empty());
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
        assert!(
            generate_coupling_actions(&crate::metrics::coupling::CouplingEvidence::derive(
                &s,
                &CouplingThresholds::default()
            ))
            .is_empty()
        );
    }

    #[test]
    fn coupling_actions_order_content_common_control() {
        let s = snap_with(vec![
            finding("src/ctrl.rs", CouplingKind::Control),
            finding("src/glob.rs", CouplingKind::Common),
            finding("src/int.rs", CouplingKind::Content),
        ]);
        let acts = generate_coupling_actions(&crate::metrics::coupling::CouplingEvidence::derive(
            &s,
            &CouplingThresholds::default(),
        ));
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
        let acts = generate_coupling_actions(&crate::metrics::coupling::CouplingEvidence::derive(
            &s,
            &CouplingThresholds::default(),
        ));
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
        let acts = generate_coupling_actions(&crate::metrics::coupling::CouplingEvidence::derive(
            &s,
            &CouplingThresholds::default(),
        ));
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
        let acts = generate_coupling_actions(&crate::metrics::coupling::CouplingEvidence::derive(
            &s,
            &CouplingThresholds::default(),
        ));
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
        assert!(
            generate_coupling_actions(&crate::metrics::coupling::CouplingEvidence::derive(
                &s,
                &CouplingThresholds::default()
            ))
            .is_empty()
        );
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
            let acts =
                generate_coupling_actions(&crate::metrics::coupling::CouplingEvidence::derive(
                    &s,
                    &CouplingThresholds::default(),
                ));
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
        let actions =
            generate_coupling_actions(&crate::metrics::coupling::CouplingEvidence::derive(
                &s,
                &crate::config::CouplingThresholds::default(),
            ));
        let texts: Vec<&str> = actions.iter().map(|a| a.text.as_str()).collect();
        assert!(texts[0].contains("src/glob.rs") && texts[0].contains("worst: common"));
        assert!(texts[1].contains("src/deep.ts") && texts[1].contains("worst: inheritance"));
        assert!(texts[2].contains("src/ctrl.rs") && texts[2].contains("worst: control"));
    }

    fn fm(name: &str) -> crate::snapshot::FunctionMetrics {
        crate::snapshot::FunctionMetrics {
            responsibility: Some(crate::snapshot::ResponsibilityProvenance {
                owner_id: "module:0".into(),
                owner_label: "module".into(),
                dependencies: Vec::new(),
            }),
            is_test: false,
            name: name.to_string(),
            loc: 10,
            cyclomatic_complexity: 2,
            max_nesting_depth: 1,
        }
    }

    fn expected_group<'a>(prefix: &'static str, names: Vec<&'a str>) -> MethodGroup<'a> {
        MethodGroup {
            owner_id: "module:0",
            owner_label: "module",
            prefix,
            names,
            dependency: None,
        }
    }

    fn owned(name: &str, owner_id: &str) -> crate::snapshot::FunctionMetrics {
        crate::snapshot::FunctionMetrics {
            responsibility: Some(crate::snapshot::ResponsibilityProvenance {
                owner_id: owner_id.into(),
                // Equal labels deliberately model separate declarations of one type.
                owner_label: "Widget".into(),
                dependencies: Vec::new(),
            }),
            ..fm(name)
        }
    }

    fn field(identity: &str) -> crate::snapshot::ResponsibilityDependency {
        crate::snapshot::ResponsibilityDependency::Field {
            identity: identity.into(),
            label: format!("self.{identity}"),
        }
    }

    fn callee(identity: &str) -> crate::snapshot::ResponsibilityDependency {
        crate::snapshot::ResponsibilityDependency::Callee {
            identity: identity.into(),
            label: format!("{identity}()"),
        }
    }

    fn dependent(
        name: &str,
        dependencies: Vec<crate::snapshot::ResponsibilityDependency>,
    ) -> crate::snapshot::FunctionMetrics {
        let mut function = fm(name);
        function.responsibility.as_mut().unwrap().dependencies = dependencies;
        function
    }

    #[test]
    fn every_prefix_respects_owner_identity_including_separate_declarations() {
        for prefix in GROUPING_PREFIXES {
            let mut functions = vec![
                owned(&format!("{prefix}a"), "impl:10"),
                owned(&format!("{prefix}b"), "impl:50"),
            ];
            for function in &mut functions {
                function.responsibility.as_mut().unwrap().dependencies = vec![field("state")];
            }
            assert!(group_methods_by_prefix(&functions).is_empty(), "{prefix}");
            functions.push(functions[0].clone());
            functions[2].name = format!("{prefix}c");
            let groups = group_methods_by_prefix(&functions);
            assert_eq!(groups.len(), 1, "{prefix}");
            assert_eq!(
                groups[0].names,
                vec![format!("{prefix}a"), format!("{prefix}c")]
            );
            assert_eq!(groups[0].owner_label, "Widget");
        }
    }

    #[test]
    fn unknown_owners_and_predicates_without_shared_evidence_are_ineligible() {
        let functions = vec![
            crate::snapshot::FunctionMetrics {
                responsibility: None,
                ..fm("render_a")
            },
            crate::snapshot::FunctionMetrics {
                responsibility: None,
                ..fm("render_b")
            },
            fm("is_ready"),
            fm("is_active"),
            dependent("has_state", vec![field("state")]),
            dependent("has_cache", vec![field("cache")]),
        ];
        assert!(group_methods_by_prefix(&functions).is_empty());
    }

    #[test]
    fn predicates_need_the_same_dependency_identity_and_kind() {
        let functions = vec![
            dependent("has_field", vec![field("shared")]),
            dependent("has_callee", vec![callee("shared")]),
            dependent(
                "is_first",
                vec![crate::snapshot::ResponsibilityDependency::Field {
                    identity: "first:state".into(),
                    label: "state".into(),
                }],
            ),
            dependent(
                "is_second",
                vec![crate::snapshot::ResponsibilityDependency::Field {
                    identity: "second:state".into(),
                    label: "state".into(),
                }],
            ),
        ];
        assert!(group_methods_by_prefix(&functions).is_empty());
    }

    #[test]
    fn duplicate_dependency_entries_do_not_satisfy_group_minimum() {
        let functions = vec![
            dependent("has_a", vec![field("state"), field("state")]),
            fm("has_b"),
        ];
        assert!(group_methods_by_prefix(&functions).is_empty());
    }

    #[test]
    fn predicate_transitive_chain_selects_one_pair_and_removes_singletons() {
        let functions = vec![
            dependent("is_a", vec![field("x")]),
            dependent("is_b", vec![field("x"), field("y")]),
            dependent("is_c", vec![field("y")]),
        ];
        let groups = group_methods_by_prefix(&functions);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].names, vec!["is_a", "is_b"]);
        assert_eq!(groups[0].dependency, Some(&field("x")));
    }

    #[test]
    fn largest_dependency_group_wins_then_remaining_members_can_form_a_group() {
        let functions = vec![
            dependent("has_a", vec![field("a"), field("z")]),
            dependent("has_b", vec![field("z")]),
            dependent("has_c", vec![field("z")]),
            dependent("has_d", vec![field("a")]),
            dependent("has_e", vec![field("a")]),
            dependent("has_f", vec![field("z")]),
        ];
        let groups = group_methods_by_prefix(&functions);
        assert_eq!(groups.len(), 2);
        let sharing = |name: &str| {
            groups
                .iter()
                .find(|group| group.dependency == Some(&field(name)))
                .map(|group| group.names.clone())
        };
        // has_a shares both fields; the larger `z` group claims it first.
        assert_eq!(sharing("z"), Some(vec!["has_a", "has_b", "has_c", "has_f"]));
        assert_eq!(sharing("a"), Some(vec!["has_d", "has_e"]));
        assert_eq!(
            groups.iter().map(|group| group.names.len()).sum::<usize>(),
            6
        );
    }

    #[test]
    fn overlapping_ties_prefer_fields_then_source_identity_and_ignore_input_order() {
        let mut functions = vec![
            dependent("is_a", vec![callee("a"), field("z"), field("b")]),
            dependent("is_b", vec![callee("a")]),
            dependent("is_c", vec![field("z")]),
            dependent("is_d", vec![field("b")]),
        ];
        let before: Vec<_> = group_methods_by_prefix(&functions)
            .iter()
            .map(|group| {
                (
                    group
                        .names
                        .iter()
                        .map(|name| name.to_string())
                        .collect::<Vec<_>>(),
                    group.dependency.cloned(),
                )
            })
            .collect();
        assert_eq!(
            before,
            vec![(vec!["is_a".into(), "is_d".into()], Some(field("b")))]
        );
        functions.reverse();
        for function in &mut functions {
            function
                .responsibility
                .as_mut()
                .unwrap()
                .dependencies
                .reverse();
        }
        let after: Vec<_> = group_methods_by_prefix(&functions)
            .iter()
            .map(|group| {
                (
                    group
                        .names
                        .iter()
                        .map(|name| name.to_string())
                        .collect::<Vec<_>>(),
                    group.dependency.cloned(),
                )
            })
            .collect();
        assert_eq!(before, after);
    }

    #[test]
    fn recognized_tests_cannot_complete_a_predicate_group() {
        let functions = vec![
            dependent("is_ready", vec![callee("check")]),
            crate::snapshot::FunctionMetrics {
                is_test: true,
                ..dependent("is_ready_test", vec![callee("check")])
            },
        ];
        assert!(group_methods_by_prefix(&functions).is_empty());
    }

    #[test]
    fn advice_names_the_owner_and_direct_field_or_callee() {
        let mut snapshot = crate::snapshot::RepoSnapshot::new(
            "/tmp".into(),
            "test".into(),
            "main".into(),
            Default::default(),
        );
        snapshot.file_metrics.insert(
            "god.rs".into(),
            crate::snapshot::FileComplexity {
                functions: vec![
                    dependent("has_a", vec![field("state")]),
                    dependent("has_b", vec![field("state")]),
                    dependent("is_a", vec![callee("check")]),
                    dependent("is_b", vec![callee("check")]),
                ],
                ..Default::default()
            },
        );
        let actions =
            generate_refactoring_actions(&snapshot, &[("god.rs".into(), "520 loc".into())]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].text, "[Health] god.rs — 520 loc — consider splitting by responsibility: module: has_* (2) shared field self.state, is_* (2) shared callee check()");
        assert_eq!(actions[0].sort_by.as_deref(), Some("complexity"));
    }

    fn owned_by(name: &str, owner_id: &str, owner_label: &str) -> crate::snapshot::FunctionMetrics {
        crate::snapshot::FunctionMetrics {
            responsibility: Some(crate::snapshot::ResponsibilityProvenance {
                owner_id: owner_id.into(),
                owner_label: owner_label.into(),
                dependencies: Vec::new(),
            }),
            ..fm(name)
        }
    }

    fn advice_for(functions: Vec<crate::snapshot::FunctionMetrics>) -> String {
        let mut snapshot = crate::snapshot::RepoSnapshot::new(
            "/tmp".into(),
            "test".into(),
            "main".into(),
            Default::default(),
        );
        snapshot.file_metrics.insert(
            "god.rs".into(),
            crate::snapshot::FileComplexity {
                functions,
                ..Default::default()
            },
        );
        let actions =
            generate_refactoring_actions(&snapshot, &[("god.rs".into(), "520 loc".into())]);
        assert_eq!(actions.len(), 1);
        actions[0]
            .text
            .strip_prefix("[Health] god.rs — 520 loc — consider splitting by responsibility: ")
            .expect("advice prefix")
            .to_owned()
    }

    #[test]
    fn advice_orders_owners_by_source_position_and_merges_each_owners_groups() {
        // Byte offset 120 precedes 1000 in the source, though "impl_item:1000"
        // sorts first as a string.
        let advice = advice_for(vec![
            owned_by("set_a", "impl_item:120", "Early (line 5)"),
            owned_by("get_a", "impl_item:120", "Early (line 5)"),
            owned_by("get_b", "impl_item:120", "Early (line 5)"),
            owned_by("set_b", "impl_item:120", "Early (line 5)"),
            owned_by("get_c", "impl_item:1000", "Later (line 40)"),
            owned_by("get_d", "impl_item:1000", "Later (line 40)"),
        ]);

        assert_eq!(
            advice,
            "Early (line 5): get_* (2), set_* (2) | Later (line 40): get_* (2)"
        );
    }

    #[test]
    fn advice_shows_at_most_five_owners_and_counts_the_remaining_groups() {
        let functions = (0..7)
            .flat_map(|owner| {
                let id = format!("impl_item:{owner}");
                let label = format!("Owner{owner} (line {owner})");
                let mut members = vec![
                    owned_by(&format!("get_a{owner}"), &id, &label),
                    owned_by(&format!("get_b{owner}"), &id, &label),
                ];
                if owner == 5 {
                    members.push(owned_by("set_a5", &id, &label));
                    members.push(owned_by("set_b5", &id, &label));
                }
                members
            })
            .collect();

        assert_eq!(
            advice_for(functions),
            "Owner0 (line 0): get_* (2) | Owner1 (line 1): get_* (2) | Owner2 (line 2): get_* (2) | Owner3 (line 3): get_* (2) | Owner5 (line 5): get_* (2), set_* (2) | +2 more groups"
        );
    }

    fn owners_with_get_groups(sizes: &[usize]) -> Vec<crate::snapshot::FunctionMetrics> {
        sizes
            .iter()
            .enumerate()
            .flat_map(|(owner, size)| {
                let id = format!("impl_item:{owner}");
                let label = format!("Owner{owner} (line {owner})");
                (0..*size)
                    .map(|member| owned_by(&format!("get_m{owner}_{member}"), &id, &label))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    #[test]
    fn advice_keeps_the_largest_owners_even_when_they_come_last_in_source() {
        assert_eq!(
            advice_for(owners_with_get_groups(&[2, 2, 2, 2, 2, 2, 6])),
            "Owner0 (line 0): get_* (2) | Owner1 (line 1): get_* (2) | Owner2 (line 2): get_* (2) | Owner3 (line 3): get_* (2) | Owner6 (line 6): get_* (6) | +2 more groups"
        );
    }

    #[test]
    fn advice_counts_every_group_of_a_hidden_owner() {
        let mut functions = owners_with_get_groups(&[5, 5, 5, 5, 5]);
        functions.extend(
            ["get_a", "get_b", "set_a", "set_b"]
                .map(|name| owned_by(name, "impl_item:9", "Owner9 (line 9)")),
        );
        assert_eq!(
            advice_for(functions),
            "Owner0 (line 0): get_* (5) | Owner1 (line 1): get_* (5) | Owner2 (line 2): get_* (5) | Owner3 (line 3): get_* (5) | Owner4 (line 4): get_* (5) | +2 more groups"
        );
    }

    #[test]
    fn advice_names_a_single_hidden_group_in_the_singular() {
        assert_eq!(
            advice_for(owners_with_get_groups(&[2, 2, 2, 2, 2, 2])),
            "Owner0 (line 0): get_* (2) | Owner1 (line 1): get_* (2) | Owner2 (line 2): get_* (2) | Owner3 (line 3): get_* (2) | Owner4 (line 4): get_* (2) | +1 more group"
        );
    }

    #[test]
    fn prefix_matching_accepts_unicode_names_without_panicking() {
        for name in ["éé", "日", "has日", "is日", "get_日", "évalidate"] {
            let _ = group_methods_by_prefix(&[fm(name)]);
        }
        assert!(matches_group_prefix("get_日", "get_"));
        assert!(!matches_group_prefix("has日", "has_"));
    }

    #[test]
    fn pinned_barad_dur_dc23fbd6_does_not_group_unrelated_has_predicates() {
        // Exact function excerpts from dc23fbd6:src/metrics/coupling/mod.rs.
        // External helper declarations and unrelated functions are omitted.
        let source = r#"
fn has_edge(graph: &HashMap<PathBuf, Vec<PathBuf>>, from: &PathBuf, to: &PathBuf) -> bool {
    graph.get(from).is_some_and(|targets| targets.contains(to))
}
fn has_import_extractable_files(snapshot: &RepoSnapshot) -> bool {
    snapshot.files.iter().any(|file| {
        let ext = file.path.extension().and_then(|e| e.to_str()).unwrap_or("");
        import_query(detect_language(&file.path.to_string_lossy()), ext).is_some()
            && crate::collector::resolves_imports(ext)
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
"#;
        let metrics =
            crate::metrics::complexity::analyse_source(Path::new("mod.rs"), source).metrics;
        assert_eq!(metrics.functions.len(), 3, "nonempty pinned extraction");
        assert!(metrics
            .functions
            .iter()
            .all(|function| function.responsibility.is_some()));
        assert!(
            group_methods_by_prefix(&metrics.functions).is_empty(),
            "the previous has_* (3) mixed unrelated predicates"
        );
    }

    #[test]
    fn pinned_ripgrep_3fce3b5b_does_not_group_eight_is_predicates_across_owners() {
        // Exact method bodies and containing declaration headers from
        // 3fce3b5b:crates/matcher/src/lib.rs. Unrelated members/docs omitted.
        let source = r#"
pub struct Match { start: usize, end: usize }
impl Match {
    pub fn len(&self) -> usize {
        self.end - self.start
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
pub struct LineTerminator(LineTerminatorImp);
impl LineTerminator {
    pub fn is_crlf(&self) -> bool {
        self.0 == LineTerminatorImp::CRLF
    }
    pub fn is_suffix(&self, slice: &[u8]) -> bool {
        slice.last().map_or(false, |&b| b == self.as_byte())
    }
}
pub trait Captures {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
pub trait Matcher {
    fn is_match(&self, haystack: &[u8]) -> Result<bool, Self::Error> {
        self.is_match_at(haystack, 0)
    }
    fn is_match_at(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, Self::Error> {
        Ok(self.shortest_match_at(haystack, at)?.is_some())
    }
}
impl<'a, M: Matcher> Matcher for &'a M {
    fn is_match(&self, haystack: &[u8]) -> Result<bool, Self::Error> {
        (*self).is_match(haystack)
    }
    fn is_match_at(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, Self::Error> {
        (*self).is_match_at(haystack, at)
    }
}
"#;
        let metrics =
            crate::metrics::complexity::analyse_source(Path::new("lib.rs"), source).metrics;
        assert_eq!(
            metrics
                .functions
                .iter()
                .filter(|function| function.name.starts_with("is_"))
                .count(),
            8,
            "nonempty pinned extraction"
        );
        assert!(metrics
            .functions
            .iter()
            .all(|function| function.responsibility.is_some()));
        assert!(
            group_methods_by_prefix(&metrics.functions).is_empty(),
            "the previous is_* (8) crossed containing declarations and shared no direct dependency"
        );
    }

    /// Groups from a real extraction, as (owner label, prefix, names, evidence label).
    fn extracted_groups(
        path: &str,
        source: &str,
    ) -> Vec<(String, &'static str, Vec<String>, Option<String>)> {
        use crate::snapshot::ResponsibilityDependency;
        let metrics = crate::metrics::complexity::analyse_source(Path::new(path), source).metrics;
        assert!(!metrics.functions.is_empty(), "nonempty extraction: {path}");
        group_methods_by_prefix(&metrics.functions)
            .into_iter()
            .map(|group| {
                (
                    group.owner_label.to_owned(),
                    group.prefix,
                    group.names.iter().map(|name| (*name).to_owned()).collect(),
                    group.dependency.map(|dependency| match dependency {
                        ResponsibilityDependency::Field { label, .. }
                        | ResponsibilityDependency::Callee { label, .. } => label.clone(),
                    }),
                )
            })
            .collect()
    }

    #[test]
    fn same_named_members_of_one_owner_count_once() {
        let accessor =
            "class A { #r; get isReady(){ return this.#r } set isReady(v){ this.#r = v } }";
        assert_eq!(extracted_groups("src/a.js", accessor), vec![]);
        let overloads = "class A { Object state; boolean isValid() { return state != null; } boolean isValid(int limit) { return state != null && limit > 0; } }";
        assert_eq!(extracted_groups("src/A.java", overloads), vec![]);
        let with_sibling = "class A { #r; get isReady(){ return this.#r } set isReady(v){ this.#r = v } isReadyLater(){ return this.#r } }";
        assert_eq!(
            extracted_groups("src/a.js", with_sibling),
            vec![(
                "A (line 1)".to_owned(),
                "is_",
                vec!["isReady".to_owned(), "isReadyLater".to_owned()],
                Some("#r".to_owned()),
            )]
        );
    }

    #[test]
    fn a_local_named_like_a_field_does_not_complete_a_field_group() {
        let source = "class Cache { string value; Dictionary<string,string> store; bool IsLoaded() => value != null; bool IsFresh(string k) => store.TryGetValue(k, out var value) && value != null; }";
        assert_eq!(extracted_groups("src/Cache.cs", source), vec![]);
    }

    #[test]
    fn a_call_with_a_different_argument_count_is_not_shared_callee_evidence() {
        let mismatched = "class Widget extends Base { boolean ready; boolean check(){ return ready; } boolean isA(){ return check(); } boolean isB(){ return check(2); } }";
        assert_eq!(extracted_groups("src/Widget.java", mismatched), vec![]);
        let matched = "class Widget extends Base { boolean ready; boolean check(){ return ready; } boolean isA(){ return check(); } boolean isB(){ return check(); } }";
        assert_eq!(
            extracted_groups("src/Widget.java", matched),
            vec![(
                "Widget (line 1)".to_owned(),
                "is_",
                vec!["isA".to_owned(), "isB".to_owned()],
                Some("check".to_owned()),
            )]
        );
    }

    #[test]
    fn tagged_template_calls_are_not_shared_callee_evidence() {
        let source = "class A { tag(s){ return s } isA(){ return this.tag`${x}`; } isB(){ return this.tag`${y}`; } }";
        assert_eq!(extracted_groups("src/a.js", source), vec![]);
    }

    #[test]
    fn a_recursive_call_is_not_evidence_shared_with_its_caller() {
        let source = "fn is_sorted(v:&[i32])->bool{ v.len()<2 || (v[0]<=v[1] && is_sorted(&v[1..])) }\nfn is_strictly_sorted(v:&[i32])->bool{ is_sorted(v) && v.windows(2).all(|w| w[0]!=w[1]) }\n";
        assert_eq!(extracted_groups("src/lib.rs", source), vec![]);
    }

    #[test]
    fn go_methods_keep_their_group_when_the_receiver_type_lives_in_another_file() {
        let source = "package api\nfunc (s *Server) handleA() {}\nfunc (s *Server) handleB() {}\nfunc (s *Server) handleC() {}\n";
        assert_eq!(
            extracted_groups("src/handlers.go", source),
            vec![(
                "*Server".to_owned(),
                "handle_",
                vec![
                    "handleA".to_owned(),
                    "handleB".to_owned(),
                    "handleC".to_owned()
                ],
                None,
            )]
        );
    }

    #[test]
    fn test_functions_are_removed_before_group_minimum_counts_and_ranking() {
        let test = |name: &str| crate::snapshot::FunctionMetrics {
            is_test: true,
            ..fm(name)
        };
        let functions = vec![
            fm("render_a"),
            fm("render_b"),
            test("render_case"),
            fm("build_a"),
            test("build_case"),
        ];
        assert_eq!(
            group_methods_by_prefix(&functions),
            vec![expected_group("render_", vec!["render_a", "render_b"])]
        );
        let mut snapshot = crate::snapshot::RepoSnapshot::new(
            "/tmp".into(),
            "test".into(),
            "main".into(),
            crate::snapshot::TimeWindow::default(),
        );
        for (path, count) in [
            ("a.rs", 2),
            ("b.rs", 3),
            ("c.rs", 4),
            ("d.rs", 5),
            ("e.rs", 6),
            ("f.rs", 7),
        ] {
            let mut functions: Vec<_> = (0..count).map(|i| fm(&format!("render_{i}"))).collect();
            if path == "a.rs" {
                functions.extend((0..50).map(|i| test(&format!("render_test_{i}"))));
                functions.extend((0..50).map(|i| fm(&format!("has_unknown_{i}"))));
                functions.extend((0..50).map(|i| crate::snapshot::FunctionMetrics {
                    responsibility: None,
                    ..fm(&format!("build_unknown_{i}"))
                }));
            }
            snapshot.file_metrics.insert(
                path.into(),
                crate::snapshot::FileComplexity {
                    loc: 520,
                    cyclomatic_complexity: 1,
                    functions,
                    ..Default::default()
                },
            );
        }
        snapshot.file_metrics.insert(
            "all.rs".into(),
            crate::snapshot::FileComplexity {
                loc: 520,
                cyclomatic_complexity: 1,
                functions: (0..100).map(|i| test(&format!("render_{i}"))).collect(),
                ..Default::default()
            },
        );
        let flagged = crate::metrics::health::god_object_files(
            &snapshot,
            &crate::config::HealthThresholds::default(),
        );
        assert_eq!(flagged.len(), 7);
        let actions = generate_refactoring_actions(&snapshot, &flagged);
        assert_eq!(actions.len(), 5);
        for (action, path) in actions.iter().zip(["f.rs", "e.rs", "d.rs", "c.rs", "b.rs"]) {
            assert!(action.text.contains(path), "{}", action.text);
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
                expected_group("handle_", vec!["handle_a", "handle_b", "handle_c"]),
                expected_group("validate_", vec!["validate_x", "validate_y"]),
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
            vec![expected_group(
                "get_",
                vec!["getUserData", "getUserProfile"]
            )]
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
            vec![expected_group(
                "handle_",
                vec!["handleSubmit", "handle_click"]
            )]
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
            vec![expected_group(
                "get_",
                vec!["GetUserData", "GetUserProfile"]
            )]
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
