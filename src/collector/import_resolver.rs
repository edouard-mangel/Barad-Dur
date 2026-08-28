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

/// Resolve one raw import specifier from `source` against the repo's known
/// files. Crate-visible for class-record base resolution (M7), which must
/// use the exact same candidate rules as the import graph.
pub(crate) fn resolve_specifier(
    raw: &str,
    source: &Path,
    known: &HashSet<&PathBuf>,
) -> Option<PathBuf> {
    resolve_single_import(raw, source, known)
}

/// Candidate-path builder for one language's import specifiers.
type CandidateFn = fn(&str, &Path) -> Vec<PathBuf>;

/// The dispatch table, and the single source of truth for which languages
/// actually produce import edges. A language with an import *query* but no
/// arm here (Kotlin) yields specifiers that resolve to nothing, so the
/// import graph stays empty — `resolves_imports` lets the graph metrics
/// tell that apart from a genuinely import-free repository.
fn candidates_for(ext: &str) -> Option<CandidateFn> {
    match ext {
        "rs" => Some(|raw, _| resolve_rust_import(raw)),
        "js" | "jsx" | "mjs" | "cjs" => Some(resolve_js_import),
        "ts" | "tsx" => Some(resolve_ts_import),
        "py" => Some(|raw, _| resolve_python_import(raw)),
        "go" => Some(resolve_go_import),
        "java" => Some(|raw, _| resolve_java_import(raw)),
        "kt" | "kts" => Some(|raw, _| resolve_kotlin_import(raw)),
        "cs" => Some(|raw, _| resolve_csharp_import(raw)),
        _ => None,
    }
}

/// Whether imports from a file with this extension can resolve to repo paths.
pub(crate) fn resolves_imports(ext: &str) -> bool {
    candidates_for(ext).is_some()
}

fn resolve_single_import(raw: &str, source: &Path, known: &HashSet<&PathBuf>) -> Option<PathBuf> {
    let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("");
    candidates_for(ext)?(raw, source)
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
    // A symbol declared right at the crate root (`use crate::helper;`)
    // produces the bare specifier "crate" — no "::" left to strip into a
    // submodule path, so it needs its own case pointing at the crate root
    // files directly, in probe order (lib crates before binaries).
    if raw == "crate" || raw == "self" {
        return vec![PathBuf::from("src/lib.rs"), PathBuf::from("src/main.rs")];
    }
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

/// Kotlin imports are dotted package paths like Java's, laid out under
/// `src/main/kotlin/` by Gradle. Unlike Java they routinely name a member
/// or top-level function rather than a type — `com.foo.Bar.baz` is declared
/// in `com/foo/Bar.kt` — so the parent of the dotted path is a candidate
/// too. Kotlin does not enforce package/directory correspondence, so a
/// non-conventional layout simply yields no candidate that exists, never a
/// wrong edge: `resolve_single_import` keeps only paths the repo really has.
fn resolve_kotlin_import(raw: &str) -> Vec<PathBuf> {
    let for_segments = |segments: String| {
        [
            PathBuf::from(format!("{segments}.kt")),
            PathBuf::from(format!("src/main/kotlin/{segments}.kt")),
        ]
    };
    let parent = raw.rsplit_once('.').map(|(head, _)| head.replace('.', "/"));
    for_segments(raw.replace('.', "/"))
        .into_iter()
        .chain(parent.into_iter().flat_map(for_segments))
        .collect()
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
    fn kotlin_import_resolves_to_package_path() {
        let files = vec![entry("com/foo/Bar.kt"), entry("com/foo/App.kt")];
        let graph = resolve_imports(&raw("com/foo/App.kt", vec!["com.foo.Bar"]), &files);
        assert_eq!(
            graph[&PathBuf::from("com/foo/App.kt")],
            vec![PathBuf::from("com/foo/Bar.kt")]
        );
    }

    #[test]
    fn kotlin_import_resolves_under_the_gradle_source_root() {
        // Gradle lays Kotlin out under src/main/kotlin/, the same shape
        // resolve_java_import already handles for src/main/java/.
        let files = vec![
            entry("src/main/kotlin/com/foo/Bar.kt"),
            entry("src/main/kotlin/com/foo/App.kt"),
        ];
        let graph = resolve_imports(
            &raw("src/main/kotlin/com/foo/App.kt", vec!["com.foo.Bar"]),
            &files,
        );
        assert_eq!(
            graph[&PathBuf::from("src/main/kotlin/com/foo/App.kt")],
            vec![PathBuf::from("src/main/kotlin/com/foo/Bar.kt")]
        );
    }

    #[test]
    fn kotlin_member_import_resolves_to_the_declaring_file() {
        // Unlike Java, Kotlin routinely imports a member or top-level
        // function: `com.foo.Bar.baz` lives in com/foo/Bar.kt, so the
        // final segment must be droppable.
        let files = vec![entry("com/foo/Bar.kt"), entry("com/foo/App.kt")];
        let graph = resolve_imports(&raw("com/foo/App.kt", vec!["com.foo.Bar.baz"]), &files);
        assert_eq!(
            graph[&PathBuf::from("com/foo/App.kt")],
            vec![PathBuf::from("com/foo/Bar.kt")]
        );
    }

    #[test]
    fn kotlin_import_of_an_unknown_file_yields_no_edge() {
        // Kotlin does not enforce package/directory correspondence, so a
        // non-conventional layout must degrade to "no edge", never a
        // wrong one.
        let files = vec![entry("com/foo/App.kt")];
        let graph = resolve_imports(&raw("com/foo/App.kt", vec!["com.elsewhere.Bar"]), &files);
        assert!(!graph.contains_key(&PathBuf::from("com/foo/App.kt")));
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

    #[test]
    fn rust_bare_crate_specifier_resolves_to_the_crate_root() {
        // Review finding: a symbol declared right at the crate root
        // (`use crate::helper;`) produces the bare specifier "crate" —
        // there's no "::" left to strip, so the general crate::-prefix
        // path-segment logic can't map it to src/lib.rs without a
        // dedicated case.
        let files = vec![entry("src/lib.rs"), entry("src/other.rs")];
        let graph = resolve_imports(&raw("src/other.rs", vec!["crate"]), &files);
        let targets = graph
            .get(&PathBuf::from("src/other.rs"))
            .expect("bare crate specifier must resolve to the crate root");
        assert_eq!(targets[0].to_string_lossy(), "src/lib.rs");
    }

    #[test]
    fn rust_bare_crate_specifier_resolves_to_main_when_no_lib() {
        let files = vec![entry("src/main.rs"), entry("src/other.rs")];
        let graph = resolve_imports(&raw("src/other.rs", vec!["crate"]), &files);
        let targets = graph
            .get(&PathBuf::from("src/other.rs"))
            .expect("bare crate specifier must resolve to the crate root");
        assert_eq!(targets[0].to_string_lossy(), "src/main.rs");
    }
}
