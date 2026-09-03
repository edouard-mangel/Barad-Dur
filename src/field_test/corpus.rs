use anyhow::{bail, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// One pinned repository in the field-test corpus.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CorpusEntry {
    pub name: String,
    /// Path relative to the corpus root.
    pub path: String,
    /// Commit the analysis is pinned to. Unpinned repos drift and every
    /// diff becomes noise.
    pub pin: String,
    pub lang: String,
    /// Clone URL for a publicly reachable repository. Entries without one
    /// exist only on the maintainer's machine and are skipped under
    /// [`Scope::Public`] (the CI subset).
    #[serde(default)]
    pub url: Option<String>,
}

/// Which corpus entries a run covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Every entry — the maintainer's full local sweep.
    All,
    /// Only entries with a `url` — what CI can clone without credentials.
    Public,
}

/// Parse `BARAD_DUR_CORPUS_SCOPE`; unset means the full corpus.
pub fn parse_scope(raw: Option<&str>) -> Result<Scope> {
    match raw {
        None | Some("all") => Ok(Scope::All),
        Some("public") => Ok(Scope::Public),
        Some(other) => bail!("BARAD_DUR_CORPUS_SCOPE must be `all` or `public`, got `{other}`"),
    }
}

/// Entries covered by `scope`, in manifest order.
pub fn select_entries(entries: Vec<CorpusEntry>, scope: Scope) -> Vec<CorpusEntry> {
    entries
        .into_iter()
        .filter(|entry| scope == Scope::All || entry.url.is_some())
        .collect()
}

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(default)]
    repo: Vec<CorpusEntry>,
}

/// Parse `field-test/corpus.toml`.
pub fn parse_corpus(toml_src: &str) -> Result<Vec<CorpusEntry>> {
    let manifest: Manifest = toml::from_str(toml_src)?;
    if manifest.repo.is_empty() {
        bail!("corpus manifest declares no [[repo]] entries");
    }
    Ok(manifest.repo)
}

/// Absolute path to a corpus entry's repository.
pub fn resolve_path(entry: &CorpusEntry, root: &Path) -> PathBuf {
    root.join(&entry.path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parses_entries_in_declaration_order() {
        let src = r#"
[[repo]]
name = "ripgrep"
path = "ripgrep"
pin  = "3fce3b5b"
lang = "Rust"

[[repo]]
name = "mautic"
path = "mautic"
pin  = "181701cd"
lang = "PHP"
"#;
        let entries = parse_corpus(src).expect("parses");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "ripgrep");
        assert_eq!(entries[0].pin, "3fce3b5b");
        assert_eq!(entries[1].lang, "PHP");
    }

    #[test]
    fn resolves_path_against_corpus_root() {
        let entry = CorpusEntry {
            name: "ripgrep".into(),
            path: "ripgrep".into(),
            pin: "3fce3b5b".into(),
            lang: "Rust".into(),
            url: None,
        };
        assert_eq!(
            resolve_path(&entry, Path::new("/home/edouard/WS")),
            Path::new("/home/edouard/WS/ripgrep")
        );
    }

    #[test]
    fn url_is_optional_and_marks_a_publicly_cloneable_entry() {
        let src = r#"
[[repo]]
name = "ripgrep"
path = "ripgrep"
pin  = "3fce3b5b"
lang = "Rust"
url  = "https://github.com/BurntSushi/ripgrep.git"

[[repo]]
name = "private-app"
path = "private-app"
pin  = "deadbeef"
lang = "TypeScript"
"#;
        let entries = parse_corpus(src).expect("parses");
        assert_eq!(
            entries[0].url.as_deref(),
            Some("https://github.com/BurntSushi/ripgrep.git")
        );
        assert_eq!(entries[1].url, None);
    }

    #[test]
    fn public_scope_keeps_only_entries_with_a_url() {
        let src = r#"
[[repo]]
name = "ripgrep"
path = "ripgrep"
pin  = "3fce3b5b"
lang = "Rust"
url  = "https://github.com/BurntSushi/ripgrep.git"

[[repo]]
name = "private-app"
path = "private-app"
pin  = "deadbeef"
lang = "TypeScript"
"#;
        let entries = parse_corpus(src).expect("parses");
        let public = select_entries(entries.clone(), Scope::Public);
        assert_eq!(public.len(), 1);
        assert_eq!(public[0].name, "ripgrep");
        assert_eq!(select_entries(entries, Scope::All).len(), 2);
    }

    #[test]
    fn scope_parses_from_the_environment_value() {
        assert_eq!(parse_scope(None).unwrap(), Scope::All);
        assert_eq!(parse_scope(Some("all")).unwrap(), Scope::All);
        assert_eq!(parse_scope(Some("public")).unwrap(), Scope::Public);
        assert!(parse_scope(Some("remote")).is_err());
    }

    #[test]
    fn rejects_a_manifest_with_no_repos() {
        assert!(parse_corpus("").is_err());
    }

    #[test]
    fn the_committed_manifest_parses_and_pins_every_repo() {
        let src = include_str!("../../field-test/corpus.toml");
        let entries = parse_corpus(src).expect("committed manifest parses");
        assert_eq!(entries.len(), 11);
        assert!(
            entries.iter().all(|e| e.pin.len() >= 7),
            "every corpus entry must carry a pin"
        );
    }
}
