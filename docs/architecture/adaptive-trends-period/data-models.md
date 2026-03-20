# Data Models — adaptive-trends-period (backfill)

**Date**: 2026-03-19
**Author**: Morgan (Solution Architect)

---

## 1. `HistoryEntry` — Added `source` Field

### Before (existing)

```rust
pub struct HistoryEntry {
    pub timestamp: i64,
    pub head: String,
    pub branch: String,
    pub overall_score: u32,
    pub categories: CategoryScores,
    pub metrics: MetricValues,
    pub counts: CountValues,
    pub schema_version: u32,
}
```

Serialized JSON (live analyze entry):
```json
{
  "timestamp": 1742329200,
  "head": "a1b2c3d4e5f6...",
  "branch": "main",
  "overall_score": 74,
  "categories": { ... },
  "metrics": { ... },
  "counts": { ... },
  "schema_version": 1
}
```

### After (with source field)

```rust
pub struct HistoryEntry {
    pub timestamp: i64,
    pub head: String,
    pub branch: String,
    pub overall_score: u32,
    pub categories: CategoryScores,
    pub metrics: MetricValues,
    pub counts: CountValues,
    pub schema_version: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}
```

Serialized JSON (backfill entry):
```json
{
  "timestamp": 1738800000,
  "head": "f6e5d4c3b2a1...",
  "branch": "main",
  "overall_score": 61,
  "categories": { ... },
  "metrics": { ... },
  "counts": { ... },
  "schema_version": 1,
  "source": "backfill"
}
```

Serialized JSON (live analyze entry — unchanged):
```json
{
  "timestamp": 1742329200,
  "head": "a1b2c3d4e5f6...",
  "branch": "main",
  "overall_score": 74,
  "categories": { ... },
  "metrics": { ... },
  "counts": { ... },
  "schema_version": 1
}
```

### `skip_serializing_if` rationale

`#[serde(skip_serializing_if = "Option::is_none")]` ensures:
- Live entries do not write a `"source": null` field to JSON, keeping the existing format byte-for-byte identical
- Existing `trends.json` files without a `source` key deserialize with `source = None` (serde fills missing optional fields with `None` by default)
- No `schema_version` bump is required (additive, backward-compatible change)

### Known values for `source`

| Value | Set by | Meaning |
|---|---|---|
| `None` (field absent) | `run_analyze` | Live analysis at HEAD |
| `Some("backfill")` | `backfill::run` | Retroactive analysis at historical SHA |

---

## 2. `BackfillConfig` — New TOML Section

### Struct

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
```

Added to `RepoConfig`:
```rust
pub struct RepoConfig {
    pub skip_blame: bool,
    #[serde(default)]
    pub backfill: BackfillConfig,
    // ... existing fields
}
```

### TOML representation

Minimal config (no backfill section — uses defaults):
```toml
skip_blame = false
```

With explicit backfill configuration:
```toml
skip_blame = false

[backfill]
sample_count = 25
```

### Valid range and clamping

| Constraint | Value |
|---|---|
| Default | 10 |
| Minimum (enforced at runtime) | 2 |
| Maximum (enforced at runtime) | 100 |
| Clamping location | `backfill::run()` — clamps before passing to `select_samples` |

Values outside the range produce a warning and are clamped, not rejected. This avoids hard errors from a misconfigured TOML breaking CI pipelines.

---

## 3. `BackfillSummary` — Internal Return Value

`BackfillSummary` is returned by `backfill::run()` and used to print a completion message to stderr. It is not serialized to any file.

```rust
pub struct BackfillSummary {
    pub analyzed: usize,   // SHAs successfully collected and written (or skipped as duplicate)
    pub skipped: usize,    // SHAs skipped because head already present in history
    pub warned: usize,     // SHAs that produced a git error (warn-and-continue)
}
```

Example terminal output (not part of the data model — for context only):

```
Backfill complete: 10 analyzed, 3 already in history, 0 errors
```

---

## 4. Sampling Algorithm

### Purpose

Given a full list of commit SHAs (newest-first) and a desired sample count `M`, produce `M` evenly-spaced SHAs that span the full history.

### Index Selection Formula

Let `N` = total number of commits. Let `M` = desired sample count (after clamping to `min(M, N)`).

**General case** (`M >= 2` and `M <= N`):

```
selected_indices = [ i * (N - 1) / (M - 1)  for i in 0 .. M ]
```

where division is integer (floor). This selects index 0 (most recent commit) and index N-1 (oldest commit) as anchors, with `M - 2` intermediate indices distributed evenly.

**Special case: `M == 1`**:

```
selected_indices = [0]
```

Selects only the most recent commit.

**Special case: `M >= N`**:

```
selected_indices = [0, 1, 2, ..., N-1]
```

All commits are selected (no sampling needed).

### Example — 1000 commits, 10 samples

```
N = 1000, M = 10
indices = [0, 111, 222, 333, 444, 555, 666, 777, 888, 999]
```

The most recent and oldest commits are always included. The 8 intermediate points divide the range as evenly as integer arithmetic allows.

### Example — 5 commits, 10 samples (M > N)

```
N = 5, M = 10 -> clamp M to 5
indices = [0, 1, 2, 3, 4]
```

All commits are returned; no duplication.

### Properties

- Deterministic: same input always produces the same output
- Anchored: index 0 (newest) and index N-1 (oldest) always selected when M >= 2
- Pure: no I/O, no randomness, no global state
- O(M) time, O(M) space

### Implementation location

`src/backfill/sampling.rs` — `pub fn select_samples(commits: &[String], count: usize) -> Vec<String>`
