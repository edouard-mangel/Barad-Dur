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
        };
        assert_eq!(
            resolve_path(&entry, Path::new("/home/edouard/WS")),
            Path::new("/home/edouard/WS/ripgrep")
        );
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
