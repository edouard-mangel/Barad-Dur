use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::composer::Psr4Root;
use crate::snapshot::FileEntry;

pub type RawImports = HashMap<PathBuf, Vec<String>>;

/// Repository-level configuration that import resolution needs beyond the
/// specifier and the source path.
///
/// Only PHP uses it today: its namespaces map to directories solely because
/// `composer.json` says so, and that mapping is per-repository state the
/// pure per-language resolvers cannot derive. Every other language resolves
/// from the specifier alone and ignores this.
#[derive(Debug, Clone, Default)]
pub(crate) struct RepoImportConfig {
    pub psr4: Vec<Psr4Root>,
}

/// Resolve raw import strings to actual file paths present in the repository.
/// Only keeps imports that map to a known file in `files`.
pub fn resolve_imports(
    raw_imports: &RawImports,
    files: &[FileEntry],
    config: &RepoImportConfig,
) -> HashMap<PathBuf, Vec<PathBuf>> {
    let known: HashSet<&PathBuf> = files.iter().map(|f| &f.path).collect();

    raw_imports
        .iter()
        .filter_map(|(source_path, imports)| {
            let resolved: Vec<PathBuf> = imports
                .iter()
                .flat_map(|raw| resolve_import_targets(raw, source_path, &known, config))
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
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
    config: &RepoImportConfig,
) -> Option<PathBuf> {
    resolve_single_import(raw, source, known, config)
}

/// Candidate-path builder for one language's import specifiers.
///
/// Takes the repo config so PHP can reach its PSR-4 roots; every other
/// language ignores it.
type CandidateFn = fn(&str, &Path, &RepoImportConfig) -> Vec<PathBuf>;

/// The dispatch table, and the single source of truth for which languages
/// actually produce import edges. A language with an import *query* but no
/// arm here (Kotlin) yields specifiers that resolve to nothing, so the
/// import graph stays empty — `resolves_imports` lets the graph metrics
/// tell that apart from a genuinely import-free repository.
fn candidates_for(ext: &str) -> Option<CandidateFn> {
    match ext {
        "rs" => Some(|raw, _, _| resolve_rust_import(raw)),
        "js" | "jsx" | "mjs" | "cjs" => Some(|raw, src, _| resolve_js_import(raw, src)),
        "ts" | "tsx" => Some(|raw, src, _| resolve_ts_import(raw, src)),
        "py" => Some(|raw, _, _| resolve_python_import(raw)),
        "go" => Some(|raw, src, _| resolve_go_import(raw, src)),
        "java" => Some(|raw, _, _| resolve_java_import(raw)),
        "kt" | "kts" => Some(|raw, source, _| resolve_kotlin_import(raw, source)),
        "cs" => Some(|raw, _, _| resolve_csharp_import(raw)),
        "php" => Some(resolve_php_import),
        _ => None,
    }
}

/// Whether imports from a file with this extension can resolve to repo paths.
pub(crate) fn resolves_imports(ext: &str) -> bool {
    candidates_for(ext).is_some()
}

fn resolve_single_import(
    raw: &str,
    source: &Path,
    known: &HashSet<&PathBuf>,
    config: &RepoImportConfig,
) -> Option<PathBuf> {
    resolve_import_targets(raw, source, known, config)
        .into_iter()
        .next()
}

fn resolve_import_targets(
    raw: &str,
    source: &Path,
    known: &HashSet<&PathBuf>,
    config: &RepoImportConfig,
) -> Vec<PathBuf> {
    let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("");
    if matches!(ext, "kt" | "kts") && raw.ends_with(".*") {
        return resolve_kotlin_wildcard(raw, source, known);
    }
    candidates_for(ext)
        .into_iter()
        .flat_map(|candidate| candidate(raw, source, config))
        .map(|c| normalize_path(&c))
        .find(|c| known.contains(c))
        .into_iter()
        .collect()
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
fn resolve_kotlin_import(raw: &str, source: &Path) -> Vec<PathBuf> {
    let for_segments = |segments: String| {
        [
            PathBuf::from(format!("{segments}.kt")),
            PathBuf::from(format!("src/main/kotlin/{segments}.kt")),
        ]
    };
    let parent = raw.rsplit_once('.').map(|(head, _)| head.replace('.', "/"));
    let segments = raw.replace('.', "/");
    let source_root = kotlin_source_root(source);
    for_segments(segments.clone())
        .into_iter()
        .chain(
            source_root
                .iter()
                .map(|root| root.join(format!("{segments}.kt"))),
        )
        .chain(parent.clone().into_iter().flat_map(for_segments))
        .chain(
            source_root
                .iter()
                .flat_map(|root| parent.iter().map(|path| root.join(format!("{path}.kt")))),
        )
        .collect()
}

fn kotlin_source_root(source: &Path) -> Option<PathBuf> {
    let components: Vec<_> = source.components().collect();
    let kotlin = components
        .iter()
        .position(|part| part.as_os_str() == "kotlin")?;
    Some(components[..=kotlin].iter().collect())
}

fn resolve_kotlin_wildcard(raw: &str, source: &Path, known: &HashSet<&PathBuf>) -> Vec<PathBuf> {
    let package = raw.trim_end_matches(".*").replace('.', "/");
    let mut dirs = vec![
        PathBuf::from(&package),
        PathBuf::from("src/main/kotlin").join(&package),
    ];
    if let Some(root) = kotlin_source_root(source) {
        dirs.push(root.join(&package));
    }
    let mut matches: Vec<PathBuf> = known
        .iter()
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext == "kt" || ext == "kts")
        })
        .filter(|path| {
            path.parent()
                .is_some_and(|parent| dirs.iter().any(|dir| parent == dir))
        })
        .map(|path| (*path).clone())
        .collect();
    matches.sort();
    matches
}

/// PHP has two import forms, and `candidates_for` dispatches on the source
/// file's extension rather than per specifier, so they are told apart by the
/// specifier's own shape.
///
/// A namespace (`App\Apios\Database\Oracle`) resolves through the PSR-4
/// roots declared in `composer.json`; the longest matching prefix wins,
/// because overlapping roots are legal and the more specific one is the one
/// composer would use. A path (`/api/version.php`, captured from
/// `require __DIR__ . '/api/version.php'`) resolves against the requiring
/// file's own directory — `__DIR__` *is* that directory, so a leading slash
/// is relative to it, not to the repository root.
fn resolve_php_import(raw: &str, source: &Path, config: &RepoImportConfig) -> Vec<PathBuf> {
    if raw.contains('\\') || (!raw.contains('.') && !raw.contains('/')) {
        return psr4_candidates(raw, &config.psr4);
    }
    let relative = raw.trim_start_matches("./").trim_start_matches('/');
    match source.parent() {
        Some(dir) => vec![dir.join(relative)],
        None => vec![PathBuf::from(relative)],
    }
}

/// Namespace to file, through the longest PSR-4 prefix that matches.
fn psr4_candidates(namespace: &str, roots: &[Psr4Root]) -> Vec<PathBuf> {
    let namespace = namespace.trim_start_matches('\\');
    let matching: Vec<&Psr4Root> = roots
        .iter()
        .filter(|root| {
            namespace == root.prefix || namespace.starts_with(&format!("{}\\", root.prefix))
        })
        .collect();
    let Some(longest) = matching.iter().map(|root| root.prefix.len()).max() else {
        return Vec::new();
    };
    matching
        .into_iter()
        .filter(|root| root.prefix.len() == longest)
        .map(|root| {
            let remainder = namespace[root.prefix.len()..].trim_start_matches('\\');
            let mut path = root.dir.clone();
            for segment in remainder.split('\\').filter(|s| !s.is_empty()) {
                path.push(segment);
            }
            // Append rather than `with_extension`, which *replaces* — a root
            // directory containing a dot (`src/App.Core`) would otherwise be
            // truncated to a different path entirely.
            PathBuf::from(format!("{}.php", path.display()))
        })
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

    /// The PSR-4 roots the validation repo's `api/composer.json` declares,
    /// including the one no heuristic would guess.
    fn apios_config() -> RepoImportConfig {
        RepoImportConfig {
            psr4: vec![
                Psr4Root {
                    prefix: "App".into(),
                    dir: PathBuf::from("api/app"),
                },
                Psr4Root {
                    prefix: "Programming".into(),
                    dir: PathBuf::from("api/contexts/programming/app"),
                },
                Psr4Root {
                    prefix: "Tests".into(),
                    dir: PathBuf::from("api/tests"),
                },
            ],
        }
    }

    #[test]
    fn php_use_resolves_through_a_psr4_root() {
        let files = vec![
            entry("api/app/Apios/Database/Oracle.php"),
            entry("api/app/Http/Controller.php"),
        ];
        let graph = resolve_imports(
            &raw(
                "api/app/Http/Controller.php",
                vec!["App\\Apios\\Database\\Oracle"],
            ),
            &files,
            &apios_config(),
        );
        assert_eq!(
            graph[&PathBuf::from("api/app/Http/Controller.php")],
            vec![PathBuf::from("api/app/Apios/Database/Oracle.php")]
        );
    }

    #[test]
    fn php_use_resolves_a_root_no_heuristic_would_guess() {
        // `Programming\` maps to contexts/programming/app, not Programming/.
        // A namespace-to-directory guess returns nothing for this entire
        // bounded context, which is why the manifest must be read.
        let files = vec![
            entry("api/contexts/programming/app/Domain/Course.php"),
            entry("api/app/Http/Controller.php"),
        ];
        let graph = resolve_imports(
            &raw(
                "api/app/Http/Controller.php",
                vec!["Programming\\Domain\\Course"],
            ),
            &files,
            &apios_config(),
        );
        assert_eq!(
            graph[&PathBuf::from("api/app/Http/Controller.php")],
            vec![PathBuf::from(
                "api/contexts/programming/app/Domain/Course.php"
            )]
        );
    }

    #[test]
    fn php_use_prefers_the_longest_matching_prefix() {
        // Overlapping roots are legal in composer.json; the more specific
        // one must win or every App\Legacy\* edge lands in the wrong tree.
        let cfg = RepoImportConfig {
            psr4: vec![
                Psr4Root {
                    prefix: "App".into(),
                    dir: PathBuf::from("api/app"),
                },
                Psr4Root {
                    prefix: "App\\Legacy".into(),
                    dir: PathBuf::from("api/legacy"),
                },
            ],
        };
        let files = vec![entry("api/legacy/Thing.php"), entry("api/app/Main.php")];
        let graph = resolve_imports(
            &raw("api/app/Main.php", vec!["App\\Legacy\\Thing"]),
            &files,
            &cfg,
        );
        assert_eq!(
            graph[&PathBuf::from("api/app/Main.php")],
            vec![PathBuf::from("api/legacy/Thing.php")]
        );
    }

    #[test]
    fn php_require_resolves_relative_to_the_requiring_file() {
        // `require __DIR__ . '/api/version.php'` captures the literal
        // `/api/version.php`; __DIR__ is the requiring file's own directory,
        // so the leading slash is relative to it, not the repo root.
        let files = vec![
            entry("api/routes/api/version.php"),
            entry("api/routes/web.php"),
        ];
        let graph = resolve_imports(
            &raw("api/routes/web.php", vec!["/api/version.php"]),
            &files,
            &RepoImportConfig::default(),
        );
        assert_eq!(
            graph[&PathBuf::from("api/routes/web.php")],
            vec![PathBuf::from("api/routes/api/version.php")]
        );
    }

    #[test]
    fn php_extensionless_require_is_a_path_not_a_namespace() {
        let files = vec![entry("api/bootstrap/autoload"), entry("api/index.php")];
        let graph = resolve_imports(
            &raw("api/index.php", vec!["bootstrap/autoload"]),
            &files,
            &RepoImportConfig::default(),
        );
        assert_eq!(
            graph[&PathBuf::from("api/index.php")],
            vec![PathBuf::from("api/bootstrap/autoload")]
        );
    }

    #[test]
    fn php_tries_every_directory_for_the_longest_psr4_prefix() {
        let cfg = RepoImportConfig {
            psr4: vec![
                Psr4Root {
                    prefix: "Acme".into(),
                    dir: PathBuf::from("src"),
                },
                Psr4Root {
                    prefix: "Acme".into(),
                    dir: PathBuf::from("generated"),
                },
            ],
        };
        let files = vec![entry("generated/Thing.php"), entry("app.php")];
        let graph = resolve_imports(&raw("app.php", vec!["Acme\\Thing"]), &files, &cfg);
        assert_eq!(
            graph[&PathBuf::from("app.php")],
            vec![PathBuf::from("generated/Thing.php")]
        );
    }

    #[test]
    fn php_bare_namespace_does_not_truncate_a_dotted_root_dir() {
        // `use App;` leaves no remainder, so the candidate ends at the root
        // directory itself. `Path::with_extension` REPLACES an extension, so
        // a root like `src/App.Core` would become `src/App.php` — silently
        // resolving into a different tree.
        let cfg = RepoImportConfig {
            psr4: vec![Psr4Root {
                prefix: "App".into(),
                dir: PathBuf::from("src/App.Core"),
            }],
        };
        let candidates = resolve_php_import("App", Path::new("src/Main.php"), &cfg);
        assert_eq!(candidates, vec![PathBuf::from("src/App.Core.php")]);
    }

    #[test]
    fn php_specifier_with_no_matching_root_yields_no_edge() {
        // Vendor namespaces and unconfigured roots must degrade to no edge,
        // never a wrong one.
        let files = vec![entry("api/app/Main.php")];
        let graph = resolve_imports(
            &raw("api/app/Main.php", vec!["Illuminate\\Support\\Facades\\DB"]),
            &files,
            &apios_config(),
        );
        assert!(!graph.contains_key(&PathBuf::from("api/app/Main.php")));
    }

    #[test]
    fn kotlin_import_resolves_to_package_path() {
        let files = vec![entry("com/foo/Bar.kt"), entry("com/foo/App.kt")];
        let graph = resolve_imports(
            &raw("com/foo/App.kt", vec!["com.foo.Bar"]),
            &files,
            &RepoImportConfig::default(),
        );
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
            &RepoImportConfig::default(),
        );
        assert_eq!(
            graph[&PathBuf::from("src/main/kotlin/com/foo/App.kt")],
            vec![PathBuf::from("src/main/kotlin/com/foo/Bar.kt")]
        );
    }

    #[test]
    fn kotlin_import_resolves_in_module_test_source_root() {
        let files = vec![
            entry("app/src/test/kotlin/com/foo/Helper.kt"),
            entry("app/src/test/kotlin/com/foo/AppTest.kt"),
        ];
        let graph = resolve_imports(
            &raw(
                "app/src/test/kotlin/com/foo/AppTest.kt",
                vec!["com.foo.Helper"],
            ),
            &files,
            &RepoImportConfig::default(),
        );
        assert_eq!(
            graph[&PathBuf::from("app/src/test/kotlin/com/foo/AppTest.kt")],
            vec![PathBuf::from("app/src/test/kotlin/com/foo/Helper.kt")]
        );
    }

    #[test]
    fn kotlin_wildcard_import_resolves_every_file_in_package() {
        let files = vec![
            entry("app/src/main/kotlin/com/foo/A.kt"),
            entry("app/src/main/kotlin/com/foo/B.kt"),
            entry("app/src/main/kotlin/com/bar/App.kt"),
        ];
        let graph = resolve_imports(
            &raw("app/src/main/kotlin/com/bar/App.kt", vec!["com.foo.*"]),
            &files,
            &RepoImportConfig::default(),
        );
        assert_eq!(
            graph[&PathBuf::from("app/src/main/kotlin/com/bar/App.kt")],
            vec![
                PathBuf::from("app/src/main/kotlin/com/foo/A.kt"),
                PathBuf::from("app/src/main/kotlin/com/foo/B.kt"),
            ]
        );
    }

    #[test]
    fn kotlin_member_import_resolves_to_the_declaring_file() {
        // Unlike Java, Kotlin routinely imports a member or top-level
        // function: `com.foo.Bar.baz` lives in com/foo/Bar.kt, so the
        // final segment must be droppable.
        let files = vec![entry("com/foo/Bar.kt"), entry("com/foo/App.kt")];
        let graph = resolve_imports(
            &raw("com/foo/App.kt", vec!["com.foo.Bar.baz"]),
            &files,
            &RepoImportConfig::default(),
        );
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
        let graph = resolve_imports(
            &raw("com/foo/App.kt", vec!["com.elsewhere.Bar"]),
            &files,
            &RepoImportConfig::default(),
        );
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
            &RepoImportConfig::default(),
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
        let graph = resolve_imports(
            &raw("src/a/b.ts", vec!["../shared/util"]),
            &files,
            &RepoImportConfig::default(),
        );

        let targets = graph
            .get(&PathBuf::from("src/a/b.ts"))
            .expect("parent-directory import must resolve");
        assert_eq!(targets[0].to_string_lossy(), "src/shared/util.ts");
    }

    #[test]
    fn js_relative_import_resolves_to_normalized_path() {
        let files = vec![entry("web/main.js"), entry("web/lib/api.js")];
        let graph = resolve_imports(
            &raw("web/main.js", vec!["./lib/api"]),
            &files,
            &RepoImportConfig::default(),
        );

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
        let graph = resolve_imports(
            &raw("src/other.rs", vec!["crate"]),
            &files,
            &RepoImportConfig::default(),
        );
        let targets = graph
            .get(&PathBuf::from("src/other.rs"))
            .expect("bare crate specifier must resolve to the crate root");
        assert_eq!(targets[0].to_string_lossy(), "src/lib.rs");
    }

    #[test]
    fn rust_bare_crate_specifier_resolves_to_main_when_no_lib() {
        let files = vec![entry("src/main.rs"), entry("src/other.rs")];
        let graph = resolve_imports(
            &raw("src/other.rs", vec!["crate"]),
            &files,
            &RepoImportConfig::default(),
        );
        let targets = graph
            .get(&PathBuf::from("src/other.rs"))
            .expect("bare crate specifier must resolve to the crate root");
        assert_eq!(targets[0].to_string_lossy(), "src/main.rs");
    }
}
