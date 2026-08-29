# Changelog

All notable changes to this project will be documented here.

## [Unreleased]

### Added
- **PHP language support.** `.php` files are now parsed with tree-sitter
  rather than falling through to the generic line-counter: imports, cyclomatic
  complexity, public methods, properties, functions and nesting. `use`
  statements resolve through the PSR-4 roots declared in every `composer.json`
  in the tree (both `autoload` and `autoload-dev`), and `require`/`include`
  resolve their string literal against the requiring file's own directory.

### Changed
- **PHP repositories will see their scores move**, because PHP files
  previously reported `cyclomatic_complexity = 0` and contributed no import
  edges. Measured on a Laravel monorepo with 870 PHP files: god objects went
  from 25 to 124 flagged files as PHP structure became visible, while complex
  hotspots *fell* from 51 to 44 — that metric is percentile-relative, so
  adding files with real complexity shifts the threshold. Import-graph edge
  density roughly tripled and cross-community co-change pairs went from 263 to
  5,095. Overall that repository moved 72 to 74.
- **Breaking config change**: `thresholds.health.god_node_degree_multiplier`
  is replaced by `god_node_degree_percentile` (default 0.90). A config
  carrying the old key now fails validation.

  The old rule flagged a structural hub when `degree > median * 4`, and that
  term never bound. Measured across five real repositories the import-graph
  degree median is 0-2, so `median * 4` topped out at 8 — never above
  `god_node_min_degree`, also 8 — so the relative term never once decided an
  outcome, in any repository tested. The floor was the whole rule.

  The threshold is now `max(god_node_min_degree, p90 of the repo's degree
  distribution)`. The floor still lets an uncoupled repository flag nothing,
  which a purely relative rule cannot; the percentile raises the bar where
  most files are heavily connected and a degree of 8 is unremarkable.

  Measured effect: Apios-Web 124 -> 112 flagged files, mautic 547 -> 488,
  barad-dûr unchanged at 22 (its p90 is 5, below the floor, so the floor
  still governs). No category score changed band in any of the three.
- `Language` is now `#[non_exhaustive]`, for the reason `CouplingPair` and
  `AnalysisReport` already carry it: the enum gains a variant every time a
  language is taught to the collector, and on an exhaustive public enum each
  of those is a breaking change. Downstream matches need a `_` arm.
- Not resolved, and recorded as known gaps: Laravel path helpers
  (`base_path()`), `require $var`, PSR-0, and `classmap`/`files` autoload
  sections. PHP is deliberately absent from `DETECTABLE_EXTS`, so the four
  Pressman coupling metrics correctly stay unscored for it.

## [0.21.0] - 2026-08-26

### Added
- **Test safety net** metric (Crime Scene Ch. 9) — flags source files whose paired tests are eroding, with the `coupling.test_safety_net_min_ratio` threshold, tooltip copy and suggested actions.
- **Knowledge loss** metric (Ch. 13) and **cross-team coupling** metric (Ch. 12).
- **Trends**: churn timeline and coupling-pair growth (M1, Ch. 14), code/test growth balance (M2, Ch. 9), coupling-reach decay annotation (M3, Ch. 8).
- **Call graph**: TS/JS and Rust call-edge extraction, plus a `call_graph` report section gated by a trust floor.
- Community detection and a structural hub signal shared by the coupling and health categories.
- File-role classification separating code, tests and config.
- Hotspot naming and vocabulary heuristics.

### Changed
- **Breaking (API)**: `compute_coupling`, `compute_team` and `compute_health` each take an additional parameter, and `scorer::build_report` takes 7 rather than 5. Library consumers must update call sites; the CLI is unaffected.
- Maintainability report scoring recalibrated, and report scoring made context-aware.

### Fixed
- **Import-derived metrics no longer report a repository as clean when nothing was measured.** Afferent coupling, efferent coupling and circular dependencies now return *unscored* when the import graph could not be built, instead of a perfect 100. This affects every repository whose language has no import resolution — PHP, Ruby, C/C++, Swift, Scala, **and Kotlin**, which has an import query but no resolver arm and so never produced edges. Scores for these repositories will change: the affected metrics stop contributing a fabricated 100 to the Coupling category.
- **Change-coupling smells are scored from co-change alone when no import data exists.** Previously a pair was kept only if both files carried an import-graph community, so on a repository with no import extraction every finding was reported at a perfect score. Community data now refutes a pair only when both files are known and share a community; absence of data is no longer treated as evidence of separation.
- Circular dependencies no longer merge a direct `A↔B` cycle with `A→B→C→A`; cycles are keyed by their full sorted member list, so counts and evidence no longer depend on hash iteration order.
- Test safety net counts co-changes exactly rather than via the floored pair table, and covers reverse-direction stems in its candidate index.
- CI: h2 advisory patched, stale `deny` ignore dropped.

### Internal
- `community_corroboration`'s documentation claimed it never changed the score; it drops refuted pairs, so disabling it can only lower the score. Corrected and covered by a test.
- Mutation-testing coverage holes closed across gate/backfill validation, call-graph extraction and community/hub detection.

## [0.20.0] - 2026-07-18

> **Note on versioning.** No `v0.19.0` tag was ever cut. The `[0.19.0]`
> section below describes the `.baraddurignore` exclusion work, which
> shipped in the **v0.20.0** tag. This section records the rest of that
> release, which had gone undocumented.

### Added
- **Pressman coupling milestones M4–M7**: hotspot cross-referencing (M4), corroboration weighting over qualifying co-change pairs (M5), per-file refactoring actions surfaced in CLI, HTML and dashboard (M6), and the inheritance-depth rung with an `inheritance_min_depth` threshold and `Ih` hotspot badge (M7).
- **Gate ratchet**: `--no-new-coupling`, `--max-new-coupling` and `--baseline-ref` for a pure ratchet verdict over baseline/head findings.
- Opt-in blob-based AST pass so historical (backfill) snapshots carry coupling findings; snapshot cache bumped to v2 to carry resolved class records.
- Pressman finding counts embedded in the analysis report, recorded in trend history and shown in the trends tooltip.

### Fixed
- Pressman metrics render unscored when the AST pass did not run, rather than reading as clean.
- Barrel re-exports followed during inheritance resolution; abstract-class chains detected.
- No keyword-counted complexity for unknown languages.
- `crossbeam-epoch` bumped to 0.9.20 for RUSTSEC-2026-0204.

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
