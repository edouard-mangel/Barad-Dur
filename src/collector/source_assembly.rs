//! Named intermediate records for source analysis, between the per-file
//! `SourceAnalysis` results and the final `RepoSnapshot`.
//!
//! Two readers feed these — the working tree and a commit's blobs — and
//! neither record is serialized: the snapshot's own fields remain the wire
//! and cache contract. Naming the channels makes adding one a matter of
//! adding a field, not of threading a new tuple position through both
//! readers.
use std::collections::HashMap;
use std::path::PathBuf;

use crate::metrics::complexity::{
    RawBaseRef, RawCallEdge, RawCalleeRef, RawClassRecord, RawReExport, RawReExportKind,
    SourceAnalysis,
};
use crate::snapshot::{
    BaseRef, CallRecord, CalleeRef, ClassRecord, CouplingFinding, FileComplexity, FileEntry,
    ReExportKind, ReExportRecord,
};

use super::composer::psr4_roots_from_tree;
use super::import_resolver::{
    index_import_files, resolve_imports, resolve_specifier, ImportFileIndex, RawImports,
    RepoImportConfig,
};

/// Everything a source pass collected, keyed by unresolved specifiers:
/// per-file metrics plus the raw import, class, re-export, and call
/// channels still waiting to be matched against the repository's files.
#[derive(Debug, Default)]
pub(crate) struct RawSourceChannels {
    pub file_metrics: HashMap<PathBuf, FileComplexity>,
    pub imports: RawImports,
    /// Sorted by (path, line); the only channel resolution leaves as is.
    pub coupling_findings: Vec<CouplingFinding>,
    pub classes: HashMap<PathBuf, Vec<RawClassRecord>>,
    pub reexports: HashMap<PathBuf, Vec<RawReExport>>,
    pub calls: HashMap<PathBuf, Vec<RawCallEdge>>,
}

/// The source channels in the exact shape the snapshot stores them.
/// `Default` is the AST-free collection (ADR-005 backfill): empty metrics
/// mean "not collected", never "clean" — `coupling::detection_ran` reads
/// that emptiness.
#[derive(Debug, Default)]
pub(crate) struct ResolvedSourceChannels {
    pub file_metrics: HashMap<PathBuf, FileComplexity>,
    pub import_graph: HashMap<PathBuf, Vec<PathBuf>>,
    /// Specifiers extracted from files with a known-unreliable resolver.
    pub unreliable_import_specifiers: usize,
    pub coupling_findings: Vec<CouplingFinding>,
    pub class_records: Vec<ClassRecord>,
    pub reexports: Vec<ReExportRecord>,
    pub call_records: Vec<CallRecord>,
}

impl RawSourceChannels {
    /// One aggregation policy for both readers: every analysed file gets
    /// a metrics entry, the other channels only when non-empty, and the
    /// findings are sorted by (path, line) so the reader's completion
    /// order — parallel live, sequential historical — cannot leak out.
    pub(crate) fn aggregate(analyses: impl IntoIterator<Item = (PathBuf, SourceAnalysis)>) -> Self {
        let mut raw = analyses
            .into_iter()
            .fold(Self::default(), |mut raw, (path, analysis)| {
                raw.file_metrics.insert(path.clone(), analysis.metrics);
                if !analysis.imports.is_empty() {
                    raw.imports.insert(path.clone(), analysis.imports);
                }
                if !analysis.class_records.is_empty() {
                    raw.classes.insert(path.clone(), analysis.class_records);
                }
                if !analysis.reexports.is_empty() {
                    raw.reexports.insert(path.clone(), analysis.reexports);
                }
                if !analysis.call_edges.is_empty() {
                    raw.calls.insert(path, analysis.call_edges);
                }
                raw.coupling_findings.extend(analysis.coupling_findings);
                raw
            });
        raw.coupling_findings
            .sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
        raw
    }

    /// Match every raw channel against the repository's files. Manifest
    /// content (PSR-4 roots) comes through `read_manifest`, which is the
    /// reader's provenance made explicit: the working tree reads disk, the
    /// historical pass reads the blob at its commit — never a hidden disk
    /// fallback.
    pub(crate) fn resolve(
        self,
        files: &[FileEntry],
        read_manifest: impl Fn(&FileEntry) -> Option<String>,
    ) -> ResolvedSourceChannels {
        let import_config = RepoImportConfig {
            psr4: psr4_roots_from_tree(files, read_manifest),
        };
        ResolvedSourceChannels {
            file_metrics: self.file_metrics,
            import_graph: resolve_imports(&self.imports, files, &import_config),
            unreliable_import_specifiers: count_unreliable_specifiers(&self.imports),
            coupling_findings: self.coupling_findings,
            class_records: resolve_class_records(self.classes, files),
            reexports: resolve_reexports(self.reexports, files),
            call_records: resolve_call_records(self.calls, files),
        }
    }
}

/// Shared skeleton of the three raw→snapshot resolvers: build the known
/// file-set once, map every raw item (with `resolve_specifier` access via
/// the known set), and sort deterministically. `map` returning `None`
/// drops the item (re-exports drop unresolvable specifiers; the other
/// resolvers never drop).
fn resolve_against_files<R, T, K: Ord>(
    raw: HashMap<PathBuf, Vec<R>>,
    files: &[FileEntry],
    map: impl Fn(&PathBuf, R, &ImportFileIndex<'_>) -> Option<T>,
    sort_key: impl Fn(&T) -> K,
) -> Vec<T> {
    let known = index_import_files(files.iter().map(|f| &f.path));
    let mut records: Vec<T> = raw
        .into_iter()
        .flat_map(|(path, items)| {
            items
                .into_iter()
                .filter_map(|item| map(&path, item, &known))
                .collect::<Vec<_>>()
        })
        .collect();
    records.sort_by_key(sort_key);
    records
}

/// Resolve raw re-export specifiers against the repo's file set, producing
/// the snapshot's `reexports` (sorted by path). Unresolvable specifiers
/// (external packages) are dropped — they can't lead to a project-local
/// class record.
fn resolve_reexports(
    raw: HashMap<PathBuf, Vec<RawReExport>>,
    files: &[FileEntry],
) -> Vec<ReExportRecord> {
    resolve_against_files(
        raw,
        files,
        |path, r, known| {
            let target =
                resolve_specifier(&r.specifier, path, known, &RepoImportConfig::default())?;
            let kind = match r.kind {
                RawReExportKind::Named { exported, source } => {
                    ReExportKind::Named { exported, source }
                }
                RawReExportKind::Star => ReExportKind::Star,
            };
            Some(ReExportRecord {
                path: path.clone(),
                target,
                kind,
            })
        },
        |r| (r.path.clone(), r.target.clone()),
    )
}

/// Resolve raw call edges' import specifiers against the repo's file set,
/// producing the snapshot's `call_records` (sorted by path, caller, callee).
/// An unresolvable specifier (external package) becomes `Unresolved` —
/// kept, never dropped, so unresolved calls stay countable (design §4).
fn resolve_call_records(
    raw: HashMap<PathBuf, Vec<RawCallEdge>>,
    files: &[FileEntry],
) -> Vec<CallRecord> {
    resolve_against_files(
        raw,
        files,
        |path, e, known| {
            let callee = match e.callee {
                RawCalleeRef::SameFile(name) => CalleeRef::SameFile(name),
                RawCalleeRef::Unresolved { name } => CalleeRef::Unresolved { name },
                RawCalleeRef::Specifier { specifier, name } => {
                    match resolve_specifier(&specifier, path, known, &RepoImportConfig::default()) {
                        Some(target) => CalleeRef::Resolved { path: target, name },
                        None => CalleeRef::Unresolved { name },
                    }
                }
            };
            Some(CallRecord {
                path: path.clone(),
                caller: e.caller,
                callee,
                count: e.count,
            })
        },
        |r| (r.path.clone(), r.caller.clone(), r.callee.clone()),
    )
}

/// Count the specifiers that came from files whose resolver is known to be
/// wrong. A zero here means every extracted specifier came from a resolver
/// we trust, so an empty graph is evidence rather than a blind spot.
fn count_unreliable_specifiers(raw_imports: &RawImports) -> usize {
    raw_imports
        .iter()
        .filter(|(path, _)| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(super::resolver_is_unreliable)
        })
        .map(|(_, specifiers)| specifiers.len())
        .sum()
}

/// Resolve raw class records' import specifiers against the repo's file
/// set, producing the snapshot's `class_records` (sorted by path, line).
fn resolve_class_records(
    raw: HashMap<PathBuf, Vec<RawClassRecord>>,
    files: &[FileEntry],
) -> Vec<ClassRecord> {
    resolve_against_files(
        raw,
        files,
        |path, r, known| {
            let base = match r.base {
                RawBaseRef::SameFile(name) => BaseRef::SameFile(name),
                RawBaseRef::Unresolvable => BaseRef::Unresolvable,
                RawBaseRef::Specifier { specifier, name } => {
                    match resolve_specifier(&specifier, path, known, &RepoImportConfig::default()) {
                        Some(target) => BaseRef::Resolved { path: target, name },
                        None => BaseRef::Unresolvable,
                    }
                }
            };
            Some(ClassRecord {
                path: path.clone(),
                line: r.line,
                class_name: r.class_name,
                base,
            })
        },
        |r| (r.path.clone(), r.line),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::complexity::{RawBaseRef, RawCalleeRef, SourceAnalysis};
    use crate::snapshot::CouplingKind;
    use std::path::Path;

    fn finding(path: &str, line: usize) -> CouplingFinding {
        CouplingFinding {
            path: path.into(),
            line: Some(line),
            kind: CouplingKind::Common,
            evidence: format!("{path}:{line}"),
        }
    }

    #[test]
    fn aggregate_keeps_only_non_empty_channels_and_sorts_findings() {
        // Input deliberately out of order and with one channel empty per
        // file: the aggregation must not depend on reader order and must
        // not manufacture empty entries.
        let b = SourceAnalysis {
            metrics: FileComplexity::default(),
            imports: vec![],
            coupling_findings: vec![finding("src/b.ts", 7), finding("src/b.ts", 2)],
            class_records: vec![RawClassRecord {
                line: 1,
                class_name: "B".into(),
                base: RawBaseRef::Unresolvable,
            }],
            reexports: vec![],
            call_edges: vec![],
        };
        let a = SourceAnalysis {
            metrics: FileComplexity::default(),
            imports: vec!["./b".into()],
            coupling_findings: vec![finding("src/a.ts", 9)],
            class_records: vec![],
            reexports: vec![],
            call_edges: vec![RawCallEdge {
                caller: "f".into(),
                callee: RawCalleeRef::SameFile("g".into()),
                count: 1,
            }],
        };
        let raw = RawSourceChannels::aggregate([
            (PathBuf::from("src/b.ts"), b),
            (PathBuf::from("src/a.ts"), a),
        ]);
        assert_eq!(raw.file_metrics.len(), 2);
        assert_eq!(
            raw.imports.keys().collect::<Vec<_>>(),
            [&PathBuf::from("src/a.ts")]
        );
        assert_eq!(
            raw.classes.keys().collect::<Vec<_>>(),
            [&PathBuf::from("src/b.ts")]
        );
        assert_eq!(
            raw.calls.keys().collect::<Vec<_>>(),
            [&PathBuf::from("src/a.ts")]
        );
        assert!(raw.reexports.is_empty());
        let order: Vec<(String, Option<usize>)> = raw
            .coupling_findings
            .iter()
            .map(|f| (f.path.display().to_string(), f.line))
            .collect();
        assert_eq!(
            order,
            [
                ("src/a.ts".to_string(), Some(9)),
                ("src/b.ts".to_string(), Some(2)),
                ("src/b.ts".to_string(), Some(7)),
            ]
        );
    }

    #[test]
    fn resolve_reads_manifests_through_the_supplied_reader_only() {
        use crate::metrics::testutil::make_file;
        let files = vec![
            make_file("api/composer.json"),
            make_file("api/app/Foo.php"),
            make_file("api/app/Bar.php"),
            make_file("cmd/x.go"),
        ];
        let raw = || RawSourceChannels {
            imports: [
                (
                    PathBuf::from("api/app/Foo.php"),
                    vec!["App\\Bar".to_string()],
                ),
                (PathBuf::from("cmd/x.go"), vec!["fmt".to_string()]),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        let manifest = "{\"autoload\":{\"psr-4\":{\"App\\\\\":\"app/\"}}}";
        let with = raw().resolve(&files, |entry| {
            (entry.path.ends_with("composer.json")).then(|| manifest.to_string())
        });
        assert_eq!(
            with.import_graph.get(Path::new("api/app/Foo.php")),
            Some(&vec![PathBuf::from("api/app/Bar.php")])
        );
        assert_eq!(with.unreliable_import_specifiers, 1);

        let without = raw().resolve(&files, |_| None);
        assert_eq!(
            without.import_graph.get(Path::new("api/app/Foo.php")),
            None,
            "no manifest, no PSR-4 root, no edge — never a disk fallback"
        );
        assert_eq!(without.unreliable_import_specifiers, 1);
    }

    #[test]
    fn only_unreliable_resolvers_contribute_to_the_specifier_count() {
        // The guard that blanks import metrics reads this number. Counting
        // every specifier would let a working language's external-only
        // imports trip it; counting none would let C# and Go score a
        // perfect 100 on a repository nobody can measure.
        let raw: RawImports = [
            (
                PathBuf::from("src/Domain.cs"),
                vec!["System.Linq".to_string(), "Acme.Core".to_string()],
            ),
            (PathBuf::from("cmd/main.go"), vec!["fmt".to_string()]),
            (
                PathBuf::from("src/lib.rs"),
                vec!["serde".to_string(), "std::fmt".to_string()],
            ),
            (PathBuf::from("src/app.ts"), vec!["react".to_string()]),
        ]
        .into_iter()
        .collect();

        assert_eq!(count_unreliable_specifiers(&raw), 3);
    }

    #[test]
    fn a_repository_without_an_unreliable_resolver_counts_none() {
        let raw: RawImports = [(
            PathBuf::from("src/app.ts"),
            vec!["react".to_string(), "./local".to_string()],
        )]
        .into_iter()
        .collect();

        assert_eq!(count_unreliable_specifiers(&raw), 0);
    }

    #[test]
    fn resolve_call_records_resolves_specifiers_keeps_unresolved_and_sorts() {
        use crate::metrics::complexity::{RawCallEdge, RawCalleeRef};
        use crate::snapshot::CalleeRef;
        let files = vec![
            crate::metrics::testutil::make_file("src/a.ts"),
            crate::metrics::testutil::make_file("src/b.ts"),
        ];
        let mut raw = HashMap::new();
        raw.insert(
            PathBuf::from("src/b.ts"),
            vec![
                RawCallEdge {
                    caller: "g".into(),
                    callee: RawCalleeRef::Specifier {
                        specifier: "react".into(),
                        name: "useState".into(),
                    },
                    count: 2,
                },
                RawCallEdge {
                    caller: "f".into(),
                    callee: RawCalleeRef::Specifier {
                        specifier: "./a".into(),
                        name: "helper".into(),
                    },
                    count: 3,
                },
            ],
        );
        let records = resolve_call_records(raw, &files);
        assert_eq!(records.len(), 2);
        // Sorted by (path, caller): f before g.
        assert_eq!(records[0].caller, "f");
        assert_eq!(records[0].count, 3);
        assert_eq!(
            records[0].callee,
            CalleeRef::Resolved {
                path: "src/a.ts".into(),
                name: "helper".into()
            }
        );
        // External package: kept as Unresolved for honest accounting —
        // never dropped (unlike class records) and never Resolved.
        assert_eq!(records[1].caller, "g");
        assert_eq!(records[1].count, 2);
        assert_eq!(
            records[1].callee,
            CalleeRef::Unresolved {
                name: "useState".into()
            }
        );
    }

    #[test]
    fn resolve_call_records_sort_uses_every_key_component() {
        use crate::metrics::complexity::{RawCallEdge, RawCalleeRef};
        use crate::snapshot::CalleeRef;
        let files = vec![
            crate::metrics::testutil::make_file("src/a.ts"),
            crate::metrics::testutil::make_file("src/b.ts"),
            crate::metrics::testutil::make_file("src/c.ts"),
            crate::metrics::testutil::make_file("src/z.ts"),
        ];
        let edge = |callee: RawCalleeRef| RawCallEdge {
            caller: "f".into(),
            callee,
            count: 1,
        };
        let spec = |s: &str, n: &str| RawCalleeRef::Specifier {
            specifier: s.into(),
            name: n.into(),
        };
        let mut raw = HashMap::new();
        // Cross-file ordering must come from the sort, not HashMap luck.
        raw.insert(
            PathBuf::from("src/z.ts"),
            vec![edge(RawCalleeRef::SameFile("a".into()))],
        );
        // Same (path, caller) throughout, input deliberately in REVERSE of
        // the expected order: `sort_by` is stable, so a degenerate
        // `callee_sort_key` (any constant) would keep this order and fail.
        raw.insert(
            PathBuf::from("src/a.ts"),
            vec![
                edge(RawCalleeRef::Unresolved { name: "z".into() }),
                edge(spec("./c", "m")),
                edge(spec("./b", "x")),
                edge(spec("./b", "a")),
                edge(RawCalleeRef::SameFile("z".into())),
                edge(RawCalleeRef::SameFile("a".into())),
            ],
        );
        let callees: Vec<(String, CalleeRef)> = resolve_call_records(raw, &files)
            .into_iter()
            .map(|r| (r.path.display().to_string(), r.callee))
            .collect();
        let resolved = |p: &str, n: &str| CalleeRef::Resolved {
            path: p.into(),
            name: n.into(),
        };
        assert_eq!(
            callees,
            vec![
                // variant rank first: SameFile < Resolved < Unresolved,
                // then path, then name within a path.
                ("src/a.ts".into(), CalleeRef::SameFile("a".into())),
                ("src/a.ts".into(), CalleeRef::SameFile("z".into())),
                ("src/a.ts".into(), resolved("src/b.ts", "a")),
                ("src/a.ts".into(), resolved("src/b.ts", "x")),
                ("src/a.ts".into(), resolved("src/c.ts", "m")),
                (
                    "src/a.ts".into(),
                    CalleeRef::Unresolved { name: "z".into() }
                ),
                ("src/z.ts".into(), CalleeRef::SameFile("a".into())),
            ]
        );
    }

    #[test]
    fn resolve_class_records_resolves_specifiers_and_sorts() {
        use crate::metrics::complexity::{RawBaseRef, RawClassRecord};
        use crate::snapshot::BaseRef;
        let files = vec![
            crate::metrics::testutil::make_file("src/a.ts"),
            crate::metrics::testutil::make_file("src/b.ts"),
        ];
        let mut raw = HashMap::new();
        raw.insert(
            PathBuf::from("src/b.ts"),
            vec![
                RawClassRecord {
                    line: 9,
                    class_name: "X".into(),
                    base: RawBaseRef::Specifier {
                        specifier: "react".into(),
                        name: "Component".into(),
                    },
                },
                RawClassRecord {
                    line: 2,
                    class_name: "B".into(),
                    base: RawBaseRef::Specifier {
                        specifier: "./a".into(),
                        name: "A".into(),
                    },
                },
            ],
        );
        let records = resolve_class_records(raw, &files);
        assert_eq!(records.len(), 2);
        // sorted by (path, line): B (line 2) before X (line 9)
        assert_eq!(records[0].class_name, "B");
        assert_eq!(
            records[0].base,
            BaseRef::Resolved {
                path: "src/a.ts".into(),
                name: "A".into()
            }
        );
        assert_eq!(
            records[1].base,
            BaseRef::Unresolvable,
            "external package must not resolve"
        );
    }
}
