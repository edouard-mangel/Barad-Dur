//! Barrel re-export chasing, shared across metric categories (design D6):
//! follow `export … from` chains until a (file, symbol) key lands on a file
//! that actually declares the symbol. Consumers: inheritance depth (DIT,
//! coupling) and call-graph target resolution. Pure — cycle-safe, no I/O.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::snapshot::{ReExportKind, ReExportRecord};

/// A project-local symbol reference: (declaring-or-barrel file, name).
pub(crate) type SymbolKey<'a> = (&'a PathBuf, &'a str);

/// Re-export records grouped by their (barrel) file.
pub(crate) type ReExportIndex<'a> = HashMap<&'a PathBuf, Vec<&'a ReExportRecord>>;

/// Group a snapshot's re-export records by barrel file for chasing.
pub(crate) fn reexport_index(reexports: &[ReExportRecord]) -> ReExportIndex<'_> {
    reexports.iter().fold(HashMap::new(), |mut m, r| {
        m.entry(&r.path).or_default().push(r);
        m
    })
}

/// Follow barrel re-exports until `key` names a symbol its file actually
/// declares (per `declares`): a named re-export matching the looked-up
/// name hops to (target, source); a star re-export hops to (target, same
/// name). Returns `None` when no chain of hops lands on a declaration
/// (cycles cut via `visited`).
pub(crate) fn resolve_symbol<'a>(
    key: SymbolKey<'a>,
    declares: &dyn Fn(SymbolKey<'a>) -> bool,
    rx: &ReExportIndex<'a>,
    visited: &mut Vec<SymbolKey<'a>>,
) -> Option<SymbolKey<'a>> {
    if declares(key) {
        return Some(key);
    }
    if visited.contains(&key) {
        return None;
    }
    visited.push(key);
    let recs = rx.get(key.0)?;
    recs.iter()
        .find_map(|r| match &r.kind {
            ReExportKind::Named { exported, source } if exported.as_str() == key.1 => {
                resolve_symbol((&r.target, source.as_str()), declares, rx, visited)
            }
            _ => None,
        })
        .or_else(|| {
            recs.iter()
                .filter(|r| matches!(r.kind, ReExportKind::Star))
                .find_map(|r| resolve_symbol((&r.target, key.1), declares, rx, visited))
        })
}

/// Follow *named* re-exports for `key` to their terminal file, without
/// requiring any declaration check: a `export { X } from './y'` entry is
/// an explicit fact worth following even when the symbol has no
/// `FunctionMetrics` identity (arrow-const exports, classes — review F4).
/// Star re-exports are NOT followed here: without a declaration to
/// confirm, a star hop is a guess. Cycles cut; returns the last key
/// reached (the input itself when nothing matches).
pub(crate) fn chase_named<'a>(key: SymbolKey<'a>, rx: &ReExportIndex<'a>) -> SymbolKey<'a> {
    let mut visited: Vec<SymbolKey<'a>> = Vec::new();
    let mut current = key;
    loop {
        if visited.contains(&current) {
            return current;
        }
        visited.push(current);
        let hop = rx.get(current.0).and_then(|recs| {
            recs.iter().find_map(|r| match &r.kind {
                ReExportKind::Named { exported, source } if exported.as_str() == current.1 => {
                    Some((&r.target, source.as_str()))
                }
                _ => None,
            })
        });
        match hop {
            Some(next) => current = next,
            None => return current,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chase_named_follows_explicit_reexports_without_declarations() {
        // Review F4: arrow-const exports have no FunctionMetrics identity,
        // so the declares-based chase dead-ends — but a Named re-export is
        // an explicit fact we can follow to the terminal file regardless.
        let records = vec![ReExportRecord {
            path: "src/index.ts".into(),
            target: "src/use_fetch.ts".into(),
            kind: ReExportKind::Named {
                exported: "useFetch".into(),
                source: "useFetch".into(),
            },
        }];
        let rx = reexport_index(&records);
        let barrel = PathBuf::from("src/index.ts");
        let terminal = PathBuf::from("src/use_fetch.ts");
        assert_eq!(
            chase_named((&barrel, "useFetch"), &rx),
            (&terminal, "useFetch")
        );
    }

    #[test]
    fn chase_named_stops_at_cycles_and_unmatched_names() {
        let records = vec![
            ReExportRecord {
                path: "src/a.ts".into(),
                target: "src/b.ts".into(),
                kind: ReExportKind::Named {
                    exported: "X".into(),
                    source: "X".into(),
                },
            },
            ReExportRecord {
                path: "src/b.ts".into(),
                target: "src/a.ts".into(),
                kind: ReExportKind::Named {
                    exported: "X".into(),
                    source: "X".into(),
                },
            },
        ];
        let rx = reexport_index(&records);
        let a = PathBuf::from("src/a.ts");
        let c = PathBuf::from("src/c.ts");
        // Cycle: terminates without hanging, lands somewhere in the cycle.
        let (_, name) = chase_named((&a, "X"), &rx);
        assert_eq!(name, "X");
        // No matching entry anywhere: key returned unchanged.
        assert_eq!(chase_named((&c, "Y"), &rx), (&c, "Y"));
    }

    #[test]
    fn named_hop_requires_matching_exported_name() {
        // The barrel forwards Y as X — looking up "B" must NOT follow that
        // entry, even though blindly hopping would land on a declared
        // symbol (kills the `exported == key` guard-to-true mutant).
        let records = vec![ReExportRecord {
            path: "src/index.ts".into(),
            target: "src/b.ts".into(),
            kind: ReExportKind::Named {
                exported: "X".into(),
                source: "Y".into(),
            },
        }];
        let rx = reexport_index(&records);
        let b = PathBuf::from("src/b.ts");
        let declares = move |k: SymbolKey<'_>| *k.0 == b && k.1 == "Y";
        let barrel = PathBuf::from("src/index.ts");
        assert_eq!(
            resolve_symbol((&barrel, "B"), &declares, &rx, &mut Vec::new()),
            None,
            "a named re-export of a different name must not be followed"
        );
    }
}
