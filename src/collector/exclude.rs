use std::path::Path;

/// Default file extensions excluded from analysis (translation/resource files).
/// These files change together by definition and inflate coupling/churn metrics.
const DEFAULT_EXCLUDE_EXTENSIONS: &[&str] = &[
    "resx", "po", "pot", "xlf", "xliff", "strings", "arb", "lproj",
];

/// Default path patterns excluded from analysis (tooling config, lockfiles).
/// Lockfiles inflate churn/coupling metrics without reflecting real code changes.
const DEFAULT_EXCLUDE_PATTERNS: &[&str] = &[
    // Lockfiles
    "pnpm-lock.yaml",
    "package-lock.json",
    "yarn.lock",
    "Cargo.lock",
    "Gemfile.lock",
    "poetry.lock",
    "composer.lock",
    "go.sum",
    "flake.lock",
    "**/*.lock",
    // App / environment config (churn noise, not real code changes)
    "**/appsettings*.json",
    "**/launchSettings.json",
    "**/.env*",
    // Tooling directories
    ".claude/**",
    ".cursor/**",
    ".idea/**",
    ".vscode/**",
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
];

/// Returns true if the file should be excluded based on the given glob patterns
/// and (optionally) the built-in default extension list.
pub fn is_excluded(path: &Path, patterns: &[String], use_defaults: bool) -> bool {
    let path_str = path.to_string_lossy();

    // Check built-in defaults (by extension and path pattern)
    if use_defaults {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_lower = ext.to_lowercase();
            if DEFAULT_EXCLUDE_EXTENSIONS.iter().any(|&e| e == ext_lower) {
                return true;
            }
        }
        if DEFAULT_EXCLUDE_PATTERNS
            .iter()
            .any(|p| glob_match::glob_match(p, &path_str))
        {
            return true;
        }
    }

    // Check user-provided glob patterns
    for pattern in patterns {
        if glob_match::glob_match(pattern, &path_str) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_excluded_matches_default_extensions() {
        let p = Path::new("src/Resources/Strings.resx");
        assert!(is_excluded(p, &[], true));
        assert!(!is_excluded(p, &[], false));
    }

    #[test]
    fn is_excluded_matches_po_files() {
        assert!(is_excluded(Path::new("locale/fr/messages.po"), &[], true));
        assert!(is_excluded(Path::new("lang/en.pot"), &[], true));
        assert!(is_excluded(Path::new("i18n/strings.xlf"), &[], true));
    }

    #[test]
    fn is_excluded_matches_user_globs() {
        let patterns = vec!["**/i18n/**".to_string()];
        assert!(is_excluded(
            Path::new("src/assets/i18n/sfk-messages/fr-FR.ts"),
            &patterns,
            false
        ));
        assert!(!is_excluded(Path::new("src/main.rs"), &patterns, false));
    }

    #[test]
    fn is_excluded_combines_defaults_and_user_patterns() {
        let patterns = vec!["**/i18n/**".to_string()];
        // Matched by default extension
        assert!(is_excluded(Path::new("foo.resx"), &patterns, true));
        // Matched by user pattern
        assert!(is_excluded(Path::new("src/i18n/en.ts"), &patterns, true));
        // Not matched by either
        assert!(!is_excluded(Path::new("src/main.rs"), &patterns, true));
    }

    #[test]
    fn is_excluded_matches_config_files_by_default() {
        assert!(is_excluded(
            Path::new("src/server/BusinessHub.API/appsettings.json"),
            &[],
            true,
        ));
        assert!(is_excluded(
            Path::new("src/server/BusinessHub.API/appsettings.Development.json"),
            &[],
            true,
        ));
        assert!(is_excluded(
            Path::new("Properties/launchSettings.json"),
            &[],
            true,
        ));
        assert!(is_excluded(Path::new("some/path/foo.lock"), &[], true));
        assert!(is_excluded(Path::new(".env.production"), &[], true));
        // Regular JSON should NOT be excluded
        assert!(!is_excluded(Path::new("src/data/schema.json"), &[], true));
    }

    #[test]
    fn is_excluded_matches_i18n_directories_by_default() {
        assert!(is_excluded(
            Path::new("src/client/src/assets/i18n/sfk-messages/en-US.ts"),
            &[],
            true
        ));
        assert!(is_excluded(Path::new("app/l10n/strings_fr.arb"), &[], true));
        assert!(is_excluded(Path::new("src/locales/en.json"), &[], true));
        assert!(is_excluded(Path::new("config/locale/fr.yml"), &[], true));
        // Non-i18n .ts files should NOT be excluded
        assert!(!is_excluded(Path::new("src/main.ts"), &[], true));
    }

    #[test]
    fn is_excluded_case_insensitive_extension() {
        assert!(is_excluded(Path::new("Strings.RESX"), &[], true));
        assert!(is_excluded(Path::new("lang.Resx"), &[], true));
    }

    #[test]
    fn is_excluded_matches_default_lockfiles() {
        assert!(is_excluded(Path::new("pnpm-lock.yaml"), &[], true));
        assert!(is_excluded(Path::new("package-lock.json"), &[], true));
        assert!(is_excluded(Path::new("yarn.lock"), &[], true));
        assert!(is_excluded(Path::new("Cargo.lock"), &[], true));
        assert!(is_excluded(Path::new("go.sum"), &[], true));
        assert!(is_excluded(Path::new("poetry.lock"), &[], true));
        // Not excluded when defaults disabled
        assert!(!is_excluded(Path::new("pnpm-lock.yaml"), &[], false));
    }

    #[test]
    fn is_excluded_matches_orm_generated_files() {
        // EF Core
        assert!(is_excluded(
            Path::new("Data/Migrations/20240101_Init.Designer.cs"),
            &[],
            true
        ));
        assert!(is_excluded(
            Path::new("Data/Migrations/AppDbContextModelSnapshot.cs"),
            &[],
            true
        ));
        // Django
        assert!(is_excluded(
            Path::new("myapp/migrations/0001_initial.py"),
            &[],
            true
        ));
        // Rails
        assert!(is_excluded(Path::new("db/schema.rb"), &[], true));
        // Prisma
        assert!(is_excluded(
            Path::new("prisma/migrations/20240101/migration.sql"),
            &[],
            true
        ));
        // Regular source should not match
        assert!(!is_excluded(Path::new("src/Models/User.cs"), &[], true));
    }

    #[test]
    fn is_excluded_matches_default_tooling_dirs() {
        assert!(is_excluded(Path::new(".claude/settings.json"), &[], true));
        assert!(is_excluded(Path::new(".cursor/rules/my-rule"), &[], true));
        assert!(is_excluded(Path::new(".idea/workspace.xml"), &[], true));
        assert!(is_excluded(Path::new(".vscode/settings.json"), &[], true));
        // Not excluded when defaults disabled
        assert!(!is_excluded(Path::new(".claude/settings.json"), &[], false));
    }
}
