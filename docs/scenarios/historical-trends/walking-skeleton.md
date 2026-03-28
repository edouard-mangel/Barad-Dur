# Walking Skeleton: historical-trends

## What is the Walking Skeleton?

The walking skeleton proves that a user can accomplish the core goal end-to-end
before any edge cases are handled. For this feature, the core goal is:

> A developer who runs `barad-dur analyze .` for the first time sees their score
> recorded automatically, and on the second run sees how their score has changed.

This is demo-able to a non-technical stakeholder: "Run the tool. Your history is
recorded. Run it again. Your progress is visible."

---

## Walking Skeleton Tests

Three tests constitute the walking skeleton (all in `tests/trend_walking_skeleton.rs`):

### Skeleton 1: `first_run_creates_trend_store`

**User goal**: A developer runs the tool for the first time and their score is
automatically saved — no config, no flags, no manual steps.

**Observable outcome**: `trends.json` is created, the terminal shows
"Trend: first snapshot recorded", and the tool exits successfully.

**Enables**: All of US-01 is gated on this test passing. Nothing else can work
until the tool writes trends.json on a successful run.

### Skeleton 2: `second_run_appends_to_trend_store`

**User goal**: A developer who has run the tool before comes back a week later
and their history grows automatically.

**Observable outcome**: `trends.json` has two entries after two runs, ordered
chronologically. The developer can see their history accumulating without any
manual action.

**Enables**: US-02 delta display. You cannot show a delta until you can store and
retrieve two entries.

### Skeleton 3: `delta_displayed_inline_after_prior_run_on_same_branch`

**User goal**: A developer runs the tool after a period of work and immediately
sees whether their codebase improved — directly in the terminal, without opening
any other file.

**Observable outcome**: The terminal output contains "vs last run" and a direction
word (improving / declining / stable). The developer can answer "are we improving?"
in under 5 seconds.

**Enables**: US-02 full AC coverage and US-04 JSON trend output. If inline delta
works, the data pipeline (load history → compute trend → render) is validated.

---

## Implementation Order

Enable tests one at a time in this order. Each test must pass before enabling
the next. This is the outer loop of Outside-In TDD — the acceptance test
defines "done" for each implementation increment.

```
Step 1  [enable] first_run_creates_trend_store
        [implement]
          - src/cache/history.rs: append_trend_entry(), read_trend_history()
          - src/scorer.rs: add branch + schema_version to HistoryEntry
          - src/main.rs: call record_entry() after scoring
          - src/renderer/cli.rs: emit "Trend: first snapshot recorded" message
        [commit] "feat: auto-record trend snapshot on first run"

Step 2  [enable] second_run_appends_to_trend_store
        [implement]
          - src/cache/history.rs: NDJSON append (not overwrite)
          - src/cache/history.rs: deduplication by commit SHA
        [commit] "feat: append trend entries on subsequent runs"

Step 3  [enable] delta_displayed_inline_after_prior_run_on_same_branch
        [implement]
          - src/trend.rs: compute_trend() pure function
          - src/renderer/cli.rs: inject TrendSummary into score line and sparkline
        [commit] "feat: show inline delta and direction indicator"
```

After the walking skeleton passes, continue with milestone-1 tests in this order:

```
Step 4   ac_01_3_first_run_outputs_first_snapshot_message
Step 5   ac_01_4_corrupt_trends_file_is_archived_and_replaced
Step 6   ac_01_5_no_cache_flag_still_records_trend
Step 7   ac_01_6_trend_recording_overhead_under_500ms
Step 8   ac_02_1_delta_shown_inline_with_overall_score
Step 9   ac_02_2_per_category_deltas_shown
Step 10  ac_02_3_sparkline_and_direction_indicator_shown
Step 11  ac_02_4_branch_mismatch_suppresses_delta_shows_warning
Step 12  ac_02_5_output_score_format_unchanged_for_script_compatibility
Step 13  ac_04_2_json_without_trend_flag_is_structurally_unchanged  ← run this early
Step 14  ac_04_5_velocity_is_null_when_fewer_than_2_prior_snapshots
Step 15  ac_04_1_json_trend_flag_outputs_trend_key_with_required_fields
Step 16  ac_04_1_trend_includes_delta_and_velocity_fields
Step 17  ac_04_6_direction_field_reflects_improving_trajectory
Step 18  ac_04_6_direction_is_declining_when_score_drops
```

Note: T-015 (AC-04.2 backward-compat contract test) should be enabled at Step 13,
early in the US-04 sequence, so any accidental breakage of the existing JSON
schema is caught immediately rather than at the end.

---

## Stakeholder Demo Script

After Step 3 passes, a non-technical stakeholder can observe:

1. "Here is a fresh repository with no history."
2. `barad-dur analyze .` — "The tool runs and says: Trend: first snapshot recorded."
3. `cat .repository-analysis/trends.json` — "Here is the recorded snapshot."
4. `barad-dur analyze .` — "The tool runs again and now shows the delta inline."

This is a complete demo without any internal implementation details exposed.

---

## Litmus Test Checklist (per Mandate 3)

- [x] Skeleton titles describe user goals ("first run creates trend store")
      not technical flows ("NDJSON append touches all layers")
- [x] Given/When steps describe user actions and context, not system state setup
      (uses real git repo, real binary invocation — not mocked internals)
- [x] Then steps describe user observations (file exists, stdout contains message)
      not internal side effects (no assertions on struct fields or DB state)
- [x] A non-technical stakeholder can confirm: "yes, that is what users need"
