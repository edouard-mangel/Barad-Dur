# Changelog

All notable changes to this project will be documented here.

## [0.19.0] - 2026-07-02

### Added
- `.baraddurignore` file at the repository root with full `.gitignore` semantics (comments, `!` negation, anchoring, directory-only rules, last-match-wins) via the `ignore` crate. A `!` rule re-includes files the built-in defaults would otherwise drop.
- `barad-dur init` now writes a starter `.baraddurignore` from auto-detected exclude patterns (translation files, vendored dirs, i18n, generated code).

### Changed
- **Breaking**: file exclusions moved out of the TOML `[exclude]` section. Exclusions are now configured via `.baraddurignore` (repo root) and the `--exclude`/`--exclude-ext` CLI flags. Precedence, highest first: CLI flags → `.baraddurignore` → built-in defaults. A leftover `[exclude]` section in `barad-dur.toml` is ignored (with an unknown-section warning); move its `patterns`/`extensions` into `.baraddurignore`.
- The snapshot cache now invalidates when exclusion inputs change (`--exclude`/`--exclude-ext`, `--no-default-excludes`, or `.baraddurignore` contents) — editing exclusions no longer requires `--no-cache` to take effect.
- `barad-dur backfill` now applies the same exclusions (built-in defaults + `.baraddurignore`) as `analyze`/`gate`, so historical trend points are comparable to live scores.
- Built-in defaults now exclude barad-dûr's own artifacts (`.baraddurignore`, `.repository-analysis/`) so the tool no longer measures its own config as source.
- `--cache-only` now fails when the cache is stale (HEAD, time window, or exclusion inputs changed) instead of silently performing a full re-collection.

### Fixed
- `barad-dur init` now correctly detects and writes exclude patterns for files in subdirectories (previously a shell-glob recount dropped nested translation/vendor files, so `.baraddurignore` was often not written at all).

### Internal
- `CouplingPair` and `AnalysisReport` marked `#[non_exhaustive]` — prevents semver violations when output fields are added in future releases
- CI: `cargo publish --allow-dirty` to handle Cargo.lock regeneration during publish

## [0.17.2] - 2026-06-08

### Added
- Light/dark theme toggle in HTML report header — persists choice in `localStorage`
- `body.light` CSS block with full light-mode palette
- `body.light.cbf` CSS block for CBF + light mode combination
- `initTheme` / `toggleTheme` JS functions in shared JS layer

### Changed
- All semantic UI colors extracted to CSS custom properties (`--c-good`, `--c-warn`, `--c-danger`, etc.) — bare hex removed from JS

## [0.17.1] - 2026-05-21

### Fixed
- CI: fixed `CARGO_HOME` path and release-publish idempotency for crates.io job
- CI: switched to `cargo login` + `git clean` approach for reliable crates.io publish

## [0.17.0] - 2026-05-20

### Added
- `--exclude-ext <EXT>` CLI flag and `[exclude] extensions = [...]` TOML key — skip files by extension (bare ext, compound ext like `min.js`, case-insensitive)

### Fixed
- Cache: `.repository-analysis/` directory always written on its own line in `.gitignore` (no longer appended inline)

### Changed
- CI: mutation testing now counts `TIMEOUT` outcomes as caught (contributing to kill rate)

### Internal
- `contributors` module: `pub(crate)` on internal helpers, improved doc coverage

## [0.14.1] - 2026-05-06

### Changed
- CI: automated `cargo publish` to crates.io on tagged releases via `CARGO_REGISTRY_TOKEN`
- CI: Docker registry login uses `--password-stdin` for secure credential handling

## [0.14.0] - 2026-05-06

### Added
- New **Audit** tab in HTML report: legacy-codebase diagnostics (crisis files, dead files, dir concentration, velocity buckets)
- Tooltips and methodology explanations throughout the HTML report
- `watch` subcommand: installs a post-commit git hook that re-runs analysis on every commit

### Fixed
- Kotlin complexity: `public_methods` and `properties` counters now wired correctly

## [0.12.0] - 2026-04-14

### Added
- New **Dependencies** category (20% weight when active): libyear drift + CVE detection
  - Supports Cargo, npm, pip, NuGet lock files
  - Release dates from crates.io, npmjs.org, pypi.org, nuget.org
  - CVE detection via OSV API (api.osv.dev) — covers all four ecosystems
  - Results cached 7 days in `.repository-analysis/deps-cache.json`
  - Per-ecosystem breakdown with critical callouts (stale >5y or has CVE)
  - Activated via `--deps` flag — offline by default
- New Dependencies tab in HTML report (safe DOM, no external deps)
- Updated category weights: Health 35%, Evolution 20%, Dependencies 20%, Hygiene 15%, Team 10%

## [0.11.0] - 2026-04-14

### Changed
- CI: merged clippy into build job to reduce runner allocations
- CI: run fmt, clippy, and test in parallel for faster pipelines
- CI: add timeouts, `cargo audit`, and binary artifact reuse between jobs

### Fixed
- Upgrade `git2` to 0.20 (resolves RUSTSEC-2026-0008 — potential UB on `Buf` dereference)

### Internal
- Extract `run_analyze` phases and test `parse_relative` as pure functions
- Extract shared test helpers to `metrics::testutil`

## [0.10.0] - 2026-03-28

### Added
- RLE-aware ownership: run-length encoding for blame output reduces memory on large files
- Stable median calculation for more deterministic scoring
- `BARAD_DUR_TEST_REPO` env var for integration test isolation

### Fixed
- `manual_is_multiple_of` clippy lint for stable Rust compatibility

## [0.9.0] - 2026-03-20

### Added
- Distribution template (`templates/analyze.yml`) for including barad-dur in other GitLab CI pipelines
- GitLab Pages landing page (`/`) with the HTML report at `/report.html`
- Binary download from package registry (no Docker required in consumer pipelines)

### Fixed
- Accessibility issues in the Pages landing page

## [0.8.0] - 2026-03-10

### Added
- Temporal coupling matrix heatmap tab with dimension filter checkboxes
- Interactive force-directed graph in HTML coupling report
- Multi-repo coupling detection
- Cross-repo CI pipeline integration

### Changed
- Coupling algorithm: replaced O(n²) pairwise loop with merged-timeline scan (significant speedup on large repos)
- Pre-sized collections and per-repo data caching for coupling

## [0.7.0] — earlier

### Added
- File age tab with staleness bands (Fresh / >3mo / >6mo / >1y)
- Ownership tab with per-file blame breakdown and author legend

## [0.6.0]

### Added
- Author report cards
- Cross-tab drill-through links in HTML report
- CI quality gate (`barad-dur gate`)
- Parallel complexity analysis (9× speedup on large repos)

## [0.5.0]

### Added
- AST-based complexity analysis via tree-sitter (Rust, JS, TS, Python, Go, Java, C#)
- Historical trend tracking with backfill
- Per-blob blame cache
