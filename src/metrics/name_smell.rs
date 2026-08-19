//! Name-based hotspot triage: a generic, responsibility-agnostic file name
//! (`Manager`, `Helper`, `Util`) is itself weak evidence a flagged file has
//! no single responsibility — a cheap annotation on hotspots you already
//! found, not a new source of hotspots (see `god_objects.rs::god_reason`).

use std::path::Path;

const SMELLY_NAME_STEMS: &[&str] = &[
    "manager",
    "helper",
    "util",
    "utils",
    "handler",
    "processor",
    "service",
    "common",
    "misc",
];

/// Cross-language "module index" filenames whose own stem never carries the
/// module's meaning — the meaningful name lives on the parent directory
/// instead (`src/util/mod.rs`, `handlers/index.ts`, `helpers/__init__.py`).
const MODULE_INDEX_STEMS: &[&str] = &["mod", "index", "__init__"];

pub(crate) fn has_smelly_name(path: &Path) -> bool {
    // Case-PRESERVING stem: tokenize_name needs the original casing to find
    // camelCase/PascalCase word boundaries (stem_lower would already have
    // destroyed them by lowercasing the whole stem up front).
    let stem = stem_preserving_case(path);
    if name_has_smelly_word(&stem) {
        return true;
    }
    if !MODULE_INDEX_STEMS.contains(&stem.to_lowercase().as_str()) {
        return false;
    }
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(name_has_smelly_word)
        .unwrap_or(false)
}

/// File name minus its last extension, case preserved — same rule as
/// `file_role::stem_lower` (compound suffixes like `.test.tsx` keep their
/// `.test` part), but without lowercasing, since `tokenize_name` needs the
/// original casing to find word boundaries.
fn stem_preserving_case(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    match name.rfind('.') {
        Some(pos) if pos > 0 => name[..pos].to_string(),
        _ => name.to_string(),
    }
}

/// True if `name` contains one of `SMELLY_NAME_STEMS` as a whole word —
/// not as a substring of an unrelated word. "UncommonCase" and
/// "Serviceable" contain "common"/"service" as raw substrings but are
/// specific, legitimate names, not generic ones; "servicedesk" (no
/// separator, no case boundary) can't be split further and stays one word,
/// which correctly never equals "service".
fn name_has_smelly_word(name: &str) -> bool {
    tokenize_name(name).iter().any(|word| {
        // A trailing 's' is tolerated as a plural, not treated as a
        // substring match — "handlers"/"helpers" must still match
        // "handler"/"helper", but "servicedesk" (no 's' suffix at all)
        // stays unmatched rather than mis-parsing as "service" + "desk".
        let singular = word.strip_suffix('s').unwrap_or(word);
        SMELLY_NAME_STEMS.contains(&word.as_str()) || SMELLY_NAME_STEMS.contains(&singular)
    })
}

/// Split a name into words on `_`/`-`/space and camelCase/PascalCase
/// boundaries (a lowercase-to-uppercase transition), lowercasing each word.
fn tokenize_name(name: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut prev_is_lower = false;
    for c in name.chars() {
        if c == '_' || c == '-' || c == ' ' {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            prev_is_lower = false;
            continue;
        }
        if c.is_uppercase() && prev_is_lower && !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
        current.push(c.to_ascii_lowercase());
        prev_is_lower = c.is_lowercase();
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn smelly_stems_are_detected() {
        assert!(has_smelly_name(&PathBuf::from("src/UserManager.rs")));
        assert!(has_smelly_name(&PathBuf::from("src/user_service.py")));
        assert!(has_smelly_name(&PathBuf::from("src/Helper.ts")));
        assert!(has_smelly_name(&PathBuf::from("src/common.rs")));
    }

    #[test]
    fn non_smelly_names_are_not_flagged() {
        assert!(!has_smelly_name(&PathBuf::from("src/main.rs")));
        assert!(!has_smelly_name(&PathBuf::from("src/engine.rs")));
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert!(has_smelly_name(&PathBuf::from("src/DATA_MANAGER.rs")));
    }

    #[test]
    fn module_index_files_check_the_directory_name_instead() {
        // Rust's mod.rs (and equivalent index.ts/__init__.py conventions)
        // put the meaningful name on the directory, not the file — a
        // bloated src/util/mod.rs is exactly the smell this feature
        // targets, but its own stem ("mod") never matches anything.
        assert!(has_smelly_name(&PathBuf::from("src/util/mod.rs")));
        assert!(has_smelly_name(&PathBuf::from("src/handlers/index.ts")));
        assert!(has_smelly_name(&PathBuf::from("app/helpers/__init__.py")));
    }

    #[test]
    fn module_index_files_in_a_legitimately_named_directory_are_not_flagged() {
        // This repo's own layout (src/metrics/health/mod.rs) must not be
        // flagged — "health" is a legitimate, narrow domain name.
        assert!(!has_smelly_name(&PathBuf::from(
            "src/metrics/health/mod.rs"
        )));
    }

    #[test]
    fn a_top_level_module_index_file_with_no_parent_is_not_flagged() {
        assert!(!has_smelly_name(&PathBuf::from("mod.rs")));
    }

    #[test]
    fn specific_names_containing_a_smelly_stem_as_a_substring_are_not_flagged() {
        // "UncommonCase" and "Serviceable" contain "common" and "service"
        // as raw substrings but are specific, legitimate names, not generic
        // ones — matching must be by whole word, not substring.
        assert!(!has_smelly_name(&PathBuf::from("src/UncommonCase.rs")));
        assert!(!has_smelly_name(&PathBuf::from("src/Serviceable.rs")));
        assert!(!has_smelly_name(&PathBuf::from("src/servicedesk.rs")));
    }

    #[test]
    fn a_module_index_directory_containing_a_smelly_stem_as_a_substring_is_not_flagged() {
        assert!(!has_smelly_name(&PathBuf::from("src/servicedesk/mod.rs")));
    }

    #[test]
    fn camel_case_word_boundaries_are_still_detected_as_separate_words() {
        // The word-boundary fix must not regress the original camelCase
        // detection this feature started with.
        assert!(has_smelly_name(&PathBuf::from("src/UserManager.rs")));
        assert!(has_smelly_name(&PathBuf::from("src/DataUtils.rs")));
    }
}
