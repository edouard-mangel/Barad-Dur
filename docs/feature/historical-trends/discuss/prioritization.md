# Prioritization: historical-trends

## Release Priority

| Priority | Release | Target Outcome | Rationale |
|----------|---------|----------------|-----------|
| 1 | Walking Skeleton | End-to-end trend loop works (record → read delta) | Validates the core assumption: passively-recorded snapshots are enough to deliver directional insight |
| 2 | Release 1: Reliable Recording + Delta | Marco can state direction with confidence | Directly addresses JS-01 (primary job). Highest push force. Used immediately by all users. |
| 3 | Release 2: Full History Table + Velocity | Priya can present trend data in sprint reviews | Addresses JS-02 and JS-03. Requires Release 1 data to be meaningful. |
| 4 | Release 3: CI Integration + Polish | Dashboards and pipelines consume trend data | Addresses JS-04. Lower urgency — CI users can parse Release 1 JSON in the meantime. |

---

## Backlog Suggestions

> Note: Story IDs assigned here for tracking. Full story text is in user-stories.md.

| Story | Release | Priority | Outcome Link | Dependencies | Effort |
|-------|---------|----------|-------------|--------------|--------|
| US-01: Auto-record trend snapshot | WS | P1 | JS-01 Record direction | Existing AnalysisReport + cache/storage patterns | 1-2 days |
| US-02: Inline delta display in CLI | WS | P1 | JS-01 Read direction | US-01 (trends.json must exist) | 1 day |
| US-03: Corrupt trends.json recovery | R1 | P2 | JS-01 Reliability | US-01 | 0.5 days |
| US-04: Branch mismatch warning | R1 | P2 | JS-01 Correctness | US-01, US-02 | 0.5 days |
| US-05: `--json --trend` schema | R1 | P2 | JS-04 CI integration | US-01, US-02 | 1 day |
| US-06: `--trend` full history table | R2 | P3 | JS-02 Validate refactoring | US-01, US-02 | 1-2 days |
| US-07: HTML trend tab | R2 | P3 | JS-02 Shareable artifact | US-06, existing HTML renderer | 1-2 days |

Total: 7 stories, ~7-10 days estimated effort.

---

## Riskiest Assumption

The entire feature rests on one assumption: **users will run `barad-dur analyze .` repeatedly over time without any extra steps.**

If users only run barad-dur once or very rarely, there will be no trend data to display, and the feature delivers no value.

**Validation strategy**: Release 1 already validates this passively. If the `.repository-analysis/trends.json` file accumulates entries in practice (observable in CI logs or user feedback), the assumption holds. If it doesn't, the feature needs a different trigger mechanism (e.g., a scheduled CI job).

This is why the walking skeleton must be invisible (zero extra steps) — lowering the barrier to accumulation is the critical design constraint.

---

## MoSCoW Classification

| Story | MoSCoW | Rationale |
|-------|--------|-----------|
| US-01: Auto-record | Must Have | Without this, no trend data exists — feature is impossible |
| US-02: Inline delta | Must Have | Without this, users have no reason to notice trend data exists |
| US-03: Corrupt recovery | Must Have | Without this, a single bad file silently breaks every future run |
| US-04: Branch warning | Should Have | Without this, branch switches produce misleading deltas — trust issue |
| US-05: JSON trend schema | Should Have | Needed for CI consumers; workaround: parse CLI output |
| US-06: Full history table | Should Have | Priya's use case; workaround: read trends.json manually |
| US-07: HTML trend tab | Could Have | Nice for presentations; workaround: share JSON or CLI screenshot |
