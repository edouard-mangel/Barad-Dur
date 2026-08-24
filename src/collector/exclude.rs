use std::path::Path;

/// Default file extensions excluded from analysis (translation/resource files).
/// These files change together by definition and inflate coupling/churn metrics.
const DEFAULT_EXCLUDE_EXTENSIONS: &[&str] = &[
    // Translation / resource files
    "resx", "po", "pot", "xlf", "xliff", "strings", "arb", "lproj",
    // Documentation files
    "md", "txt", "rst", "adoc", "textile",
];

/// Default compound extensions excluded from analysis (generated files).
/// These use suffix matching on the full filename (e.g. "pb.go" matches "user.pb.go").
const DEFAULT_EXCLUDE_COMPOUND_EXTENSIONS: &[&str] = &[
    // Protocol Buffers generated (compound extensions)
    "pb.go",
    "pb.h",
    "pb.cc",
    "pb.swift",
    // C# generated
    "g.cs",
    "generated.cs",
    // TypeScript declarations
    "d.ts",
    // Minified assets
    "min.js",
    "min.css",
];

/// Default path patterns excluded from analysis (tooling config, lockfiles).
/// Lockfiles inflate churn/coupling metrics without reflecting real code changes.
const DEFAULT_EXCLUDE_PATTERNS: &[&str] = &[
    // Lockfiles — use **/ prefix so nested paths (monorepos) are also excluded
    "**/pnpm-lock.yaml",
    "**/package-lock.json",
    "**/yarn.lock",
    "**/Cargo.lock",
    "**/Gemfile.lock",
    "**/poetry.lock",
    "**/composer.lock",
    "**/go.sum",
    "**/flake.lock",
    "**/*.lock",
    // App / environment config (churn noise, not real code changes)
    "**/appsettings*.json",
    "**/launchSettings.json",
    "**/.env*",
    // Generated Mock Service Worker browser bundles
    "**/mockServiceWorker.js",
    // Tooling directories
    ".claude/**",
    ".cursor/**",
    ".idea/**",
    ".vscode/**",
    // Barad-dur's own artifacts (don't measure our own config/cache)
    "**/.baraddurignore",
    ".repository-analysis/**",
    // ORM migrations / generated schemas (auto-generated, inflate churn)
    "**/Migrations/*.Designer.cs",
    "**/Migrations/*ModelSnapshot.cs",
    "**/migrations/*.py",
    "db/schema.rb",
    "prisma/migrations/**",
    "alembic/versions/**",
    // Internationalization / translation directories
    "**/i18n/**",
    "**/l10n/**",
    "**/locales/**",
    "**/locale/**",
    // Generated build artefact directories
    "**/node_modules/**",
    "**/vendor/**",
    "**/__pycache__/**",
    "**/*.egg-info/**",
    "**/target/**",
    "**/.next/**",
    "**/.nuxt/**",
    "**/out/**",
    "**/gen/**",
    "**/generated/**",
    "**/.gradle/**",
    "**/.mvn/**",
    "**/build/**",
    // Python protobuf generated (compound suffix, not a plain extension)
    "**/*_pb2.py",
];

/// True when the built-in default exclusions match `path` — by known
/// extension, compound extension (e.g. `min.js`, `_pb2.py`), or path glob.
pub fn is_excluded_by_defaults(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    let path_lower = path_str.to_lowercase();

    let by_extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|ext| ext.to_lowercase())
        .is_some_and(|ext| DEFAULT_EXCLUDE_EXTENSIONS.iter().any(|&e| e == ext));

    by_extension
        || DEFAULT_EXCLUDE_COMPOUND_EXTENSIONS
            .iter()
            .any(|&e| path_lower.ends_with(&format!(".{}", e)))
        || DEFAULT_EXCLUDE_PATTERNS
            .iter()
            .any(|p| glob_match::glob_match(p, &path_str))
}

/// True when the user-specified CLI layers match `path`: `--exclude` glob
/// patterns, or `--exclude-ext` extensions (simple and compound, e.g.
/// "jar", "min.js").
pub fn is_excluded_by_cli(path: &Path, patterns: &[String], extensions: &[String]) -> bool {
    let path_str = path.to_string_lossy();
    let path_lower = path_str.to_lowercase();

    extensions.iter().any(|ext| {
        let ext_lower = ext.trim_start_matches('.').to_lowercase();
        path_lower.ends_with(&format!(".{}", ext_lower))
    }) || patterns
        .iter()
        .any(|pattern| glob_match::glob_match(pattern, &path_str))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pre-split composition — kept so every historical pin below keeps
    /// asserting the same combined behavior the production layers compose.
    fn is_excluded(
        path: &Path,
        patterns: &[String],
        extensions: &[String],
        use_defaults: bool,
    ) -> bool {
        (use_defaults && is_excluded_by_defaults(path))
            || is_excluded_by_cli(path, patterns, extensions)
    }

    #[test]
    fn is_excluded_matches_default_extensions() {
        let p = Path::new("src/Resources/Strings.resx");
        assert!(is_excluded(p, &[], &[], true));
        assert!(!is_excluded(p, &[], &[], false));
    }

    #[test]
    fn is_excluded_matches_po_files() {
        assert!(is_excluded(
            Path::new("locale/fr/messages.po"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(Path::new("lang/en.pot"), &[], &[], true));
        assert!(is_excluded(Path::new("i18n/strings.xlf"), &[], &[], true));
    }

    #[test]
    fn is_excluded_matches_user_globs() {
        let patterns = vec!["**/i18n/**".to_string()];
        assert!(is_excluded(
            Path::new("src/assets/i18n/sfk-messages/fr-FR.ts"),
            &patterns,
            &[],
            false
        ));
        assert!(!is_excluded(
            Path::new("src/main.rs"),
            &patterns,
            &[],
            false
        ));
    }

    #[test]
    fn is_excluded_combines_defaults_and_user_patterns() {
        let patterns = vec!["**/i18n/**".to_string()];
        // Matched by default extension
        assert!(is_excluded(Path::new("foo.resx"), &patterns, &[], true));
        // Matched by user pattern
        assert!(is_excluded(
            Path::new("src/i18n/en.ts"),
            &patterns,
            &[],
            true
        ));
        // Not matched by either
        assert!(!is_excluded(Path::new("src/main.rs"), &patterns, &[], true));
    }

    #[test]
    fn is_excluded_matches_config_files_by_default() {
        assert!(is_excluded(
            Path::new("src/server/BusinessHub.API/appsettings.json"),
            &[],
            &[],
            true,
        ));
        assert!(is_excluded(
            Path::new("src/server/BusinessHub.API/appsettings.Development.json"),
            &[],
            &[],
            true,
        ));
        assert!(is_excluded(
            Path::new("Properties/launchSettings.json"),
            &[],
            &[],
            true,
        ));
        assert!(is_excluded(Path::new("some/path/foo.lock"), &[], &[], true));
        assert!(is_excluded(Path::new(".env.production"), &[], &[], true));
        // Regular JSON should NOT be excluded
        assert!(!is_excluded(
            Path::new("src/data/schema.json"),
            &[],
            &[],
            true
        ));
    }

    #[test]
    fn is_excluded_matches_i18n_directories_by_default() {
        assert!(is_excluded(
            Path::new("src/client/src/assets/i18n/sfk-messages/en-US.ts"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new("app/l10n/strings_fr.arb"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new("src/locales/en.json"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new("config/locale/fr.yml"),
            &[],
            &[],
            true
        ));
        // Non-i18n .ts files should NOT be excluded
        assert!(!is_excluded(Path::new("src/main.ts"), &[], &[], true));
    }

    #[test]
    fn is_excluded_case_insensitive_extension() {
        assert!(is_excluded(Path::new("Strings.RESX"), &[], &[], true));
        assert!(is_excluded(Path::new("lang.Resx"), &[], &[], true));
    }

    #[test]
    fn is_excluded_matches_documentation_files() {
        assert!(is_excluded(Path::new("README.md"), &[], &[], true));
        assert!(is_excluded(Path::new("docs/guide.rst"), &[], &[], true));
        assert!(is_excluded(Path::new("CHANGELOG.txt"), &[], &[], true));
        assert!(is_excluded(Path::new("docs/api.adoc"), &[], &[], true));
        assert!(is_excluded(Path::new("notes.textile"), &[], &[], true));
        // Not excluded when defaults disabled
        assert!(!is_excluded(Path::new("README.md"), &[], &[], false));
    }

    #[test]
    fn is_excluded_matches_default_lockfiles() {
        assert!(is_excluded(Path::new("pnpm-lock.yaml"), &[], &[], true));
        assert!(is_excluded(Path::new("package-lock.json"), &[], &[], true));
        assert!(is_excluded(Path::new("yarn.lock"), &[], &[], true));
        assert!(is_excluded(Path::new("Cargo.lock"), &[], &[], true));
        assert!(is_excluded(Path::new("go.sum"), &[], &[], true));
        assert!(is_excluded(Path::new("poetry.lock"), &[], &[], true));
        // Not excluded when defaults disabled
        assert!(!is_excluded(Path::new("pnpm-lock.yaml"), &[], &[], false));
    }

    #[test]
    fn is_excluded_matches_nested_lockfiles() {
        // Monorepo layout: lockfiles in subdirectories must also be excluded
        assert!(is_excluded(
            Path::new("apps/web/pnpm-lock.yaml"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new("packages/ui/package-lock.json"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new("services/api/go.sum"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(Path::new("backend/Cargo.lock"), &[], &[], true));
    }

    #[test]
    fn is_excluded_matches_orm_generated_files() {
        // EF Core
        assert!(is_excluded(
            Path::new("Data/Migrations/20240101_Init.Designer.cs"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new("Data/Migrations/AppDbContextModelSnapshot.cs"),
            &[],
            &[],
            true
        ));
        // Django
        assert!(is_excluded(
            Path::new("myapp/migrations/0001_initial.py"),
            &[],
            &[],
            true
        ));
        // Rails
        assert!(is_excluded(Path::new("db/schema.rb"), &[], &[], true));
        // Prisma
        assert!(is_excluded(
            Path::new("prisma/migrations/20240101/migration.sql"),
            &[],
            &[],
            true
        ));
        // Regular source should not match
        assert!(!is_excluded(
            Path::new("src/Models/User.cs"),
            &[],
            &[],
            true
        ));
    }

    #[test]
    fn is_excluded_matches_default_tooling_dirs() {
        assert!(is_excluded(
            Path::new(".claude/settings.json"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new(".cursor/rules/my-rule"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new(".idea/workspace.xml"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new(".vscode/settings.json"),
            &[],
            &[],
            true
        ));
        // Not excluded when defaults disabled
        assert!(!is_excluded(
            Path::new(".claude/settings.json"),
            &[],
            &[],
            false
        ));
    }

    #[test]
    fn is_excluded_matches_baraddur_own_artifacts() {
        // Barad-dur should not measure its own config/cache files.
        assert!(is_excluded(Path::new(".baraddurignore"), &[], &[], true));
        assert!(is_excluded(
            Path::new(".repository-analysis/snapshot.bin"),
            &[],
            &[],
            true
        ));
        // Respect the defaults toggle.
        assert!(!is_excluded(Path::new(".baraddurignore"), &[], &[], false));
    }

    #[test]
    fn is_excluded_by_user_extension_simple() {
        let exts = vec!["jar".to_string()];
        assert!(is_excluded(Path::new("lib/commons.jar"), &[], &exts, false));
        assert!(is_excluded(Path::new("deps/app.JAR"), &[], &exts, false));
        assert!(!is_excluded(Path::new("src/main.rs"), &[], &exts, false));
    }

    #[test]
    fn is_excluded_by_user_extension_compound() {
        let exts = vec!["min.js".to_string()];
        assert!(is_excluded(
            Path::new("dist/bundle.min.js"),
            &[],
            &exts,
            false
        ));
        assert!(!is_excluded(Path::new("src/app.js"), &[], &exts, false));
    }

    #[test]
    fn is_excluded_extension_leading_dot_normalised() {
        // Users may write ".jar" or "jar" — both should work.
        let exts = vec![".jar".to_string()];
        assert!(is_excluded(Path::new("lib/foo.jar"), &[], &exts, false));
    }

    #[test]
    fn is_excluded_file_without_extension_not_excluded() {
        let exts = vec!["jar".to_string()];
        assert!(!is_excluded(Path::new("Makefile"), &[], &exts, false));
        assert!(!is_excluded(Path::new("Dockerfile"), &[], &exts, false));
        assert!(!is_excluded(Path::new("LICENSE"), &[], &exts, false));
    }

    #[test]
    fn is_excluded_dotted_directory_not_confused_with_extension() {
        let exts = vec!["2".to_string()];
        // "src/v1.2/main.rs" ends with ".rs", not ".2"
        assert!(!is_excluded(
            Path::new("src/v1.2/main.rs"),
            &[],
            &exts,
            false
        ));
        // but a file literally named "v1.2" (last extension is "2") is excluded
        assert!(is_excluded(Path::new("src/v1.2"), &[], &exts, false));
    }

    #[test]
    fn is_excluded_extension_independent_of_defaults() {
        // User extensions work even when defaults are off.
        let exts = vec!["jar".to_string()];
        assert!(is_excluded(Path::new("lib/foo.jar"), &[], &exts, false));
        // And do not suppress defaults when on.
        assert!(is_excluded(Path::new("README.md"), &[], &exts, true));
    }

    #[test]
    fn is_excluded_matches_generated_directories() {
        assert!(is_excluded(
            Path::new("node_modules/lodash/index.js"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new("vendor/github.com/foo/bar.go"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new("src/__pycache__/utils.cpython-311.pyc"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new("myapp.egg-info/PKG-INFO"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new("target/debug/build/out/main.rs"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new(".next/server/pages/index.js"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new(".nuxt/components.d.ts"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(Path::new("out/Release/chrome"), &[], &[], true));
        assert!(is_excluded(
            Path::new("src/gen/proto/user.go"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new("src/generated/api/client.ts"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(Path::new(".gradle/caches/foo"), &[], &[], true));
        assert!(is_excluded(
            Path::new(".mvn/wrapper/maven-wrapper.jar"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new("build/outputs/apk/debug.apk"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(Path::new("proto/user_pb2.py"), &[], &[], true));
        // dist is intentionally NOT excluded
        assert!(!is_excluded(Path::new("dist/published.js"), &[], &[], true));
        // regular source must NOT be excluded
        assert!(!is_excluded(Path::new("src/main.rs"), &[], &[], true));
        // use_defaults=false disables these
        assert!(!is_excluded(
            Path::new("node_modules/foo/bar.js"),
            &[],
            &[],
            false
        ));
    }

    #[test]
    fn is_excluded_matches_generated_extensions() {
        // Protocol Buffers
        assert!(is_excluded(Path::new("proto/user.pb.go"), &[], &[], true));
        assert!(is_excluded(Path::new("proto/user.pb.h"), &[], &[], true));
        assert!(is_excluded(Path::new("proto/user.pb.cc"), &[], &[], true));
        assert!(is_excluded(
            Path::new("proto/user.pb.swift"),
            &[],
            &[],
            true
        ));
        // C# generated
        assert!(is_excluded(
            Path::new("src/Api/Client.g.cs"),
            &[],
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new("src/Api/Client.generated.cs"),
            &[],
            &[],
            true
        ));
        // TypeScript declarations
        assert!(is_excluded(Path::new("types/index.d.ts"), &[], &[], true));
        // Minified assets
        assert!(is_excluded(Path::new("dist/app.min.js"), &[], &[], true));
        assert!(is_excluded(
            Path::new("dist/styles.min.css"),
            &[],
            &[],
            true
        ));
        // Regular source should still pass
        assert!(!is_excluded(Path::new("src/main.rs"), &[], &[], true));
        assert!(!is_excluded(Path::new("src/user.go"), &[], &[], true));
        assert!(!is_excluded(Path::new("src/client.ts"), &[], &[], true));
        // use_defaults=false: generated extensions should NOT be excluded
        assert!(!is_excluded(Path::new("proto/user.pb.go"), &[], &[], false));
    }
}
