//! Inheritance-coupling depth (M7): pure, memoized DFS over the snapshot's
//! class records. Depth (DIT) counts only project-local edges; unresolvable
//! and external bases terminate a chain, cycles are cut.

use std::collections::HashMap;

use crate::metrics::reexport::{reexport_index, resolve_symbol, ReExportIndex, SymbolKey};
use crate::snapshot::{BaseRef, ClassRecord, CouplingFinding, CouplingKind, RepoSnapshot};

type Key<'a> = SymbolKey<'a>;
type ByKey<'a> = HashMap<Key<'a>, &'a ClassRecord>;
type RxIndex<'a> = ReExportIndex<'a>;

/// Every class whose project-local inheritance depth reaches `min_depth`,
/// as an Inheritance finding. `min_depth == 0` disables the rule.
pub(crate) fn inheritance_findings(
    snapshot: &RepoSnapshot,
    min_depth: usize,
) -> Vec<CouplingFinding> {
    if min_depth == 0 {
        return Vec::new();
    }
    let by_key: ByKey<'_> = snapshot
        .class_records
        .iter()
        .map(|r| ((&r.path, r.class_name.as_str()), r))
        .collect();
    let rx: RxIndex<'_> = reexport_index(&snapshot.reexports);
    let mut memo: HashMap<Key<'_>, usize> = HashMap::new();
    snapshot
        .class_records
        .iter()
        .filter_map(|r| {
            let key = (&r.path, r.class_name.as_str());
            let depth = depth_of(key, &by_key, &rx, &mut memo, &mut Vec::new());
            (depth >= min_depth).then(|| CouplingFinding {
                path: r.path.clone(),
                line: Some(r.line),
                kind: CouplingKind::Inheritance,
                evidence: evidence(r, depth, &by_key, &rx),
            })
        })
        .collect()
}

/// Follow barrel re-exports until `key` names an actual class record —
/// the shared `metrics::reexport` chase with "declares" = "has a class
/// record" (see design D6).
fn resolve_key<'a>(
    key: Key<'a>,
    by_key: &ByKey<'a>,
    rx: &RxIndex<'a>,
    visited: &mut Vec<Key<'a>>,
) -> Option<Key<'a>> {
    resolve_symbol(key, &|k| by_key.contains_key(&k), rx, visited)
}

fn parent_key<'a>(rec: &'a ClassRecord) -> Option<Key<'a>> {
    match &rec.base {
        BaseRef::SameFile(name) => Some((&rec.path, name.as_str())),
        BaseRef::Resolved { path, name } => Some((path, name.as_str())),
        BaseRef::Unresolvable => None,
    }
}

/// Depth = number of named, project-visible ancestors. A named base with no
/// record of its own (a plain root class) counts as one ancestor; an
/// unresolvable base counts zero; a cycle is cut before re-entering an
/// in-progress class. Memoized — diamonds cost each ancestor once.
fn depth_of<'a>(
    key: Key<'a>,
    by_key: &ByKey<'a>,
    rx: &RxIndex<'a>,
    memo: &mut HashMap<Key<'a>, usize>,
    in_progress: &mut Vec<Key<'a>>,
) -> usize {
    if let Some(&d) = memo.get(&key) {
        return d;
    }
    let Some(rec) = by_key.get(&key) else {
        return 0; // no record: a class without `extends` — chain root
    };
    let d = match parent_key(rec).map(|pk| {
        // A base resolved to a barrel file has no record there — follow
        // the barrel's re-exports to the declaring file.
        resolve_key(pk, by_key, rx, &mut Vec::new()).unwrap_or(pk)
    }) {
        None => 0, // unresolvable base: the ancestor cannot be named
        Some(pk) if in_progress.contains(&pk) => 0, // cycle: cut before the edge
        Some(pk) if by_key.contains_key(&pk) => {
            in_progress.push(key);
            let parent_depth = depth_of(pk, by_key, rx, memo, in_progress);
            in_progress.pop();
            parent_depth + 1
        }
        Some(_) => 1, // named base without a record: one countable ancestor
    };
    memo.insert(key, d);
    d
}

/// `class C extends B → A (depth 2)` — the named ancestor chain, cycle-safe.
fn evidence<'a>(
    rec: &'a ClassRecord,
    depth: usize,
    by_key: &ByKey<'a>,
    rx: &RxIndex<'a>,
) -> String {
    let mut names: Vec<String> = Vec::new();
    let mut seen: Vec<Key<'_>> = vec![(&rec.path, rec.class_name.as_str())];
    let mut cur = parent_key(rec);
    while let Some(k) = cur {
        let k = resolve_key(k, by_key, rx, &mut Vec::new()).unwrap_or(k);
        if seen.contains(&k) {
            break;
        }
        names.push(k.1.to_string());
        seen.push(k);
        cur = by_key.get(&k).and_then(|r| parent_key(r));
    }
    format!(
        "class {} extends {} (depth {})",
        rec.class_name,
        names.join(" → "),
        depth
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn record(path: &str, line: usize, name: &str, base: BaseRef) -> ClassRecord {
        ClassRecord {
            path: path.into(),
            line,
            class_name: name.into(),
            base,
        }
    }

    fn resolved(path: &str, name: &str) -> BaseRef {
        BaseRef::Resolved {
            path: path.into(),
            name: name.into(),
        }
    }

    fn snap(records: Vec<ClassRecord>) -> RepoSnapshot {
        let mut s = crate::metrics::testutil::make_snapshot();
        s.class_records = records;
        s
    }

    fn chain_abc() -> Vec<ClassRecord> {
        vec![
            record("src/b.ts", 2, "B", resolved("src/a.ts", "A")),
            record("src/c.ts", 2, "C", resolved("src/b.ts", "B")),
        ]
    }

    #[test]
    fn depth_one_is_not_flagged() {
        let s = snap(vec![record("src/b.ts", 2, "B", resolved("src/a.ts", "A"))]);
        assert!(inheritance_findings(&s, 2).is_empty());
    }

    #[test]
    fn depth_two_is_flagged_with_line_and_chain_evidence() {
        let f = inheritance_findings(&snap(chain_abc()), 2);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].path, PathBuf::from("src/c.ts"));
        assert_eq!(f[0].line, Some(2));
        assert_eq!(f[0].kind, CouplingKind::Inheritance);
        assert_eq!(f[0].evidence, "class C extends B → A (depth 2)");
    }

    #[test]
    fn same_file_chain_counts() {
        let s = snap(vec![
            record("src/x.ts", 2, "B", BaseRef::SameFile("A".into())),
            record("src/x.ts", 3, "C", BaseRef::SameFile("B".into())),
        ]);
        assert_eq!(inheritance_findings(&s, 2).len(), 1);
    }

    #[test]
    fn unresolvable_base_terminates_chain() {
        // B extends mixin(...) — B's ancestor can't be named, so C's chain
        // is C → B, depth 1: not flagged.
        let s = snap(vec![
            record("src/b.ts", 2, "B", BaseRef::Unresolvable),
            record("src/c.ts", 2, "C", resolved("src/b.ts", "B")),
        ]);
        assert!(inheritance_findings(&s, 2).is_empty());
    }

    #[test]
    fn cycle_is_cut_without_hang_and_without_findings() {
        let s = snap(vec![
            record("src/a.ts", 1, "A", BaseRef::SameFile("B".into())),
            record("src/a.ts", 2, "B", BaseRef::SameFile("A".into())),
        ]);
        assert!(inheritance_findings(&s, 2).is_empty());
    }

    #[test]
    fn diamond_shares_memoized_ancestors() {
        // C and D both extend B (which extends A): both depth 2, flagged.
        let s = snap(vec![
            record("src/b.ts", 2, "B", resolved("src/a.ts", "A")),
            record("src/c.ts", 2, "C", resolved("src/b.ts", "B")),
            record("src/d.ts", 2, "D", resolved("src/b.ts", "B")),
        ]);
        assert_eq!(inheritance_findings(&s, 2).len(), 2);
    }

    #[test]
    fn every_qualifying_class_is_flagged_independently() {
        let mut records = chain_abc();
        records.push(record("src/d.ts", 2, "D", resolved("src/c.ts", "C")));
        // C is depth 2, D is depth 3 — both qualify at threshold 2.
        assert_eq!(inheritance_findings(&snap(records), 2).len(), 2);
    }

    #[test]
    fn min_depth_zero_disables() {
        assert!(inheritance_findings(&snap(chain_abc()), 0).is_empty());
    }

    // ── Barrel re-export resolution ────────────────────────────────

    use crate::snapshot::{ReExportKind, ReExportRecord};

    fn snap_with_reexports(
        records: Vec<ClassRecord>,
        reexports: Vec<ReExportRecord>,
    ) -> RepoSnapshot {
        let mut s = snap(records);
        s.reexports = reexports;
        s
    }

    fn named_rx(path: &str, target: &str, exported: &str, source: &str) -> ReExportRecord {
        ReExportRecord {
            path: path.into(),
            target: target.into(),
            kind: ReExportKind::Named {
                exported: exported.into(),
                source: source.into(),
            },
        }
    }

    fn star_rx(path: &str, target: &str) -> ReExportRecord {
        ReExportRecord {
            path: path.into(),
            target: target.into(),
            kind: ReExportKind::Star,
        }
    }

    #[test]
    fn named_barrel_reexport_is_followed() {
        // C extends B, imported through the barrel: index.ts re-exports B
        // from src/b.ts, where B extends A — real depth 2.
        let s = snap_with_reexports(
            vec![
                record("src/b.ts", 2, "B", resolved("src/a.ts", "A")),
                record("src/c.ts", 2, "C", resolved("src/index.ts", "B")),
            ],
            vec![named_rx("src/index.ts", "src/b.ts", "B", "B")],
        );
        let f = inheritance_findings(&s, 2);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].path, PathBuf::from("src/c.ts"));
        assert_eq!(f[0].evidence, "class C extends B → A (depth 2)");
    }

    #[test]
    fn aliased_barrel_reexport_maps_exported_to_source_name() {
        // index.ts: export { B as Base } from './b' — C extends Base.
        let s = snap_with_reexports(
            vec![
                record("src/b.ts", 2, "B", resolved("src/a.ts", "A")),
                record("src/c.ts", 2, "C", resolved("src/index.ts", "Base")),
            ],
            vec![named_rx("src/index.ts", "src/b.ts", "Base", "B")],
        );
        assert_eq!(inheritance_findings(&s, 2).len(), 1);
    }

    #[test]
    fn star_barrel_reexport_is_followed() {
        let s = snap_with_reexports(
            vec![
                record("src/b.ts", 2, "B", resolved("src/a.ts", "A")),
                record("src/c.ts", 2, "C", resolved("src/index.ts", "B")),
            ],
            vec![star_rx("src/index.ts", "src/b.ts")],
        );
        assert_eq!(inheritance_findings(&s, 2).len(), 1);
    }

    #[test]
    fn barrel_of_barrels_is_followed_transitively() {
        // c.ts imports through src/index.ts, which star-re-exports
        // src/models/index.ts, which named-re-exports B from src/b.ts.
        let s = snap_with_reexports(
            vec![
                record("src/b.ts", 2, "B", resolved("src/a.ts", "A")),
                record("src/c.ts", 2, "C", resolved("src/index.ts", "B")),
            ],
            vec![
                star_rx("src/index.ts", "src/models/index.ts"),
                named_rx("src/models/index.ts", "src/b.ts", "B", "B"),
            ],
        );
        assert_eq!(inheritance_findings(&s, 2).len(), 1);
    }

    #[test]
    fn cyclic_barrels_terminate_without_hang() {
        // Two barrels star-re-exporting each other; the name never lands
        // on a class record. Chain stays at depth 1 — no finding, no hang.
        let s = snap_with_reexports(
            vec![record("src/c.ts", 2, "C", resolved("src/index.ts", "B"))],
            vec![
                star_rx("src/index.ts", "src/other.ts"),
                star_rx("src/other.ts", "src/index.ts"),
            ],
        );
        assert!(inheritance_findings(&s, 2).is_empty());
    }

    #[test]
    fn unmatched_barrel_name_still_counts_one_ancestor() {
        // The barrel doesn't re-export "B": behave exactly like a named
        // base without a record (depth 1), not like an error.
        let s = snap_with_reexports(
            vec![
                record("src/b.ts", 2, "B", resolved("src/a.ts", "A")),
                record("src/c.ts", 2, "C", resolved("src/index.ts", "B")),
            ],
            vec![named_rx("src/index.ts", "src/other.ts", "X", "X")],
        );
        assert!(inheritance_findings(&s, 2).is_empty());
    }

    #[test]
    fn min_depth_three_raises_the_bar() {
        let mut records = chain_abc();
        records.push(record("src/d.ts", 2, "D", resolved("src/c.ts", "C")));
        let f = inheritance_findings(&snap(records), 3);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].path, PathBuf::from("src/d.ts"));
        assert_eq!(f[0].evidence, "class D extends C → B → A (depth 3)");
    }
}
