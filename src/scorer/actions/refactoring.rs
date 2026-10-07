use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::scorer::types::{ActionItem, ReportTab, SortKey};

// No entry may be a prefix of another — `.find()` returns the first match,
// so overlapping entries would make grouping order-dependent.
pub(super) const GROUPING_PREFIXES: &[&str] = &[
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
pub(super) fn matches_group_prefix(name: &str, prefix_with_underscore: &str) -> bool {
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
pub(super) struct MethodGroup<'a> {
    pub(super) owner_id: &'a str,
    pub(super) owner_label: &'a str,
    pub(super) prefix: &'static str,
    pub(super) names: Vec<&'a str>,
    pub(super) dependency: Option<&'a crate::snapshot::ResponsibilityDependency>,
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
pub(super) fn group_methods_by_prefix(
    functions: &[crate::snapshot::FunctionMetrics],
) -> MethodGroups<'_> {
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
pub(in crate::scorer) fn generate_refactoring_actions(
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
                target_tab: Some(ReportTab::Hotspots),
                sort_by: Some(SortKey::Complexity),
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
