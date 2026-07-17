//! `.baraddurignore` — repository-root ignore file with full `.gitignore` semantics.
//!
//! It is the middle of three exclusion layers, in precedence order (highest first):
//! 1. CLI `--exclude` / `--exclude-ext` — force-exclude;
//! 2. `.baraddurignore` — its `!` rules re-include, overriding the defaults;
//! 3. built-in defaults ([`super::exclude::is_excluded_by_defaults`], toggled by
//!    `--no-default-excludes`).
//!
//! A whitelist rule such as `!vendor/app.min.js` re-includes a file the built-in
//! defaults would drop — but a CLI `--exclude` still wins over it.

use std::path::Path;

use anyhow::{Context, Result};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::Match;

use super::exclude::{is_excluded_by_cli, is_excluded_by_defaults};

/// Name of the ignore file, looked up at the repository root.
const IGNORE_FILE_NAME: &str = ".baraddurignore";

/// Parsed `.baraddurignore` matcher. Empty (a no-op) when the file is absent.
pub struct BaradDurIgnore {
    matcher: Gitignore,
}

impl BaradDurIgnore {
    /// Load `.baraddurignore` from `repo_root`. Returns an empty (no-op) matcher
    /// when the file does not exist. A malformed line is reported to stderr and
    /// skipped — a bad pattern never aborts analysis.
    pub fn load(repo_root: &Path) -> Result<Self> {
        let path = repo_root.join(IGNORE_FILE_NAME);
        let mut builder = GitignoreBuilder::new(repo_root);
        if path.exists() {
            if let Some(err) = builder.add(&path) {
                eprintln!("Warning: skipping invalid {IGNORE_FILE_NAME} pattern(s): {err}");
            }
        }
        let matcher = builder
            .build()
            .with_context(|| format!("Failed to parse {IGNORE_FILE_NAME}"))?;
        Ok(Self { matcher })
    }

    /// Decision for a repo-root-relative path:
    /// - `Some(true)` — an ignore rule matched → drop the file,
    /// - `Some(false)` — a whitelist (`!`) rule matched last → keep it (override),
    /// - `None` — no rule matched → defer to the other exclusion sources.
    ///
    /// `matched_path_or_any_parents` re-tests each parent directory, so a
    /// trailing-slash directory pattern (e.g. `build/`) drops the files nested
    /// beneath it even though every tracked path is a file (`is_dir = false`).
    pub fn decision(&self, rel_path: &Path) -> Option<bool> {
        match self.matcher.matched_path_or_any_parents(rel_path, false) {
            Match::None => None,
            Match::Ignore(_) => Some(true),
            Match::Whitelist(_) => Some(false),
        }
    }

    /// Build a matcher from in-memory contents. Test seam — avoids disk IO.
    #[cfg(test)]
    pub(crate) fn from_lines(repo_root: &Path, contents: &str) -> Result<Self> {
        let mut builder = GitignoreBuilder::new(repo_root);
        for line in contents.lines() {
            builder
                .add_line(None, line)
                .map_err(|e| anyhow::anyhow!(e))?;
        }
        let matcher = builder.build()?;
        Ok(Self { matcher })
    }
}

/// Whether a file survives all exclusion layers, in precedence order:
/// 1. CLI `--exclude` / `--exclude-ext` (`cli_patterns` / `cli_extensions`) —
///    force-exclude; a `.baraddurignore` whitelist cannot resurrect it;
/// 2. `.baraddurignore` — a `!` whitelist re-includes, overriding the defaults;
/// 3. built-in defaults (when `use_defaults`).
pub fn should_include(
    ignore: &BaradDurIgnore,
    path: &Path,
    cli_patterns: &[String],
    cli_extensions: &[String],
    use_defaults: bool,
) -> bool {
    // Highest precedence: a CLI flag force-excludes, beating any `.baraddurignore`
    // whitelist.
    if is_excluded_by_cli(path, cli_patterns, cli_extensions) {
        return false;
    }
    match ignore.decision(path) {
        Some(false) => true, // whitelist (`!`) → keep, overriding the defaults below
        Some(true) => false, // ignore rule → drop
        None => !(use_defaults && is_excluded_by_defaults(path)), // defaults only
    }
}

/// A stable fingerprint of every exclusion input that affects which files a
/// snapshot contains: the CLI `--exclude`/`--exclude-ext` values, whether the
/// built-in defaults apply, and the current `.baraddurignore` contents. Cached
/// snapshots are keyed on this (alongside HEAD) so changing exclusions forces a
/// re-collection even when HEAD is unchanged.
pub fn exclude_fingerprint(
    repo_root: &Path,
    cli_patterns: &[String],
    cli_extensions: &[String],
    use_defaults: bool,
) -> u64 {
    use std::hash::{Hash, Hasher};
    // DefaultHasher::new() has a fixed seed, so the result is stable across runs.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    use_defaults.hash(&mut hasher);
    cli_patterns.hash(&mut hasher);
    cli_extensions.hash(&mut hasher);
    // `Option<Vec<u8>>` distinguishes an absent file from an empty one and captures
    // any edit to its contents.
    std::fs::read(repo_root.join(IGNORE_FILE_NAME))
        .ok()
        .hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    fn ignore_from(contents: &str) -> BaradDurIgnore {
        // Patterns are matched relative to this root; a stable dummy root is fine.
        BaradDurIgnore::from_lines(Path::new("/repo"), contents).unwrap()
    }

    #[test]
    fn absent_file_is_noop() {
        let dir = TempDir::new().unwrap();
        let ignore = BaradDurIgnore::load(dir.path()).unwrap();
        // No file → no rule matches any path.
        assert_eq!(ignore.decision(Path::new("src/main.rs")), None);
        assert_eq!(ignore.decision(Path::new("a/b/c.min.js")), None);
    }

    #[test]
    fn simple_ignore_drops_file() {
        let ignore = ignore_from("*.log\n");
        assert_eq!(ignore.decision(Path::new("a/b.log")), Some(true));
    }

    #[test]
    fn negation_reincludes_default_excluded() {
        // `dist/app.min.js` is dropped by the built-in `min.js` compound default;
        // a whitelist rule must re-include it (the headline behaviour).
        let ignore = ignore_from("!dist/app.min.js\n");
        assert!(should_include(
            &ignore,
            Path::new("dist/app.min.js"),
            &[],
            &[],
            true,
        ));
    }

    #[test]
    fn comments_and_blank_lines_ignored() {
        let ignore = ignore_from("# a comment\n\n*.tmp\n");
        assert_eq!(ignore.decision(Path::new("x.tmp")), Some(true));
        assert_eq!(ignore.decision(Path::new("a-comment")), None);
    }

    #[test]
    fn leading_slash_anchors_to_root() {
        let ignore = ignore_from("/build.log\n");
        assert_eq!(ignore.decision(Path::new("build.log")), Some(true));
        assert_eq!(ignore.decision(Path::new("sub/build.log")), None);
    }

    #[test]
    fn trailing_slash_is_directory_only() {
        let ignore = ignore_from("logs/\n");
        // File under the ignored directory drops via the parent-directory check.
        assert_eq!(ignore.decision(Path::new("logs/app.log")), Some(true));
    }

    #[test]
    fn last_match_wins() {
        let keep = ignore_from("*.js\n!keep.js\n");
        assert_eq!(keep.decision(Path::new("keep.js")), Some(false));
        let drop = ignore_from("!keep.js\n*.js\n");
        assert_eq!(drop.decision(Path::new("keep.js")), Some(true));
    }

    #[test]
    fn double_star_matches_nested() {
        let ignore = ignore_from("**/generated/**\n");
        assert_eq!(ignore.decision(Path::new("a/generated/b/c.ts")), Some(true));
    }

    #[test]
    fn should_include_falls_through_to_defaults_when_silent() {
        let ignore = ignore_from("*.log\n");
        // README.md is excluded by the default extension list; the ignore file is
        // silent about it, so the default still applies.
        assert!(!should_include(
            &ignore,
            Path::new("README.md"),
            &[],
            &[],
            true,
        ));
    }

    #[test]
    fn should_include_explicit_ignore_drops_source() {
        let ignore = ignore_from("src/main.rs\n");
        assert!(!should_include(
            &ignore,
            Path::new("src/main.rs"),
            &[],
            &[],
            true,
        ));
    }

    #[test]
    fn cli_flag_beats_baraddurignore_whitelist() {
        // CLI is the highest-precedence layer: `--exclude **/keep.rs` force-drops
        // the file even though `.baraddurignore` tries to re-include it.
        let ignore = ignore_from("!keep.rs\n");
        let cli = vec!["**/keep.rs".to_string()];
        assert!(!should_include(
            &ignore,
            Path::new("keep.rs"),
            &cli,
            &[],
            false
        ));
    }

    #[test]
    fn cli_flag_force_excludes_when_ignore_silent() {
        let ignore = ignore_from(""); // no rules
        let cli = vec!["**/*.rs".to_string()];
        assert!(!should_include(
            &ignore,
            Path::new("src/main.rs"),
            &cli,
            &[],
            false
        ));
    }

    #[test]
    fn non_utf8_path_does_not_panic() {
        let ignore = ignore_from("*.log\n");
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let p = Path::new(std::ffi::OsStr::from_bytes(b"src/\xffbad.rs"));
            let _ = ignore.decision(p); // must not panic
        }
        #[cfg(not(unix))]
        {
            let _ = ignore.decision(Path::new("src/bad.rs"));
        }
    }

    #[test]
    fn exclude_fingerprint_reflects_all_inputs() {
        let dir = TempDir::new().unwrap();
        let base = exclude_fingerprint(dir.path(), &[], &[], true);
        // Toggling defaults changes it.
        assert_ne!(base, exclude_fingerprint(dir.path(), &[], &[], false));
        // A CLI pattern changes it.
        let cli = vec!["*.log".to_string()];
        assert_ne!(base, exclude_fingerprint(dir.path(), &cli, &[], true));
        // Writing / editing `.baraddurignore` changes it.
        std::fs::write(dir.path().join(".baraddurignore"), "*.tmp\n").unwrap();
        let with_file = exclude_fingerprint(dir.path(), &[], &[], true);
        assert_ne!(base, with_file);
        std::fs::write(dir.path().join(".baraddurignore"), "*.bak\n").unwrap();
        assert_ne!(with_file, exclude_fingerprint(dir.path(), &[], &[], true));
    }
}
