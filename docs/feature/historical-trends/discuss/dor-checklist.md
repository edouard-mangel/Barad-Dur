# Definition of Ready Checklist — historical-trends

Validated against all 4 user stories. All items must PASS before handoff to DESIGN wave.

---

## US-01: Auto-record Trend Snapshot

| DoR Item | Status | Evidence |
|----------|--------|----------|
| Problem statement clear, domain language | PASS | "Marco keeps a manual spreadsheet because each run produces an isolated score" — concrete domain pain |
| User/persona identified with specific characteristics | PASS | Marco Rossi (engineering lead, monthly runner); Priya Nair (post-sprint runner, CI user) |
| 3+ domain examples with real data | PASS | 3 examples: first run (Marco), CI run (Priya weekly), corrupt file recovery (Marco disk error) |
| UAT scenarios (3-7) in Given/When/Then | PASS | 4 scenarios: first run, second run, corrupt file, --no-cache |
| AC derived from UAT | PASS | 6 AC items traceable to scenarios |
| Right-sized (1-3 days, 3-7 scenarios) | PASS | Estimated 1-2 days; 4 scenarios |
| Technical notes: constraints/dependencies | PASS | Dependency on CACHE_DIR, ensure_gitignore, schema_version, deduplication rule |
| Dependencies resolved or tracked | PASS | All dependencies exist in codebase; new module suggested (src/trend.rs) |
| Outcome KPIs defined | PASS | KPI-1 (100% trend accumulation), KPI-5 (runtime guardrail) |

### DoR Status: PASSED

---

## US-02: Inline Delta Display in CLI Output

| DoR Item | Status | Evidence |
|----------|--------|----------|
| Problem statement clear, domain language | PASS | "The number alone does not answer: is this getting better or worse?" — domain pain |
| User/persona identified with specific characteristics | PASS | Marco Rossi reading CLI output after monthly run |
| 3+ domain examples with real data | PASS | 3 examples: positive delta (+5), declining score (-6), stable (+0) |
| UAT scenarios (3-7) in Given/When/Then | PASS | 4 scenarios: positive delta, negative delta, branch mismatch, first-run message |
| AC derived from UAT | PASS | 5 AC items traceable to scenarios |
| Right-sized (1-3 days, 3-7 scenarios) | PASS | Estimated 1 day; 4 scenarios |
| Technical notes: constraints/dependencies | PASS | No color-only encoding; sparkline max 8 entries; depends on US-01 |
| Dependencies resolved or tracked | PASS | US-01 is a tracked dependency (must ship first) |
| Outcome KPIs defined | PASS | KPI-2 (< 30 sec directional answer) |

### DoR Status: PASSED

---

## US-03: Full Trend History Table (`--trend` flag)

| DoR Item | Status | Evidence |
|----------|--------|----------|
| Problem statement clear, domain language | PASS | "Priya has no artifact to show the before/after at sprint review — just her word" |
| User/persona identified with specific characteristics | PASS | Priya Nair (senior developer, sprint review presenter); Marco (decay detection) |
| 3+ domain examples with real data | PASS | 3 examples: Priya's sprint review, Marco's decay detection, new team with 2 runs |
| UAT scenarios (3-7) in Given/When/Then | PASS | 3 scenarios: full table, category insights, velocity N/A |
| AC derived from UAT | PASS | 7 AC items traceable to scenarios |
| Right-sized (1-3 days, 3-7 scenarios) | PASS | Estimated 1-2 days; 3 scenarios |
| Technical notes: constraints/dependencies | PASS | 80-column terminal constraint; no git ops; velocity formula documented; depends on US-01 |
| Dependencies resolved or tracked | PASS | US-01 tracked dependency |
| Outcome KPIs defined | PASS | KPI-3 (before/after artifact in < 60 sec) |

### DoR Status: PASSED

---

## US-04: JSON Trend Schema (`--json --trend`)

| DoR Item | Status | Evidence |
|----------|--------|----------|
| Problem statement clear, domain language | PASS | "Priya is writing a Grafana dashboard but has no machine-readable trend data to consume" |
| User/persona identified with specific characteristics | PASS | Priya Nair (DevOps/CI integration role); DevOps engineer writing alerting script |
| 3+ domain examples with real data | PASS | 3 examples: Grafana dashboard, Slack alert script, CI backward compat |
| UAT scenarios (3-7) in Given/When/Then | PASS | 3 scenarios: full trend key, backward compat, direction field |
| AC derived from UAT | PASS | 6 AC items traceable to scenarios |
| Right-sized (1-3 days, 3-7 scenarios) | PASS | Estimated 1 day; 3 scenarios |
| Technical notes: constraints/dependencies | PASS | Schema version, category_scores key alignment, velocity rounding, API contract note |
| Dependencies resolved or tracked | PASS | US-01 tracked dependency; existing JSON renderer is extension point |
| Outcome KPIs defined | PASS | KPI-4 (schema integration in < 1 hour) |

### DoR Status: PASSED

---

## Overall DoR Gate

| Story | Status |
|-------|--------|
| US-01: Auto-record Trend Snapshot | PASSED |
| US-02: Inline Delta Display | PASSED |
| US-03: Full History Table | PASSED |
| US-04: JSON Trend Schema | PASSED |

### Gate Decision: ALL STORIES PASSED — Ready for DESIGN wave handoff

---

## Anti-Pattern Check

| Anti-Pattern | Check | Result |
|--------------|-------|--------|
| Implement-X | Story titles describe user outcomes, not technical tasks | PASS |
| Generic data | Examples use Marco Rossi, Priya Nair, real score numbers | PASS |
| Technical AC | All AC describe observable user outcomes or system behaviors | PASS |
| Oversized stories | Largest story (US-03) is 3 scenarios / 1-2 days | PASS |
| Abstract requirements | All stories have 3+ concrete examples with real data | PASS |
