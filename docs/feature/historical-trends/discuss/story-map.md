# Story Map: historical-trends

## User: Marco Rossi (Engineering Lead) + Priya Nair (Senior Developer)
## Goal: Understand and communicate whether repository health is improving or declining over time

---

## Backbone

| Record Snapshot | Read Trend Delta | Explore Full History | Export Trend Data |
|-----------------|-----------------|---------------------|-------------------|
| Auto-append today's scores to trends.json on every analyze run | Show inline delta vs last run in standard CLI output | Show full history table with velocity on --trend flag | Output trend data in JSON and HTML |
| Handle first run (no prior data) | Handle branch mismatch | Handle < 2 snapshots edge case | Schema versioning |
| Handle corrupt/incompatible trends.json | Handle declining / stable / improving direction | Compute velocity | HTML trend tab |
| Preserve backward compat with snapshot.bin | Performance: no extra git ops | Identify most improved / lagging category | JSON backward compat (--json without --trend unchanged) |

---

### Walking Skeleton

Minimum end-to-end slice that connects all four activities:

1. **Record**: `barad-dur analyze .` appends a JSON entry to `.repository-analysis/trends.json`
2. **Read**: On subsequent run, CLI output shows `(+N vs last run)` inline with the overall score
3. **Explore**: The two entries are readable via `--trend` in a minimal table
4. **Export**: `--json --trend` includes a `trend` key with the 2 entries

This skeleton is verifiable: a user can run the command twice and see a delta. No branch handling, no velocity, no HTML — just the core loop.

---

### Release 1: Reliable Recording + Delta Display
Target outcome: Marco can say "improved 6 points since last week" with confidence.

- Auto-record snapshot on every `analyze .` run (Walking Skeleton)
- Handle first run gracefully (informational message, no delta)
- Handle corrupt trends.json (archive + restart)
- Show inline delta in CLI output (positive, negative, zero)
- Show compact trend sparkline in CLI output
- Branch mismatch warning (no mixed-branch deltas)
- `--json --trend` outputs stable trend schema with snapshots array

---

### Release 2: Full History Table + Velocity
Target outcome: Priya can present a trend table in a sprint review.

- `--trend` flag shows complete history table
- Velocity computed and shown in footer
- Most improved / watch categories identified
- Handle < 2 snapshots gracefully in --trend output
- `--html --trend` includes trend tab in HTML report

---

### Release 3: Usability Polish + CI Integration
Target outcome: CI pipelines and dashboards can consume trend data reliably.

- Schema version field in JSON trend output
- `--trend-branch=<name>` flag to compare cross-branch snapshots
- Trend data pruning / max history configuration (barad-dur.toml)
- `barad-dur trend list` subcommand (optional convenience, not critical path)

---

## Scope Assessment: PASS

7 user stories, 2 bounded contexts (trend storage + trend display), estimated 8-10 days total.

Walking skeleton is 2-3 days. Release 1 adds 3-4 days. Release 2 adds 2-3 days.

Release 3 is optional polish; defer if delivery pressure exists.

**Note**: This is brownfield — no walking skeleton in the nWave sense (existing codebase already works). "Walking skeleton" here means the thinnest slice of the new trend feature that delivers end-to-end value.
