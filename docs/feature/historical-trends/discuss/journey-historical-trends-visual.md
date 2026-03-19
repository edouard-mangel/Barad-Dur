# Journey: Historical Trends — Visual Map

## Persona: Marco Rossi (Engineering Lead)
## Goal: Understand whether repository health is improving or declining over time
## Emotional Arc: Uncertain → Orienting → Confident → Informed

---

## Flow Overview

```
[Trigger]           [Step 1]            [Step 2]            [Step 3]
Monthly ritual  --> Run analysis    --> Read trend      --> Act or share
"What's the score   (existing cmd +     output ("is it      (share with team,
this month?"        trend side-effect)   better or worse?")  file ticket, export)

Feels: uncertain    Feels: familiar     Feels: oriented     Feels: confident
                    (same command)      ("I see direction")  ("I have evidence")
```

---

## Step 1: Run Analysis (with trend recording)

**Command** (no change from current behavior):
```
barad-dur analyze .
```

**What happens under the hood**: current analysis runs as normal. At completion, result is appended to `.repository-analysis/trends.json` as a new entry. No extra flags required. First run silently creates the trend store.

**CLI Mockup — first ever run (no prior trend data)**:

```
+-- barad-dur analyze . ------------------------------------------------+
|                                                                        |
|  Analyzing barad-dur (main)  [last 6 months]                          |
|  [####################################] done in 12s                   |
|                                                                        |
|  Overall Score: 74/100                                                 |
|                                                                        |
|  Health      82  ████████████████░░░░                                 |
|  Team        71  ██████████████░░░░░░                                 |
|  Evolution   68  █████████████░░░░░░░                                 |
|  Git Hygiene 76  ███████████████░░░░░                                 |
|                                                                        |
|  Trend: first snapshot recorded                                        |
|  Run again next week to start tracking direction.                      |
|                                                                        |
|  Top actions:                                                          |
|    1. Reduce churn in src/metrics/health.rs (hotspot score: 8.4)      |
|    2. Add contributor to reduce bus factor (currently 2)              |
+------------------------------------------------------------------------+
```

**Emotional state on exit**: familiar — same command, same output shape. Small "Trend: first snapshot recorded" line is invisible unless you look for it. Zero disruption.

---

## Step 2: Read Trend Output (subsequent runs)

**Command** (same):
```
barad-dur analyze .
```

**CLI Mockup — 4th run, 6 weeks after first**:

```
+-- barad-dur analyze . ------------------------------------------------+
|                                                                        |
|  Analyzing barad-dur (main)  [last 6 months]                          |
|  [####################################] done in 11s                   |
|                                                                        |
|  Overall Score: 79/100  (+5 vs last run · +11 vs 6 weeks ago)         |
|                                                                        |
|  Health      85  ████████████████░░░░  +3                             |
|  Team        78  ███████████████░░░░░  +7                             |
|  Evolution   72  ██████████████░░░░░░  +4                             |
|  Git Hygiene 80  ████████████████░░░░  +4                             |
|                                                                        |
|  Trend (6 snapshots, 6 weeks):                                        |
|  Overall  68 → 69 → 72 → 74 → 77 → 79   ↑ improving                  |
|  Team      58 → 61 → 65 → 71 → 74 → 78  ↑ improving (fastest)        |
|                                                                        |
|  Top actions:                                                          |
|    1. Reduce churn in src/metrics/health.rs (hotspot score: 8.4)      |
+------------------------------------------------------------------------+
```

**Emotional state**: oriented. The delta (`+5 vs last run`) answers the core question immediately. The trend line is compact but directionally clear. Marco can say "we improved 11 points in 6 weeks."

---

## Step 3: Verbose Trend View

**Command** (explicit trend detail):
```
barad-dur analyze . --trend
```

**CLI Mockup**:

```
+-- barad-dur analyze . --trend ----------------------------------------+
|                                                                        |
|  Analyzing barad-dur (main)  [last 6 months]                          |
|  [####################################] done in 11s                   |
|                                                                        |
|  Overall Score: 79/100  (+5 vs last run)                              |
|                                                                        |
|  TREND HISTORY (6 snapshots)                                          |
|  ─────────────────────────────────────────────────────────────────    |
|  Date         Overall  Health   Team     Evol.  Hygiene               |
|  2026-02-04      68      79      58       65      71                  |
|  2026-02-11      69      80      61       66      72                  |
|  2026-02-18      72      81      65       69      74                  |
|  2026-03-04      74      82      71       68      76                  |
|  2026-03-11      77      83      74       70      78                  |
|  2026-03-18 *    79      85      78       72      80  <- today        |
|  ─────────────────────────────────────────────────────────────────    |
|  Direction   ↑+11    ↑+6    ↑+20    ↑+7    ↑+9                       |
|  Velocity    +1.8/wk                                                  |
|                                                                        |
|  Most improved: Team (+20 in 6 weeks)                                 |
|  Watch:  Evolution (slowest improvement, still below target)          |
|                                                                        |
|  Top actions:                                                          |
|    1. Reduce churn in src/metrics/health.rs                           |
+------------------------------------------------------------------------+
```

**Emotional state**: confident and informed. Marco can bring this table to a team meeting or copy the numbers into a status update. Priya can screenshot or export it.

---

## Step 4: Export / Share

**Commands**:
```
# JSON with trend history
barad-dur analyze . --trend --json -o trend-report.json

# HTML with trend charts
barad-dur analyze . --trend --html -o trend-report.html --open
```

**JSON output schema (trend section)**:
```json
{
  "trend": {
    "snapshots": [
      {
        "timestamp": "2026-02-04T09:15:00Z",
        "commit": "a1b2c3d",
        "branch": "main",
        "overall_score": 68,
        "categories": {
          "Health": 79, "Team": 58, "Evolution": 65, "Git Hygiene": 71
        }
      }
    ],
    "direction": "improving",
    "delta_vs_last": 5,
    "delta_vs_oldest": 11,
    "velocity_per_week": 1.8
  }
}
```

**Emotional state**: Priya exports the HTML report, pastes the link in Slack. Manager opens it and sees the upward trend chart. The refactoring is validated.

---

## Error Paths

### No trend data yet (first run)
```
Trend: first snapshot recorded.
Run again after your next commit cycle to see direction.
```
No error. Informational. Does not clutter output.

### Trend data exists but is stale (stored at a different branch)
```
Trend: 3 snapshots found on 'feature/refactor'; 0 on current branch 'main'.
  Use --trend-branch=feature/refactor to include cross-branch history,
  or run on 'main' to start a new trend sequence.
```

### Trend store corrupted
```
Warning: .repository-analysis/trends.json could not be read (corrupt or incompatible version).
  The file has been archived to trends.json.bak and a new trend history will start from this run.
  Re-run to confirm.
```

### User requests `--trend` with `--no-cache`
```
Note: --no-cache skips the snapshot cache but trend recording is always active.
  Today's result will be appended to trend history regardless.
```

---

## Integration Checkpoints

| Checkpoint | What must be true |
|------------|-------------------|
| After Step 1 (first run) | `.repository-analysis/trends.json` created; contains 1 entry with timestamp, commit hash, branch, scores |
| After Step 2 (subsequent runs) | New entry appended; delta computed correctly; CLI output shows delta |
| After `--trend` flag | Full history table renders; velocity computed |
| After `--json --trend` | `trend` key present in JSON output; schema stable |
| After `--html --trend` | HTML report includes trend tab with sparklines or table |
| Branch switch | Trend entries tagged by branch; branch mismatch surfaced, not silently mixed |
