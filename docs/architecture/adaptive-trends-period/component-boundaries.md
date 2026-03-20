# Component Boundaries — adaptive-trends-period (backfill)

**Date**: 2026-03-19
**Author**: Morgan (Solution Architect)

This document specifies the public contracts (interfaces) for every new or modified component. Implementation decisions (internal data structures, algorithm bodies, error formatting) belong to the software crafter.

---

## New Components

### `src/backfill/mod.rs` — Backfill Orchestrator

**Responsibility**: Top-level coordination of the backfill flow. Loads configuration, retrieves the full commit list, delegates sampling, loops per sampled SHA, and writes entries to the history store. Owns error recovery (warn-and-continue on git errors).

**Public interface**:

```rust
pub fn run(args: &BackfillArgs, repo_path: &Path) -> Result<BackfillSummary>
```

**Dependencies (imports)**:
- `crate::cli::BackfillArgs`
- `crate::config` — `load_config()`
- `crate::collector` — `collect_all_commits()`, `Collector::collect_snapshot_at()`
- `crate::backfill::sampling` — `select_samples()`
- `crate::scorer` — `build_report()`, `build_history_entry()`
- `crate::cache::history` — `append_if_new_head()`
- `anyhow::Result`, `std::path::Path`

**Does NOT own**:
- Sampling index arithmetic (owned by `sampling.rs`)
- Score computation (owned by `scorer.rs`)
- Git I/O (owned by `collector/`)
- History file I/O (owned by `cache/history.rs`)
- Output rendering (owned by `renderer/`)

---

### `src/backfill/sampling.rs` — Pure Sampler

**Responsibility**: Given a list of commit SHAs and a desired sample count, return an evenly-spaced subset. Contains no I/O, no git calls, and no side-effects. This is a pure computational module.

**Public interface**:

```rust
pub fn select_samples(commits: &[String], count: usize) -> Vec<String>
```

**Contract**:
- `commits` is ordered newest-first (as returned by `collect_all_commits`)
- If `commits.len() <= count`, returns a clone of the full list
- If `count == 0`, returns an empty `Vec`
- If `count == 1`, returns `vec![commits[0].clone()]` (most recent commit)
- Otherwise, returns exactly `min(count, commits.len())` SHAs at evenly-spaced indices (see data-models.md for index formula)
- Output order preserves the input order (newest-first)

**Dependencies (imports)**:
- None beyond `std` (pure function, no external crates)

**Does NOT own**:
- Validation of sample_count range (owned by `backfill::run`)
- Commit retrieval (owned by `collector`)
- Any git operation

---

## Modified Components

### `src/cli.rs` — CLI Layer

**Responsibility**: Define the `Commands` enum and argument structs for clap parsing.

**Added to public interface**:

```rust
pub struct BackfillArgs {
    pub no_blame: bool,
}

// Added variant to Commands enum:
// Backfill(BackfillArgs)
```

**Does NOT own**:
- Dispatch logic (owned by `main.rs`)
- Business logic of any kind

---

### `src/main.rs` — Entry Point

**Responsibility**: Match on `Commands` and dispatch to the appropriate handler.

**Change**: One new `match` arm:

```rust
Commands::Backfill(args) => backfill::run(&args, &local_path)?
```

**Does NOT own**:
- Backfill orchestration (owned by `backfill::run`)

---

### `src/collector/mod.rs` — Collector

**Responsibility**: Provide snapshot collection. Existing `collect_snapshot_verbose()` is unchanged.

**Added to public interface**:

```rust
// On Collector struct:
pub fn collect_snapshot_at(
    &self,
    sha: &str,
    skip_blame: bool,
    repo_path: &Path,
) -> Result<RepoSnapshot>
```

**Contract**:
- `sha` is a full 40-character hex SHA
- `file_metrics` in the returned `RepoSnapshot` is always `HashMap::new()` (ADR-005)
- `skip_blame = true` bypasses all git blame calls; `skip_blame = false` runs blame at `sha`
- Returns `Err` on git object resolution failure (SHA not found); caller handles warn-and-continue

**Does NOT own**:
- Sampling logic
- History store I/O
- Score computation

---

### `src/collector/libgit.rs` — libgit2 Adapter

**Responsibility**: Wrap `git2` operations. Existing `collect_commits` and `collect_files` are unchanged.

**Added to public interface**:

```rust
pub fn collect_commits_at(
    repo: &git2::Repository,
    sha_str: &str,
    window: &TimeWindow,
) -> Result<Vec<CommitInfo>>

pub fn collect_files_at(
    repo: &git2::Repository,
    sha_str: &str,
) -> Result<Vec<FileEntry>>
```

**Contract**:
- `collect_commits_at` uses `revwalk.push(sha_oid)` — walks ancestry from the given SHA
- `collect_files_at` resolves `repo.find_commit(sha_oid)?.tree()` — lists files in that commit's tree
- Both return `Err` if `sha_str` cannot be resolved to a valid commit object

**Does NOT own**:
- Blame operations (owned by `gitcli.rs`)
- File content reads (owned by `collect_file_metrics`, which is not called for backfill)

---

### `src/collector/gitcli.rs` — Git CLI Adapter

**Responsibility**: Wrap git blame via `process::Command`.

**Changed signature**:

```rust
pub fn blame_file(
    repo_path: &Path,
    file: &str,
    authors: &mut AuthorMap,
    progress: Option<&ProgressBar>,
    at_rev: Option<&str>,
) -> Result<()>
```

**Contract**:
- `at_rev = None` — runs `git blame -- <file>` (HEAD, existing behavior)
- `at_rev = Some(sha)` — runs `git blame <sha> -- <file>` (historical SHA)
- All existing callers updated to pass `None`; no behavior change for `analyze` flow

**Does NOT own**:
- Decision of whether to skip blame (owned by caller)
- SHA selection (owned by `backfill::run`)

---

### `src/config.rs` — Configuration

**Responsibility**: Load and expose `RepoConfig` from TOML.

**Added to public interface**:

```rust
#[derive(Debug, Deserialize)]
pub struct BackfillConfig {
    pub sample_count: u32,
}

impl Default for BackfillConfig {
    fn default() -> Self {
        BackfillConfig { sample_count: 10 }
    }
}

// Added field to RepoConfig:
// #[serde(default)]
// pub backfill: BackfillConfig,
```

**Does NOT own**:
- Range validation of `sample_count` (owned by `backfill::run`, which clamps to 2–100)

---

### `src/scorer.rs` — Scorer

**Responsibility**: Build `AnalysisReport` and `HistoryEntry` from a `RepoSnapshot`.

**Changed data model**:

```rust
// Added field to HistoryEntry:
#[serde(skip_serializing_if = "Option::is_none")]
pub source: Option<String>,
```

**Contract**:
- `build_history_entry()` accepts an optional `source: Option<String>` parameter (or equivalent)
- Live `analyze` calls pass `None` — field omitted from serialized JSON
- Backfill calls pass `Some("backfill".to_string())`
- Existing deserialization of `trends.json` files without `source` field continues to work (field defaults to `None`)

**Does NOT own**:
- Decision of what `source` value to set (owned by caller: `backfill::run` or `run_analyze`)

---

## Boundary Invariants

| Invariant | Enforced by |
|---|---|
| `sampling.rs` performs no I/O | Rust module system: no `std::fs`, `std::process`, or `git2` imports permitted |
| `backfill/` does not import `renderer/` | Rust `pub(crate)` visibility + compiler |
| `collector/` does not import `backfill/` | Rust module system; would create a circular dependency |
| `file_metrics` is empty in all backfill snapshots | Single call site in `collect_snapshot_at`; no other path sets it for historical SHAs |
| Existing `collect_commits`, `collect_files`, `blame_file` (with `None`) behavior is unchanged | Additive changes only; existing callers updated to pass `None` for new `at_rev` param |
