//! Class-record extraction for the inheritance-coupling rung (M7):
//! per-file `class … extends …` facts. Pure — no I/O, no hierarchy
//! resolution (that happens at metric time in `metrics/coupling`).

use std::collections::HashMap;
use std::path::Path;

use tree_sitter::Node;

use super::fallback::{detect_language, Language};
use super::lang_dispatch::grammar_for;
use super::pressman::descendants;
use super::treesitter::parse;

/// A `class … extends …` site, before import-specifier resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct RawClassRecord {
    /// 1-based declaration line.
    pub line: usize,
    pub class_name: String,
    pub base: RawBaseRef,
}

/// The extends-target as extraction sees it. `Specifier` is resolved to a
/// repo path (or `Unresolvable`) by the collector's snapshot builder.
#[derive(Debug, Clone, PartialEq)]
pub enum RawBaseRef {
    /// Base identifier not bound by any import — assumed same-file.
    SameFile(String),
    /// Base bound by an import: module specifier + exported name
    /// (aliases unwrapped: `import { A as B }` records name "A").
    Specifier { specifier: String, name: String },
    /// Non-identifier extends expression (`extends mixin(Base)`);
    /// terminates depth counting.
    Unresolvable,
}

/// A re-export site as extraction sees it, before specifier resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct RawReExport {
    pub specifier: String,
    pub kind: RawReExportKind,
}

/// Mirrors `snapshot::ReExportKind`, pre-resolution.
#[derive(Debug, Clone, PartialEq)]
pub enum RawReExportKind {
    /// `export { source as exported } from …` (alias unwrapped), or a bare
    /// `export { A }` of an import binding.
    Named { exported: String, source: String },
    /// `export * from …`.
    Star,
}

/// Extract re-export edges from one TS/JS file (barrel files). Other
/// languages yield none — inheritance resolution is TS/JS-only by design.
pub fn extract_reexports(path: &Path, content: &str) -> Vec<RawReExport> {
    let lang = detect_language(&path.to_string_lossy());
    if !matches!(lang, Language::JsTs) {
        return Vec::new();
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let Some(grammar) = grammar_for(lang, ext) else {
        return Vec::new();
    };
    let Some(tree) = parse(content, &grammar) else {
        return Vec::new();
    };
    reexports_from_tree(tree.root_node(), content)
}

/// Tree-level core of [`extract_reexports`], for callers that already hold
/// a parsed TS/JS tree (shared-parse path).
pub(super) fn reexports_from_tree(root: Node<'_>, content: &str) -> Vec<RawReExport> {
    let imports = import_bindings(root, content);
    descendants(root)
        .into_iter()
        .filter(|n| n.kind() == "export_statement")
        .flat_map(|stmt| statement_reexports(stmt, content, &imports))
        .collect()
}

fn statement_reexports(
    stmt: Node<'_>,
    content: &str,
    imports: &HashMap<String, (String, String)>,
) -> Vec<RawReExport> {
    let source = stmt.child_by_field_name("source").map(|s| {
        text(s, content)
            .trim_matches(|c| c == '"' || c == '\'')
            .to_string()
    });
    match source {
        // `export * from './x'` — a direct `*` child; `export * as ns` wraps
        // the star in a namespace_export and is deliberately skipped (its
        // symbols are only reachable through the namespace).
        Some(specifier)
            if (0..stmt.child_count())
                .filter_map(|i| stmt.child(i as u32))
                .any(|c| c.kind() == "*") =>
        {
            vec![RawReExport {
                specifier,
                kind: RawReExportKind::Star,
            }]
        }
        // `export { A, B as C } from './x'`.
        Some(specifier) => export_specifiers(stmt, content)
            .map(|(name, exported)| RawReExport {
                specifier: specifier.clone(),
                kind: RawReExportKind::Named {
                    exported,
                    source: name,
                },
            })
            .collect(),
        // Bare `export { A }` — a re-export only when A is import-bound.
        None => export_specifiers(stmt, content)
            .filter_map(|(local, exported)| {
                let (specifier, original) = imports.get(&local)?;
                Some(RawReExport {
                    specifier: specifier.clone(),
                    kind: RawReExportKind::Named {
                        exported,
                        source: original.clone(),
                    },
                })
            })
            .collect(),
    }
}

/// Each `export_specifier` of a statement as (name, exported-as) — the
/// alias when present, otherwise the name itself.
fn export_specifiers<'a>(
    stmt: Node<'a>,
    content: &'a str,
) -> impl Iterator<Item = (String, String)> + 'a {
    descendants(stmt)
        .into_iter()
        .filter(|n| n.kind() == "export_specifier")
        .filter_map(move |n| {
            let name = text(n.child_by_field_name("name")?, content).to_string();
            let exported = n
                .child_by_field_name("alias")
                .map(|a| text(a, content).to_string())
                .unwrap_or_else(|| name.clone());
            Some((name, exported))
        })
}

/// Extract class records from one TS/JS file. Other languages (including
/// Rust) yield no records — the rung is TS/JS-only by design.
pub fn extract_class_records(path: &Path, content: &str) -> Vec<RawClassRecord> {
    let lang = detect_language(&path.to_string_lossy());
    if !matches!(lang, Language::JsTs) {
        return Vec::new();
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let Some(grammar) = grammar_for(lang, ext) else {
        return Vec::new();
    };
    let Some(tree) = parse(content, &grammar) else {
        return Vec::new();
    };
    class_records_from_tree(tree.root_node(), content)
}

/// Tree-level core of [`extract_class_records`], for callers that already
/// hold a parsed TS/JS tree (shared-parse path).
pub(super) fn class_records_from_tree(root: Node<'_>, content: &str) -> Vec<RawClassRecord> {
    let imports = import_bindings(root, content);
    descendants(root)
        .into_iter()
        .filter(|n| {
            matches!(
                n.kind(),
                "class_declaration" | "class" | "abstract_class_declaration"
            )
        })
        .filter_map(|class_node| class_record(class_node, content, &imports))
        .collect()
}

fn class_record(
    class_node: Node<'_>,
    content: &str,
    imports: &HashMap<String, (String, String)>,
) -> Option<RawClassRecord> {
    let heritage = child_of_kind(class_node, "class_heritage")?;
    let expr = base_expression(heritage)?; // None = no extends (e.g. implements-only)
    let base = if expr.kind() == "identifier" {
        let ident = text(expr, content).to_string();
        match imports.get(&ident) {
            Some((specifier, name)) => RawBaseRef::Specifier {
                specifier: specifier.clone(),
                name: name.clone(),
            },
            None => RawBaseRef::SameFile(ident),
        }
    } else {
        RawBaseRef::Unresolvable
    };
    // Multiple anonymous class expressions in one file collapse onto the
    // same (path, "default") key — depth lookup keeps the last record seen,
    // a known bounded limitation; disambiguate by line if it ever matters.
    let class_name = class_node
        .child_by_field_name("name")
        .map(|n| text(n, content).to_string())
        .unwrap_or_else(|| "default".to_string()); // `export default class extends X`
    Some(RawClassRecord {
        line: class_node.start_position().row + 1,
        class_name,
        base,
    })
}

/// The TS grammar wraps the extends target in an `extends_clause`
/// (field `value`); the JS grammar puts the expression directly under
/// `class_heritage`. TS `implements`-only heritage is not inheritance.
fn base_expression(heritage: Node<'_>) -> Option<Node<'_>> {
    if let Some(clause) = child_of_kind(heritage, "extends_clause") {
        return clause.child_by_field_name("value");
    }
    if child_of_kind(heritage, "implements_clause").is_some() {
        return None;
    }
    (0..heritage.named_child_count())
        .filter_map(|i| heritage.named_child(i as u32))
        .next()
}

/// local binding → (module specifier, exported name). Default imports map
/// to their local name (best-effort: a renamed default import terminates
/// the chain at resolution time instead — under-count, never over-count).
fn import_bindings(root: Node<'_>, content: &str) -> HashMap<String, (String, String)> {
    descendants(root)
        .into_iter()
        .filter(|n| n.kind() == "import_statement")
        .filter_map(|stmt| {
            let source = stmt.child_by_field_name("source")?;
            let specifier = text(source, content)
                .trim_matches(|c| c == '"' || c == '\'')
                .to_string();
            Some((stmt, specifier))
        })
        .flat_map(|(stmt, specifier)| {
            descendants(stmt)
                .into_iter()
                .filter_map(move |n| binding(n, content, &specifier))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn binding(n: Node<'_>, content: &str, specifier: &str) -> Option<(String, (String, String))> {
    match n.kind() {
        // `import A from './a'` — the clause's direct identifier child.
        "import_clause" => {
            let c = n.named_child(0).filter(|c| c.kind() == "identifier")?;
            let name = text(c, content).to_string();
            Some((name.clone(), (specifier.to_string(), name)))
        }
        // `import { A }` / `import { A as B }`.
        "import_specifier" => {
            let exported = text(n.child_by_field_name("name")?, content).to_string();
            let local = n
                .child_by_field_name("alias")
                .map(|a| text(a, content).to_string())
                .unwrap_or_else(|| exported.clone());
            Some((local, (specifier.to_string(), exported)))
        }
        _ => None,
    }
}

fn child_of_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    (0..node.child_count())
        .filter_map(|i| node.child(i as u32))
        .find(|c| c.kind() == kind)
}

fn text<'a>(node: Node<'_>, content: &'a str) -> &'a str {
    &content[node.byte_range()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn records(name: &str, src: &str) -> Vec<RawClassRecord> {
        extract_class_records(Path::new(name), src)
    }

    #[test]
    fn same_file_extends() {
        let r = records("src/a.ts", "class A {}\nclass B extends A {}\n");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].class_name, "B");
        assert_eq!(r[0].line, 2);
        assert_eq!(r[0].base, RawBaseRef::SameFile("A".into()));
    }

    #[test]
    fn imported_named_base() {
        let src = "import { A } from './a';\nexport class B extends A {}\n";
        let r = records("src/b.ts", src);
        assert_eq!(
            r[0].base,
            RawBaseRef::Specifier {
                specifier: "./a".into(),
                name: "A".into()
            }
        );
    }

    #[test]
    fn aliased_import_records_exported_name() {
        let src = "import { A as Base } from './a';\nclass B extends Base {}\n";
        let r = records("src/b.ts", src);
        assert_eq!(
            r[0].base,
            RawBaseRef::Specifier {
                specifier: "./a".into(),
                name: "A".into()
            }
        );
    }

    #[test]
    fn default_import_base_maps_to_local_name() {
        let src = "import A from './a';\nclass B extends A {}\n";
        let r = records("src/b.js", src);
        assert_eq!(
            r[0].base,
            RawBaseRef::Specifier {
                specifier: "./a".into(),
                name: "A".into()
            }
        );
    }

    // ── Re-export extraction ───────────────────────────────────────

    fn reexports(name: &str, src: &str) -> Vec<RawReExport> {
        extract_reexports(Path::new(name), src)
    }

    #[test]
    fn named_reexport_from_source() {
        let r = reexports("src/index.ts", "export { A } from './a';\n");
        assert_eq!(
            r,
            vec![RawReExport {
                specifier: "./a".into(),
                kind: RawReExportKind::Named {
                    exported: "A".into(),
                    source: "A".into()
                },
            }]
        );
    }

    #[test]
    fn aliased_reexport_records_both_names() {
        let r = reexports("src/index.ts", "export { A as Base } from './a';\n");
        assert_eq!(
            r[0].kind,
            RawReExportKind::Named {
                exported: "Base".into(),
                source: "A".into()
            }
        );
    }

    #[test]
    fn star_reexport() {
        let r = reexports("src/index.ts", "export * from './a';\n");
        assert_eq!(
            r,
            vec![RawReExport {
                specifier: "./a".into(),
                kind: RawReExportKind::Star,
            }]
        );
    }

    #[test]
    fn bare_export_of_import_binding_is_a_reexport() {
        let src = "import { A } from './a';\nexport { A };\n";
        let r = reexports("src/index.ts", src);
        assert_eq!(
            r,
            vec![RawReExport {
                specifier: "./a".into(),
                kind: RawReExportKind::Named {
                    exported: "A".into(),
                    source: "A".into()
                },
            }]
        );
    }

    #[test]
    fn namespace_reexport_is_skipped() {
        assert!(reexports("src/index.ts", "export * as ns from './a';\n").is_empty());
    }

    #[test]
    fn plain_exports_are_not_reexports() {
        let src = "export class A {}\nexport const x = 1;\nexport function f() {}\n";
        assert!(reexports("src/index.ts", src).is_empty());
    }

    #[test]
    fn non_jsts_yields_no_reexports() {
        assert!(reexports("src/lib.rs", "pub use crate::a::A;\n").is_empty());
    }

    #[test]
    fn mixin_call_is_unresolvable() {
        let r = records("src/b.ts", "class B extends mixin(Base) {}\n");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].base, RawBaseRef::Unresolvable);
    }

    #[test]
    fn class_without_extends_yields_no_record() {
        assert!(records("src/a.ts", "class A { m() {} }\n").is_empty());
    }

    #[test]
    fn implements_only_yields_no_record() {
        let src = "interface I {}\nclass A implements I {}\n";
        assert!(records("src/a.ts", src).is_empty());
    }

    #[test]
    fn export_default_anonymous_class_is_captured() {
        let src = "class A {}\nexport default class extends A {}\n";
        let r = records("src/b.ts", src);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].class_name, "default");
        assert_eq!(r[0].base, RawBaseRef::SameFile("A".into()));
    }

    #[test]
    fn rust_files_yield_no_records() {
        let src = "pub trait T {}\npub struct S;\nimpl T for S {}\n";
        assert!(records("src/lib.rs", src).is_empty());
    }

    #[test]
    fn js_grammar_heritage_shape_works_too() {
        // The JS grammar has no extends_clause wrapper — the heritage
        // node's child is the expression itself.
        let r = records("src/b.js", "class A {}\nclass B extends A {}\n");
        assert_eq!(r[0].base, RawBaseRef::SameFile("A".into()));
    }

    #[test]
    fn ts_type_arguments_on_base_still_resolve_the_identifier() {
        let src = "class A<T> {}\nclass B extends A<number> {}\n";
        let r = records("src/b.ts", src);
        assert_eq!(r[0].base, RawBaseRef::SameFile("A".into()));
    }

    #[test]
    fn abstract_class_with_extends_is_captured() {
        let r = records("src/b.ts", "abstract class B extends A {}\n");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].class_name, "B");
        assert_eq!(r[0].base, RawBaseRef::SameFile("A".into()));
    }
}
