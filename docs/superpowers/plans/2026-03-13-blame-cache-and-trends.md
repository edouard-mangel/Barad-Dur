# Per-Blob Blame Cache + Historical Trends Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make repeat analysis runs near-instant by caching blame per blob OID, and track score history over time with a Trends tab in the HTML report.

**Architecture:** Two independent features sharing no code. Feature 1 adds a content-addressed blame cache (blob OID to blame lines) that survives snapshot invalidation. Feature 2 appends a JSONL history entry per unique HEAD and renders it in a new Trends tab with a per-metric line chart.

**Tech Stack:** Rust, bincode (blame cache serialization), serde_json (history JSONL), indicatif (progress), vanilla JS/SVG (chart)

**Spec:** `docs/superpowers/specs/2026-03-13-blame-cache-and-trends-design.md`

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src/snapshot.rs` | Modify | Add `blob_oid: String` to `FileEntry` |
| `src/collector/libgit.rs` | Modify | Populate `blob_oid` from `entry.id()` in tree walk |
| `src/collector/gitcli.rs` | Modify | Accept `&BlameCache`, skip cached blobs, return new entries |
| `src/collector/mod.rs` | Modify | Load/save/prune blame cache around blame phase |
| `src/cache/blame.rs` | Create | `BlameCache` struct, `load`/`save`/`prune` functions |
| `src/cache/mod.rs` | Modify | Add `pub mod blame;` and re-exports |
| `src/scorer.rs` | Modify | Add `HistoryEntry` struct + `build_history_entry` function |
| `src/cache/history.rs` | Create | `append_if_new_head` + `load_history` functions |
| `src/main.rs` | Modify | Wire blame cache + history recording |
| `src/renderer/html.rs` | Modify | Embed history in report data, add Trends tab (CSS + JS) |

---

## Chunk 1: Per-Blob Blame Cache

### Task 1: Add `blob_oid` to `FileEntry`

**Files:**
- Modify: `src/snapshot.rs:37-42` (FileEntry struct)
- Modify: `src/collector/libgit.rs:178-209` (collect_files tree walk)

- [ ] **Step 1: Write failing test — blob_oid exists on FileEntry**

In `src/collector/mod.rs` tests section, add:

```rust
#[test]
fn collect_files_populates_blob_oid() {
    let collector = Collector::open(std::path::Path::new("."), TimeWindow::default())
        .expect("should open repo");
    let files = collector.collect_files().expect("should collect files");
    assert!(!files.is_empty());
    for f in &files {
        assert!(
            !f.blob_oid.is_empty(),
            "blob_oid should be populated for {}",
            f.path.display()
        );
        assert_eq!(f.blob_oid.len(), 40, "blob_oid should be 40 hex chars");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib collect_files_populates_blob_oid`
Expected: compile error — `no field blob_oid on type FileEntry`

- [ ] **Step 3: Add `blob_oid` field to FileEntry**

In `src/snapshot.rs`, add to `FileEntry`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub is_binary: bool,
    pub depth: usize,
    pub blob_oid: String,
}
```

Update `RepoSnapshot::new()` — it doesn't construct FileEntry directly, so no change needed there.

- [ ] **Step 4: Populate blob_oid in collect_files**

In `src/collector/libgit.rs`, in `collect_files`, change the `files.push` block:

```rust
files.push(FileEntry {
    path,
    size_bytes,
    is_binary,
    depth,
    blob_oid: entry.id().to_string(),
});
```

- [ ] **Step 5: Fix any test compilation errors**

Other tests that construct `FileEntry` directly (e.g., in integration tests or scorer tests) need the new field. Search for `FileEntry {` across the codebase and add `blob_oid: String::new()` or `blob_oid: "0".repeat(40)` to each.

Run: `cargo test --lib`
Expected: all tests pass including the new one.

- [ ] **Step 6: Commit**

```bash
git add src/snapshot.rs src/collector/libgit.rs src/collector/mod.rs
# plus any other files that needed FileEntry fix
git commit -m "feat: add blob_oid to FileEntry for content-addressed blame caching"
```

---

### Task 2: Create BlameCache module

**Files:**
- Create: `src/cache/blame.rs`
- Modify: `src/cache/mod.rs`

- [ ] **Step 1: Write failing tests for BlameCache**

Create `src/cache/blame.rs` with test module:

```rust
use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::snapshot::BlameLine;

/// Content-addressed blame cache: blob OID to blame lines.
/// Lives in `.ncrunch/blame_cache.bin`, independent of the snapshot cache.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BlameCache {
    pub entries: HashMap<String, Vec<BlameLine>>,
}

// TODO: implement load, save, prune

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::TempDir;

    fn make_blame_line() -> BlameLine {
        BlameLine {
            author_id: 0,
            commit_id: "abc123".to_string(),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn blame_cache_save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let mut cache = BlameCache::default();
        cache.entries.insert(
            "a".repeat(40),
            vec![make_blame_line()],
        );
        save(&cache, dir.path()).unwrap();

        let loaded = load(dir.path()).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert!(loaded.entries.contains_key(&"a".repeat(40)));
    }

    #[test]
    fn blame_cache_load_missing_returns_empty() {
        let dir = TempDir::new().unwrap();
        let loaded = load(dir.path()).unwrap();
        assert!(loaded.entries.is_empty());
    }

    #[test]
    fn blame_cache_prune_removes_stale_entries() {
        let mut cache = BlameCache::default();
        cache.entries.insert("keep".repeat(10), vec![make_blame_line()]);
        cache.entries.insert("gone".repeat(10), vec![make_blame_line()]);

        let current_oids: std::collections::HashSet<String> =
            vec!["keep".repeat(10)].into_iter().collect();
        cache.prune(&current_oids);

        assert_eq!(cache.entries.len(), 1);
        assert!(cache.entries.contains_key(&"keep".repeat(10)));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Add `pub mod blame;` to `src/cache/mod.rs`.

Run: `cargo test --lib cache::blame`
Expected: FAIL — `save`, `load`, `prune` not found.

- [ ] **Step 3: Implement load, save, prune**

In `src/cache/blame.rs`, add before the tests module:

```rust
use crate::cache::storage::CACHE_DIR;

const BLAME_CACHE_FILE: &str = "blame_cache.bin";

impl BlameCache {
    /// Remove entries not in the current file tree.
    pub fn prune(&mut self, current_blob_oids: &std::collections::HashSet<String>) {
        self.entries.retain(|oid, _| current_blob_oids.contains(oid));
    }
}

pub fn load(repo_path: &Path) -> Result<BlameCache> {
    let cache_file = repo_path.join(CACHE_DIR).join(BLAME_CACHE_FILE);
    if !cache_file.exists() {
        return Ok(BlameCache::default());
    }
    let data = std::fs::read(&cache_file)?;
    match bincode::deserialize(&data) {
        Ok(cache) => Ok(cache),
        Err(_) => {
            let _ = std::fs::remove_file(&cache_file);
            Ok(BlameCache::default())
        }
    }
}

pub fn save(cache: &BlameCache, repo_path: &Path) -> Result<()> {
    let cache_dir = repo_path.join(CACHE_DIR);
    std::fs::create_dir_all(&cache_dir)?;
    let data = bincode::serialize(cache)?;
    std::fs::write(cache_dir.join(BLAME_CACHE_FILE), data)?;
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib cache::blame`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/cache/blame.rs src/cache/mod.rs
git commit -m "feat: add BlameCache module with load/save/prune"
```

---

### Task 3: Wire blame cache into collection flow

**Files:**
- Modify: `src/collector/gitcli.rs:12-38` (collect_blame function)
- Modify: `src/collector/mod.rs` (collect_snapshot_inner)
- Modify: `src/main.rs` (pass no_cache flag)

- [ ] **Step 1: Write failing test — collect_blame skips cached blobs**

In `src/collector/mod.rs` tests, add:

```rust
#[test]
fn collect_blame_uses_cache_for_known_blobs() {
    let collector = Collector::open(std::path::Path::new("."), TimeWindow::default())
        .expect("should open repo");
    let files = collector.collect_files().expect("should collect files");
    let collection = collector.collect_commits().expect("should collect commits");

    // First run: no cache
    let blame_cache = crate::cache::blame::BlameCache::default();
    let (blame_map, new_cache) = collector
        .collect_blame_cached(&files, &collection.authors, &blame_cache, &NoProgress)
        .expect("should collect blame");

    assert!(!blame_map.is_empty());
    assert!(!new_cache.entries.is_empty());

    // Second run: all blobs cached — should produce identical results
    let (blame_map2, _) = collector
        .collect_blame_cached(&files, &collection.authors, &new_cache, &NoProgress)
        .expect("should collect blame from cache");

    assert_eq!(blame_map.len(), blame_map2.len());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib collect_blame_uses_cache`
Expected: compile error — `collect_blame_cached` not found.

- [ ] **Step 3: Add `collect_blame_cached` to Collector**

In `src/collector/mod.rs`, add a new method:

```rust
/// Collect blame data, reusing cached entries for unchanged blobs.
/// Returns (blame_map, updated_cache).
pub fn collect_blame_cached(
    &self,
    files: &[FileEntry],
    authors: &[Author],
    cache: &crate::cache::blame::BlameCache,
    progress: &dyn Progress,
) -> Result<(HashMap<PathBuf, Vec<BlameLine>>, crate::cache::blame::BlameCache)> {
    gitcli::collect_blame_cached(self.repo_path(), files, authors, cache, progress)
}
```

- [ ] **Step 4: Implement `collect_blame_cached` in gitcli.rs**

In `src/collector/gitcli.rs`, add a new function alongside the existing `collect_blame`:

```rust
use crate::cache::blame::BlameCache;

/// Collect blame, reusing cached entries where blob OID matches.
pub fn collect_blame_cached(
    repo_path: &Path,
    files: &[FileEntry],
    authors: &[Author],
    cache: &BlameCache,
    progress: &(dyn super::Progress),
) -> Result<(HashMap<PathBuf, Vec<BlameLine>>, BlameCache)> {
    let email_to_id: HashMap<&str, AuthorId> =
        authors.iter().map(|a| (a.email.as_str(), a.id)).collect();

    let non_binary: Vec<&FileEntry> = files.iter().filter(|f| !f.is_binary).collect();

    let results: Vec<(PathBuf, Vec<BlameLine>, String)> = non_binary
        .par_iter()
        .filter_map(|f| {
            let lines = if let Some(cached) = cache.entries.get(&f.blob_oid) {
                cached.clone()
            } else {
                match blame_file(repo_path, &f.path, &email_to_id) {
                    Ok(lines) => lines,
                    Err(_) => Vec::new(),
                }
            };
            progress.inc(1);
            if lines.is_empty() {
                None
            } else {
                Some((f.path.clone(), lines, f.blob_oid.clone()))
            }
        })
        .collect();

    let mut new_cache = BlameCache::default();
    let mut blame_map = HashMap::new();
    for (path, lines, oid) in results {
        new_cache.entries.insert(oid, lines.clone());
        blame_map.insert(path, lines);
    }

    Ok((blame_map, new_cache))
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib collect_blame_uses_cache`
Expected: PASS.

- [ ] **Step 6: Wire into collect_snapshot_inner**

In `src/collector/mod.rs`, update `collect_snapshot_inner` to:
1. Load blame cache (unless `skip_blame`)
2. Call `collect_blame_cached` instead of `collect_blame`
3. Prune the cache against current blob OIDs
4. Save the blame cache

Replace the existing blame phase block. Add a `no_cache: bool` parameter alongside `skip_blame` to control whether to read/write the blame cache. Thread from `main.rs`.

The key logic:

```rust
// Phase 3: blame
let t = Instant::now();
let blame_map = if skip_blame {
    if show_progress {
        eprintln!(
            "  Skipping blame ({} files) -- use without --skip-blame for full analysis",
            non_binary
        );
    }
    HashMap::new()
} else {
    let blame_cache = if no_cache {
        crate::cache::blame::BlameCache::default()
    } else {
        crate::cache::blame::load(self.repo_path()).unwrap_or_default()
    };
    let cached_count = files
        .iter()
        .filter(|f| !f.is_binary && blame_cache.entries.contains_key(&f.blob_oid))
        .count();
    if show_progress && cached_count > 0 {
        eprintln!("  Blame cache: {}/{} files cached", cached_count, non_binary);
    }
    let blame_bar = if show_progress {
        let pb = ProgressBar::new(non_binary);
        pb.set_style(bar_style.clone());
        pb.set_message("Blaming files");
        pb.enable_steady_tick(std::time::Duration::from_millis(80));
        Some(pb)
    } else {
        None
    };
    let blame_progress: &dyn Progress = match &blame_bar {
        Some(pb) => pb,
        None => &NoProgress,
    };
    let (map, mut updated_cache) =
        self.collect_blame_cached(&files, &collection.authors, &blame_cache, blame_progress)?;
    if let Some(pb) = blame_bar {
        pb.finish_and_clear();
    }
    // Prune stale entries
    let current_oids: std::collections::HashSet<String> =
        files.iter().map(|f| f.blob_oid.clone()).collect();
    updated_cache.prune(&current_oids);
    // Save
    if let Err(e) = crate::cache::blame::save(&updated_cache, self.repo_path()) {
        eprintln!("Warning: Failed to save blame cache: {}", e);
    }
    map
};
let blame_ms = t.elapsed().as_millis();
```

- [ ] **Step 7: Update main.rs to pass `no_cache`**

Thread `args.no_cache` through `collect_and_cache` to `collect_snapshot_verbose` to `collect_snapshot_inner`.

- [ ] **Step 8: Run full test suite**

Run: `cargo test --lib`
Expected: all tests pass.

Run: `cargo test --test collector_tests -- --skip repo_name`
Expected: all integration tests pass. Note: integration tests calling `collect_blame` directly still work with the old API (they don't use the cache).

- [ ] **Step 9: Commit**

```bash
git add src/collector/gitcli.rs src/collector/mod.rs src/main.rs
git commit -m "feat: per-blob blame cache -- skip re-blame for unchanged files"
```

---

### Task 4: Manual verification of blame cache

- [ ] **Step 1: Build and install**

```bash
cargo install --path .
```

- [ ] **Step 2: First run on FW.Runtime (populates cache)**

```bash
cd /home/edouard/WS/FW.All/repos/FW.Runtime
time barad-dur analyze . --no-cache --html -o /tmp/fw-cached1.html -v
```

Expected: approx 90s (full blame, but blame cache now saved).
Verify: `.ncrunch/blame_cache.bin` exists.

- [ ] **Step 3: Second run (should use blame cache)**

```bash
time barad-dur analyze . --html -o /tmp/fw-cached2.html -v
```

Expected: blame phase near 0ms (all blobs cached). Total approx 5s.
Timing line should show `blame Xms` with X being very low.

- [ ] **Step 4: Verify output is identical**

Compare overall scores between both reports to confirm blame cache produces identical results.

---

## Chunk 2: Historical Trend Tracking

### Task 5: Create HistoryEntry and history module

**Files:**
- Modify: `src/scorer.rs` — add `HistoryEntry` struct + `build_history_entry`
- Create: `src/cache/history.rs` — append/load history JSONL
- Modify: `src/cache/mod.rs` — add `pub mod history;`

- [ ] **Step 1: Write failing tests**

In `src/scorer.rs` tests, add:

```rust
#[test]
fn build_history_entry_contains_all_metrics() {
    let snapshot = RepoSnapshot::new(
        PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    let categories = vec![];
    let report = build_report(&snapshot, categories, None);
    let entry = build_history_entry(&report, "abc123");

    assert_eq!(entry.head, "abc123");
    assert_eq!(entry.overall_score, report.overall_score);
}
```

In `src/cache/history.rs`, add tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_entry(head: &str, score: u32) -> HistoryEntry {
        HistoryEntry {
            timestamp: chrono::Utc::now(),
            head: head.to_string(),
            overall_score: score,
            categories: HashMap::new(),
            metrics: HashMap::new(),
            counts: HistoryCounts {
                commits: 10,
                files: 50,
                authors: 3,
            },
        }
    }

    #[test]
    fn append_if_new_head_writes_entry() {
        let dir = TempDir::new().unwrap();
        let entry = make_entry("abc123", 72);
        append_if_new_head(&entry, dir.path()).unwrap();

        let history = load_history(dir.path()).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].head, "abc123");
    }

    #[test]
    fn append_if_new_head_skips_duplicate() {
        let dir = TempDir::new().unwrap();
        let entry = make_entry("abc123", 72);
        append_if_new_head(&entry, dir.path()).unwrap();
        append_if_new_head(&entry, dir.path()).unwrap();

        let history = load_history(dir.path()).unwrap();
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn append_different_heads() {
        let dir = TempDir::new().unwrap();
        append_if_new_head(&make_entry("aaa", 70), dir.path()).unwrap();
        append_if_new_head(&make_entry("bbb", 75), dir.path()).unwrap();

        let history = load_history(dir.path()).unwrap();
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn load_history_empty_file() {
        let dir = TempDir::new().unwrap();
        let history = load_history(dir.path()).unwrap();
        assert!(history.is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib history`
Expected: compile errors — structs and functions don't exist yet.

- [ ] **Step 3: Add HistoryEntry to scorer.rs**

```rust
use std::collections::HashMap as StdHashMap;

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct HistoryCounts {
    pub commits: usize,
    pub files: usize,
    pub authors: usize,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct HistoryEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub head: String,
    pub overall_score: u32,
    pub categories: StdHashMap<String, u32>,
    pub metrics: StdHashMap<String, u32>,
    pub counts: HistoryCounts,
}

pub fn build_history_entry(report: &AnalysisReport, head: &str) -> HistoryEntry {
    let mut categories = StdHashMap::new();
    let mut metrics = StdHashMap::new();

    for cat in &report.categories {
        categories.insert(cat.name.clone(), cat.score);
        for m in &cat.metrics {
            metrics.insert(m.name.clone(), m.score);
        }
    }

    HistoryEntry {
        timestamp: chrono::Utc::now(),
        head: head.to_string(),
        overall_score: report.overall_score,
        categories,
        metrics,
        counts: HistoryCounts {
            commits: report.total_commits,
            files: report.total_files,
            authors: report.total_authors,
        },
    }
}
```

- [ ] **Step 4: Implement history.rs**

Create `src/cache/history.rs`:

```rust
use anyhow::Result;
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::Path;

use crate::cache::storage::CACHE_DIR;
use crate::scorer::HistoryEntry;

const HISTORY_FILE: &str = "history.json";

pub fn load_history(repo_path: &Path) -> Result<Vec<HistoryEntry>> {
    let path = repo_path.join(CACHE_DIR).join(HISTORY_FILE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = std::fs::File::open(&path)?;
    let reader = std::io::BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<HistoryEntry>(&line) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

pub fn append_if_new_head(entry: &HistoryEntry, repo_path: &Path) -> Result<()> {
    let path = repo_path.join(CACHE_DIR).join(HISTORY_FILE);

    // Check last entry's HEAD
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        if let Some(last_line) = content.lines().rev().find(|l| !l.trim().is_empty()) {
            if let Ok(last) = serde_json::from_str::<HistoryEntry>(last_line) {
                if last.head == entry.head {
                    return Ok(()); // Duplicate HEAD — skip
                }
            }
        }
    }

    std::fs::create_dir_all(repo_path.join(CACHE_DIR))?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    let json = serde_json::to_string(entry)?;
    writeln!(file, "{}", json)?;
    Ok(())
}
```

Add to `src/cache/mod.rs`:
```rust
pub mod history;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib history`
Run: `cargo test --lib build_history_entry`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/scorer.rs src/cache/history.rs src/cache/mod.rs
git commit -m "feat: add HistoryEntry and JSONL history append/load"
```

---

### Task 6: Wire history recording into main.rs

**Files:**
- Modify: `src/main.rs`
- Modify: `src/scorer.rs`

- [ ] **Step 1: Add `history` field to AnalysisReport**

In `src/scorer.rs`, add to `AnalysisReport`:
```rust
pub history: Vec<HistoryEntry>,
```

In `build_report`, initialize `history: Vec::new()`.

- [ ] **Step 2: Add history recording in main.rs**

After `build_report` and before rendering, add:

```rust
// Record history entry (deduplicated by HEAD)
let history_entry = scorer::build_history_entry(&report, &current_head);
if let Err(e) = cache::history::append_if_new_head(&history_entry, &local_path) {
    eprintln!("Warning: Failed to record history: {}", e);
}

// Load history for HTML report
let history = cache::history::load_history(&local_path).unwrap_or_default();
report.history = history;
```

- [ ] **Step 3: Run full test suite**

Run: `cargo test --lib`
Expected: all pass. Some tests may need `history: Vec::new()` added to AnalysisReport construction.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs src/scorer.rs
git commit -m "feat: record history entry after scoring, embed in report"
```

---

### Task 7: HTML Trends tab

**Files:**
- Modify: `src/renderer/html.rs`

- [ ] **Step 1: Write failing tests**

In `src/renderer/html.rs` tests, add:

```rust
#[test]
fn html_contains_trends_tab() {
    let html = render(&make_treemap_report()).unwrap();
    assert!(html.contains("Trends"), "Should have Trends tab");
}

#[test]
fn html_trends_has_metric_select() {
    let html = render(&make_treemap_report()).unwrap();
    assert!(
        html.contains("tr-metric-select"),
        "Should have metric selector dropdown"
    );
}

#[test]
fn html_trends_has_chart() {
    let html = render(&make_treemap_report()).unwrap();
    assert!(
        html.contains("tr-chart"),
        "Should have trends chart container"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib html_contains_trends_tab html_trends`
Expected: FAIL — "Trends" not in output.

- [ ] **Step 3: Add CSS for Trends tab**

In the CSS constant in `html.rs`, add:

```css
.tr-controls { display: flex; gap: 12px; align-items: center; margin-bottom: 16px; }
.tr-select { background: #0d1117; color: #c9d1d9; border: 1px solid #1e293b;
  border-radius: 6px; padding: 6px 12px; font-size: 14px; }
.tr-chart { width: 100%; background: #161b22; border-radius: 8px; padding: 16px; }
.tr-dot { cursor: pointer; }
.tr-dot:hover { r: 5; }
.tr-tooltip { position: fixed; background: #1e293b; color: #c9d1d9;
  padding: 8px 12px; border-radius: 6px; font-size: 12px;
  pointer-events: none; z-index: 1000; display: none; white-space: pre-line; }
.tr-empty { text-align: center; color: #8b949e; padding: 60px 20px; font-size: 16px; }
```

- [ ] **Step 4: Implement `buildTrendsTab()` JS function**

Add a new function in the JS section. Key elements:
- Read `R.history` array
- If fewer than 2 entries, show empty state with explanation
- Build info banner using `buildTabInfo()`
- Metric selector dropdown: "Overall Score", then 4 category names, then all individual metric names (extracted from first history entry's keys)
- SVG line chart: viewBox 0 0 900 350, with Y-axis gridlines at 0/25/50/75/100, X-axis date labels, polyline connecting data points, circle dots per data point
- `scoreColor()` function for green/yellow/red coloring based on score thresholds
- Tooltip on dot hover showing date, HEAD (first 7 chars), score, and counts
- `select.change` event redraws chart (no layout change, just new scores)
- Use `setTimeout(renderChart, 0)` for deferred initial render after DOM insertion

Use safe DOM methods (createElement, textContent, appendChild) for all user-facing text. SVG can be built as a string since it contains no user input (all data is numeric scores and hex hashes from our own JSONL).

- [ ] **Step 5: Register the Trends tab**

In `renderApp()` (around line 2384), update:

```javascript
var tabNames = ['Overview', 'Hotspots', 'Coupling', 'Ownership', 'Age', 'Treemap', 'Trends'];
var tabContents = [
  buildOverviewTab, buildHotspotsTab, buildCouplingTab,
  buildOwnershipTab, buildAgeTab, buildTreemapTab, buildTrendsTab
];
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --lib html_contains_trends html_trends`
Expected: all 3 new tests pass.

Run: `cargo test --lib`
Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/renderer/html.rs
git commit -m "feat(html): add Trends tab with per-metric line chart"
```

---

### Task 8: End-to-end verification

- [ ] **Step 1: Build and install**

```bash
cargo install --path .
```

- [ ] **Step 2: Run on barad-dur repo twice to populate history**

```bash
cd /home/edouard/WS/tool/myTool
barad-dur analyze . --no-cache --html -o /tmp/trends1.html -v
# Make a trivial change to force a new HEAD:
git commit --allow-empty -m "chore: test history tracking"
barad-dur analyze . --no-cache --html -o /tmp/trends2.html -v
```

- [ ] **Step 3: Open report and verify Trends tab**

Open `/tmp/trends2.html` in browser. Click "Trends" tab:
- Verify: 2 data points visible on chart
- Verify: dropdown has Overall, 4 categories, and 17 metrics
- Verify: switching metrics redraws the chart
- Verify: hovering a dot shows tooltip with date, HEAD, score, counts

- [ ] **Step 4: Verify blame cache savings on FW.Runtime**

```bash
cd /home/edouard/WS/FW.All/repos/FW.Runtime
time barad-dur analyze . --no-cache --html -o /tmp/fw-blame1.html -v
time barad-dur analyze . --html -o /tmp/fw-blame2.html -v
```

First run: approx 90s (populates blame cache).
Second run: expected under 5s (all blobs cached).

- [ ] **Step 5: Push**

```bash
git push
```
