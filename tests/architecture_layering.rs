//! The pipeline's direction is a structural contract:
//!
//! ```text
//! CLI → Collector → RepoSnapshot → Metrics → Scorer → Renderer
//! ```
//!
//! A module may depend on what comes before it, never on what comes after.
//! The report type lives in `scorer`, so a metric that imports it, or a
//! collector that imports a renderer, is a cycle waiting for the next
//! refactor to trip over. Nothing in the type system forbids it, so this test
//! reads the `crate::…` paths in the source and does.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// `(layer, layers it must not depend on)`.
const FORBIDDEN: &[(&str, &[&str])] = &[
    ("collector", &["scorer", "renderer", "cmd", "cli"]),
    ("snapshot", &["scorer", "renderer", "cmd", "cli"]),
    ("metrics", &["scorer", "renderer", "cmd", "cli"]),
    ("analysis", &["scorer", "renderer", "cmd", "cli"]),
    ("scorer", &["renderer", "cmd", "cli"]),
];

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .flat_map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                rust_files(&path)
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                vec![path]
            } else {
                Vec::new()
            }
        })
        .collect()
}

/// The top-level modules a source text reaches through `crate::`, including
/// the leading names of a braced group: `use crate::{scorer, metrics::x};`.
fn crate_roots(source: &str) -> BTreeSet<String> {
    let ident = |text: &str| -> String {
        text.trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect()
    };
    source
        .split("crate::")
        .skip(1)
        .flat_map(|after| match after.strip_prefix('{') {
            Some(group) => top_level_items(group).into_iter().map(ident).collect(),
            None => vec![ident(after)],
        })
        .filter(|name| !name.is_empty())
        .collect()
}

/// The comma-separated items of a braced group, up to its matching `}`:
/// nested groups stay inside their item. `group` starts just after the `{`.
fn top_level_items(group: &str) -> Vec<&str> {
    let (mut depth, mut start, mut items) = (0usize, 0usize, Vec::new());
    for (index, ch) in group.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' if depth == 0 => {
                items.push(&group[start..index]);
                return items;
            }
            '}' => depth -= 1,
            ',' if depth == 0 => {
                items.push(&group[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    items.push(&group[start..]);
    items
}

/// `layer`'s own files: `src/<layer>/**` and `src/<layer>.rs`.
fn files_of(layer: &str) -> Vec<PathBuf> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = rust_files(&src.join(layer));
    let single = src.join(format!("{layer}.rs"));
    if single.exists() {
        files.push(single);
    }
    files
}

#[test]
fn no_layer_depends_on_a_layer_after_it() {
    let violations: Vec<String> = FORBIDDEN
        .iter()
        .flat_map(|(layer, forbidden)| {
            files_of(layer).into_iter().flat_map(move |file| {
                let roots = crate_roots(&fs::read_to_string(&file).unwrap());
                forbidden
                    .iter()
                    .filter(|name| roots.contains(**name))
                    .map(|name| format!("{} reaches crate::{name}", file.display()))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    assert!(
        violations.is_empty(),
        "layering violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn every_governed_layer_actually_has_source_files() {
    // A typo in a layer name would make the check above vacuously green.
    for (layer, _) in FORBIDDEN {
        assert!(!files_of(layer).is_empty(), "no source files for {layer}");
    }
}

#[test]
fn crate_roots_reads_plain_and_braced_paths() {
    let roots = crate_roots(
        "use crate::scorer::ActionItem;\n\
         use crate::{metrics::x, renderer, config::{a, b}};\n\
         let v = crate::cmd::run();",
    );
    let found: Vec<&str> = roots.iter().map(String::as_str).collect();
    assert_eq!(found, ["cmd", "config", "metrics", "renderer", "scorer"]);
}
