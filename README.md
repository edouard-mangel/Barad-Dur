# Barad-dur

The all-seeing repository analyzer. Get health metrics, team insights, and actionable recommendations for any git repository — local or remote.

Named after the Dark Tower of Mordor — because nothing escapes its gaze.

## What it does

Barad-dur analyzes git metadata (commits, blame, file tree) and source code complexity, then produces a scored report across 4 categories:

| Category | Metrics | Weight |
|----------|---------|--------|
| **Health** | Bus factor, churn hotspots, temporal coupling, stale code, file complexity | 30% |
| **Team** | Knowledge distribution (Gini), contributor activity, ownership clarity, silos, merge patterns | 30% |
| **Evolution** | Growth trend, refactoring ratio, code age, commit cadence | 20% |
| **Git Hygiene** | Commit message quality, history cleanliness, gitignore coverage | 20% |

Each metric scores 0-100. Category scores are averages. The overall score is a weighted average. The report includes **Top Actions** — concrete suggestions from the lowest-scoring metrics.

### File-level analysis

Beyond git metadata, Barad-dur performs **static complexity analysis** on source files with language-aware parsing:

| Language | Extensions | What's measured |
|----------|-----------|-----------------|
| Rust | `.rs` | `pub fn` / `pub async fn`, public struct fields, cyclomatic complexity |
| JavaScript/TypeScript | `.js`, `.ts`, `.jsx`, `.tsx`, `.mjs`, `.cjs` | Exports, public class members, properties |
| Python | `.py` | Public defs, `self.*` properties |
| Go | `.go` | Exported functions (uppercase), exported struct fields |
| JVM (Java/Kotlin/C#) | `.java`, `.kt`, `.kts`, `.cs` | Public methods, field declarations |

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

- **Overview tab** — score gauge, radar chart, expandable category cards, top recommendations
- **Hotspots tab** — scatter plot (complexity vs churn, radius = LOC) + sortable table
- **Coupling tab** — temporal coupling pairs ranked by coupling percentage
- **Ownership tab** — per-file ownership bars derived from blame, with author legend
- **Age tab** — file staleness with age bands (Fresh / > 3mo / > 6mo / > 1y)

No external dependencies — all CSS, JS, and data are inlined. Works offline. Dark theme.

## Installation

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

## Usage

```bash
# Analyze current directory (all categories, last 6 months)
barad-dur analyze .

# Verbose output (show individual metrics)
barad-dur analyze . -v
barad-dur analyze . -vv   # also show raw values

# JSON output (for CI/CD integration)
barad-dur analyze . --json
barad-dur analyze . --json --pretty

# HTML report (self-contained, open in browser)
barad-dur analyze . --html
barad-dur analyze . --html -o report.html

# Single category
barad-dur analyze . --health
barad-dur analyze . --team
barad-dur analyze . --evolution
barad-dur analyze . --hygiene

# Custom time window
barad-dur analyze . --since 3months
barad-dur analyze . --since 2024-01-01 --until 2024-12-31
barad-dur analyze . --all   # full history

# Output to file
barad-dur analyze . --json -o report.json

# Cache control
barad-dur analyze . --no-cache     # force re-collection
barad-dur analyze . --cache-only   # fail if no cache
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

- **Cache**: Snapshots are cached at `.ncrunch/snapshot.bin` (auto-added to `.gitignore`). Subsequent runs are instant if HEAD hasn't changed. Use `--no-cache` to force re-collection, `--cache-only` to fail if no cache exists.
- **Progress**: In interactive mode (non-JSON, non-HTML), a progress spinner shows collection stages (commits, file tree, blame, complexity, indexes).
- **Shallow clones**: Detected automatically with a warning. For accurate CI/CD results, ensure a full clone (`GIT_DEPTH=0` in GitLab CI).

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
| `file_hotspots` | array | Files ranked by hotspot score (churn x complexity x LOC) |
| `coupling_pairs` | array | Temporally coupled file pairs with coupling percentage |
| `author_ownership` | array | Per-file ownership breakdown from blame |
| `file_ages` | array | File staleness (days since last modification) |

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
# Run all tests (116 tests)
cargo test

# Lint
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings

# Run specific test suites
cargo test --lib                    # 94 unit tests
cargo test --test collector_tests   # 14 integration tests
cargo test --test integration_tests # 8 end-to-end tests

# Dogfood
cargo run -- analyze . -v
```

## Roadmap (v2)

- AST analysis via tree-sitter (replace current line-based heuristics with proper syntax-tree parsing for more accurate complexity, method, and property detection)
- PR/merge request analysis (review turnaround, approval patterns)
- Historical trend tracking (score over time)
- Configuration file (`.barad-dur.toml`) for custom thresholds
- GitHub/GitLab API integration for PR data
- Interactive config editor (see [backlog](docs/BACKLOG.md))

## License

TBD
