# Wave Decisions Summary — historical-trends DISCUSS wave

## Feature
historical-trends: historical trend analysis for barad-dur repository health scores

## Date
2026-03-18

---

## Key Decisions Made in This Wave

### D-01: Zero-flag default (auto-recording)
**Decision**: Trend snapshots are recorded automatically on every `analyze` run. No `--record-trend` or `--save` flag required.

**Rationale**: The primary adoption anxiety (from JTBD four forces analysis) is "this will be slow or require configuration." Making recording invisible eliminates the anxiety and ensures the feature builds value passively. If recording required a new flag, users would not use it until they already wanted trend data — by which time there is no history.

**Alternatives rejected**: `--record-trend` flag (requires opt-in before value appears), `barad-dur trend init` subcommand (too much ceremony).

---

### D-02: No re-analysis of past commits
**Decision**: Historical trend data is built exclusively from forward-running analyses. Past commit analysis is out of scope.

**Rationale**: The performance anxiety identified in four forces analysis is decisive. Re-analyzing 20 past commits at 85s each (blame-dominated) would produce a 28-minute command. This would cause the feature to be avoided entirely. The zero-overhead forward-only model is the only design that survives the performance constraint.

**Trade-off**: Users who start using the feature today will not have historical context for the past. They must wait weeks or months to build a meaningful trend. This is acceptable because the alternative (hours-long first run) is worse.

**Mitigation**: First-run message sets expectation clearly: "Trend: first snapshot recorded. Run again next week to start tracking direction."

---

### D-03: `--trend` flag for explicit history view
**Decision**: The full history table and velocity computation require an explicit `--trend` flag. The default output only shows the compact delta and sparkline.

**Rationale**: Material honesty and progressive disclosure. The default CLI output is already content-dense. Appending a 10-row table to every run would clutter the output for users who only want the current score. The delta (US-02) is inline and lightweight; the full table (US-03) is intentional exploration.

**Alternatives rejected**: Always show full table (too noisy), `barad-dur trend show` subcommand (diverges from the existing `analyze` command pattern).

---

### D-04: Branch isolation for delta computation
**Decision**: Delta is computed only using entries from the same branch as the current run. Cross-branch deltas are suppressed with a warning.

**Rationale**: A score "improvement" from branch main→feature/refactor could reflect the branch's work-in-progress state, not real improvement. Silently mixing branches would produce misleading deltas, eroding trust in the tool.

**Trade-off**: Users who switch branches frequently will see the "no prior same-branch snapshots" message more often. This is acceptable and honest.

**Future**: `--trend-branch=<name>` flag deferred to Release 3 for users who want explicit cross-branch comparison.

---

### D-05: Backward compatibility is a hard constraint
**Decision**: `--json` without `--trend` produces output structurally identical to the current version. The `trend` key only appears when `--trend` is explicitly specified.

**Rationale**: Priya's CI pipeline depends on the JSON schema. Any accidental change to `--json` output without the flag would break downstream consumers silently. This constraint is non-negotiable.

**Implementation note**: The JSON renderer must explicitly check for `--trend` before adding the trend key. Default serialization of a trend-aware AnalysisReport struct must not include the trend field unless requested.

---

### D-06: Deduplication by commit SHA
**Decision**: If the last entry in trends.json has the same commit SHA as the current analysis, the append is skipped (idempotent).

**Rationale**: Developers often run `analyze .` multiple times in a row without committing. Without deduplication, velocity calculations would be skewed by duplicate entries with identical timestamps and scores. The rule is simple and predictable.

**Trade-off**: If a user changes `--since` window or `--exclude` flags and re-runs on the same commit, the second run is skipped. This is acceptable — the scores would differ, but the commit context is the same. Advanced users can use `--no-cache` to force a new entry (this is an open question for DESIGN wave).

---

## Stories Produced

| Story | Release | MoSCoW |
|-------|---------|--------|
| US-01: Auto-record Trend Snapshot | Walking Skeleton | Must Have |
| US-02: Inline Delta Display | Walking Skeleton | Must Have |
| US-03: Full Trend History Table | Release 2 | Should Have |
| US-04: JSON Trend Schema | Release 1 | Should Have |

Note: US-03 (corrupt recovery) and US-04 (branch mismatch) from prioritization.md are embedded as acceptance criteria in US-01 and US-02 respectively rather than separate stories. They are below the right-sizing threshold as standalone stories (< 0.5 day effort each).

---

## Open Questions for DESIGN Wave

1. **Deduplication override**: Should `--force-trend` or a similar flag allow recording a new entry even when the commit SHA matches the last entry? Relevant for users who change analysis configuration between runs on the same commit.

2. **Trend store pruning**: Should `barad-dur.toml` support a `max_trend_entries` config? trends.json grows indefinitely at the current design (52KB/year is acceptable, but 5+ years adds up). The DESIGN wave should decide whether a pruning mechanism is needed.

3. **Trend display in `--html` without `--trend`**: Should the HTML report always include trend data if trends.json exists, or should it also require the `--trend` flag for consistency? The DESIGN wave should align this with the CLI behavior.

4. **Velocity computation stability**: The current velocity formula uses calendar weeks between first and last snapshot. Should it use a rolling window (e.g., last 8 snapshots only) to be more sensitive to recent changes? This is a design/algorithmic decision.

5. **`schema_version` upgrade path**: The DESIGN wave should define the migration strategy when schema_version needs to increment. Archive-and-replace (like corrupt file handling) or in-place migration?

---

## Handoff Package

Files produced in this wave:

| File | Purpose |
|------|---------|
| `jtbd-job-stories.md` | 4 job stories with three dimensions and forces; primary job identified |
| `jtbd-four-forces.md` | Forces analysis with switch likelihood assessment and design implications |
| `journey-historical-trends-visual.md` | ASCII mockups for all 4 journey steps + error paths |
| `journey-historical-trends.yaml` | Machine-readable journey schema with integration checkpoints |
| `journey-historical-trends.feature` | Gherkin scenarios for all journey steps including @property NFRs |
| `shared-artifacts-registry.md` | All ${variables} with sources, consumers, and integration risks |
| `story-map.md` | Story map backbone, walking skeleton, release slices |
| `prioritization.md` | Prioritized backlog with MoSCoW and release assignments |
| `requirements.md` | FR, NFR, business rules, out of scope, dependencies |
| `user-stories.md` | 4 LeanUX stories with full template (problem/examples/UAT/AC/KPIs) |
| `outcome-kpis.md` | 5 KPIs with measurement plan and hypothesis |
| `dor-checklist.md` | DoR validation for all 4 stories — all PASSED |
| `wave-decisions.md` | This file — 6 key decisions and open questions |

### Handoff to DESIGN wave (solution-architect)
All stories are DoR PASSED. Peer review not required for this automated subagent execution.

The DESIGN wave should begin with the walking skeleton (US-01 + US-02) and address the 5 open questions above before finalizing the architecture for Release 1.
