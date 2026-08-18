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
