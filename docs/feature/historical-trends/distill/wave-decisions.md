# Wave Decisions: historical-trends DISTILL

## D-01: Test Framework Choice — Rust Native Tests + assert_cmd

**Decision**: Write acceptance tests as Rust integration tests using
`#[test]` + `assert_cmd::Command` with Given/When/Then comment headers.
Do not introduce a Gherkin runner (cucumber-rs or similar).

**Rationale**:
- The existing project uses `assert_cmd` integration tests. Adding a Gherkin
  runner would create two test paradigms in the same project with no benefit
  for this feature scope.
- `assert_cmd` invokes the real binary through the CLI driving port, satisfying
  the hexagonal boundary mandate.
- Given/When/Then comment headers inside Rust tests provide the same intent
  documentation as `.feature` files without requiring a step-definition mapping
  layer.
- `#[ignore]` maps directly to the "one test at a time" mandate.

**Trade-off accepted**: Gherkin scenarios are less readable to non-Rust
stakeholders. Mitigated by maintaining `test-scenarios.md` as a human-readable
scenario registry.

---

## D-02: Scope — Walking Skeleton + Release 1 Only (US-03 excluded)

**Decision**: This DISTILL wave covers US-01, US-02, and US-04 only.
US-03 (Full History Table with `--trend` flag for CLI table output) is
excluded and will be covered in a subsequent DISTILL wave.

**Rationale**:
- US-03 is explicitly marked "Release 2" in the prioritization document.
- Including US-03 would add 5+ scenarios and a new CLI flag (`--trend` for
  table output vs. `--trend --json`) with interactions that are not yet fully
  specified.
- The walking skeleton (US-01 + US-02) delivers the primary user value
  (observable trend data without flags) and validates the full data pipeline.
- US-04 (`--json --trend`) is included because its backward-compatibility
  contract test (AC-04.2) is a regression safety net that should be established
  before any JSON renderer changes land.

---

## D-03: NDJSON Format for trends.json

**Decision**: `trends.json` uses NDJSON (newline-delimited JSON, one object per
line) rather than a JSON array.

**Rationale** (from architecture-design wave):
- NDJSON append is an O(1) file write: `open(APPEND) + write_line`. A JSON array
  requires reading the whole file, deserializing, pushing, and rewriting.
- Supports streaming reads — the file can be tailed or piped without loading
  the full history.
- Resilient to partial writes: a corrupt final line does not invalidate prior
  entries.

**Impact on tests**: The `read_trends()` helper in test files parses NDJSON by
splitting on newlines and deserializing each line, then returns `Vec<serde_json::Value>`.

---

## D-04: chrono Dependency in Test Files

**Decision**: Test files `trend_milestone_1.rs` uses `chrono::Utc::now()` and
`chrono::Duration` to build seeded trends.json entries with realistic timestamps.

**Rationale**: `chrono` is already a `[dependencies]` entry in `Cargo.toml`
(not just dev-dependencies). It is available to integration tests automatically.
No new dependency is required.

**Alternative considered**: Using hardcoded timestamp strings (e.g.,
`"2025-01-01T00:00:00Z"`). Rejected because tests that seed timestamps relative
to "now" are more robust — they do not need updating as time passes and the
"score drops from 99" scenario relies on the seeded entry predating the
current run.

---

## D-05: `ac_01_6_trend_recording_overhead_under_500ms` Test Design

**Decision**: The performance test (AC-01.6) measures the elapsed-time delta
between two consecutive runs on the same pre-warmed repository, rather than
using a fixed absolute time bound.

**Rationale**: An absolute bound (e.g., "total run must complete in < 2s") would
be flaky across machines with different I/O speeds and CPU counts. The delta
approach isolates trend-recording overhead specifically, which is what AC-01.6
requires ("adds at most 0.5 seconds").

**Caveat**: This test may still be flaky on very slow CI machines. If it becomes
a problem in practice, the software-crafter may add `#[ignore]` to it and add a
separate benchmark with `criterion`.

---

## D-06: Contract Test for Backward Compatibility (AC-04.2)

**Decision**: The backward-compat test captures the set of top-level JSON keys
from a baseline run and asserts the post-trend-feature run produces the same
key set.

**Rationale**: AC-04.2 requires structural identity, not value identity. Comparing
key sets (via `BTreeSet<String>`) catches any accidental addition or removal of
top-level keys while being resilient to score value changes (which would vary
between runs on a live repo).

**What it catches**: Any code change that adds a top-level key to `--json` output
without a `--trend` flag — for example, accidentally adding `"trend": null` to
the base renderer.

**What it does not catch**: Changes to nested field names within `categories[]`
or `top_actions[]`. Full structural validation of nested fields is covered by
the existing `analyze_json_is_valid` test in `integration_tests.rs`.

---

## Upstream Issues Found

### Issue 1: `--trend` flag dual-use ambiguity

The `--trend` flag is used in two contexts: `--json --trend` (US-04, JSON
trend schema) and `--trend` alone (US-03, CLI trend table). These are different
behaviors on the same flag, differentiated by presence of `--json`.

The architecture-design wave documents this as intentional (same flag, different
renderers). No design change requested. Noted here because the CLI tests for
US-04 must always include both `--trend` and `--json`; omitting either changes
the behavior under test.

### Issue 2: `tempfile` crate in `[dependencies]` vs `[dev-dependencies]`

`Cargo.toml` lists `tempfile = "3"` in `[dependencies]` (production code), not
`[dev-dependencies]`. This is unusual for a test utility. Existing tests rely on
this placement. No change made; noted in case a future dependency audit flags it.

### Issue 3: `serde_json` not in `[dev-dependencies]`

Integration tests use `serde_json` for JSON parsing. It is currently in
`[dependencies]`. Same observation as Issue 2 — it is available to tests but
lives in the production dependency section. No action required; existing
integration tests already depend on this.
