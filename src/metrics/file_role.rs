//! Path-based file classification: what *kind* of file is this?
//!
//! Health metrics and hotspot views use the role to separate signal from
//! noise — a 1000-line test suite is not a god object, and a churning CI
//! file is not a code hotspot. Classification is a pure function of the
//! path, so it never touches the snapshot cache.

use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileRole {
    Source,
    Test,
    Config,
    Docs,
    Other,
}

/// Extensions treated as program source code (mirrors the languages the
/// AST collector understands, plus common ones we only count lines for).
pub(crate) fn has_source_extension(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).unwrap_or(""),
        "rs" | "py"
            | "go"
            | "java"
            | "cs"
            | "js"
            | "ts"
            | "tsx"
            | "jsx"
            | "kt"
            | "cpp"
            | "c"
            | "h"
            | "hpp"
            | "rb"
            | "php"
            | "swift"
            | "scala"
    )
}

const TEST_DIR_NAMES: &[&str] = &["test", "tests", "__tests__", "spec", "specs"];
const TEST_STEM_NAMES: &[&str] = &["test", "tests", "testutil", "testutils", "conftest"];
const TEST_STEM_PREFIXES: &[&str] = &["test_", "tests_"];
const TEST_STEM_SUFFIXES: &[&str] = &[
    "_test", "_tests", "_spec", "-test", "-spec", ".test", ".spec",
];

const DOC_EXTENSIONS: &[&str] = &["md", "rst", "adoc", "txt"];
const CONFIG_EXTENSIONS: &[&str] = &[
    "yml",
    "yaml",
    "toml",
    "json",
    "ini",
    "cfg",
    "conf",
    "lock",
    "properties",
];
const CONFIG_FILE_NAMES: &[&str] = &["dockerfile", "makefile", "justfile"];

/// File name minus its last extension, lowercased — compound suffixes like
/// `.test.tsx` keep their `.test` part.
pub(crate) fn stem_lower(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    match name.rfind('.') {
        Some(pos) if pos > 0 => name[..pos].to_lowercase(),
        _ => name.to_lowercase(),
    }
}

fn in_test_dir(path: &Path) -> bool {
    path.components().any(|c| {
        c.as_os_str()
            .to_str()
            .map(|s| TEST_DIR_NAMES.contains(&s.to_lowercase().as_str()))
            .unwrap_or(false)
    })
}

fn has_test_name(path: &Path) -> bool {
    let stem = stem_lower(path);
    TEST_STEM_NAMES.contains(&stem.as_str())
        || TEST_STEM_PREFIXES.iter().any(|p| stem.starts_with(p))
        || TEST_STEM_SUFFIXES.iter().any(|s| stem.ends_with(s))
}

fn is_docs(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    DOC_EXTENSIONS.contains(&ext.to_lowercase().as_str())
        || path.components().any(|c| {
            c.as_os_str()
                .to_str()
                .map(|s| s.eq_ignore_ascii_case("docs"))
                .unwrap_or(false)
        })
}

fn is_config(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    name.starts_with('.')
        || CONFIG_FILE_NAMES.contains(&name.as_str())
        || CONFIG_EXTENSIONS.contains(&ext.to_lowercase().as_str())
        || stem_lower(path).ends_with(".config")
}

/// Classify a repo-relative path. Precedence: Test > Docs > Config > Source.
/// Test wins so `tests/fixtures/data.json` counts as test material, not
/// config; Config beats Source so `vite.config.ts` is not code under review.
pub fn classify(path: &Path) -> FileRole {
    if in_test_dir(path) || has_test_name(path) {
        FileRole::Test
    } else if is_docs(path) {
        FileRole::Docs
    } else if is_config(path) {
        FileRole::Config
    } else if has_source_extension(path) {
        FileRole::Source
    } else {
        FileRole::Other
    }
}

pub(crate) fn pair_stem(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    // Strip only the last extension so compound extensions like .test.ts are preserved
    match name.rfind('.') {
        Some(pos) => name[..pos].to_string(),
        None => name.to_string(),
    }
}

fn is_test_of(prod: &str, test: &str) -> bool {
    test == format!("{}test", prod)
        || test == format!("{}tests", prod)
        || test == format!("{}.test", prod)
        || test == format!("{}.spec", prod)
        || test == format!("{}_test", prod)
        || test == format!("{}_spec", prod)
        || test == format!("test_{}", prod)
}

/// Stem-based source↔test pairing (user.go ↔ user_test.go, parser.ts ↔
/// parser.spec.ts, …). Single source of truth shared by the coupling-pair
/// badge and the safety-net metric — extracted per the M5 precedent so two
/// call sites can't drift on what "a test pair" means.
pub fn is_test_pair(a: &Path, b: &Path) -> bool {
    let (Some(a), Some(b)) = (a.to_str(), b.to_str()) else {
        return false;
    };
    let sa = pair_stem(a).to_lowercase();
    let sb = pair_stem(b).to_lowercase();
    is_test_of(&sa, &sb) || is_test_of(&sb, &sa)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn role(p: &str) -> FileRole {
        classify(&PathBuf::from(p))
    }

    #[test]
    fn is_test_pair_parity_with_scorer_builder_cases() {
        assert!(is_test_pair(
            &PathBuf::from("user.go"),
            &PathBuf::from("user_test.go")
        ));
        assert!(is_test_pair(
            &PathBuf::from("parser.ts"),
            &PathBuf::from("parser.spec.ts")
        ));
        assert!(is_test_pair(
            &PathBuf::from("api.py"),
            &PathBuf::from("test_api.py")
        ));
        assert!(is_test_pair(
            &PathBuf::from("Widget.cs"),
            &PathBuf::from("WidgetTests.cs")
        ));
        assert!(is_test_pair(
            &PathBuf::from("a/b/mod.rs"),
            &PathBuf::from("a/b/mod_test.rs")
        ));
        assert!(!is_test_pair(
            &PathBuf::from("user.go"),
            &PathBuf::from("order_test.go")
        ));
        assert!(!is_test_pair(
            &PathBuf::from("a.rs"),
            &PathBuf::from("b.rs")
        ));
    }

    #[test]
    fn is_test_pair_detects_suffix_test() {
        assert!(is_test_pair(
            &PathBuf::from("src/UserService.java"),
            &PathBuf::from("tests/UserServiceTest.java")
        ));
        assert!(is_test_pair(
            &PathBuf::from("src/UserService.java"),
            &PathBuf::from("tests/UserServiceTests.java")
        ));
        assert!(is_test_pair(
            &PathBuf::from("tests/UserServiceTest.java"),
            &PathBuf::from("src/UserService.java")
        )); // symmetric
    }

    #[test]
    fn is_test_pair_detects_dot_test_spec() {
        assert!(is_test_pair(
            &PathBuf::from("src/parser.ts"),
            &PathBuf::from("src/parser.test.ts")
        ));
        assert!(is_test_pair(
            &PathBuf::from("src/parser.ts"),
            &PathBuf::from("src/parser.spec.ts")
        ));
        assert!(is_test_pair(
            &PathBuf::from("src/parser.test.ts"),
            &PathBuf::from("src/parser.ts")
        ));
    }

    #[test]
    fn is_test_pair_detects_underscore_test_spec() {
        assert!(is_test_pair(
            &PathBuf::from("user.go"),
            &PathBuf::from("user_test.go")
        ));
        assert!(is_test_pair(
            &PathBuf::from("user.go"),
            &PathBuf::from("user_spec.go")
        ));
        assert!(is_test_pair(
            &PathBuf::from("user_test.go"),
            &PathBuf::from("user.go")
        ));
    }

    #[test]
    fn is_test_pair_detects_test_prefix() {
        assert!(is_test_pair(
            &PathBuf::from("user.py"),
            &PathBuf::from("test_user.py")
        ));
        assert!(is_test_pair(
            &PathBuf::from("test_user.py"),
            &PathBuf::from("user.py")
        ));
    }

    #[test]
    fn is_test_pair_case_insensitive() {
        assert!(is_test_pair(
            &PathBuf::from("UserService.cs"),
            &PathBuf::from("USERSERVICETEST.cs")
        ));
    }

    #[test]
    fn is_test_pair_rejects_unrelated_pairs() {
        assert!(!is_test_pair(
            &PathBuf::from("src/user.rs"),
            &PathBuf::from("src/order.rs")
        ));
        assert!(!is_test_pair(
            &PathBuf::from("src/user.rs"),
            &PathBuf::from("src/user_handler.rs")
        ));
    }

    #[test]
    fn plain_code_files_are_source() {
        assert_eq!(role("src/main.rs"), FileRole::Source);
        assert_eq!(role("src/metrics/health/god_objects.rs"), FileRole::Source);
        assert_eq!(
            role("dashboard/src/components/HotspotsView.tsx"),
            FileRole::Source
        );
        assert_eq!(role("src/renderer/templates/shared.js"), FileRole::Source);
    }

    #[test]
    fn files_under_a_test_dir_are_tests() {
        assert_eq!(role("tests/coupling_milestone_1.rs"), FileRole::Test);
        assert_eq!(role("pkg/__tests__/util.js"), FileRole::Test);
        assert_eq!(role("spec/models/user_spec.rb"), FileRole::Test);
    }

    #[test]
    fn test_named_files_are_tests() {
        assert_eq!(role("src/metrics/coupling/tests.rs"), FileRole::Test);
        assert_eq!(role("src/renderer/html/tests_extra.rs"), FileRole::Test);
        assert_eq!(role("src/metrics/testutil.rs"), FileRole::Test);
        assert_eq!(
            role("dashboard/src/components/HotspotsView.test.tsx"),
            FileRole::Test
        );
        assert_eq!(role("src/parser.spec.ts"), FileRole::Test);
        assert_eq!(role("pkg/foo_test.go"), FileRole::Test);
        assert_eq!(role("app/test_views.py"), FileRole::Test);
        assert_eq!(role("app/conftest.py"), FileRole::Test);
    }

    #[test]
    fn test_prefix_requires_separator_so_lookalikes_stay_source() {
        assert_eq!(role("src/components/Testimonials.tsx"), FileRole::Source);
        assert_eq!(role("src/attestation.rs"), FileRole::Source);
    }

    #[test]
    fn test_fixtures_beat_config_extension() {
        assert_eq!(role("tests/fixtures/report.json"), FileRole::Test);
    }

    #[test]
    fn ci_and_config_files_are_config() {
        assert_eq!(role(".gitlab-ci.yml"), FileRole::Config);
        assert_eq!(role(".github/workflows/ci.yml"), FileRole::Config);
        assert_eq!(role("Cargo.toml"), FileRole::Config);
        assert_eq!(role("Cargo.lock"), FileRole::Config);
        assert_eq!(role("package.json"), FileRole::Config);
        assert_eq!(role("Dockerfile"), FileRole::Config);
        assert_eq!(role("Makefile"), FileRole::Config);
        assert_eq!(role(".baraddurignore"), FileRole::Config);
    }

    #[test]
    fn tool_config_scripts_are_config_despite_source_extension() {
        assert_eq!(role("dashboard/vite.config.ts"), FileRole::Config);
        assert_eq!(role("dashboard/tailwind.config.js"), FileRole::Config);
        assert_eq!(role("eslint.config.js"), FileRole::Config);
    }

    #[test]
    fn docs_are_docs() {
        assert_eq!(role("README.md"), FileRole::Docs);
        assert_eq!(role("docs/architecture.rst"), FileRole::Docs);
        assert_eq!(role("CHANGELOG.txt"), FileRole::Docs);
    }

    #[test]
    fn unknown_extensions_are_other() {
        assert_eq!(role("assets/logo.svg"), FileRole::Other);
        assert_eq!(role("scripts/build.sh"), FileRole::Other);
    }

    #[test]
    fn serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&FileRole::Source).unwrap(),
            "\"source\""
        );
        assert_eq!(serde_json::to_string(&FileRole::Test).unwrap(), "\"test\"");
    }
}
