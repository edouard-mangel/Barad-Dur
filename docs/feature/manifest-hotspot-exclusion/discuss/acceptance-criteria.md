# Acceptance Criteria — manifest-hotspot-exclusion

All criteria are testable as unit tests in `src/collector/exclude.rs` (mirroring the
existing `is_excluded_*` block), except AC-5 which is an integration-level assertion.

## AC-1 — Core manifests excluded under defaults (US-1, FR-1)
```gherkin
Given use_defaults = true
Then is_excluded("package.json")        is true
And  is_excluded("Cargo.toml")          is true
And  is_excluded("go.mod")              is true
And  is_excluded("src/App/App.csproj")  is true
And  is_excluded("pom.xml")             is true
And  is_excluded("build.gradle")        is true
And  is_excluded("build.gradle.kts")    is true
And  is_excluded("requirements.txt")    is true
And  is_excluded("pyproject.toml")      is true
```

## AC-2 — Nested manifests excluded (US-1, FR-1)
```gherkin
Given use_defaults = true
Then is_excluded("apps/web/package.json")       is true
And  is_excluded("crates/core/Cargo.toml")      is true
And  is_excluded("services/api/go.mod")         is true
```

## AC-3 — Real source NOT excluded (US-1, regression guard)
```gherkin
Given use_defaults = true
Then is_excluded("src/main.rs")        is false
And  is_excluded("src/index.ts")       is false
And  is_excluded("src/data/schema.json") is false   # plain JSON, not a manifest
And  is_excluded("cmd/server/main.go")  is false
```
> Note: guards against an over-broad glob (e.g. `**/*.json`) catching real files.

## AC-4 — Defaults off re-includes manifests (US-2, FR-2)
```gherkin
Given use_defaults = false
Then is_excluded("package.json")  is false
And  is_excluded("Cargo.toml")    is false
And  is_excluded("pyproject.toml") is false
```

## AC-5 — Dependency features unaffected (US-3, NFR-1)
```gherkin
Given a fixture repo with package.json + Cargo.toml on disk
When collect_locked_deps() and dependency-coupling parsing run
Then their results are identical with manifests excluded from the snapshot
And  identical to a run before the exclusion globs were added
```
> Rationale: both paths read from disk via repo_root.join(...); this AC documents the
> invariant and pins it with a test so a future refactor can't silently break it.

## Testability
Every AC reduces to a boolean assertion on `is_excluded` or a value-equality check on
existing disk-read collectors — all directly testable, no mocking of I/O required.
