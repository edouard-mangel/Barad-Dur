# Definition of Ready Checklist -- Cross-Repository Coupling Detection

## Validation Date: 2026-03-25

---

## US-01: Coupling Subcommand Discovers Repos in Root Directory

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | Adriana suspects 5-6 pairs are coupled; needs one command to scan repos |
| 2 | User/persona with specific characteristics | PASS | Adriana Kowalski, VP Eng, 60-person company, 24 microservices |
| 3 | 3+ domain examples with real data | PASS | 3 examples: 24 repos with skips, 5 repos small workspace, no repos found |
| 4 | UAT in Given/When/Then (3-7 scenarios) | PASS | 4 scenarios: discovery, single repo, no repos, non-existent dir |
| 5 | AC derived from UAT | PASS | 5 AC items traceable to scenarios |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | 1-2 days estimated, 4 scenarios |
| 7 | Technical notes: constraints/dependencies | PASS | CouplingArgs in cli.rs, first-level scan, reuses git validation |
| 8 | Dependencies resolved or tracked | PASS | No blocking dependencies; existing git validation reusable |
| 9 | Outcome KPIs defined | PASS | KPI-01: coupling identification speed; days to minutes |

### DoR Result: PASS

---

## US-02: Snapshot Collection with Progress and Skip-on-Error

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | Frozen terminal during 22-repo collection; one corrupt repo kills batch |
| 2 | User/persona with specific characteristics | PASS | Adriana running coupling analysis on 22 repos interactively |
| 3 | 3+ domain examples with real data | PASS | 3 examples: progress on 22 repos, corrupt pack file skip, cached repos fast |
| 4 | UAT in Given/When/Then (3-7 scenarios) | PASS | 3 scenarios: progress bar, cached repos, collection failure |
| 5 | AC derived from UAT | PASS | 6 AC items covering progress, cache, failure, summary |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | 1-2 days estimated, 3 scenarios |
| 7 | Technical notes: constraints/dependencies | PASS | Reuses Collector pipeline, indicatif for progress, rayon for parallel |
| 8 | Dependencies resolved or tracked | PASS | Collector pipeline stable; indicatif already in Cargo.toml |
| 9 | Outcome KPIs defined | PASS | KPI-05: collection resilience; one failure does not stop batch |

### DoR Result: PASS

---

## US-03: Temporal Coupling Analysis Across Repo Pairs

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | Manual CI log correlation to detect temporal coupling; days of effort |
| 2 | User/persona with specific characteristics | PASS | Adriana, payment-gateway <> billing-service correlation |
| 3 | 3+ domain examples with real data | PASS | 3 examples: 78% coupling (42/54), 74% coupling (28/38), 13% below threshold |
| 4 | UAT in Given/When/Then (3-7 scenarios) | PASS | 5 scenarios: high coupling, ranking, no coupling, custom window, minimum threshold |
| 5 | AC derived from UAT | PASS | 7 AC items covering computation, confidence, ranking, progress |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | 2-3 days estimated, 5 scenarios |
| 7 | Technical notes: constraints/dependencies | PASS | Binary search on sorted timestamps, O(P*N*logN), rayon parallel |
| 8 | Dependencies resolved or tracked | PASS | RepoSnapshot.commits already sorted by date |
| 9 | Outcome KPIs defined | PASS | KPI-01: identification speed; KPI-02: detection accuracy |

### DoR Result: PASS

---

## US-04: CLI Output with Ranked Coupling Pairs

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | 231 pairs need ranked display; raw data is unreadable |
| 2 | User/persona with specific characteristics | PASS | Adriana reviewing results, Yuki reading summary |
| 3 | 3+ domain examples with real data | PASS | 3 examples: ranked output, summary stats, no coupling case |
| 4 | UAT in Given/When/Then (3-7 scenarios) | PASS | 4 scenarios: ranked table, threshold filtering, skipped repos, no coupling |
| 5 | AC derived from UAT | PASS | 7 AC items covering header, ranking, summary, filtering, readability |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | 1 day estimated, 4 scenarios |
| 7 | Technical notes: constraints/dependencies | PASS | CLI renderer patterns, descending sort, 120-col terminal |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-03 for coupling data; same release |
| 9 | Outcome KPIs defined | PASS | KPI-01: top coupling pair identifiable in 5 seconds |

### DoR Result: PASS

---

## US-05: Team Coupling Detection (Shared Authors and Bridges)

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | Yuki is single-author bridge; invisible until she was unavailable |
| 2 | User/persona with specific characteristics | PASS | Yuki (bridge), Adriana (management view), team leads |
| 3 | 3+ domain examples with real data | PASS | 3 examples: single-author bridge (Yuki), 3 shared authors, email normalization (Carlos) |
| 4 | UAT in Given/When/Then (3-7 scenarios) | PASS | 3 scenarios: single bridge, multiple shared, name normalization |
| 5 | AC derived from UAT | PASS | 5 AC items covering score, listing, bridge flagging, normalization |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | 2 days estimated, 3 scenarios |
| 7 | Technical notes: constraints/dependencies | PASS | Lowercase name comparison, RepoSnapshot.authors, future mailmap |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-02 for snapshots; RepoSnapshot.authors already indexed |
| 9 | Outcome KPIs defined | PASS | KPI-03: single-author bridge detection; proactive not reactive |

### DoR Result: PASS

---

## US-06: Dependency Coupling Detection (Manifest Scanning)

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | Tomasz broke 5 services updating shared-libs; no blast radius map |
| 2 | User/persona with specific characteristics | PASS | Tomasz, platform lead, shared-libs maintainer |
| 3 | 3+ domain examples with real data | PASS | 3 examples: 5 consumers of shared-libs, 3 shared external deps, mixed ecosystems |
| 4 | UAT in Given/When/Then (3-7 scenarios) | PASS | 3 scenarios: shared Cargo.toml deps, blast radius, no manifest |
| 5 | AC derived from UAT | PASS | 6 AC items covering scanning, direction, blast radius, graceful handling |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | 2-3 days estimated, 3 scenarios |
| 7 | Technical notes: constraints/dependencies | PASS | toml crate for Cargo.toml, serde_json for package.json, path deps |
| 8 | Dependencies resolved or tracked | PASS | toml crate already in Cargo.toml |
| 9 | Outcome KPIs defined | PASS | KPI-04: blast radius visibility; 30 min to seconds |

### DoR Result: PASS

---

## US-07: JSON Coupling Output with Versioned Schema

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | Tomasz needs stable JSON for CI/Grafana; no programmatic output exists |
| 2 | User/persona with specific characteristics | PASS | Tomasz, CI/CD pipelines, Grafana dashboards |
| 3 | 3+ domain examples with real data | PASS | 3 examples: Grafana integration, alert script with jq, schema versioning |
| 4 | UAT in Given/When/Then (3-7 scenarios) | PASS | 3 scenarios: JSON structure, per-pair fields, analyze unchanged |
| 5 | AC derived from UAT | PASS | 6 AC items covering schema fields, version, backward compat |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | 1 day estimated, 3 scenarios |
| 7 | Technical notes: constraints/dependencies | PASS | Schema as API contract, serde_json patterns |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-03/US-04 for coupling data structure |
| 9 | Outcome KPIs defined | PASS | KPI-07: programmatic data access; stable schema |

### DoR Result: PASS

---

## US-08: HTML Coupling Visualization

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | Architecture review needs visual coupling map; terminal output insufficient |
| 2 | User/persona with specific characteristics | PASS | Adriana, quarterly CTO presentations, architecture reviews |
| 3 | 3+ domain examples with real data | PASS | 3 examples: graph generation, matrix cluster pattern, dimension filtering |
| 4 | UAT in Given/When/Then (3-7 scenarios) | PASS | 4 scenarios: HTML generation, graph, matrix, --open flag |
| 5 | AC derived from UAT | PASS | 7 AC items covering tabs, graph, matrix, filtering, self-containment |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | 3-4 days estimated, 4 scenarios |
| 7 | Technical notes: constraints/dependencies | PASS | New renderer module, inline JS graph, follows html.rs pattern |
| 8 | Dependencies resolved or tracked | PASS | Depends on all coupling dimensions (US-03/US-05/US-06) |
| 9 | Outcome KPIs defined | PASS | KPI-06: visualization assembly; 2 hours to 2 minutes |

### DoR Result: PASS

---

## US-09: Dimension Filtering in HTML Visualization

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | All dimensions mixed is noisy; need to isolate for clear analysis |
| 2 | User/persona with specific characteristics | PASS | Adriana (isolate temporal), Yuki (isolate team), Tomasz (isolate deps) |
| 3 | 3+ domain examples with real data | PASS | 3 examples: temporal only, team only (Yuki bridge), dependency only (shared-libs hub) |
| 4 | UAT in Given/When/Then (3-7 scenarios) | PASS | 3 scenarios: toggle off, single dimension, re-enable all |
| 5 | AC derived from UAT | PASS | 5 AC items covering checkboxes, graph/matrix/list updates, instant changes |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | 1-2 days estimated, 3 scenarios |
| 7 | Technical notes: constraints/dependencies | PASS | JS event handlers, CSS visibility toggle, filtered recalculation |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-08 for HTML structure |
| 9 | Outcome KPIs defined | PASS | KPI-06: part of visualization assembly KPI |

### DoR Result: PASS

---

## Summary

| Story | DoR | Release |
|-------|-----|---------|
| US-01 | PASS | Release 1 |
| US-02 | PASS | Release 1 |
| US-03 | PASS | Release 1 |
| US-04 | PASS | Release 1 |
| US-05 | PASS | Release 2 |
| US-06 | PASS | Release 2 |
| US-07 | PASS | Release 2 |
| US-08 | PASS | Release 3 |
| US-09 | PASS | Release 3 |

All 9 stories pass the 9-item DoR checklist. Ready for handoff to DESIGN wave.
