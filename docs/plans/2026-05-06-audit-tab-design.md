# Audit Tab Design

**Date:** 2026-05-06  
**Status:** Approved

## Goal

Add five legacy-audit diagnostic metrics surfaced in a new "Audit" HTML tab, inspired by piechowski.io/post/how-i-audit-a-legacy-rails-codebase/.

## Approach

Option A: new `src/scorer/audit.rs` module. All five builders take `&RepoSnapshot` and return typed structs. Results attach to `AnalysisReport` as `audit: Option<AuditReport>`. HTML rendered by a new `src/renderer/html/js_audit.rs` module.

## Data Model

New types added to `src/scorer/types.rs`:

```rust
pub struct CrisisFile {
    pub path: String,
    pub crisis_commit_count: usize,
    pub total_commit_count: usize,
    pub crisis_ratio: f64,
}

pub struct DirConcentration {
    pub dir: String,
    pub file_count: usize,
    pub loc: usize,
    pub pct_of_total: f64,
}

pub struct DeadFile {
    pub path: String,
    pub days_since_modified: i64,
    pub churn_count: usize,
}

pub struct VelocityBucket {
    pub week_start: String,   // "YYYY-MM-DD"
    pub commit_count: usize,
    pub author_count: usize,
}

pub struct AuditReport {
    pub crisis_files: Vec<CrisisFile>,
    pub dir_concentration: Vec<DirConcentration>,
    pub dead_files: Vec<DeadFile>,
    pub velocity_buckets: Vec<VelocityBucket>,
}
```

`AnalysisReport` gains `pub audit: Option<AuditReport>` (optional for backward compat with cached reports).

## Builder Logic (`src/scorer/audit.rs`)

### Crisis files
- Keywords: `fix`, `hotfix`, `revert`, `urgent`, `broken`, `oops`, `emergency`, `critical`, `crash`
- Classify commits once into a `HashSet<CommitId>`
- Per file: join `commits_by_file` against the set → `crisis_commit_count / total_commit_count`
- Return top 20 sorted by `crisis_ratio` desc

### Code concentration
- Aggregate `file_metrics` total_lines by directory prefix (first path component up to last `/`)
- Sort by loc desc; compute `pct_of_total = dir_loc / total_loc * 100.0`
- Include files at root as dir `"(root)"`

### Dead files
- A file is "possibly dead" if `days_since_modified > 180 AND churn_count <= 1`
- Reuse `FileAge` data (already built in `build_file_ages`) — don't recompute
- Sort by days_since_modified desc

### Commit velocity
- Bucket commits by ISO week (`timestamp.iso_week()`)
- Per bucket: commit count + distinct author count
- Return chronologically sorted

## HTML Tab (`src/renderer/html/js_audit.rs`)

Tab name: `"Audit"` — inserted after `"Dependencies"` in `js_authors.rs`.

Four sections:

1. **Crisis Files** — table: path | total commits | crisis commits | crisis ratio (inline red bar)
2. **Code Concentration** — horizontal bar chart per directory, LoC + % label
3. **Possibly Dead Files** — table: path | days since last touch | churn count
4. **Commit Velocity** — SVG bar chart: one bar per week, height = commit count

## Files to Create/Modify

| File | Change |
|------|--------|
| `src/scorer/types.rs` | Add 5 new structs + `audit` field on `AnalysisReport` |
| `src/scorer/audit.rs` | New — 4 builder functions |
| `src/scorer/mod.rs` (scorer.rs) | Call `build_audit_report()` in `build_report()` |
| `src/renderer/html/js_audit.rs` | New — `buildAuditTab()` JS |
| `src/renderer/html.rs` | Add `mod js_audit` + include in `build_js()` |
| `src/renderer/html/js_authors.rs` | Add `"Audit"` to `tabNames` + `buildAuditTab` to `tabContents` |

## Testing

- Unit tests in `src/scorer/audit.rs` for each builder (empty snapshot, populated snapshot)
- Renderer test: `audit` field present in rendered HTML
