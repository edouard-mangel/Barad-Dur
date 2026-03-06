# Barad-dur Design Document

> The all-seeing repository analyzer. A CLI tool that computes heuristics and health metrics about any git repository.

**Date:** 2026-03-06
**Status:** Approved

---

## 1. Overview

Barad-dur is a Rust CLI tool that analyzes git repositories to produce actionable health metrics and scores. It targets four user personas: developers doing self-assessment, team leads spotting risks, CI/CD pipelines tracking metrics over time, and reviewers onboarding to unfamiliar codebases.

### Design Principles

- **v1 scope:** Git metadata only (commits, blame, files, authors). No AST/code parsing.
- **v2 scope:** Add AST analysis for language-aware metrics (complexity, dependencies).
- **Architecture:** Layered with shared `RepoSnapshot` data model (Approach C).
- **Output:** Human-readable CLI by default, `--json` flag for machine consumption.
- **Scoring:** 0-100 overall score + per-category sub-scores.
- **Caching:** Serialized snapshots to avoid re-walking full git history.

---

## 2. Architecture

```
┌─────────────────────────────────────────────────────┐
│                    CLI (clap)                        │
│  barad-dur analyze [path] [--health] [--team]       │
│  [--evolution] [--hygiene] [--since] [--until]      │
│  [--json] [--all] [--no-cache]                      │
└──────────────────────┬──────────────────────────────┘
                       │
              ┌────────▼────────┐
              │     Cache       │
              │  (staleness     │
              │   check)        │
              └───┬─────────┬───┘
           fresh  │         │ stale/miss
      ┌───────────┘         └──────────┐
      ▼                                ▼
 Load from              ┌──────────────────┐
 .barad-dur/            │    Collector      │
 snapshot.bin           │  (git2 + CLI)     │
      │                 └────────┬─────────┘
      │                          │
      │                 ┌────────▼────────┐
      │                 │  Save to cache   │
      │                 └────────┬────────┘
      │                          │
      └────────────┬─────────────┘
              ┌────▼────────┐
              │ RepoSnapshot │
              └────┬────────┘
                   │
     ┌─────────────┼──────────────┬──────────────┐
     ▼             ▼              ▼              ▼
┌─────────┐  ┌─────────┐  ┌───────────┐  ┌──────────┐
│ Health   │  │  Team   │  │ Evolution │  │ Hygiene  │
│ Metrics  │  │ Metrics │  │  Metrics  │  │ Metrics  │
└────┬─────┘  └────┬────┘  └─────┬─────┘  └────┬─────┘
     └──────────┬──┘─────────────┘──────────────┘
           ┌────▼──────┐
           │  Scorer    │
           │ (0-100)    │
           └────┬──────┘
                │
     ┌──────────┴──────────┐
     ▼                     ▼
┌──────────┐          ┌──────────┐
│ CLI      │          │ JSON     │
│ Renderer │          │ Renderer │
└──────────┘          └──────────┘
```

---

## 3. RepoSnapshot Data Model

The core shared data structure, populated once from git and cached to disk.

```rust
#[derive(Serialize, Deserialize)]
struct RepoSnapshot {
    // Repository metadata
    path: PathBuf,
    name: String,
    default_branch: String,
    time_window: TimeWindow,
    head_commit: String,           // for cache staleness
    created_at: DateTime<Utc>,     // when snapshot was built

    // Core git data
    commits: Vec<Commit>,
    files: Vec<FileEntry>,
    authors: Vec<Author>,
    blame_map: HashMap<PathBuf, Vec<BlameLine>>,

    // Derived indexes
    commits_by_author: HashMap<AuthorId, Vec<CommitId>>,
    commits_by_file: HashMap<PathBuf, Vec<CommitId>>,
    file_change_pairs: Vec<(PathBuf, PathBuf, usize)>,  // temporal coupling
}

struct Commit {
    id: String,
    author: AuthorId,
    timestamp: DateTime<Utc>,
    message: String,
    files_changed: Vec<FileChange>,
    is_merge: bool,
    parent_count: usize,
}

struct FileChange {
    path: PathBuf,
    additions: u32,
    deletions: u32,
    change_type: ChangeType,  // Added, Modified, Deleted, Renamed
}

struct FileEntry {
    path: PathBuf,
    size_bytes: u64,
    is_binary: bool,
    depth: usize,             // nesting level
}

struct Author {
    id: AuthorId,
    name: String,
    email: String,
}

struct BlameLine {
    author_id: AuthorId,
    commit_id: String,
    timestamp: DateTime<Utc>,
}

struct TimeWindow {
    since: Option<DateTime<Utc>>,
    until: Option<DateTime<Utc>>,
    default_months: u32,       // default: 6
}
```

---

## 4. Metrics

### 4.1 Health Metrics (5)

| Metric | Description | Key Inputs | Scoring |
|---|---|---|---|
| **Bus factor** | Minimum authors to cover 50% of code knowledge per file/directory | blame_map | <2 = bad, 2-3 = ok, 4+ = good |
| **Churn hotspots** | Files with highest change frequency relative to size | commits_by_file, files | Top 5% of churn concentration |
| **Temporal coupling** | File pairs that change together >70% of the time | file_change_pairs | Count of suspicious pairs |
| **Stale code** | Files untouched within time window despite project activity | commits_by_file, time_window | % of files stale |
| **File complexity proxies** | File size distribution, directory depth, binary file count | files | Files >1000 lines, depth >5 |

### 4.2 Team Metrics (5)

| Metric | Description | Key Inputs | Scoring |
|---|---|---|---|
| **Knowledge distribution** | Gini coefficient of code ownership concentration | blame_map, authors | 0-1 where 0 = equal, 1 = monopoly |
| **Contributor activity** | Ratio of active vs total contributors in window | commits_by_author, time_window | % active |
| **Ownership clarity** | % of files with a primary owner (>50% blame) | blame_map | Higher = clearer |
| **Collaboration patterns** | Knowledge silos detection (dirs with single author) | commits_by_file, commits_by_author | Count of silos |
| **PR/merge patterns** | Merge commit frequency, branch lifetime estimation | commits (merge analysis) | Avg branch lifetime |

### 4.3 Evolution Metrics (4)

| Metric | Description | Key Inputs | Scoring |
|---|---|---|---|
| **Growth trend** | Rate of file/line additions over time | commits | % growth in window |
| **Refactoring ratio** | Ratio of modification commits vs pure addition commits | commits | 0.2-0.4 = healthy |
| **Code age distribution** | How old the surviving code is (from blame) | blame_map | Median age |
| **Commit cadence** | Regularity and frequency of commit activity | commits | Commits/day + variance |

### 4.4 Git Hygiene Metrics (3)

| Metric | Description | Key Inputs | Scoring |
|---|---|---|---|
| **Commit message quality** | Length, imperative mood, conventional commits detection | commits | % following conventions |
| **History cleanliness** | Force push detection, rebase vs merge patterns | commits | Force push count |
| **.gitignore coverage** | Tracked files that probably shouldn't be (env, binaries, etc.) | files | Count of suspicious files |

---

## 5. Scoring

Each category produces a **0-100 score**. The overall score is a weighted average:

| Category | Weight | Rationale |
|---|---|---|
| Health | 30% | Core codebase sustainability |
| Team | 30% | Bus factor and knowledge distribution are critical |
| Evolution | 20% | Important but less immediately actionable |
| Git Hygiene | 20% | Good practices indicator |

Each individual metric maps to a score via a configurable threshold function. For example:
- Bus factor: 1 = 0pts, 2 = 50pts, 3 = 75pts, 4+ = 100pts
- Commit message quality: linear scale from 0% to 100% conformance

---

## 6. Snapshot Cache

### Storage

- **Format:** Binary via `bincode` crate (fast, compact)
- **Location:** `.barad-dur/snapshot.bin` at repo root
- **Auto-gitignore:** Tool creates/appends `.barad-dur/` to `.gitignore` on first run

### Staleness Detection

1. Store HEAD commit hash + timestamp in cached snapshot
2. On next run, compare with current HEAD
3. If HEAD unchanged → cache is **fresh** → load and compute metrics (instant)
4. If HEAD changed → cache is **stale** → incremental update (walk only new commits) → merge → save

### Incremental Updates

When cache is stale:
1. Find the cached HEAD commit in the DAG
2. Walk only commits between cached HEAD and current HEAD
3. Re-run blame only on files modified in new commits
4. Merge new data into existing snapshot
5. Recompute derived indexes

### CLI Controls

```bash
barad-dur analyze .             # use cache if fresh
barad-dur analyze . --no-cache  # force full re-collection
barad-dur analyze . --cache-only # fail if no cache (CI mode)
```

---

## 7. Git Data Collection

### Primary: libgit2 (git2 crate)

Used for:
- Walking commit history (RevWalk)
- Reading file tree (TreeWalk)
- Diffing commits (Diff)
- Author extraction with .mailmap support
- Branch/tag enumeration

### Fallback: git CLI

Used for:
- `git blame` (more reliable across edge cases than libgit2 blame)
- `git log --format` for specific data not easily available via libgit2
- Shallow clone detection (`git rev-parse --is-shallow-repository`)

---

## 8. CLI Interface

```bash
# Full analysis (all categories, default 6-month window)
barad-dur analyze [path]

# Category selection
barad-dur analyze . --health --team --evolution --hygiene

# Time window
barad-dur analyze . --since 3months
barad-dur analyze . --since 2024-01-01 --until 2024-06-30
barad-dur analyze . --all  # full history

# Output
barad-dur analyze . --json           # machine-readable JSON
barad-dur analyze . --json --pretty  # formatted JSON
barad-dur analyze . -o report.json   # save to file

# Verbosity
barad-dur analyze . -v    # per-file detail
barad-dur analyze . -vv   # all raw data

# Cache
barad-dur analyze . --no-cache   # force full re-collection
barad-dur analyze . --cache-only # fail if no cache
```

---

## 9. CLI Output Format

```
  Barad-dur -- Repository Analysis
══════════════════════════════════════════════════

  Project: my-app    Branch: main    Window: last 6 months
  Commits: 847       Authors: 12     Files: 342

  Overall Score: 68/100  ████████████████░░░░

── Health ─────────────────────────────── 72/100 ──
  Bus factor .............. 3 (good)
  Churn hotspots .......... 5 files account for 40% of changes
  Temporal coupling ....... 8 suspicious file pairs detected
  Stale code .............. 12% of files untouched in window
  File complexity ......... 4 files >1000 lines, 2 deep-nested dirs

── Team ───────────────────────────────── 65/100 ──
  Knowledge distribution .. Gini: 0.42 (moderate concentration)
  Active contributors ..... 8/12 active in window
  Ownership clarity ....... 78% of files have clear owner
  Collaboration ........... 3 knowledge silos detected
  PR/merge patterns ....... avg branch lifetime: 4.2 days

── Evolution ──────────────────────────── 74/100 ──
  Growth trend ............ +15% files, +22% lines in window
  Refactoring ratio ....... 0.31 (healthy)
  Code age ................ median: 8 months
  Commit cadence .......... regular (4.2 commits/day avg)

── Git Hygiene ────────────────────────── 61/100 ──
  Commit messages ......... 67% follow conventions
  History cleanliness ..... 3 force pushes detected
  .gitignore .............. 2 suspicious tracked files

══════════════════════════════════════════════════
  Top actions:
  1. Share knowledge of src/auth/ (bus factor: 1)
  2. Investigate coupling: api.rs <> db.rs (94% co-change)
  3. Review .env.local -- may contain secrets
```

---

## 10. Project Structure

```
barad-dur/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point
│   ├── cli.rs               # clap argument parsing & dispatch
│   ├── collector/
│   │   ├── mod.rs            # Collector orchestration
│   │   ├── libgit.rs         # git2 crate implementation
│   │   └── gitcli.rs         # git CLI fallback (blame, etc.)
│   ├── snapshot.rs           # RepoSnapshot struct & builders
│   ├── cache/
│   │   ├── mod.rs            # Cache orchestration
│   │   ├── storage.rs        # bincode serialize/deserialize
│   │   └── staleness.rs      # HEAD comparison, incremental update
│   ├── metrics/
│   │   ├── mod.rs            # MetricResult trait, category registry
│   │   ├── health.rs         # bus factor, churn, coupling, stale, complexity
│   │   ├── team.rs           # knowledge dist, activity, ownership, collab, PR
│   │   ├── evolution.rs      # growth, refactoring, age, cadence
│   │   └── hygiene.rs        # commit messages, force push, gitignore
│   ├── scorer.rs             # Raw metrics -> 0-100 scores
│   └── renderer/
│       ├── mod.rs            # Renderer trait
│       ├── cli.rs            # Terminal output (crossterm/colored)
│       └── json.rs           # JSON serialization (serde_json)
└── tests/
    ├── fixtures/             # Small test git repos
    ├── snapshot_tests.rs
    ├── metric_tests.rs
    └── integration_tests.rs
```

---

## 11. Dependencies

| Crate | Version | Purpose |
|---|---|---|
| `git2` | latest | libgit2 bindings for repo access |
| `clap` | 4.x (derive) | CLI argument parsing |
| `serde` + `serde_json` | latest | Serialization |
| `bincode` | latest | Binary snapshot serialization |
| `chrono` | latest | Date/time handling |
| `crossterm` or `colored` | latest | Terminal colors/formatting |
| `rayon` | latest | Parallel iteration for large repos |
| `indicatif` | latest | Progress bars |
| `anyhow` | latest | Error handling |

---

## 12. Error Handling & Edge Cases

| Scenario | Handling |
|---|---|
| Not a git repo | Clear error: "Not a git repository. Run from a repo root or pass a path." |
| Empty repo | Report available data, flag missing metrics as N/A |
| Shallow clone | Detect via `git rev-parse --is-shallow-repository`, warn metrics may be incomplete |
| Large repos (>100k commits) | Progress bar, parallel commit walking with rayon |
| Binary files | Excluded from blame/churn, tracked in complexity metrics |
| .mailmap | Respect git mailmap for author deduplication |
| No blame data available | Gracefully degrade: skip blame-dependent metrics, note in output |
| Corrupt cache | Delete and rebuild silently |

---

## 13. v2 Roadmap (Future)

- **AST analysis:** Tree-sitter integration for language-aware complexity (cyclomatic, cognitive)
- **Dependency analysis:** Import/call graph from AST
- **Trend tracking:** Store historical scores, show improvement/regression over time
- **Config file:** `.barad-dur.toml` for custom thresholds and scoring weights
- **MCP server:** Expose metrics as MCP tools for AI agents
- **Web dashboard:** Optional `--serve` flag for browser-based exploration

---

## 14. Decisions Log

| Decision | Choice | Rationale |
|---|---|---|
| Language | Rust | Performance on large repos, single binary distribution |
| Git access | git2 + CLI fallback | Best of both: fast bulk access + reliable blame |
| Architecture | Layered (Approach C) | Clean separation without plugin overhead, easy v2 path |
| Output | CLI + JSON export | Human-readable default, machine-consumable with --json |
| Scoring | 0-100 per category + overall | Opinionated but actionable, like Lighthouse |
| Caching | bincode serialized RepoSnapshot | Fast serialization, incremental updates, instant re-runs |
| v1 scope | Git metadata only | Ship fast, prove value, then add AST in v2 |
| Time window | Default 6 months, configurable | Most relevant for active health assessment |
| Metrics count | 17 across 4 categories | Comprehensive but not overwhelming |
