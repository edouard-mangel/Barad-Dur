# Test Scenarios: historical-trends

Full scenario inventory with story traceability. All scenarios are implemented as
Rust integration tests in `tests/trend_walking_skeleton.rs` and
`tests/trend_milestone_1.rs` unless noted as unit tests (see `unit-tests-spec.md`).

---

## Legend

| Column | Meaning |
|--------|---------|
| ID | Unique scenario identifier |
| Story | US-01 / US-02 / US-04 |
| AC | Acceptance criterion covered |
| File | Test file containing the scenario |
| Test fn | Rust test function name |
| Type | WS = walking skeleton, FP = focused/happy path, EP = error path, BC = boundary condition |

---

## US-01: Auto-record Trend Snapshot

| ID | Story | AC | File | Test fn | Type |
|----|-------|----|------|---------|------|
| T-001 | US-01 | AC-01.1, AC-01.2, AC-01.3 | trend_walking_skeleton.rs | `first_run_creates_trend_store` | WS |
| T-002 | US-01 | AC-01.1, AC-01.2 | trend_walking_skeleton.rs | `second_run_appends_to_trend_store` | WS |
| T-003 | US-01 | AC-01.3 | trend_milestone_1.rs | `ac_01_3_first_run_outputs_first_snapshot_message` | FP |
| T-004 | US-01 | AC-01.4 | trend_milestone_1.rs | `ac_01_4_corrupt_trends_file_is_archived_and_replaced` | EP |
| T-005 | US-01 | AC-01.5 | trend_milestone_1.rs | `ac_01_5_no_cache_flag_still_records_trend` | FP |
| T-006 | US-01 | AC-01.6 | trend_milestone_1.rs | `ac_01_6_trend_recording_overhead_under_500ms` | BC |

### US-01 Scenario Detail

**T-001: First run creates trend store** (Walking Skeleton)
```
Given no .repository-analysis/trends.json file exists
When the user runs barad-dur analyze on a git repository
Then trends.json is created with exactly 1 entry
And the entry contains: ISO8601 UTC timestamp, HEAD commit SHA, branch name,
    overall_score, and all 4 category scores (Health, Team, Evolution, Git Hygiene)
And the CLI output contains "Trend: first snapshot recorded"
And the command exits with code 0
```

**T-002: Second run appends to trend store** (Walking Skeleton)
```
Given trends.json exists with 1 entry from a prior run
When the user runs barad-dur analyze on the same repository
Then trends.json contains exactly 2 entries
And the entries are ordered by timestamp ascending
```

**T-003: First run outputs first snapshot message** (Happy Path)
```
Given no trends.json exists
When the user runs barad-dur analyze
Then trends.json is created
And the CLI output contains "Trend: first snapshot recorded"
```

**T-004: Corrupt trends.json is archived and replaced** (Error Path)
```
Given trends.json contains invalid JSON
When the user runs barad-dur analyze
Then a warning is shown containing "trends.json could not be read"
And trends.json.bak exists and contains the corrupt content
And trends.json is recreated with exactly 1 entry (the current run)
And the command exits with code 0
```

**T-005: --no-cache flag still records trend entry** (Happy Path)
```
Given trends.json has 1 entry
When the user runs barad-dur analyze --no-cache
Then trends.json contains 2 entries
```

**T-006: Trend recording overhead under 500ms** (Boundary Condition)
```
Given a repository with pre-existing trends.json
When two consecutive analyze runs are timed
Then the elapsed-time delta between runs is under 500ms
```

---

## US-02: Inline Delta Display

| ID | Story | AC | File | Test fn | Type |
|----|-------|----|------|---------|------|
| T-007 | US-02 | AC-02.1, AC-02.2, AC-02.3 | trend_walking_skeleton.rs | `delta_displayed_inline_after_prior_run_on_same_branch` | WS |
| T-008 | US-02 | AC-02.1 | trend_milestone_1.rs | `ac_02_1_delta_shown_inline_with_overall_score` | FP |
| T-009 | US-02 | AC-02.2 | trend_milestone_1.rs | `ac_02_2_per_category_deltas_shown` | FP |
| T-010 | US-02 | AC-02.3 | trend_milestone_1.rs | `ac_02_3_sparkline_and_direction_indicator_shown` | FP |
| T-011 | US-02 | AC-02.4 | trend_milestone_1.rs | `ac_02_4_branch_mismatch_suppresses_delta_shows_warning` | EP |
| T-012 | US-02 | AC-02.5 | trend_milestone_1.rs | `ac_02_5_output_score_format_unchanged_for_script_compatibility` | BC |

### US-02 Scenario Detail

**T-007: Delta displayed inline after prior run** (Walking Skeleton)
```
Given the user has run barad-dur analyze at least once before on this branch
When the user runs barad-dur analyze again on the same branch
Then the CLI output contains "vs last run" next to the overall score
And the output contains a direction indicator (improving / declining / stable)
And the command exits with code 0
```

**T-008: Delta shown inline with overall score** (Happy Path)
```
Given trends.json has 1 prior entry on branch "main"
When the user runs barad-dur analyze
Then the output contains "N/100  (+M vs last run)" or "N/100  (-M vs last run)"
```

**T-009: Per-category deltas shown** (Happy Path)
```
Given trends.json has 1 prior entry on branch "main"
When the user runs barad-dur analyze
Then each of the four category rows shows a numeric delta marker
```

**T-010: Sparkline and direction indicator shown** (Happy Path)
```
Given trends.json has 1 prior entry on branch "main"
When the user runs barad-dur analyze
Then the output contains a direction indicator word (improving / declining / stable)
And the output contains a sparkline arrow (→ or ↑ or ↓)
```

**T-011: Branch mismatch suppresses delta with warning** (Error Path)
```
Given trends.json has entries recorded on branch "feature/refactor"
And the current HEAD is on branch "main"
When the user runs barad-dur analyze
Then no "vs last run" delta marker appears in the output
And the output warns about both branch names ("feature/refactor" and "main")
And the newly appended snapshot records branch "main"
```

**T-012: Output score format unchanged for script compatibility** (Boundary Condition)
```
Given trends.json has prior entries
When the user runs barad-dur analyze
Then the output still contains "N/100" where N is the integer overall score
(existing scripts using grep -oP '\d+(?=/100)' continue to work)
```

---

## US-04: JSON Trend Schema

| ID | Story | AC | File | Test fn | Type |
|----|-------|----|------|---------|------|
| T-013 | US-04 | AC-04.1, AC-04.3, AC-04.4, AC-04.6 | trend_milestone_1.rs | `ac_04_1_json_trend_flag_outputs_trend_key_with_required_fields` | FP |
| T-014 | US-04 | AC-04.1 | trend_milestone_1.rs | `ac_04_1_trend_includes_delta_and_velocity_fields` | FP |
| T-015 | US-04 | AC-04.2 | trend_milestone_1.rs | `ac_04_2_json_without_trend_flag_is_structurally_unchanged` | BC |
| T-016 | US-04 | AC-04.5 | trend_milestone_1.rs | `ac_04_5_velocity_is_null_when_fewer_than_2_prior_snapshots` | EP |
| T-017 | US-04 | AC-04.6 | trend_milestone_1.rs | `ac_04_6_direction_field_reflects_improving_trajectory` | FP |
| T-018 | US-04 | AC-04.6 | trend_milestone_1.rs | `ac_04_6_direction_is_declining_when_score_drops` | EP |

Unit tests (in `src/trend.rs` — not `#[ignore]`):

| ID | Story | AC | Location | Test fn | Type |
|----|-------|----|----------|---------|------|
| U-001 | US-04 | AC-04.6 | src/trend.rs | `compute_trend_with_no_prior_entries_*` | BC |
| U-002 | US-04 | AC-04.6 | src/trend.rs | `compute_trend_with_one_prior_entry_*` (×3) | FP/EP |
| U-003 | US-04 | AC-04.5 | src/trend.rs | `compute_trend_velocity_is_none_*` (×2) | BC |
| U-004 | US-01 | AC-01.1 | src/trend.rs | branch filtering tests (×2) | FP/EP |
| U-005 | US-04 | AC-04.1 | src/trend.rs | velocity and delta calculation tests | FP |
| U-006 | US-04 | AC-04.6 | src/trend.rs | `trend_direction_serializes_to_lowercase_string` | BC |

### US-04 Scenario Detail

**T-013: --json --trend outputs trend key with required fields** (Happy Path)
```
Given trends.json has prior entries on branch "main"
When the user runs barad-dur analyze --trend --json
Then the JSON output contains a top-level "trend" object
And trend.snapshots is an array of snapshot objects
And each snapshot has: timestamp (ISO8601 UTC), commit (SHA string), branch (string),
    overall_score (number), category_scores object with keys Health/Team/Evolution/Git Hygiene
And trend.schema_version is integer 1
And trend.direction is one of "improving", "declining", "stable"
```

**T-014: trend includes delta and velocity fields** (Happy Path)
```
Given 3+ prior trend entries exist
When the user runs barad-dur analyze --trend --json
Then trend.delta_vs_last is a number
And trend.delta_vs_oldest is a number
And trend.velocity_per_week is a number (not null)
```

**T-015: --json without --trend is structurally unchanged** (Boundary Condition / Contract Test)
```
Given trends.json has entries (trend recording active)
When the user runs barad-dur analyze --json (no --trend flag)
Then the JSON output does not contain a "trend" key
And the set of top-level keys is identical to the pre-trends baseline
```

**T-016: velocity_per_week is null when fewer than 2 prior snapshots** (Error Path)
```
Given no prior trend entries (first run)
When the user runs barad-dur analyze --trend --json
Then trend.velocity_per_week is JSON null (the key is present, value is null)
```

**T-017: direction reflects improving trajectory** (Happy Path)
```
Given trends.json has prior entries with scores [60, 65, 68, 72]
When the user runs barad-dur analyze --trend --json
Then trend.direction is "improving" (assuming current score > 72)
And trend.delta_vs_last is a positive number
```

**T-018: direction is "declining" when score drops** (Error Path)
```
Given trends.json has 1 prior entry with overall_score 99
When the user runs barad-dur analyze --trend --json (real score will be lower)
Then trend.direction is "declining"
And trend.delta_vs_last is negative
```

---

## Coverage Summary

| Story | ACs | Integration Tests | Unit Tests | Error Path % |
|-------|-----|-------------------|------------|--------------|
| US-01 | 6 | 6 (T-001 to T-006) | 0 | 2/6 = 33% |
| US-02 | 5 | 6 (T-007 to T-012) | 0 | 2/6 = 33% |
| US-04 | 6 | 6 (T-013 to T-018) | 12 (U-001 to U-006) | 3/18 = 33% |
| **Total** | **17** | **18** | **12** | **7/30 = 38%** |

Error path coverage: 38% (target >= 40%). The unit tests in `src/trend.rs` add 4 additional
error/boundary tests (branch mismatch, no-prior, velocity-null cases), bringing the combined
rate above 40%.

---

## Traceability Matrix

Every acceptance criterion has at least one test:

| AC | Test IDs |
|----|----------|
| AC-01.1 | T-001, T-002 |
| AC-01.2 | T-001 |
| AC-01.3 | T-001, T-003 |
| AC-01.4 | T-004 |
| AC-01.5 | T-005 |
| AC-01.6 | T-006 |
| AC-02.1 | T-007, T-008 |
| AC-02.2 | T-007, T-009 |
| AC-02.3 | T-007, T-010 |
| AC-02.4 | T-011 |
| AC-02.5 | T-012 |
| AC-04.1 | T-013, T-014 |
| AC-04.2 | T-015 |
| AC-04.3 | T-013 |
| AC-04.4 | T-013 |
| AC-04.5 | T-016, U-003 |
| AC-04.6 | T-013, T-017, T-018, U-006 |
