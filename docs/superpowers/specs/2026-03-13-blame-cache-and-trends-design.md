# Per-Blob Blame Cache + Historical Trend Tracking

**Date**: 2026-03-13
**Status**: Approved

## Problem

1. **Blame is 95% of runtime** on large repos (85s of 90s on FW.Runtime with 8k files). The existing snapshot cache helps on identical HEAD, but any commit invalidates the entire cache — forcing a full re-blame even if only 2 files changed.

2. **No visibility into score trends** over time. Users can't tell if their codebase is improving or degrading without manually tracking scores across runs.

## Feature 1: Per-Blob Blame Cache

### Mechanism

Git blob OIDs are content-addressed hashes. If a file's blob OID hasn't changed, its blame output is identical regardless of what else changed in the repo. This enables surgical blame caching.

### Data Model Changes

**`FileEntry`** gains a new field:
```rust
pub struct FileEntry {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub is_binary: bool,
    pub depth: usize,
    pub blob_oid: String,  // NEW: 40-char hex from git tree entry
}
```

Populated in `collect_files` (libgit.rs) from `entry.id().to_string()`.

### Cache Format

**File**: `.ncrunch/blame_cache.bin` (bincode-serialized)

**Structure**: `HashMap<String, Vec<BlameLine>>` where key = blob OID.

Separate from `snapshot.bin` — snapshot invalidation (HEAD change) does not destroy blame data.

### Collection Flow

1. Load blame cache from `.ncrunch/blame_cache.bin` (or empty HashMap if missing/corrupt)
2. For each non-binary file:
   - If `blob_oid` exists in cache → reuse cached blame lines
   - Otherwise → run `git blame --porcelain`, store result in cache
3. Save updated blame cache
4. **Prune**: Remove entries whose blob OID doesn't appear in the current file tree (prevents unbounded growth from deleted/renamed files)

### Expected Performance

On FW.Runtime after initial run:
- Typical commit touches ~20 files → re-blame ~20 files instead of 8,329
- Expected: **<5s** instead of 85s for blame phase
- First run: no change (still ~85s, but blame cache is populated for next run)

### Interaction with `--skip-blame`

When `--skip-blame` is set, the blame cache is neither read nor written. The flag remains a way to skip blame entirely for the fastest possible partial analysis.

### Interaction with `--no-cache`

`--no-cache` currently forces full re-collection. With this change, `--no-cache` also ignores the blame cache (forces re-blame of all files). The blame cache is still saved afterward so subsequent runs benefit.

## Feature 2: Historical Trend Tracking

### Storage

**File**: `.ncrunch/history.json` (JSONL — one JSON object per line)

**Entry schema**:
```json
{
  "timestamp": "2026-03-13T10:30:00Z",
  "head": "abc123def456...",
  "overall_score": 72,
  "categories": {
    "Health": 63,
    "Team": 73,
    "Evolution": 63,
    "Git Hygiene": 93
  },
  "metrics": {
    "Bus factor": 20,
    "Churn hotspots": 90,
    "Temporal coupling": 25,
    "Stale code": 100,
    "File complexity": 80,
    "Knowledge distribution": 75,
    "Contributor activity": 100,
    "Ownership clarity": 90,
    "Collaboration patterns": 50,
    "Merge patterns": 50,
    "Growth trend": 40,
    "Refactoring ratio": 55,
    "Code age": 70,
    "Commit cadence": 90,
    "Commit message quality": 90,
    "History cleanliness": 90,
    "Gitignore coverage": 100
  },
  "counts": {
    "commits": 53,
    "files": 53,
    "authors": 3
  }
}
```

### Recording Logic

After scoring completes in `main.rs`:
1. Read last line of `.ncrunch/history.json` (if exists)
2. If HEAD hash matches last entry → skip (no duplicate)
3. Otherwise → append new JSONL line

Runs with `--skip-blame` still record (scores reflect what was computed). This gives honest history — if the user chose to skip blame, the entry reflects that.

### Growth

~300 bytes per entry. A repo analyzed once per commit for 10,000 commits = ~3MB. No pruning needed.

### HTML Report: Trends Tab

A new "Trends" tab (7th tab after Treemap) with:

- **Metric selector**: dropdown listing "Overall", 4 categories, and all 17 individual metrics
- **Line chart**: SVG, vanilla JS (consistent with other tabs)
  - X-axis: time (dates)
  - Y-axis: score 0-100
  - Data points as circles, connected by lines
- **Tooltip on hover**: date, HEAD short hash (first 7 chars), exact score, plus commit/file/author counts as secondary info
- **Color coding**: line color follows the score-color scheme (green > 70, yellow 40-70, red < 40) — the latest data point determines the line color
- **Empty state**: if fewer than 2 history entries, show a message explaining that trends appear after multiple runs

History data is embedded in `window.R.history` alongside existing report data.

## Files Changed

| File | Change |
|------|--------|
| `src/snapshot.rs` | Add `blob_oid` to `FileEntry` |
| `src/collector/libgit.rs` | Populate `blob_oid` from `entry.id()` |
| `src/collector/mod.rs` | Blame cache load/save/prune logic in collection flow |
| `src/collector/gitcli.rs` | Accept blame cache, skip cached files |
| `src/cache/storage.rs` | Add `save_blame_cache` / `load_blame_cache` functions |
| `src/cache/mod.rs` | Re-export new functions |
| `src/scorer.rs` | Add `HistoryEntry` struct, `build_history_entry` function |
| `src/main.rs` | Record history after scoring, pass blame cache through |
| `src/renderer/html.rs` | Embed history in report data, add Trends tab (CSS + JS) |
| `src/cli.rs` | No changes (existing flags cover the interactions) |

## Testing

### Blame Cache
- `blob_oid_populated_in_file_entry` — FileEntry has non-empty blob_oid after collect_files
- `blame_cache_roundtrip` — save and load blame cache produces identical data
- `blame_cache_reuses_matching_blobs` — files with cached blob OID skip git blame
- `blame_cache_prune_removes_stale` — entries not in current tree are removed
- `blame_cache_ignored_with_skip_blame` — --skip-blame doesn't read/write blame cache

### History
- `history_entry_recorded_on_new_head` — new HEAD appends to history
- `history_entry_skipped_on_same_head` — same HEAD doesn't duplicate
- `history_entry_contains_all_metrics` — all 17 metrics present in entry

### HTML
- `html_contains_trends_tab` — output contains "Trends" tab
- `html_trends_has_metric_select` — contains metric dropdown
- `html_trends_has_chart` — contains SVG chart container
