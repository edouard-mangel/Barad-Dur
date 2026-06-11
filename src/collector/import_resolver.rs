use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::snapshot::FileEntry;

pub type RawImports = HashMap<PathBuf, Vec<String>>;

/// Resolve raw import strings to actual file paths present in the repository.
/// Only keeps imports that map to a known file in `files`.
pub fn resolve_imports(
    raw_imports: &RawImports,
    files: &[FileEntry],
) -> HashMap<PathBuf, Vec<PathBuf>> {
    let known: HashSet<&PathBuf> = files.iter().map(|f| &f.path).collect();

    raw_imports
        .iter()
        .filter_map(|(source_path, imports)| {
            let resolved: Vec<PathBuf> = imports
                .iter()
                .filter_map(|raw| resolve_single_import(raw, source_path, &known))
                .collect();
            if resolved.is_empty() {
                None
            } else {
                Some((source_path.clone(), resolved))
            }
        })
        .collect()
}

fn resolve_single_import(raw: &str, source: &Path, known: &HashSet<&PathBuf>) -> Option<PathBuf> {
    let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("");
    let candidates = match ext {
        "rs" => resolve_rust_import(raw),
        "js" | "jsx" | "mjs" | "cjs" => resolve_js_import(raw, source),
        "ts" | "tsx" => resolve_ts_import(raw, source),
        "py" => resolve_python_import(raw),
        "go" => resolve_go_import(raw, source),
        "java" => resolve_java_import(raw),
        "cs" => resolve_csharp_import(raw),
        _ => Vec::new(),
    };
    candidates
        .into_iter()
        .map(|c| normalize_path(&c))
        .find(|c| known.contains(c))
}

/// Lexically normalize a path: drop `.` segments and fold `..` into the
/// preceding component. Renderers join files by their *string* form, so
/// resolved paths must serialize identically to `snapshot.files` paths;
/// folding `..` also lets parent-directory imports match `known` at all
/// (`Path` equality keeps `..` components).
fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    path.components().fold(PathBuf::new(), |mut acc, comp| {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                acc.pop();
            }
            other => acc.push(other),
        }
        acc
    })
}

fn resolve_rust_import(raw: &str) -> Vec<PathBuf> {
    // crate::foo::bar → src/foo/bar.rs or src/foo/bar/mod.rs
    let path_part = raw
        .strip_prefix("crate::")
        .or_else(|| raw.strip_prefix("self::"))
        .unwrap_or(raw);
    let path_part = path_part
        .split("::{")
        .next()
        .unwrap_or(path_part)
        .trim_end_matches("::*");
    let segments = path_part.replace("::", "/");
    vec![
        PathBuf::from(format!("src/{}.rs", segments)),
        PathBuf::from(format!("src/{}/mod.rs", segments)),
    ]
}

fn resolve_js_import(raw: &str, source: &Path) -> Vec<PathBuf> {
    if !raw.starts_with('.') {
        return Vec::new(); // external package
    }
    let base = source.parent().unwrap_or_else(|| Path::new(""));
    let resolved = base.join(raw);
    vec![
        resolved.with_extension("js"),
        resolved.with_extension("jsx"),
        resolved.with_extension("mjs"),
        resolved.join("index.js"),
    ]
}

fn resolve_ts_import(raw: &str, source: &Path) -> Vec<PathBuf> {
    if !raw.starts_with('.') {
        return Vec::new();
    }
    let base = source.parent().unwrap_or_else(|| Path::new(""));
    let resolved = base.join(raw);
    vec![
        resolved.with_extension("ts"),
        resolved.with_extension("tsx"),
        resolved.with_extension("js"),
        resolved.join("index.ts"),
        resolved.join("index.tsx"),
        resolved.join("index.js"),
    ]
}

fn resolve_python_import(raw: &str) -> Vec<PathBuf> {
    let segments = raw.replace('.', "/");
    vec![
        PathBuf::from(format!("{}.py", segments)),
        PathBuf::from(format!("{}/__init__.py", segments)),
    ]
}

fn resolve_go_import(raw: &str, source: &Path) -> Vec<PathBuf> {
    let last = raw.rsplit('/').next().unwrap_or(raw);
    let base = source.parent().unwrap_or_else(|| Path::new(""));
    vec![base.join(last).join("*.go")]
}

fn resolve_java_import(raw: &str) -> Vec<PathBuf> {
    let segments = raw.replace('.', "/");
    vec![
        PathBuf::from(format!("{}.java", segments)),
        PathBuf::from(format!("src/main/java/{}.java", segments)),
    ]
}

fn resolve_csharp_import(raw: &str) -> Vec<PathBuf> {
    let segments = raw.replace('.', "/");
    vec![PathBuf::from(format!("{}.cs", segments))]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            size_bytes: 100,
            is_binary: false,
            depth: 1,
            blob_oid: String::new(),
        }
    }

    fn raw(source: &str, imports: Vec<&str>) -> RawImports {
        let mut m = RawImports::new();
        m.insert(
            PathBuf::from(source),
            imports.into_iter().map(String::from).collect(),
        );
        m
    }

    #[test]
    fn ts_relative_import_resolves_to_normalized_path() {
        // The resolved path must serialize WITHOUT the ./ segment —
        // renderers join on the string form, not on Path equality.
        let files = vec![
            entry("dashboard/src/App.tsx"),
            entry("dashboard/src/pages/Landing.tsx"),
        ];
        let graph = resolve_imports(
            &raw("dashboard/src/App.tsx", vec!["./pages/Landing"]),
            &files,
        );

        let targets = &graph[&PathBuf::from("dashboard/src/App.tsx")];
        assert_eq!(
            targets[0].to_string_lossy(),
            "dashboard/src/pages/Landing.tsx",
            "resolved path must not contain a ./ segment"
        );
    }

    #[test]
    fn ts_parent_import_resolves_across_directories() {
        // ../shared/util from src/a/b.ts must reach src/shared/util.ts.
        // Path::components() keeps ParentDir, so without lexical
        // normalization this import silently fails to resolve.
        let files = vec![entry("src/a/b.ts"), entry("src/shared/util.ts")];
        let graph = resolve_imports(&raw("src/a/b.ts", vec!["../shared/util"]), &files);

        let targets = graph
            .get(&PathBuf::from("src/a/b.ts"))
            .expect("parent-directory import must resolve");
        assert_eq!(targets[0].to_string_lossy(), "src/shared/util.ts");
    }

    #[test]
    fn js_relative_import_resolves_to_normalized_path() {
        let files = vec![entry("web/main.js"), entry("web/lib/api.js")];
        let graph = resolve_imports(&raw("web/main.js", vec!["./lib/api"]), &files);

        let targets = &graph[&PathBuf::from("web/main.js")];
        assert_eq!(targets[0].to_string_lossy(), "web/lib/api.js");
    }
}
