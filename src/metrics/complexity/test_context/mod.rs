//! Bounded, file-local test evidence. Ranges classify existing extracted functions;
//! they never change extraction or numeric measurements.
mod javascript;
mod managed;
mod php;
mod python;
mod rust;

use super::Language;
use crate::metrics::file_role::{classify, FileRole};
use std::{ops::Range, path::Path};
use tree_sitter::Node;

/// Named nodes of a subtree in pre-order. The walk keeps its own stack, so an
/// arbitrarily deep AST never becomes call-stack depth on a rayon worker.
/// `descend` decides whether a visited node's children are walked too.
pub(super) fn named_preorder<'tree>(
    root: Node<'tree>,
    descend: impl Fn(Node<'tree>) -> bool,
) -> impl Iterator<Item = Node<'tree>> {
    let mut pending = vec![root];
    std::iter::from_fn(move || {
        let node = pending.pop()?;
        if descend(node) {
            let children: Vec<_> = node.named_children(&mut node.walk()).collect();
            pending.extend(children.into_iter().rev());
        }
        Some(node)
    })
}

pub(super) struct TestContext(Vec<Range<usize>>);

impl TestContext {
    pub(super) fn build(
        root: Node<'_>,
        source: &[u8],
        lang: Language,
        path: Option<&Path>,
    ) -> Self {
        let ranges = if path.is_some_and(|path| classify(path) == FileRole::Test) {
            vec![root.byte_range()]
        } else {
            match lang {
                Language::Rust => rust::ranges(root, source),
                Language::JsTs => javascript::ranges(root, source),
                Language::Python => python::ranges(root, source),
                Language::Php => php::ranges(root, source),
                Language::Java | Language::CSharp | Language::Kotlin => {
                    managed::ranges(root, source, lang)
                }
                Language::Go | Language::Generic => Vec::new(),
            }
        };
        Self::from_ranges(ranges)
    }

    fn from_ranges(mut ranges: Vec<Range<usize>>) -> Self {
        ranges.sort_by_key(|range| (range.start, range.end));
        Self(
            ranges
                .into_iter()
                .fold(Vec::<Range<usize>>::new(), |mut merged, range| {
                    if let Some(previous) = merged
                        .last_mut()
                        .filter(|previous| range.start <= previous.end)
                    {
                        previous.end = previous.end.max(range.end);
                    } else {
                        merged.push(range);
                    }
                    merged
                }),
        )
    }

    pub(super) fn contains(&self, range: Range<usize>) -> bool {
        let position = self
            .0
            .partition_point(|candidate| candidate.start <= range.start);
        position
            .checked_sub(1)
            .is_some_and(|i| self.0[i].end >= range.end)
    }
}

#[cfg(test)]
mod depth_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn index_merges_overlaps_and_checks_full_containment() {
        let index = TestContext::from_ranges(vec![20..30, 5..12, 0..10, 21..22]);
        assert!(index.contains(0..12));
        assert!(index.contains(25..30));
        assert!(!index.contains(12..21));
        assert!(!index.contains(19..30));
        assert!(!index.contains(25..31));
        assert!(!TestContext::from_ranges(vec![]).contains(0..1));
    }
    #[test]
    fn test_file_context_is_authoritative_for_every_grammar() {
        let cases = [
            ("rs", "fn render_helper() {}", Language::Rust),
            ("js", "function render_helper() {}", Language::JsTs),
            (
                "jsx",
                "function render_helper() { return <div/>; }",
                Language::JsTs,
            ),
            ("ts", "function render_helper(): void {}", Language::JsTs),
            (
                "tsx",
                "function render_helper() { return <div/>; }",
                Language::JsTs,
            ),
            ("py", "def render_helper(): pass", Language::Python),
            ("go", "package sample\nfunc RenderHelper() {}", Language::Go),
            (
                "java",
                "class Example { void renderHelper() {} }",
                Language::Java,
            ),
            (
                "cs",
                "class Example { void RenderHelper() {} }",
                Language::CSharp,
            ),
            ("kt", "fun renderHelper() {}", Language::Kotlin),
            ("kts", "fun renderHelper() {}", Language::Kotlin),
            ("php", "<?php function renderHelper() {}", Language::Php),
        ];
        for (ext, source, lang) in cases {
            let path = format!("tests/fixture.{ext}");
            let metrics = crate::metrics::complexity::analyse_file(Path::new(&path), source);
            assert!(!metrics.functions.is_empty(), "{ext}");
            assert!(metrics.functions.iter().all(|f| f.is_test), "{ext}");
            let production = crate::metrics::complexity::analyse_file(
                Path::new(&format!("src/fixture.{ext}")),
                source,
            );
            let mut unclassified = metrics.clone();
            for function in &mut unclassified.functions {
                function.is_test = false;
            }
            assert_eq!(unclassified, production, "measurements changed for {ext}");
            assert!(
                crate::metrics::complexity::analyse_content(source, lang)
                    .functions
                    .iter()
                    .all(|f| !f.is_test),
                "{ext}"
            );
        }
    }

    #[test]
    fn go_names_and_imports_need_test_file_evidence() {
        let source = "package sample\nimport \"testing\"\nfunc TestRender(t *testing.T) {}\nfunc BenchmarkRender(b *testing.B) {}\nfunc FuzzRender(f *testing.F) {}\nfunc ExampleRender() {}\nfunc renderHelper() {}\n";
        for (path, expected) in [("src/render.go", false), ("src/render_test.go", true)] {
            let functions =
                crate::metrics::complexity::analyse_file(Path::new(path), source).functions;
            assert_eq!(functions.len(), 5);
            assert!(functions.iter().all(|f| f.is_test == expected));
        }
    }
}
