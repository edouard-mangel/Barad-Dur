# Journey: Reading the HTML Trend Graph

**Persona**: Marco Rossi — engineering lead who runs `barad-dur analyze --html` to share a self-contained report with his team.
**Goal**: Understand whether repository health is improving, identify which data points come from live analysis vs historical backfill, and form a confident conclusion.

---

## Journey Flow (ASCII)

```
[Open HTML file]
      |
      v
[Scan header / tabs]
      |
      v
[Click "Trends" tab]
      |
      v
[See trend graph]
      |                 <-- PAIN POINT: all dots look identical
      v
[Hover over a dot]    <-- existing tooltip: date, SHA, score
      |
      v
[Try to distinguish  <-- CONFUSION: no visual cue for backfill vs live
 backfill from live]
      |
      v
[Read directional    <-- Is the improvement real? Or just backfill seeds?
 meaning from graph]
      |
      v
[Form conclusion]
      |
      v
[Share / act on it]
```

---

## Step-by-Step Journey

### Step 1 — Open the HTML Report

**Action**: Marco runs `barad-dur analyze --html -o report.html` and opens the file in a browser.

**UI State**:
```
+----------------------------------------------------------+
| Barad-dur | my-project    [main] [142 commits] [8 authors]|
+----------------------------------------------------------+
| Overview | Hotspots | Coupling | Ownership | Age | Trends |
+----------------------------------------------------------+
| Overview tab (default active)                            |
|   [Gauge: 72]   [Radar chart]   [Category cards]        |
+----------------------------------------------------------+
```

**Emotional State**: Efficient, purposeful. Marco knows what he wants.
**Confusion Points**: None yet.

---

### Step 2 — Navigate to the Trends Tab

**Action**: Marco clicks "Trends".

**UI State (current)**:
```
+----------------------------------------------------------+
| Overview | Hotspots | Coupling | Ownership | Age | Trends |
|                                          ^^^^^^^^        |
+----------------------------------------------------------+
| Score Trends                                             |
| Track how your repository scores change over time.       |
|                                                          |
| Metric: [Overall Score v]                                |
|                                                          |
| +------------------------------------------------------+ |
| |                                    *                 | |
| |                               *         *            | |
| |                         *                     *      | |
| |               *                                      | |
| |          *                                           | |
| | *                                                    | |
| |------------------------------------------------------| |
| | 2025-01   2025-04   2025-07   2025-10   2026-01      | |
| +------------------------------------------------------+ |
+----------------------------------------------------------+
```

**Emotional State**: Curious. The graph shows a clear upward trend. But something feels off.
**Confusion Points**: "There are 12 points here. I've only run analyze 2 or 3 times. Where did all these historical points come from?"

---

### Step 3 — Try to Understand the Trend

**Action**: Marco looks at the shape of the graph. The line goes up steadily over many months, then has some fluctuation at the right end.

**UI State (current — problem visible)**:
```
  12 data points, all rendered identically:

  Backfill entries (10):     Live entries (2):
       *                           *
  [filled circle,           [filled circle,
   same color,               same color,
   same size]                same size]

  Marco cannot tell them apart.
```

**Emotional State**: Confused, slightly frustrated. "Is this trend real? Did the score actually improve that much, or is this just backfill reconstruction? I can't trust the graph."
**Confusion Points**:
- All dots look the same — no distinction between historical reconstruction and real live runs
- The steady climb might be an artifact of backfill data (past state reconstructed retroactively), not real improvement
- The tooltip on hover shows date/SHA/score but not the source type

---

### Step 4 — Hover Over a Dot (Existing Tooltip)

**Action**: Marco hovers over a dot in the middle of the graph.

**UI State (current)**:
```
        +---------------------------+
        | 2025-06-15 (a3b4c5d)      |
        | Overall Score: 58         |
        | 47 commits, 31 files,     |
        | 3 authors                 |
        +---------------------------+
              ^
              | (tooltip, fixed position)
              |
     *--------*--------*
```

**Emotional State**: Slightly reassured — the tooltip is helpful. But still missing the key piece: "is this a real run or a backfill entry?"
**Confusion Points**: Tooltip does not show `source` — Marco cannot answer the key question.

---

### Step 5 — Wants to Distinguish Backfill from Live

**Action**: Marco wants to understand which points represent real analysis runs he performed and which are historical reconstructions from the backfill command.

**Mental model Marco needs**:
- Backfill = "reconstructed past state" = useful context but not a real-time measurement
- Live = "snapshot I actually ran" = the ground truth of improvement

**DESIGN OPTIONS explored here (as alternative UI states)**:

**Option A — Hollow vs Filled dots (preferred)**:
```
  Backfill points:         Live analysis points:
       o                         *
  [hollow circle]           [filled circle]
  [stroke-only SVG]         [solid fill SVG]
```

**Option B — Color distinction**:
```
  Backfill points:         Live analysis points:
       *                         *
  [dim gray]                [score color: green/amber/red]
```

**Option C — Tooltip enhancement only** (invisible distinction):
```
  Both look the same, but hovering reveals:
  +---------------------------+
  | 2025-06-15 (a3b4c5d)      |
  | Source: Backfill          |  <-- new field
  | Overall Score: 58         |
  +---------------------------+
```

**Option D — Legend + hollow/filled**:
```
  +------------------------------------------+
  | Score Trends           [Legend: o Backfill / * Live] |
  +------------------------------------------+
```

**Emotional State**: Expectant. Marco wants the graph to just show him the difference visually.

---

### Step 6 — Form a Conclusion

**Action**: With proper visual encoding, Marco scans the full graph and makes sense of it.

**UI State (desired — after feature is built)**:
```
+----------------------------------------------------------+
| Score Trends                                             |
|                                                          |
| Metric: [Overall Score v]    o Backfill  * Live          |
|                                                          |
| +------------------------------------------------------+ |
| |                                    *                 | |
| |                               o         *            | |
| |                         o                     o      | |
| |               o                                      | |
| |          o                                           | |
| | o                                                    | |
| |------------------------------------------------------| |
| | 2025-01   2025-04   2025-07   2025-10   2026-01      | |
| +------------------------------------------------------+ |
|                                                          |
|  Hover tooltip (live point):                             |
|  +------------------------------+                        |
|  | 2026-01-15 (d4e5f6a)         |                        |
|  | Source: Live analysis        |                        |
|  | Overall Score: 78            |                        |
|  | 142 commits, 31 files        |                        |
|  +------------------------------+                        |
+----------------------------------------------------------+
```

**Emotional State**: Confident, satisfied. "Now I can see: the backfill established the historical baseline (the steady climb was from reconstructed history). My two live runs show what's actually happening now — and it's improving."
**Confusion Points**: None. The story is clear.

---

### Step 7 — Share or Act

**Action**: Marco shares the report file with his team, or takes action based on what he sees.

**Emotional State**: Empowered. The graph told a story. He didn't have to explain "ignore the dots that are just from backfill."

---

## Emotional Arc

```
STEP 1         STEP 2         STEP 3         STEP 4         STEP 5         STEP 6
Purposeful --> Curious   --> Confused   --> Reassured  --> Expectant  --> Confident

Opens          Sees the       All dots       Tooltip        Wants the      Understands
report         upward         look the       helps but      visual         the difference,
with           trend          same           misses         distinction    forms
intent                                       source                        conclusion
```

Peak frustration: **Step 3** — "I can't tell backfill from live data."
Resolution: **Step 6** — hollow vs filled dots + source in tooltip + legend.

---

## Key UX Observations

1. **The confusion is about signal integrity, not aesthetics**. Marco's concern is epistemic: "Can I trust this trend?" The visual must answer that question without hover required.

2. **Hover tooltip is necessary but not sufficient**. Tooltip reveals detail on demand, but the baseline interpretation should be visible at a glance.

3. **Legend must be minimal**. Two items: `o Backfill` and a filled circle `Live`. No extra chrome.

4. **Zero backfill entries must work gracefully**. When no backfill data exists, all dots are filled (live) and the legend should either be absent or show only the live symbol.

5. **Color encoding alone (Option B) is insufficient**. Dim-vs-bright is fragile (colorblind users, different monitor gammas). Hollow-vs-filled is a shape distinction — robust across all viewing conditions.
