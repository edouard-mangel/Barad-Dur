# Shared Artifacts Registry — historical-trends

All `${variables}` referenced across journey steps are documented here with single sources of truth and consumers.

---

## Registry

### overall_score

| Field | Value |
|-------|-------|
| Source of truth | `AnalysisReport.overall_score` in `src/scorer.rs` |
| Owner | scorer module |
| Integration risk | HIGH — if score shown in CLI differs from score stored in trends.json, trend deltas will be wrong |
| Validation | Integration test: run analysis, parse CLI output score, parse trends.json score, assert equal |

Consumers:
- CLI renderer (current run output)
- `trends.json` snapshot entry (persisted)
- Delta computation (step 2: delta = current_overall_score - previous_entry.overall_score)
- JSON output `overall_score` field
- HTML report overview

---

### category_scores

| Field | Value |
|-------|-------|
| Source of truth | `AnalysisReport.categories[*].score` in `src/scorer.rs` |
| Owner | scorer module |
| Integration risk | HIGH — category scores stored in trend entry must exactly match what is rendered in CLI |
| Validation | Integration test: parse CLI category rows, compare to trends.json last entry category_scores object |

Consumers:
- CLI renderer (category rows with bars)
- `trends.json` snapshot `category_scores` object (Health, Team, Evolution, "Git Hygiene")
- Per-category deltas in CLI step 2
- Full trend table columns in CLI step 3 (`--trend`)
- JSON `trend.snapshots[*].category_scores`
- HTML trend tab category columns

---

### commit_hash

| Field | Value |
|-------|-------|
| Source of truth | `RepoSnapshot.head_commit` (or equivalent in `src/snapshot.rs`) |
| Owner | snapshot / collector module |
| Integration risk | MEDIUM — used for correctness validation and deduplication, not displayed by default |
| Validation | Check trends.json entry commit field is a 40-char hex SHA |

Consumers:
- `trends.json` snapshot `commit` field
- Branch mismatch detection (compare stored commit's branch to current branch)
- Deduplication guard: do not append a duplicate entry if same commit analyzed twice

---

### branch

| Field | Value |
|-------|-------|
| Source of truth | `AnalysisReport.branch` in `src/scorer.rs` |
| Owner | scorer module |
| Integration risk | HIGH — mixing cross-branch snapshots silently produces misleading deltas |
| Validation | trends.json entries must include branch; CLI delta display must filter by current branch |

Consumers:
- CLI renderer (header line `Analyzing ${repo_name} (${branch})`)
- `trends.json` snapshot `branch` field
- Branch mismatch warning logic
- JSON `trend.snapshots[*].branch`

---

### timestamp

| Field | Value |
|-------|-------|
| Source of truth | System time at analysis completion (UTC) |
| Owner | trend recording module (new) |
| Integration risk | MEDIUM — velocity computation depends on accurate timestamps; must be ISO8601 UTC |
| Validation | trends.json timestamps must parse as valid ISO8601; velocity test uses fixed timestamps |

Consumers:
- `trends.json` snapshot `timestamp` field
- Date column in CLI trend history table (step 3)
- Velocity computation: `(last_score - first_score) / weeks_between(first.timestamp, last.timestamp)`
- JSON `trend.snapshots[*].timestamp`

---

### delta_last

| Field | Value |
|-------|-------|
| Source of truth | Computed: `current.overall_score - previous_entry.overall_score` |
| Owner | trend display module (new) |
| Integration risk | MEDIUM — displayed in CLI inline with score; must match JSON delta_vs_last |
| Validation | Integration test: assert CLI delta text matches JSON `trend.delta_vs_last` |

Consumers:
- CLI inline delta `(+${delta_last} vs last run)`
- JSON `trend.delta_vs_last`

---

### trend_sparkline

| Field | Value |
|-------|-------|
| Source of truth | All overall_score values from `trends.json` ordered by timestamp, plus current run |
| Owner | trend display module (new) |
| Integration risk | LOW — display only, not parsed downstream |
| Validation | Visual review; check that sequence matches trends.json entry order |

Consumers:
- CLI compact trend line in step 2 (`68 → 69 → 74 ↑ improving`)
- HTML trend tab chart (separate rendering from sparkline string)

---

### velocity

| Field | Value |
|-------|-------|
| Source of truth | Computed: `(last_overall_score - first_overall_score) / weeks_between_first_and_last` |
| Owner | trend display module (new) |
| Integration risk | LOW — informational; not used in automated decisions |
| Validation | Unit test with fixed inputs; check rounding to 1 decimal place |

Consumers:
- CLI `--trend` footer `Velocity: ${velocity}/wk`
- JSON `trend.velocity_per_week` (float, null when < 2 prior snapshots)

---

### trend_json_schema

| Field | Value |
|-------|-------|
| Source of truth | Rust struct `TrendReport` (to be defined; suggested: `src/trend.rs` or `src/scorer.rs`) |
| Owner | trend module + JSON renderer |
| Integration risk | HIGH — CI scripts and dashboards depend on this schema being stable |
| Validation | Schema version field in JSON output; contract tests against fixed fixture |

Consumers:
- JSON output `trend` key (`--json --trend`)
- HTML renderer (reads trend data to generate charts)
- CI/CD pipeline parsers (external consumers)
- Acceptance test fixtures

---

### trends_file_path

| Field | Value |
|-------|-------|
| Source of truth | Constant: `.repository-analysis/trends.json` |
| Owner | trend storage module (new, parallel to `cache/storage.rs`) |
| Integration risk | MEDIUM — must co-exist with `snapshot.bin` without conflict; both gitignored |
| Validation | Integration test: verify both files exist after analysis; verify neither is staged |

Consumers:
- Trend recording (write)
- Trend reading (read on each subsequent analysis)
- `.gitignore` entry (already covered by `CACHE_DIR = ".repository-analysis"`)

---

## Integration Risk Summary

| Risk Level | Artifacts | Mitigation |
|------------|-----------|------------|
| HIGH | overall_score, category_scores, branch, trend_json_schema | Integration tests asserting CLI output = stored values; schema contract test |
| MEDIUM | commit_hash, timestamp, delta_last, trends_file_path | Unit tests with fixed inputs; integration test for file creation |
| LOW | trend_sparkline, velocity | Visual review; unit test for velocity formula |

---

## Gitignore Verification

`.repository-analysis/` is already in `.gitignore` (enforced by `cache::storage::ensure_gitignore`). `trends.json` inherits this coverage because it lives in `.repository-analysis/`. No additional gitignore entry needed.

If a user adds `.repository-analysis/trends.json` explicitly to git, it should not cause errors — the trend recording should still work and the file will just be committed alongside other artifacts. This is acceptable.
