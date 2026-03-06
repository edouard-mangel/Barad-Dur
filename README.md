# Barad-dur

The all-seeing repository analyzer. Get health metrics, team insights, and actionable recommendations for any git repository.

Named after the Dark Tower of Mordor — because nothing escapes its gaze.

## What it does

Barad-dur analyzes git metadata (commits, blame, file tree) and produces a scored report across 4 categories:

| Category | Metrics | Weight |
|----------|---------|--------|
| **Health** | Bus factor, churn hotspots, temporal coupling, stale code, file complexity | 30% |
| **Team** | Knowledge distribution (Gini), contributor activity, ownership clarity, silos, merge patterns | 30% |
| **Evolution** | Growth trend, refactoring ratio, code age, commit cadence | 20% |
| **Git Hygiene** | Commit message quality, history cleanliness, gitignore coverage | 20% |

Each metric scores 0-100. Category scores are averages. The overall score is a weighted average. The report includes **Top Actions** — concrete suggestions from the lowest-scoring metrics.

## Example output

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

# JSON output (for CI/CD integration)
barad-dur analyze . --json
barad-dur analyze . --json --pretty

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

### Cache

Barad-dur caches the repository snapshot at `.barad-dur/snapshot.bin` (automatically added to `.gitignore`). Subsequent runs are instant if HEAD hasn't changed. Use `--no-cache` to force a fresh collection.

## CI/CD Integration

The JSON output is designed for pipeline consumption:

```yaml
barad-dur:
  stage: analysis
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

## Architecture

```
CLI (clap) → Collector (git2 + git CLI) → RepoSnapshot → Metrics → Scorer → Renderer
                                              ↕
                                        Cache (bincode)
```

- **Collector**: git2 for commits/files, `git blame --porcelain` (parallel via rayon) for blame
- **RepoSnapshot**: shared data model with derived indexes (commits by author/file, change pairs)
- **Metrics**: pure functions `(snapshot) → MetricValue`, independently testable
- **Scorer**: weighted category scores + top action suggestions
- **Renderer**: colored CLI or JSON output

See [Architecture Decision Record](docs/adr/001-architecture-decisions.md) for detailed design rationale.

## Development

```bash
# Run all tests (86 tests)
cargo test

# Lint
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings

# Run specific test suites
cargo test --lib                    # 64 unit tests
cargo test --test collector_tests   # 14 integration tests
cargo test --test integration_tests # 8 end-to-end tests

# Dogfood
cargo run -- analyze . -v
```

## Roadmap (v2)

- AST analysis via tree-sitter (cyclomatic complexity, function length)
- PR/merge request analysis (review turnaround, approval patterns)
- Historical trend tracking (score over time)
- Configuration file (`.barad-dur.toml`) for custom thresholds
- GitHub/GitLab API integration for PR data

## License

TBD
