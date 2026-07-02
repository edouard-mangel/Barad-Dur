# Barad-dur

[![CI](https://lab.frogg.it/Edouard_Mangel/barad-dur/badges/main/pipeline.svg)](https://lab.frogg.it/Edouard_Mangel/barad-dur/-/pipelines)
[![crates.io](https://img.shields.io/crates/v/barad-dur)](https://crates.io/crates/barad-dur)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85+-orange)](https://www.rust-lang.org)

The all-seeing repository analyzer. Get health metrics, team insights, and actionable recommendations for any git repository — local or remote.

Named after the Dark Tower of Mordor — because nothing escapes its gaze.

## What it does

Barad-dur analyzes git metadata (commits, blame, file tree) and source code complexity, then produces a scored report across six categories (Dependencies is opt-in):

| Category | Metrics | Weight |
|----------|---------|--------|
| **Health** | Bus factor, churn hotspots, stale code, file complexity | 35% |
| **Coupling** | Afferent/efferent coupling, circular deps, change coupling smells | 20% |
| **Evolution** | Growth trend, refactoring ratio, code age, commit cadence | 20% |
| **Git Hygiene** | Commit message quality, history cleanliness, gitignore coverage | 15% |
| **Team** | Knowledge distribution (Gini), contributor activity, ownership clarity, silos, merge patterns | 10% |
| **Dependencies** *(optional)* | Dependency drift (libyear), vulnerability detection via OSV | 0% by default |

Each metric scores 0-100. Category scores are averages. The overall score is a weighted average. The report includes **Top Actions** — concrete suggestions from the lowest-scoring metrics.

### File-level analysis

Beyond git metadata, Barad-dur performs **static complexity analysis** on source files with language-aware parsing:

| Language | Extensions | What's measured |
|----------|-----------|-----------------|
| Rust | `.rs` | `pub fn` / `pub async fn`, public struct fields, cyclomatic complexity |
| JavaScript/TypeScript | `.js`, `.ts`, `.jsx`, `.tsx`, `.mjs`, `.cjs` | Exports, public class members, properties |
| Python | `.py` | Public defs, `self.*` properties |
| Go | `.go` | Exported functions (uppercase), exported struct fields |
| JVM (Java/Kotlin) | `.java`, `.kt`, `.kts` | Public methods, field declarations |
| CLR (C#) | `.cs` | Public methods, field declarations |

This produces per-file metrics: **LOC** (excluding blanks/comments), **cyclomatic complexity** (decision points), **public methods**, and **properties**. These feed into the hotspot analysis (churn x complexity x size).

## Example output

### CLI (default)

```
━━━ Barad-dur ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Repository: myTool on main
  Scope: 18 commits, 2 authors, 32 files
  Window: last 6 months

  Overall Score: ███████████████░░░░░ 77/100

  ▸ Health        ████████░░░░ 72/100
  ▸ Team          ████████░░░░ 74/100
  ▸ Evolution     ████████░░░░ 72/100
  ▸ Git Hygiene   ███████████░ 93/100

  Top Actions:
  1. [Health] Bus factor (score: 20) — Increase code review coverage
  2. [Team] Collaboration patterns (score: 25) — Break directory silos
  3. [Evolution] Growth trend (score: 40) — Monitor growth rate
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### HTML report (`--html`)

A self-contained single-file HTML report with:

- **Overview** — score gauge, radar chart, expandable category cards, top recommendations
- **Hotspots** — scatter plot (complexity vs churn, radius = LOC) with axis ticks + sortable, filterable table (score, CC, churn, bug-fix commits, LOC); clicking a row or a bubble highlights its counterpart
- **Coupling** — temporal coupling pairs ranked by coupling percentage, with auto-exclusion of expected pairs (lock files, test pairs, module indexes) and a per-file instability table (Ca / Ce / I)
- **Graph** — interactive force-directed import dependency graph: click-to-focus neighbourhoods, circular dependencies as dashed red edges, directory grouping, min-degree filter, SVG export
- **Ownership** — per-file ownership bars derived from blame, with author legend
- **Age** — file staleness with age bands (Fresh / > 3mo / > 6mo / > 1y)
- **Treemap** — zoomable file treemap (rectangles or circle packing) sized by LOC, colorable by metric, with per-file detail panel
- **Trends** — historical score evolution from `trends.json` (see `backfill`)
- **Authors** — per-contributor report cards
- **Dependencies** — libyear drift and CVE findings (when `--deps` is enabled)
- **Audit** — crisis files, directory concentration, dead files, velocity buckets

Tabs are connected: file cells drill through to the Hotspots and Graph views, and the current tab + selected file are mirrored into the URL hash (`#tab=graph&file=src/scorer.rs`) so any view can be shared as a deep link.

No external dependencies — all CSS, JS, and data are inlined. Works offline. Dark theme with a light-mode toggle.

**[Live example — this repo](https://barad-dur-b514a9.froggit.page/report.html)** (updated on every push to `main`)

## Installation

### From crates.io

```bash
cargo install barad-dur
```

> **Install fails to resolve `clap = "^4"`** (cargo lists only `3.2.x` candidates)?
> Your local crates.io index is stale or behind an incomplete registry mirror — the
> published crate correctly requires clap 4 and resolves fine against a current index.
> Fixes, cheapest first:
>
> ```bash
> cargo install barad-dur --locked   # uses the shipped Cargo.lock, skips re-resolution
> rm -rf ~/.cargo/registry/index && cargo install barad-dur --locked   # force a fresh index
> rustup update                      # update cargo itself, then retry
> ```
>
> If `~/.cargo/config.toml` has a `[source.crates-io] replace-with` mirror, that mirror
> is the culprit — bypass it or have it re-synced.

### Prerequisites

- Rust 1.85+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- System deps: `build-essential cmake pkg-config libssl-dev` (for libgit2)
- `git` in PATH (used for blame collection)

### Build from source

```bash
git clone git@lab.frogg.it:Edouard_Mangel/barad-dur.git
cd barad-dur
./init.sh          # installs deps + builds
# or manually:
cargo build --release
```

The binary is at `target/release/barad-dur`.

### Docker

#### Pull the pre-built image

Pre-built images are published to the GitLab container registry on every release:

```bash
docker pull lab.frogg.it:5050/edouard_mangel/barad-dur:latest
# or pin to a specific version:
docker pull lab.frogg.it:5050/edouard_mangel/barad-dur:v0.18.0
```

The image's entrypoint is the `barad-dur` binary (default command: `analyze .`, working directory `/repo`), so every subcommand works by mounting a repository into `/repo`:

```bash
# Analyze: CLI summary, JSON, or self-contained HTML report
docker run --rm -v /path/to/repo:/repo lab.frogg.it:5050/edouard_mangel/barad-dur
docker run --rm -v /path/to/repo:/repo lab.frogg.it:5050/edouard_mangel/barad-dur analyze . -v
docker run --rm -v /path/to/repo:/repo lab.frogg.it:5050/edouard_mangel/barad-dur analyze . --json
docker run --rm -v /path/to/repo:/repo -v $(pwd):/output \
  lab.frogg.it:5050/edouard_mangel/barad-dur analyze . --html -o /output/report.html

# Quality gate in CI — exits non-zero when scores fall below the threshold
docker run --rm -v /path/to/repo:/repo lab.frogg.it:5050/edouard_mangel/barad-dur gate . --min-score 70

# Analyze a remote repository by URL — no local clone or mount needed
docker run --rm lab.frogg.it:5050/edouard_mangel/barad-dur analyze https://github.com/BurntSushi/ripgrep

# Other subcommands work the same way
docker run --rm -v /path/to/repo:/repo lab.frogg.it:5050/edouard_mangel/barad-dur contributors .
docker run --rm -v /path/to/repo:/repo lab.frogg.it:5050/edouard_mangel/barad-dur init .
```

The snapshot cache is written to `/repo/.repository-analysis/`, so repeated runs against the same mount skip re-collection (use `--no-cache` to force it).

All images are signed with [cosign](https://github.com/sigstore/cosign). Verify with the public key in `cosign.pub`:

```bash
cosign verify --key cosign.pub lab.frogg.it:5050/edouard_mangel/barad-dur:latest
```

#### Build from source

Build a minimal (~31MB) container image from scratch:

```bash
# Using the install script (recommended)
./install.sh --docker                        # builds barad-dur:latest
./install.sh --docker -t myorg/barad-dur:v1  # custom image tag

# Or directly with docker
docker build -t barad-dur .
```

Run it by mounting a repository into `/repo`:

```bash
docker run --rm -v /path/to/repo:/repo barad-dur                          # CLI summary
docker run --rm -v /path/to/repo:/repo barad-dur analyze . -v             # verbose
docker run --rm -v /path/to/repo:/repo barad-dur analyze . --json         # JSON
docker run --rm -v /path/to/repo:/repo -v $(pwd):/output \
  barad-dur analyze . --html -o /output/report.html                       # HTML report
```

#### Distributing as a tarball

Export the image for sharing without a registry:

```bash
docker save barad-dur:latest | gzip > barad-dur.tar.gz
```

Load it on another machine:

```bash
docker load < barad-dur.tar.gz
```

## Usage

### analyze

```bash
# Analyze current directory (all categories, last 6 months)
barad-dur analyze .

# Verbose output (show individual metrics)
barad-dur analyze . -v
barad-dur analyze . -vv   # also show raw values

# JSON output (for CI/CD integration)
barad-dur analyze . --json
barad-dur analyze . --json --pretty

# JSON with trend history and velocity
barad-dur analyze . --json --trend --pretty

# HTML report (self-contained, open in browser)
barad-dur analyze . --html
barad-dur analyze . --html -o report.html
barad-dur analyze . --open   # generate HTML + open immediately in browser

# Single category
barad-dur analyze . --health
barad-dur analyze . --team
barad-dur analyze . --evolution
barad-dur analyze . --hygiene
barad-dur analyze . --deps   # dependency drift + CVE detection (requires network)

# Custom time window
barad-dur analyze . --since 3months
barad-dur analyze . --since 2024-01-01 --until 2024-12-31
barad-dur analyze . --all   # full history

# Output to file
barad-dur analyze . --json -o report.json

# Cache control
barad-dur analyze . --no-cache     # force re-collection
barad-dur analyze . --cache-only   # fail if no cache

# Performance
barad-dur analyze . --skip-blame   # skip blame phase (faster; blame-dependent metrics get defaults)

# Filtering — exclude files from all analysis phases
barad-dur analyze . --exclude '*.resx' --exclude 'i18n/**'   # glob patterns
barad-dur analyze . --exclude-ext jar --exclude-ext min.js    # by extension (case-insensitive, no dot needed)
barad-dur analyze . --no-default-excludes                     # disable built-in translation/resource exclusions
```

### gate

Quality gate for CI/CD — exits non-zero if scores fall below threshold.

```bash
barad-dur gate .                              # overall score >= 60 (default)
barad-dur gate . --min-score 70              # custom threshold
barad-dur gate . --category health           # check health category only
barad-dur gate . --category health --category team  # check multiple categories
barad-dur gate . --max-decline 2.0           # also fail if score drops > 2 pts/run on average
barad-dur gate . --skip-blame                # faster check; blame metrics get defaults
```

### init

Generate a `.repository-analysis/barad-dur.toml` config file with smart defaults.

```bash
barad-dur init .               # auto-detect patterns and write config
barad-dur init . --interactive # guided wizard
barad-dur init . --force       # overwrite existing config
```

### watch

Install or remove a post-commit git hook that re-runs analysis automatically.

```bash
barad-dur watch .              # install hook
barad-dur watch . --uninstall  # remove hook
barad-dur watch . --skip-blame # install hook without blame (faster commits)
```

### contributors

Detect suspected duplicate contributors and suggest `.mailmap` entries.

```bash
barad-dur contributors .                # show suggested deduplication
barad-dur contributors . --write        # append suggestions to .mailmap
barad-dur contributors . --since 3months
```

### backfill

Walk commit history to populate `trends.json` with historical analysis snapshots.

```bash
barad-dur backfill .            # backfill full history
barad-dur backfill . --no-blame # skip blame (faster)
```

### coupling

Analyze cross-repository coupling — discovers repos under a root directory and
ranks pairs by temporal, team, and dependency coupling signals.

```bash
barad-dur coupling /path/to/workspace         # analyze all repos
barad-dur coupling . --min-score 50           # only show pairs scoring >= 50
barad-dur coupling . --coupling-window 12h    # 12-hour temporal window (default: 24h)
barad-dur coupling . --since 3months --json   # limit history + JSON output
```

### Remote repository analysis

Barad-dur can analyze any remote repository by URL — it clones into a temp directory, runs analysis, and cleans up automatically:

```bash
# Analyze a remote repo (HTTPS or SSH)
barad-dur analyze https://github.com/BurntSushi/ripgrep
barad-dur analyze git@github.com:BurntSushi/ripgrep.git

# With GitHub API enrichment (stars, description, language, open issues)
barad-dur analyze https://github.com/BurntSushi/ripgrep --token ghp_xxxxxxxxxxxx
```

When a `--token` is provided and the target is a GitHub URL, the report is enriched with metadata from the GitHub API (stars, primary language, description, open issues count). The token needs at least `public_repo` scope (or `repo` for private repositories).

### Operational notes

- **Cache**: Snapshots are cached at `.repository-analysis/snapshot.bin` (auto-added to `.gitignore`). Subsequent runs are instant if HEAD hasn't changed. Use `--no-cache` to force re-collection, `--cache-only` to fail if no cache exists.
- **Progress**: In interactive mode (non-JSON, non-HTML), a progress spinner shows collection stages (commits, file tree, blame, complexity, indexes).
- **Shallow clones**: Detected automatically with a warning. For accurate CI/CD results, ensure a full clone (`GIT_DEPTH=0` in GitLab CI).

## Configuration

`barad-dur init .` generates `.repository-analysis/barad-dur.toml` with smart defaults
detected from the repository (translation files, vendored paths, team patterns). You can
also run `barad-dur init . --interactive` for a guided wizard.

```toml
# .repository-analysis/barad-dur.toml

[analysis]
skip_blame = false

[weights]
# Must sum to 100. 'deps' is 0 by default (opt-in via --deps).
health    = 35
team      = 10
evolution = 20
hygiene   = 15
coupling  = 20
deps      = 0
```

File exclusions are **not** configured here — they live in `.baraddurignore` and
CLI flags (see below).

### File exclusion & `.baraddurignore`

Files are excluded from **all** analysis phases through three layers, in
precedence order (highest first):

| Precedence | Source | Semantics |
|-----------:|--------|-----------|
| 1 (highest) | `--exclude` / `--exclude-ext` CLI flags | Force-exclude for this run; a `.baraddurignore` `!` cannot re-include a CLI-excluded file |
| 2 | **`.baraddurignore`** (repo root) | Full `.gitignore` syntax incl. `!` negation — its `!` rules re-include files the defaults drop |
| 3 (lowest) | Built-in defaults | Applied unless `--no-default-excludes` |

`.baraddurignore` lives at the repository root (next to `.gitignore`) and uses
identical syntax — comments (`#`), blank lines, `!` negation, leading-`/`
anchoring, trailing-`/` directory-only rules, `**`, and last-match-wins ordering:

```gitignore
# .baraddurignore
**/snapshots/**        # drop snapshot fixtures everywhere
/legacy.rs             # anchored: only the root-level legacy.rs

# Re-include a file the built-in defaults would drop (they exclude *.min.js):
!vendor/app.min.js
```

A `.baraddurignore` whitelist (`!`) is the only way to analyze a file the built-in
defaults otherwise remove. `barad-dur init` auto-detects likely-excludable paths
(translation files, vendored dirs, i18n, generated code) and writes a starter
`.baraddurignore` for you.

Note: only **git-tracked** files are analyzed to begin with, so `.baraddurignore`
acts as an extra filter over tracked paths — it never resurrects untracked files.

<details>
<summary>Built-in default exclusions (disable with <code>--no-default-excludes</code>)</summary>

- **Extensions** (churn/coupling noise): `resx po pot xlf xliff strings arb lproj`
  (translations) and `md txt rst adoc textile` (docs).
- **Generated (compound suffix)**: `*.pb.{go,h,cc,swift} *.g.cs *.generated.cs
  *.d.ts *.min.js *.min.css *_pb2.py`.
- **Lockfiles**: `pnpm-lock.yaml package-lock.json yarn.lock Cargo.lock
  Gemfile.lock poetry.lock composer.lock go.sum flake.lock *.lock`.
- **Config churn**: `appsettings*.json launchSettings.json .env*`.
- **Tooling dirs**: `.claude/ .cursor/ .idea/ .vscode/`.
- **ORM migrations / schemas**: `**/Migrations/*.Designer.cs`,
  `**/Migrations/*ModelSnapshot.cs`, `**/migrations/*.py`, `db/schema.rb`,
  `prisma/migrations/**`, `alembic/versions/**`.
- **i18n dirs**: `i18n l10n locales locale`.
- **Build/vendor dirs**: `node_modules vendor __pycache__ *.egg-info target .next
  .nuxt out gen generated .gradle .mvn build`.

</details>

> **Note**: changing exclusions — editing `.baraddurignore`, or passing different
> `--exclude`/`--exclude-ext`/`--no-default-excludes` flags — automatically
> invalidates the snapshot cache, so the change takes effect on the next run
> without needing `--no-cache`.

## CI/CD Integration

The JSON output is designed for pipeline consumption:

```yaml
barad-dur:
  stage: analysis
  variables:
    GIT_DEPTH: 0  # full clone for accurate metrics
  script:
    - barad-dur analyze . --json -o report.json
  artifacts:
    paths:
      - report.json
```

Parse the JSON to enforce thresholds:

```bash
SCORE=$(barad-dur analyze . --json | jq '.overall_score')
if [ "$SCORE" -lt 50 ]; then
  echo "Repository health score $SCORE is below threshold"
  exit 1
fi
```

Use `--html -o report.html` instead of `--json` to generate an HTML artifact.

## JSON output schema

The JSON output includes these top-level fields:

| Field | Type | Description |
|-------|------|-------------|
| `repo_name` | string | Repository name |
| `branch` | string | Current branch |
| `time_window_months` | number | Analysis window (0 = full history) |
| `total_commits` | number | Commits in window |
| `total_authors` | number | Unique authors |
| `total_files` | number | Files in tree |
| `overall_score` | number | Weighted score (0-100) |
| `categories` | array | Per-category scores and metrics |
| `top_actions` | array | Suggested improvements |
| `remote_meta` | object \| null | Remote repo metadata (populated for URL targets; enriched with GitHub API data when `--token` is provided) |
| `file_hotspots` | array | Files ranked by hotspot score (churn x complexity x LOC), incl. bug-fix commit counts |
| `coupling_pairs` | array | Temporally coupled file pairs with coupling percentage |
| `per_file_coupling` | array | Per-file afferent/efferent coupling and instability (Ca / Ce / I) |
| `import_edges` | array | Directed static import graph edges (`from` imports `to`) |
| `import_cycles` | array | Circular import groups (member file lists, depth 1–2) |
| `author_ownership` | array | Per-file ownership breakdown from blame |
| `file_ages` | array | File staleness (days since last modification) |
| `author_cards` | array | Per-contributor activity report cards |
| `history` | array | Prior analysis snapshots (from `trends.json`) |
| `dep_ecosystem_reports` | array | Per-ecosystem dependency drift and CVE findings (with `--deps`) |
| `audit` | object \| null | Crisis files, directory concentration, dead files, velocity buckets |
| `score_thresholds` | object | Score band cut-offs (good/warn) used by all renderers |

## Architecture

```
CLI (clap) → Collector (git2 + git CLI) → RepoSnapshot → Metrics → Scorer → Renderer
                                              ↕                         ↓
                                        Cache (bincode)          CLI / JSON / HTML
```

- **Collector**: git2 for commits/files, `git blame --porcelain` (parallel via rayon) for blame, static file analysis for complexity
- **RepoSnapshot**: shared data model with derived indexes (commits by author/file, change pairs, file metrics)
- **Metrics**: pure functions `(snapshot) → MetricValue`, independently testable
- **Scorer**: weighted category scores + top action suggestions + file-level analysis (hotspots, coupling, ownership, ages)
- **Renderer**: colored CLI, JSON, or self-contained HTML output

See [Architecture Decision Record](docs/adr/001-architecture-decisions.md) for detailed design rationale.

## Development

```bash
# Run all tests
cargo test

# Lint
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings

# Run specific test suites
cargo test --lib                    # unit tests
cargo test --test collector_tests   # collector integration tests
cargo test --test integration_tests # end-to-end tests

# Dogfood
cargo run -- analyze . -v
```

Test quality is enforced with mutation testing (cargo-mutants): every push to `main` runs mutants scoped to the change's diff behind a ≥ 80% kill-rate gate, and a nightly job covers the full codebase.

## Shipped

- **v0.5.0** — AST analysis via tree-sitter (Rust, JS, TS, Python, Go, Java, C#), historical trend tracking with backfill, per-blob blame cache
- **v0.6.0** — Author report cards, cross-tab drill-through links, CI quality gate (`barad-dur gate`), parallel complexity analysis (9× speedup on large repos)
- **v0.7.0** — File age tab (staleness bands), ownership tab (per-file blame breakdown and author legend)
- **v0.8.0** — Temporal coupling matrix heatmap, interactive force-directed graph, multi-repo coupling, O(n) coupling algorithm
- **v0.9.0** — GitLab Pages landing page, reusable CI template (`templates/analyze.yml`), binary download from package registry
- **v0.10.0** — RLE-aware blame (lower memory on large files), stable median scoring, `BARAD_DUR_TEST_REPO` env var for test isolation
- **v0.11.0** — CI: parallel fmt/clippy/test, `cargo audit`, binary artifact reuse between jobs; `git2` upgrade (RUSTSEC-2026-0008)
- **v0.12.0** — New **Dependencies** category: libyear drift + CVE detection via OSV API; supports Cargo, npm, pip, NuGet; results cached 7 days; Dependencies tab in HTML report
- **v0.13.0** — `barad-dur init` (config wizard with auto-detection), `barad-dur watch` (post-commit hook), `barad-dur contributors` (duplicate detection + `.mailmap` suggestions)
- **v0.14.0** — `barad-dur backfill` (adaptive-sampling history walk), `--trend` flag (velocity + delta in JSON), `--open` flag (HTML + browser), `--skip-blame` for analyze and gate
- **v0.15.0** — File exclusion: `--exclude` (glob), `--exclude-ext` (extension), `--no-default-excludes`; built-in exclusions for translation/resource files (*.resx, *.po, *.xlf, *.strings…)
- **v0.16.0** — `gate --max-decline` (fail if score drops faster than N pts/run averaged over 8 runs), `coupling --coupling-window` (configurable temporal window)
- **v0.17.0** — `[exclude] extensions` config key in `barad-dur.toml`, compound extension support (`min.js`), case-insensitive extension matching
- **Unreleased** — import dependency **Graph tab** (cycles, click-to-focus, directory grouping, min-degree filter, SVG export), raw `import_edges`/`import_cycles` in JSON, `../`-import resolution fix, Hotspots polish (Bugs column, axis ticks, filter, bidirectional row↔dot highlight), cross-tab file navigation with URL-hash deep links, per-feature mutation testing gate in CI

## Roadmap

- PR/merge request analysis (review turnaround, approval patterns)
- GitHub/GitLab API integration for PR data
- Multi-repo dashboard (aggregate scores across repositories)
- Interactive config editor — TUI wizard (see [backlog](docs/BACKLOG.md))

## License

GPL-3.0-only — see [LICENSE](LICENSE).
