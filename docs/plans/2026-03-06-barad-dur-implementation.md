# Barad-dur Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Rust CLI tool that analyzes git repositories and outputs health metrics with scores.

**Architecture:** Layered with shared RepoSnapshot data model. Collector populates snapshot from git, metric functions consume it, scorer aggregates results, renderer formats output.

**Tech Stack:** Rust, git2 (libgit2), clap (derive), serde/bincode, chrono, colored, rayon, indicatif, anyhow

---

## Task 0: Install Rust and Scaffold Project

**Files:**
- Create: `barad-dur/Cargo.toml`
- Create: `barad-dur/src/main.rs`
- Create: `barad-dur/.gitignore`

**Step 1: Install Rust toolchain**

Run: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && source ~/.cargo/env`
Expected: `rustc --version` prints version

**Step 2: Initialize the cargo project**

Run: `cd /home/edouard/WS/tool/myTool && cargo init --name barad-dur`

**Step 3: Add all dependencies to Cargo.toml**

Replace `Cargo.toml` with:

```toml
[package]
name = "barad-dur"
version = "0.1.0"
edition = "2021"
description = "The all-seeing repository analyzer"

[dependencies]
git2 = "0.19"
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
bincode = "1"
chrono = { version = "0.4", features = ["serde"] }
colored = "2"
rayon = "1"
indicatif = "0.17"
anyhow = "1"
```

**Step 4: Verify it compiles**

Run: `cd /home/edouard/WS/tool/myTool && cargo build`
Expected: Compiles with no errors

**Step 5: Initialize git repo and commit**

```bash
cd /home/edouard/WS/tool/myTool
git init
echo "target/" >> .gitignore
git add .
git commit -m "Init barad-dur project with dependencies"
```

---

## Task 1: Data Model — Core Types

**Files:**
- Create: `src/snapshot.rs`
- Modify: `src/main.rs` (add module declaration)

**Step 1: Write unit tests for the data model**

Create `src/snapshot.rs` with type definitions AND tests. The types are: `TimeWindow`, `AuthorId`, `CommitId`, `ChangeType`, `FileChange`, `Commit`, `FileEntry`, `Author`, `BlameLine`, `RepoSnapshot`.

Test that:
- `TimeWindow::default()` creates a 6-month window ending at now
- `TimeWindow::contains(timestamp)` returns true/false correctly
- `RepoSnapshot` can be created with `RepoSnapshot::new(path, name, branch, window)`
- `RepoSnapshot` derives `Serialize` and `Deserialize` (round-trip test via bincode)

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib snapshot`
Expected: FAIL — module doesn't exist yet or types missing

**Step 3: Implement the types**

Key design decisions for the implementation:
- `AuthorId` and `CommitId` are newtypes over `usize` and `String` respectively
- `ChangeType` is an enum: `Added, Modified, Deleted, Renamed`
- `RepoSnapshot::new()` initializes empty vecs and hashmaps
- `TimeWindow::default()` uses `Utc::now() - Duration::days(180)` as `since`
- All types derive `Serialize, Deserialize, Debug, Clone`

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib snapshot`
Expected: All tests PASS

**Step 5: Commit**

```bash
git add src/snapshot.rs src/main.rs
git commit -m "Add core data model types (RepoSnapshot, Commit, etc.)"
```

---

## Task 2: CLI Argument Parsing

**Files:**
- Create: `src/cli.rs`
- Modify: `src/main.rs`

**Step 1: Write tests for CLI parsing**

Test that:
- Default args: `analyze .` produces `AnalyzeArgs` with path=`.`, all categories enabled, default window, json=false
- Category flags: `analyze . --health` enables only health
- Time flags: `analyze . --since 3months` parses correctly
- Date flags: `analyze . --since 2024-01-01 --until 2024-06-30` parses dates
- `--all` sets since=None (full history)
- `--json` enables JSON output
- `--no-cache` disables cache
- `--json --pretty` enables pretty JSON
- `-o report.json` sets output file
- `-v` and `-vv` set verbosity levels

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib cli`
Expected: FAIL

**Step 3: Implement CLI with clap derive**

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "barad-dur", about = "The all-seeing repository analyzer")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Analyze a git repository
    Analyze(AnalyzeArgs),
}

#[derive(clap::Args, Debug)]
pub struct AnalyzeArgs {
    /// Path to the git repository (default: current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    // Category flags
    #[arg(long, help = "Run health metrics")]
    pub health: bool,
    #[arg(long, help = "Run team metrics")]
    pub team: bool,
    #[arg(long, help = "Run evolution metrics")]
    pub evolution: bool,
    #[arg(long, help = "Run git hygiene metrics")]
    pub hygiene: bool,

    // Time window
    #[arg(long, help = "Start of analysis window (e.g., '3months', '2024-01-01')")]
    pub since: Option<String>,
    #[arg(long, help = "End of analysis window (e.g., '2024-06-30')")]
    pub until: Option<String>,
    #[arg(long, help = "Analyze full history")]
    pub all: bool,

    // Output
    #[arg(long, help = "Output as JSON")]
    pub json: bool,
    #[arg(long, help = "Pretty-print JSON output")]
    pub pretty: bool,
    #[arg(short, long, help = "Write output to file")]
    pub output: Option<PathBuf>,

    // Verbosity
    #[arg(short, long, action = clap::ArgAction::Count, help = "Increase verbosity")]
    pub verbose: u8,

    // Cache
    #[arg(long, help = "Skip cache, force full re-collection")]
    pub no_cache: bool,
    #[arg(long, help = "Only use cache, fail if none exists")]
    pub cache_only: bool,
}
```

Add helper method `AnalyzeArgs::all_categories(&self) -> bool` — returns true if no category flag was set (meaning run all).

**Step 4: Wire into main.rs**

```rust
mod cli;
mod snapshot;

use clap::Parser;
use cli::Cli;

fn main() {
    let cli = Cli::parse();
    println!("{:?}", cli);
}
```

**Step 5: Run tests and verify**

Run: `cargo test --lib cli && cargo run -- analyze .`
Expected: Tests pass, CLI prints parsed args

**Step 6: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "Add CLI argument parsing with clap derive"
```

---

## Task 3: Collector — libgit2 Commit Walking

**Files:**
- Create: `src/collector/mod.rs`
- Create: `src/collector/libgit.rs`
- Modify: `src/main.rs` (add module)

**Step 1: Write integration test using a real git repo**

Create `tests/collector_tests.rs`. Use the barad-dur repo itself as a test fixture (it's a git repo by now). Test that:
- `open_repo(".")` succeeds and returns repo name
- `collect_commits(repo, time_window)` returns non-empty `Vec<Commit>`
- Each commit has: non-empty id, author, timestamp, message
- `collect_files(repo)` returns non-empty `Vec<FileEntry>` including `Cargo.toml`
- `collect_authors(commits)` returns deduplicated authors

**Step 2: Run tests to verify they fail**

Run: `cargo test --test collector_tests`
Expected: FAIL

**Step 3: Implement collector/mod.rs**

Define the `Collector` struct and public API:
```rust
pub struct Collector {
    repo: git2::Repository,
    time_window: TimeWindow,
}

impl Collector {
    pub fn open(path: &Path) -> Result<Self>;
    pub fn repo_name(&self) -> String;
    pub fn default_branch(&self) -> String;
    pub fn collect_commits(&self) -> Result<Vec<Commit>>;
    pub fn collect_files(&self) -> Result<Vec<FileEntry>>;
    pub fn collect_authors(commits: &[Commit]) -> Vec<Author>;
}
```

**Step 4: Implement collector/libgit.rs**

Key implementation details:
- `open`: Use `Repository::discover(path)` to find repo even from subdirectories
- `collect_commits`: Use `RevWalk` with `Sort::TIME | Sort::TOPOLOGICAL`, push HEAD. For each commit, diff against parent to get `FileChange` list. Filter by `time_window`.
- `collect_files`: Use HEAD tree, walk recursively. Compute `depth` from path components. Detect binary via `blob.is_binary()`.
- `collect_authors`: Deduplicate by email (lowercase), assign sequential `AuthorId`.

**Step 5: Run tests**

Run: `cargo test --test collector_tests`
Expected: PASS

**Step 6: Commit**

```bash
git add src/collector/ tests/collector_tests.rs src/main.rs
git commit -m "Add libgit2 collector for commits, files, and authors"
```

---

## Task 4: Collector — Git CLI Blame

**Files:**
- Create: `src/collector/gitcli.rs`
- Modify: `src/collector/mod.rs`

**Step 1: Write test for blame collection**

In `tests/collector_tests.rs`, add:
- `collect_blame(path, files, authors)` returns `HashMap<PathBuf, Vec<BlameLine>>` for at least `Cargo.toml`
- Each `BlameLine` has valid `author_id` that maps to a known author
- Binary files are skipped (no blame entry)

**Step 2: Run tests to verify they fail**

Run: `cargo test --test collector_tests blame`
Expected: FAIL

**Step 3: Implement gitcli.rs**

Shell out to `git blame --porcelain <file>` for each non-binary file. Parse the porcelain format to extract author email + commit hash per line. Map author emails to existing `AuthorId` via the author list. Use `rayon::par_iter` to blame files in parallel.

Key: Use `std::process::Command` to run git blame. Parse output line by line — porcelain format has `author-mail <email>` lines.

**Step 4: Add shallow clone detection**

Run `git rev-parse --is-shallow-repository` and store result. If shallow, log a warning that blame data may be incomplete.

**Step 5: Run tests**

Run: `cargo test --test collector_tests`
Expected: PASS

**Step 6: Commit**

```bash
git add src/collector/gitcli.rs src/collector/mod.rs tests/collector_tests.rs
git commit -m "Add git CLI blame collection with parallel execution"
```

---

## Task 5: Snapshot Builder — Derived Indexes

**Files:**
- Modify: `src/snapshot.rs`

**Step 1: Write tests for derived index building**

Test `RepoSnapshot::build_indexes(&mut self)`:
- `commits_by_author`: Given 3 commits by 2 authors, correctly groups them
- `commits_by_file`: Given commits touching files, correctly maps file→commit_ids
- `file_change_pairs`: Given commits where files A+B always change together, detects the pair with correct count

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib snapshot::tests`
Expected: FAIL

**Step 3: Implement build_indexes**

```rust
impl RepoSnapshot {
    pub fn build_indexes(&mut self) {
        self.build_commits_by_author();
        self.build_commits_by_file();
        self.build_file_change_pairs();
    }
}
```

For `file_change_pairs`: For each commit, get all changed file paths. For each pair of files in the same commit, increment a counter in a HashMap. Keep pairs where co-change count > some threshold (e.g., >= 3 co-changes).

**Step 4: Run tests**

Run: `cargo test --lib snapshot`
Expected: PASS

**Step 5: Commit**

```bash
git add src/snapshot.rs
git commit -m "Add derived index building for snapshot"
```

---

## Task 6: Full Snapshot Assembly

**Files:**
- Modify: `src/collector/mod.rs`

**Step 1: Write integration test for full snapshot**

In `tests/collector_tests.rs`:
- `Collector::collect_snapshot()` returns a fully populated `RepoSnapshot`
- Snapshot has non-empty commits, files, authors
- Derived indexes are populated
- `head_commit` is set to current HEAD hash

**Step 2: Run test to verify it fails**

Run: `cargo test --test collector_tests collect_snapshot`
Expected: FAIL

**Step 3: Implement collect_snapshot**

```rust
impl Collector {
    pub fn collect_snapshot(&self) -> Result<RepoSnapshot> {
        let commits = self.collect_commits()?;
        let files = self.collect_files()?;
        let authors = Self::collect_authors(&commits);
        let blame_map = self.collect_blame(&files, &authors)?;
        let head = self.head_commit_hash()?;

        let mut snapshot = RepoSnapshot {
            path: self.repo.workdir().unwrap().to_path_buf(),
            name: self.repo_name(),
            default_branch: self.default_branch(),
            time_window: self.time_window.clone(),
            head_commit: head,
            created_at: Utc::now(),
            commits, files, authors, blame_map,
            commits_by_author: HashMap::new(),
            commits_by_file: HashMap::new(),
            file_change_pairs: Vec::new(),
        };
        snapshot.build_indexes();
        Ok(snapshot)
    }
}
```

**Step 4: Run tests**

Run: `cargo test --test collector_tests`
Expected: PASS

**Step 5: Commit**

```bash
git add src/collector/mod.rs tests/collector_tests.rs
git commit -m "Add full snapshot assembly from collector"
```

---

## Task 7: Cache — Storage and Staleness

**Files:**
- Create: `src/cache/mod.rs`
- Create: `src/cache/storage.rs`
- Create: `src/cache/staleness.rs`
- Modify: `src/main.rs`

**Step 1: Write tests for cache storage**

Test in `src/cache/storage.rs` tests:
- `save(snapshot, path)` creates `.barad-dur/snapshot.bin` file
- `load(path)` returns the same snapshot (round-trip via bincode)
- `load` on nonexistent path returns `None`
- Corrupt file returns error (graceful degradation)

**Step 2: Write tests for staleness**

Test in `src/cache/staleness.rs` tests:
- `is_stale(cached_head, current_head)` returns false when equal
- `is_stale(cached_head, current_head)` returns true when different

**Step 3: Run tests to verify they fail**

Run: `cargo test --lib cache`
Expected: FAIL

**Step 4: Implement storage.rs**

```rust
use std::path::Path;
use std::fs;

const CACHE_DIR: &str = ".barad-dur";
const CACHE_FILE: &str = "snapshot.bin";

pub fn save(snapshot: &RepoSnapshot, repo_path: &Path) -> Result<()> {
    let cache_dir = repo_path.join(CACHE_DIR);
    fs::create_dir_all(&cache_dir)?;
    let data = bincode::serialize(snapshot)?;
    fs::write(cache_dir.join(CACHE_FILE), data)?;
    ensure_gitignore(repo_path)?;
    Ok(())
}

pub fn load(repo_path: &Path) -> Result<Option<RepoSnapshot>> {
    let cache_file = repo_path.join(CACHE_DIR).join(CACHE_FILE);
    if !cache_file.exists() { return Ok(None); }
    let data = fs::read(&cache_file)?;
    match bincode::deserialize(&data) {
        Ok(snapshot) => Ok(Some(snapshot)),
        Err(_) => { fs::remove_file(&cache_file)?; Ok(None) }
    }
}
```

`ensure_gitignore`: Read `.gitignore`, append `.barad-dur/` if not present.

**Step 5: Implement staleness.rs**

Simple: compare `snapshot.head_commit` with current HEAD hash from git2.

**Step 6: Run tests**

Run: `cargo test --lib cache`
Expected: PASS

**Step 7: Commit**

```bash
git add src/cache/ src/main.rs
git commit -m "Add snapshot cache with bincode storage and staleness detection"
```

---

## Task 8: Metrics Module Scaffold + Health Metrics

**Files:**
- Create: `src/metrics/mod.rs`
- Create: `src/metrics/health.rs`
- Modify: `src/main.rs`

**Step 1: Define MetricResult types in mod.rs**

```rust
pub struct MetricValue {
    pub name: String,
    pub description: String,
    pub raw_value: RawValue,
    pub score: u32,  // 0-100
}

pub enum RawValue {
    Integer(i64),
    Float(f64),
    Percentage(f64),
    Count(usize),
    Text(String),
    List(Vec<String>),
}

pub struct CategoryResult {
    pub name: String,
    pub score: u32,
    pub metrics: Vec<MetricValue>,
}
```

**Step 2: Write tests for each health metric**

In `src/metrics/health.rs` tests, create small mock `RepoSnapshot` structs with known data:

- **bus_factor**: 3 files, 2 authors. Author A owns 80% of file1. Bus factor for file1 = 1. Score = 0.
- **churn_hotspots**: 10 files, 1 file has 50% of all commits. Returns that file as top hotspot.
- **temporal_coupling**: Files A and B changed together in 9 out of 10 commits = 90% coupling. Detected.
- **stale_code**: 5 files, 2 untouched in window. Returns 40% stale.
- **file_complexity**: 1 file >1000 lines, 1 dir depth >5. Both flagged.

**Step 3: Run tests to verify they fail**

Run: `cargo test --lib metrics::health`
Expected: FAIL

**Step 4: Implement health metrics**

Each metric is a function: `fn metric_name(snapshot: &RepoSnapshot) -> MetricValue`

Top-level: `pub fn compute_health(snapshot: &RepoSnapshot) -> CategoryResult`

Key algorithms:
- **bus_factor**: For each file in blame_map, count lines per author sorted descending. Find min authors to cover 50%. Take the minimum across all files as the repo bus factor.
- **churn_hotspots**: Sort files by `commits_by_file[path].len()`. Top 5% by commit count are hotspots.
- **temporal_coupling**: From `file_change_pairs`, filter pairs where `co_change_count / min(file_a_changes, file_b_changes) > 0.7`.
- **stale_code**: Count files where `commits_by_file[path]` has no commits in time_window.
- **file_complexity**: Count files with `size_bytes > threshold`, dirs with `depth > 5`.

**Step 5: Run tests**

Run: `cargo test --lib metrics::health`
Expected: PASS

**Step 6: Commit**

```bash
git add src/metrics/ src/main.rs
git commit -m "Add health metrics: bus factor, churn, coupling, stale, complexity"
```

---

## Task 9: Team Metrics

**Files:**
- Create: `src/metrics/team.rs`
- Modify: `src/metrics/mod.rs`

**Step 1: Write tests for each team metric**

Mock snapshots with known data:
- **knowledge_distribution**: 3 authors, one owns 80% of lines. Gini > 0.5.
- **contributor_activity**: 5 authors total, 3 with commits in window. Returns 60%.
- **ownership_clarity**: 10 files, 7 have >50% blame to single author. Returns 70%.
- **collaboration_patterns**: Directory `src/auth/` only touched by 1 author = silo.
- **merge_patterns**: 20 commits, 5 are merges. Estimates avg branch lifetime from merge-to-merge intervals.

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib metrics::team`
Expected: FAIL

**Step 3: Implement team metrics**

Key algorithms:
- **knowledge_distribution (Gini)**: Sum of blame lines per author. Sort. Compute Gini coefficient: `G = (2 * sum(i * x_i)) / (n * sum(x_i)) - (n + 1) / n`
- **contributor_activity**: Filter `commits_by_author` where any commit timestamp falls in window.
- **ownership_clarity**: For each file in blame_map, find max author %. If > 50%, file has clear owner.
- **collaboration_patterns**: Group files by top-level directory. If a directory's commits are >80% from one author, it's a silo.
- **merge_patterns**: Filter `commits` where `is_merge == true`. Compute intervals between consecutive merges as proxy for branch lifetime.

**Step 4: Run tests**

Run: `cargo test --lib metrics::team`
Expected: PASS

**Step 5: Commit**

```bash
git add src/metrics/team.rs src/metrics/mod.rs
git commit -m "Add team metrics: knowledge distribution, activity, ownership, silos, merges"
```

---

## Task 10: Evolution Metrics

**Files:**
- Create: `src/metrics/evolution.rs`
- Modify: `src/metrics/mod.rs`

**Step 1: Write tests**

- **growth_trend**: 100 files at start of window, 115 now. Returns +15%.
- **refactoring_ratio**: 50 commits total. 15 are pure additions (only Added changes), 35 modify existing code. Ratio = 0.70.
- **code_age**: Blame lines with median timestamp 8 months ago. Returns "8 months".
- **commit_cadence**: 120 commits over 30 days. 4.0 commits/day. Low variance = "regular".

**Step 2: Run tests, verify fail**

Run: `cargo test --lib metrics::evolution`

**Step 3: Implement**

Key algorithms:
- **growth_trend**: Compare file count at earliest commit in window vs current. Also sum net line additions.
- **refactoring_ratio**: Classify commits: if all FileChanges are `Added` → "addition commit". Otherwise "modification". Ratio = modifications / total.
- **code_age**: Collect all blame line timestamps. Compute median. Express as duration from now.
- **commit_cadence**: Group commits by day. Compute mean and std deviation of daily count. High variance = "irregular".

**Step 4: Run tests, verify pass**

Run: `cargo test --lib metrics::evolution`

**Step 5: Commit**

```bash
git add src/metrics/evolution.rs src/metrics/mod.rs
git commit -m "Add evolution metrics: growth, refactoring ratio, code age, cadence"
```

---

## Task 11: Git Hygiene Metrics

**Files:**
- Create: `src/metrics/hygiene.rs`
- Modify: `src/metrics/mod.rs`

**Step 1: Write tests**

- **commit_message_quality**: Given messages ["Add login feature", "fix", "Update README", "wip"], 2/4 (50%) follow conventions (>10 chars, capitalized, imperative).
- **history_cleanliness**: Detect commits whose parent is NOT an ancestor of HEAD (proxy for force pushes — commit exists in reflog but not in main DAG). For v1, count merge commits with >2 parents as unusual.
- **gitignore_coverage**: Files list includes `.env`, `node_modules/package.json`, `*.log`. All flagged as suspicious.

**Step 2: Run tests, verify fail**

Run: `cargo test --lib metrics::hygiene`

**Step 3: Implement**

Key algorithms:
- **commit_message_quality**: Check each commit message for: length > 10 chars, starts with capital letter, doesn't end with period, first word looks imperative (not past tense). Also detect conventional commits pattern (`feat:`, `fix:`, etc.).
- **history_cleanliness**: Count merge commits. For force-push detection in v1, simply note it as "not detectable from commit data alone" and score based on merge hygiene.
- **gitignore_coverage**: Check tracked files against a suspicious patterns list: `[".env", ".env.*", "*.log", "*.key", "*.pem", "credentials", "secret", "node_modules", "*.pyc", "__pycache__", ".DS_Store", "Thumbs.db"]`.

**Step 4: Run tests, verify pass**

Run: `cargo test --lib metrics::hygiene`

**Step 5: Commit**

```bash
git add src/metrics/hygiene.rs src/metrics/mod.rs
git commit -m "Add hygiene metrics: commit messages, history cleanliness, gitignore"
```

---

## Task 12: Scorer

**Files:**
- Create: `src/scorer.rs`
- Modify: `src/main.rs`

**Step 1: Write tests**

- Individual metric scores are already set by each metric function (0-100)
- Category score = weighted average of its metrics' scores
- Overall score = weighted average of category scores (Health 30%, Team 30%, Evolution 20%, Hygiene 20%)
- Test: Health=80, Team=60, Evolution=70, Hygiene=50 → Overall = 80*0.3 + 60*0.3 + 70*0.2 + 50*0.2 = 24+18+14+10 = 66

**Step 2: Run tests, verify fail**

Run: `cargo test --lib scorer`

**Step 3: Implement**

```rust
pub struct AnalysisReport {
    pub repo_name: String,
    pub branch: String,
    pub time_window: TimeWindow,
    pub total_commits: usize,
    pub total_authors: usize,
    pub total_files: usize,
    pub overall_score: u32,
    pub categories: Vec<CategoryResult>,
    pub top_actions: Vec<String>,
}

pub fn score(categories: Vec<CategoryResult>) -> (u32, Vec<CategoryResult>) {
    // Apply weights and compute overall
}

pub fn generate_top_actions(categories: &[CategoryResult], snapshot: &RepoSnapshot) -> Vec<String> {
    // Pick top 3 most impactful suggestions from low-scoring metrics
}
```

**Step 4: Run tests, verify pass**

Run: `cargo test --lib scorer`

**Step 5: Commit**

```bash
git add src/scorer.rs src/main.rs
git commit -m "Add scorer with weighted category and overall scores"
```

---

## Task 13: CLI Renderer

**Files:**
- Create: `src/renderer/mod.rs`
- Create: `src/renderer/cli.rs`
- Modify: `src/main.rs`

**Step 1: Write tests**

Test that `render_cli(report)` produces a string containing:
- The header line "Barad-dur"
- Project name, branch, window info
- Overall score with progress bar
- Each category name and score
- Each metric name and value
- Top actions section

**Step 2: Run tests, verify fail**

Run: `cargo test --lib renderer`

**Step 3: Implement CLI renderer**

Use `colored` crate for terminal colors. Build the output string section by section matching the design mockup:

- Score colors: 0-40 red, 41-70 yellow, 71-100 green
- Progress bar: `████░░░░` using unicode block chars
- Metric values right-aligned with dot leaders
- Section headers with box-drawing characters

**Step 4: Run tests, verify pass**

Run: `cargo test --lib renderer`

**Step 5: Commit**

```bash
git add src/renderer/ src/main.rs
git commit -m "Add CLI renderer with colored output and progress bars"
```

---

## Task 14: JSON Renderer

**Files:**
- Create: `src/renderer/json.rs`
- Modify: `src/renderer/mod.rs`

**Step 1: Write tests**

Test that `render_json(report, pretty)`:
- Returns valid JSON (can be parsed back)
- Contains all expected fields: `overall_score`, `categories`, `top_actions`
- Pretty mode produces indented output
- Compact mode produces single-line output

**Step 2: Run tests, verify fail**

Run: `cargo test --lib renderer::json`

**Step 3: Implement**

Derive `Serialize` on `AnalysisReport` and `CategoryResult`. Use `serde_json::to_string` or `to_string_pretty` based on flag.

**Step 4: Run tests, verify pass**

Run: `cargo test --lib renderer::json`

**Step 5: Commit**

```bash
git add src/renderer/json.rs src/renderer/mod.rs
git commit -m "Add JSON renderer with pretty/compact modes"
```

---

## Task 15: Main Pipeline Wiring

**Files:**
- Modify: `src/main.rs`

**Step 1: Wire the full pipeline**

```rust
fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Analyze(args) => run_analyze(args)?,
    }
    Ok(())
}

fn run_analyze(args: AnalyzeArgs) -> Result<()> {
    // 1. Open repo
    let collector = Collector::open(&args.path)?;

    // 2. Check cache
    let snapshot = if !args.no_cache {
        if let Some(cached) = cache::load(&args.path)? {
            if !cache::is_stale(&cached, &collector)? {
                cached  // Cache hit!
            } else {
                collect_and_cache(&collector)?
            }
        } else if args.cache_only {
            anyhow::bail!("No cache found. Run without --cache-only first.");
        } else {
            collect_and_cache(&collector)?
        }
    } else {
        collect_and_cache(&collector)?
    };

    // 3. Compute selected metrics
    let categories = compute_selected_metrics(&snapshot, &args);

    // 4. Score
    let report = scorer::build_report(snapshot, categories);

    // 5. Render
    let output = if args.json {
        renderer::json::render(&report, args.pretty)?
    } else {
        renderer::cli::render(&report, args.verbose)?
    };

    // 6. Write output
    if let Some(path) = &args.output {
        std::fs::write(path, &output)?;
    } else {
        println!("{}", output);
    }

    Ok(())
}
```

**Step 2: Test end-to-end**

Run barad-dur against its own repo:

```bash
cargo run -- analyze .
cargo run -- analyze . --json --pretty
cargo run -- analyze . --health
cargo run -- analyze . --since 1month
```

Expected: Each produces valid output with scores.

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "Wire full analysis pipeline: collect -> cache -> metrics -> score -> render"
```

---

## Task 16: Progress Bars and Polish

**Files:**
- Modify: `src/collector/mod.rs`
- Modify: `src/collector/gitcli.rs`
- Modify: `src/main.rs`

**Step 1: Add progress indicators**

Use `indicatif` crate:
- Spinner during repo opening
- Progress bar during commit walking (total = estimated commit count)
- Progress bar during blame collection (total = file count)
- Spinner during metric computation

**Step 2: Add error messages for edge cases**

- Not a git repo: "Error: '{}' is not a git repository. Run from a repo root or pass a path."
- Shallow clone: "Warning: This is a shallow clone. Metrics may be incomplete."
- Empty repo: "Warning: No commits found in the specified time window."

**Step 3: Test edge cases manually**

```bash
cargo run -- analyze /tmp           # not a repo
cargo run -- analyze . --since 1day # possibly empty window
```

**Step 4: Commit**

```bash
git add src/
git commit -m "Add progress bars, error messages, and edge case handling"
```

---

## Task 17: Final Integration Test

**Files:**
- Create: `tests/integration_tests.rs`

**Step 1: Write end-to-end tests**

Using `assert_cmd` crate (add to `[dev-dependencies]`):
- `barad-dur analyze .` exits 0, output contains "Barad-dur"
- `barad-dur analyze . --json` exits 0, output is valid JSON
- `barad-dur analyze . --json --pretty` output is indented JSON
- `barad-dur analyze /nonexistent` exits non-zero
- `barad-dur analyze . --health` output contains "Health" but not "Team"

**Step 2: Run all tests**

Run: `cargo test`
Expected: All unit + integration tests pass

**Step 3: Final commit**

```bash
git add tests/integration_tests.rs Cargo.toml
git commit -m "Add end-to-end integration tests"
```

---

## Summary

| Task | Description | Estimated Complexity |
|------|-------------|---------------------|
| 0 | Project scaffold + deps | Low |
| 1 | Data model types | Low |
| 2 | CLI argument parsing | Low |
| 3 | Collector — libgit2 commits/files | Medium |
| 4 | Collector — git CLI blame | Medium |
| 5 | Snapshot derived indexes | Low |
| 6 | Full snapshot assembly | Low |
| 7 | Cache storage + staleness | Medium |
| 8 | Health metrics (5) | High |
| 9 | Team metrics (5) | High |
| 10 | Evolution metrics (4) | Medium |
| 11 | Hygiene metrics (3) | Medium |
| 12 | Scorer | Low |
| 13 | CLI renderer | Medium |
| 14 | JSON renderer | Low |
| 15 | Main pipeline wiring | Medium |
| 16 | Progress bars + polish | Low |
| 17 | Integration tests | Low |

**Total: 18 tasks, ~17 commits**
