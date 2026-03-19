# Data Models — Historical Trends

Feature: `historical-trends`
Wave: DESIGN
Date: 2026-03-18

---

## Modified: `HistoryEntry` (src/scorer.rs)

Two fields are added. Both are backward-compatible via `#[serde(default)]`.

```
pub struct HistoryEntry {
    // existing fields (unchanged)
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub head: String,
    pub overall_score: u32,
    pub categories: HashMap<String, u32>,
    pub metrics: HashMap<String, u32>,
    pub counts: HistoryCounts,

    // NEW fields
    #[serde(default)]
    pub branch: String,          // branch name at time of analysis (D-04)

    #[serde(default)]
    pub schema_version: u32,     // current value = 1; 0 = legacy entry (DA-05)
}
```

Migration: entries written without `branch` will deserialise with `branch = ""`. The delta computation in `trend.rs` treats an empty branch as "unknown" and emits `branch_mismatch_warning = true` when the current branch is non-empty and the stored branch is empty. This allows graceful first-run-after-upgrade behaviour without data loss.

---

## New types in `src/trend.rs`

All types are `#[derive(Debug, Clone)]`. Types exposed to renderers are `Serialize` where needed.

### `TrendDelta`

Represents the score change between the most recent same-branch entry and the current run.

```
pub struct TrendDelta {
    pub overall: i32,
    pub categories: HashMap<String, i32>,
    pub is_first: bool,
}
```

- `overall`: positive = improved, negative = declined, 0 = stable
- `categories`: per-category delta, keyed by category name
- `is_first`: true when there are no prior same-branch entries

### `SparklinePoint`

One data point for the sparkline chart.

```
pub struct SparklinePoint {
    pub score: u32,
    pub head_short: String,   // first 7 chars of HEAD SHA
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
```

### `VelocityDirection`

```
pub enum VelocityDirection {
    Improving,   // velocity > +0.5 pts/run
    Declining,   // velocity < -0.5 pts/run
    Stable,      // -0.5 <= velocity <= +0.5
}
```

### `TrendVelocity`

```
pub struct TrendVelocity {
    pub points_per_run: f64,
    pub window_size: usize,
    pub direction: VelocityDirection,
}
```

### `TrendSummary`

The single output of `compute_trend()`. Passed to renderers.

```
pub struct TrendSummary {
    pub delta: TrendDelta,
    pub sparkline: Vec<SparklinePoint>,
    pub velocity: Option<TrendVelocity>,
    pub branch_mismatch_warning: bool,
    pub history: Vec<HistoryEntry>,   // same-branch entries only, chronological
}
```

---

## Trend store file format

**File**: `.repository-analysis/trends.json`
**Format**: NDJSON — one valid JSON object per line, no trailing comma, no wrapping array
**Encoding**: UTF-8
**Line ending**: `\n` (LF)

### Per-line schema (v1)

```json
{
  "schema_version": 1,
  "timestamp": "2026-03-18T14:22:01Z",
  "head": "a3f8c21d9b0e1f2345678901234567890abcdef0",
  "branch": "main",
  "overall_score": 74,
  "categories": {
    "Health": 80,
    "Team": 72,
    "Evolution": 68,
    "Git Hygiene": 76
  },
  "metrics": {
    "Bus factor": 65,
    "Churn hotspots": 90,
    "Temporal coupling": 88,
    "Stale code": 72,
    "File complexity": 77
  },
  "counts": {
    "commits": 312,
    "files": 47,
    "authors": 4
  }
}
```

### Backward compatibility rules

| Rule | Detail |
|---|---|
| Missing `schema_version` field | Treated as version 0 (legacy). Entry remains valid; `branch` defaults to `""` |
| Unknown extra field | Ignored by deserialiser (`#[serde(deny_unknown_fields)]` NOT set) |
| `schema_version > CURRENT` | Trigger archive-and-replace (DA-05) |
| Malformed line | Skip silently (existing behaviour in `load_history`) |

---

## JSON output schema — `trend` key (FR-05)

When `--trend` is specified alongside `--json`, the existing JSON output gains one additional top-level key. All existing keys remain structurally identical (NFR-03).

```json
{
  "repo_name": "...",
  "overall_score": 74,
  "categories": [...],
  "...existing fields...": "...",
  "trend": {
    "is_first": false,
    "delta": {
      "overall": 3,
      "categories": {
        "Health": 2,
        "Team": 5,
        "Evolution": -1,
        "Git Hygiene": 6
      }
    },
    "velocity": {
      "points_per_run": 1.2,
      "window_size": 7,
      "direction": "Improving"
    },
    "sparkline": [
      { "score": 68, "head_short": "a3f8c21", "timestamp": "2026-02-10T09:00:00Z" },
      { "score": 70, "head_short": "b4e9d12", "timestamp": "2026-02-25T11:30:00Z" },
      { "score": 74, "head_short": "c5f0e23", "timestamp": "2026-03-18T14:22:01Z" }
    ],
    "branch_mismatch_warning": false
  }
}
```

When `--json` without `--trend`: the `trend` key is absent. The output is byte-for-byte identical in structure to the current implementation (NFR-03 verified).

---

## CLI output format (FR-02, FR-03, FR-04)

### Default (no `--trend`, ≥2 same-branch entries)

```
  Overall Score:  ████████████░░░░░░░░  74/100
  Trend:          ▁▃▄▆█  +3 vs last run  (+1.2 pts/run, improving)
```

### First run (no prior entries)

```
  Trend: first snapshot recorded
```

### Branch mismatch (current branch has no prior entries, other branches do)

```
  Trend: first snapshot on branch 'feature/xyz'  [prior data on 'main']
```

### `--trend` full table

```
━━━ Trend History ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Branch: main   8 snapshots recorded

  Run   HEAD      Date         Score   Health  Team   Evol   Hygiene
  1     a3f8c21   2026-02-10    68      77      65     62      71
  2     b4e9d12   2026-02-25    70      78      67     64      73
  ...
  8     c5f0e23   2026-03-18    74      80      72     68      76

  Velocity: +1.2 pts/run (improving, 8-run window)
  Best category: Git Hygiene (+5 vs run 7)
  Watch:         Evolution  (-1 vs run 7)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

The sparkline uses Unicode block characters: `▁▂▃▄▅▆▇█` (8 levels mapped linearly from the min to the max score in the window). For terminals that do not support Unicode, `format_sparkline` falls back to ASCII `._-+=^` (detected via `TERM` and `NO_COLOR` env vars).
