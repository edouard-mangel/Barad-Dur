# Selective Blame — Cold-Run Coverage Gap Design

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make bus factor and knowledge distribution accurate on every run, not just after the blame cache has fully warmed up.

**Architecture:** Two complementary fixes. (1) After building `blame_map` from changed files, merge in any cached blame entries for unchanged files — this costs nothing and fixes accuracy on warm runs. (2) Fix `bus_factor` and `knowledge_distribution` to use the total non-binary file count as their denominator so cold-run partial coverage produces conservative scores rather than distorted ones.

**Tech Stack:** Rust, `src/collector/snapshot_builder.rs`, `src/metrics/health/bus_factor.rs`, `src/metrics/health/churn_ownership.rs`, `src/metrics/team/mod.rs`.

---

## Root Cause

`snapshot_builder.rs` builds `blame_files` from only the files touched in the time window:

```rust
let blame_files: Vec<FileEntry> = files
    .iter()
    .filter(|f| !f.is_binary && changed_paths.contains(&f.path))
    .cloned()
    .collect();
```

The per-blob cache is then consulted only for this subset. Cache entries for **unchanged files** are loaded into `blame_cache` but never merged into the final `blame_map`. As a result:

- `blame_map.len()` equals the number of changed files, not total files.
- `bus_factor` computes `dominated / blame_map.len()` — wrong denominator on every run until the cache is warm.
- `knowledge_distribution` Gini coefficient is biased toward recently-changed files.
- **This is not just a cold-run problem** — it affects every run because unchanged-file cache entries are discarded after loading.

## Approach

### Fix 1 — Merge cached entries for unchanged files (snapshot_builder.rs)

After the blame phase, iterate all non-binary files that were NOT in `changed_paths`. For each, check the loaded `blame_cache`; if an entry exists, insert it into `blame_map`.

```rust
// Merge cached blame for unchanged files
for f in files.iter().filter(|f| !f.is_binary && !changed_paths.contains(&f.path)) {
    if let Some(lines) = blame_cache.entries.get(&f.blob_oid) {
        blame_map.insert(f.path.clone(), lines.clone());
    }
}
```

**Effect:**
- Warm run (cache has entries for unchanged files): `blame_map` covers full codebase. Free — no `git blame` calls.
- Cold first run: unchanged files remain absent. Same as today — still partial.
- No performance impact on warm runs (HashMap lookups only).

### Fix 2 — Correct denominators in bus_factor and knowledge_distribution

Both metrics use `blame_map.len()` as if it were the total file count. Replace with the actual total.

**`bus_factor.rs`** — pass total non-binary file count alongside `blame_map`:

```rust
// Before
let total_files = snapshot.blame_map.len();

// After
let total_files = snapshot
    .files
    .iter()
    .filter(|f| !f.is_binary)
    .count()
    .max(snapshot.blame_map.len()); // never divide by less than what we have
```

Files absent from `blame_map` count as neither dominated nor safe — they are simply unsampled. The score reflects known risk over total codebase size, which is conservative on cold runs and accurate on warm runs.

**`team/mod.rs` (knowledge_distribution)** — the Gini coefficient is already line-count-based, so the denominator issue is less direct, but the sample bias is still real. Add a coverage note to the metric description when blame_map covers fewer than 80% of files.

### What this does NOT fix

Cold first-run accuracy for bus factor remains approximate until the cache warms up (one full blame of unchanged files). A future `--full-blame` flag could force blaming all non-binary files regardless of change history, warming the cache in one shot. Out of scope here.

## Files to Touch

| File | Change |
|------|--------|
| `src/collector/snapshot_builder.rs` | Merge cached blame for unchanged files after the blame phase |
| `src/metrics/health/bus_factor.rs` | Use total non-binary file count as denominator |
| `src/metrics/team/mod.rs` | Use total non-binary file count for coverage context |

## Testing

- Unit test: `blame_map` on a warm snapshot includes unchanged files whose cache entries exist.
- Unit test: `bus_factor` with 20/100 files in `blame_map` and 5 dominated → score reflects `5/100`, not `5/20`.
- Unit test: `bus_factor` with empty `blame_map` still returns score 50 (existing guard unchanged).
- Existing tests must all pass.
