//! PSR-4 autoload roots, read from `composer.json`.
//!
//! PHP has no directory convention to lean on the way Java and Kotlin do:
//! a namespace maps to a directory only because `composer.json` says so.
//! In a real Laravel repository `App\` maps to `app/` — guessable — but
//! `Programming\` maps to `contexts/programming/app/`, which is not. So a
//! namespace-to-path heuristic is not a weaker version of this; it is
//! wrong, and reading the manifest is the only way to resolve PHP imports.

use std::path::{Path, PathBuf};

/// One PSR-4 mapping: a namespace prefix and the directories it resolves
/// into, each already made relative to the repository root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Psr4Root {
    /// Namespace prefix, without its trailing separator (`App`).
    pub prefix: String,
    /// Repo-root-relative directory the prefix maps to (`api/app`).
    pub dir: PathBuf,
}

/// PSR-4 roots declared by one `composer.json`, with each directory
/// rebased onto `manifest_dir` so the results are repo-root-relative and
/// directly comparable with the collector's file list.
///
/// `autoload-dev` counts alongside `autoload`: test files import through
/// those prefixes, and a resolver that ignored them would silently drop
/// every edge out of the test suite. Anything malformed yields no roots
/// rather than an error — a broken manifest must not fail collection.
pub(crate) fn psr4_roots(manifest_dir: &Path, json: &str) -> Vec<Psr4Root> {
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    ["autoload", "autoload-dev"]
        .iter()
        .filter_map(|section| doc.get(section)?.get("psr-4")?.as_object())
        .flat_map(|map| map.iter())
        .filter_map(|(prefix, target)| {
            let dir = target.as_str()?;
            Some(Psr4Root {
                prefix: prefix.trim_end_matches('\\').to_string(),
                dir: rebase(manifest_dir, dir),
            })
        })
        .collect()
}

/// Every PSR-4 root declared anywhere in the tree.
///
/// `read` supplies a manifest's content, and differs by collection path: the
/// working-tree pass reads from disk, the historical pass reads the blob at
/// that commit. A manifest that cannot be read is skipped rather than
/// failing collection — the historical path can legitimately miss one.
pub(crate) fn psr4_roots_from_tree(
    files: &[crate::snapshot::FileEntry],
    read: impl Fn(&crate::snapshot::FileEntry) -> Option<String>,
) -> Vec<Psr4Root> {
    files
        .iter()
        .filter(|entry| entry.path.file_name().is_some_and(|n| n == "composer.json"))
        .filter_map(|entry| {
            let json = read(entry)?;
            let dir = entry.path.parent().unwrap_or(Path::new(""));
            Some(psr4_roots(dir, &json))
        })
        .flatten()
        .collect()
}

/// Join a manifest-relative PSR-4 target onto the manifest's own directory,
/// dropping the trailing separator PSR-4 targets conventionally carry.
fn rebase(manifest_dir: &Path, target: &str) -> PathBuf {
    let target = target.trim_end_matches('/');
    if manifest_dir.as_os_str().is_empty() {
        PathBuf::from(target)
    } else {
        manifest_dir.join(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::FileEntry;

    fn file(path: &str) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            size_bytes: 100,
            is_binary: false,
            depth: 1,
            blob_oid: String::new(),
        }
    }

    #[test]
    fn roots_are_collected_from_every_manifest_in_the_tree() {
        // A monorepo carries several composer.json files; each one's targets
        // are relative to its own directory, so they cannot be parsed against
        // a single root.
        let files = vec![
            file("api/composer.json"),
            file("packages/cli/composer.json"),
            file("api/app/Main.php"),
        ];
        let roots = psr4_roots_from_tree(&files, |entry| match entry.path.to_str().unwrap() {
            "api/composer.json" => Some(r#"{"autoload":{"psr-4":{"App\\":"app/"}}}"#.into()),
            "packages/cli/composer.json" => {
                Some(r#"{"autoload":{"psr-4":{"Cli\\":"src/"}}}"#.into())
            }
            _ => None,
        });
        let mut rendered: Vec<String> = roots
            .iter()
            .map(|r| format!("{}={}", r.prefix, r.dir.display()))
            .collect();
        rendered.sort();
        assert_eq!(rendered, vec!["App=api/app", "Cli=packages/cli/src"]);
    }

    #[test]
    fn an_unreadable_manifest_is_skipped_rather_than_failing() {
        // The historical path reads blobs, which can be missing; collection
        // must continue with whatever roots it could read.
        let files = vec![file("api/composer.json"), file("bad/composer.json")];
        let roots = psr4_roots_from_tree(&files, |entry| {
            (entry.path.starts_with("api"))
                .then(|| r#"{"autoload":{"psr-4":{"App\\":"app/"}}}"#.to_string())
        });
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].dir, PathBuf::from("api/app"));
    }

    #[test]
    fn files_that_are_not_manifests_are_ignored() {
        let files = vec![file("api/app/composer.json.bak"), file("api/app/Main.php")];
        let roots = psr4_roots_from_tree(&files, |_| {
            Some(r#"{"autoload":{"psr-4":{"X\\":"x/"}}}"#.to_string())
        });
        assert!(roots.is_empty());
    }

    /// The real `api/composer.json` from a Laravel monorepo: four roots,
    /// one of which (`Programming\`) is deliberately unguessable.
    const APIOS_COMPOSER: &str = r#"{
      "autoload": {
        "psr-4": {
          "App\\": "app/",
          "Programming\\": "contexts/programming/app/",
          "Database\\Factories\\": "database/factories/",
          "Database\\Seeders\\": "database/seeders/"
        }
      },
      "autoload-dev": {
        "psr-4": { "Tests\\": "tests/" }
      }
    }"#;

    fn roots_of(manifest_dir: &str, json: &str) -> Vec<Psr4Root> {
        let mut r = psr4_roots(Path::new(manifest_dir), json);
        r.sort_by(|a, b| a.prefix.cmp(&b.prefix));
        r
    }

    #[test]
    fn psr4_roots_are_relative_to_the_manifest_not_the_repo() {
        // composer.json lives in api/, so `app/` is api/app — resolving
        // against the repo root would miss every file.
        let roots = roots_of("api", APIOS_COMPOSER);
        let app = roots.iter().find(|r| r.prefix == "App").expect("App root");
        assert_eq!(app.dir, PathBuf::from("api/app"));
    }

    #[test]
    fn psr4_reads_roots_that_no_heuristic_would_guess() {
        let roots = roots_of("api", APIOS_COMPOSER);
        let prog = roots
            .iter()
            .find(|r| r.prefix == "Programming")
            .expect("Programming root");
        assert_eq!(prog.dir, PathBuf::from("api/contexts/programming/app"));
    }

    #[test]
    fn psr4_includes_autoload_dev_and_multi_segment_prefixes() {
        let roots = roots_of("api", APIOS_COMPOSER);
        let prefixes: Vec<&str> = roots.iter().map(|r| r.prefix.as_str()).collect();
        assert_eq!(
            prefixes,
            vec![
                "App",
                "Database\\Factories",
                "Database\\Seeders",
                "Programming",
                "Tests"
            ],
            "autoload-dev roots count too — test files import through them"
        );
    }

    #[test]
    fn a_manifest_at_the_repo_root_yields_bare_directories() {
        let roots = roots_of("", r#"{"autoload":{"psr-4":{"Src\\":"src/"}}}"#);
        assert_eq!(roots[0].dir, PathBuf::from("src"));
    }

    #[test]
    fn malformed_or_absent_autoload_yields_no_roots_rather_than_failing() {
        // A composer.json without psr-4, or one that is not valid JSON at
        // all, must degrade to "no mapping" — never panic the collector.
        assert!(psr4_roots(Path::new("api"), "{}").is_empty());
        assert!(psr4_roots(Path::new("api"), "not json at all").is_empty());
        assert!(psr4_roots(Path::new("api"), r#"{"autoload":{}}"#).is_empty());
        assert!(psr4_roots(Path::new("api"), r#"{"autoload":{"psr-4":[]}}"#).is_empty());
    }
}
