# Outcome KPIs -- Cross-Repository Coupling Detection

## Purpose

Each KPI defines a measurable behavior change that validates the feature is solving the jobs it was designed for. KPIs are tied to specific job stories and measured through testing or user feedback.

---

## KPI-01: Coupling Identification Speed

**Hypothesis**: If we provide automated temporal coupling detection across repos, then users will identify coupled repo pairs in minutes instead of days.

| Dimension | Value |
|-----------|-------|
| **Who** | Engineering leaders investigating cross-repo coupling (Adriana persona) |
| **Does what** | Identify temporally coupled repo pairs from a single command |
| **By how much** | From days (manual CI log correlation) to under 5 minutes (single invocation) |
| **Measured by** | Integration test: coupling scores computed for known test repos with planted co-changes |
| **Baseline** | Manual correlation of CI failure logs, Slack conversations, Jira tickets -- days of effort |
| **Target** | Top coupling pair identified in under 5 seconds (read first row of output) |
| **JTBD trace** | JS-01 (detect temporal coupling) |

---

## KPI-02: Coupling Detection Accuracy

**Hypothesis**: If we use configurable time windows with confidence indicators, then false positive rates stay below 10% and users trust the data.

| Dimension | Value |
|-----------|-------|
| **Who** | All coupling analysis users |
| **Does what** | Trust the coupling scores as accurate signals, not noise |
| **By how much** | False positive rate under 10% (pairs flagged as coupled that are not meaningfully related) |
| **Measured by** | Integration test: known-independent repos show coupling below threshold; known-coupled repos show high coupling |
| **Baseline** | No baseline exists (manual correlation is incomplete and unreliable) |
| **Target** | Planted coupling in test repos detected at 90%+ accuracy; independent repos score below 15% |
| **JTBD trace** | JS-01 |

---

## KPI-03: Single-Author Bridge Detection

**Hypothesis**: If we surface single-author bridges automatically, then teams will identify bus factor risks across repo boundaries before they become incidents.

| Dimension | Value |
|-----------|-------|
| **Who** | Team leads managing developer assignments (Adriana, team leads) |
| **Does what** | Identify single-author bridges between repos proactively |
| **By how much** | From "discovered after the person is unavailable" to "visible in coupling report" |
| **Measured by** | Integration test: planted single-author bridge detected and flagged |
| **Baseline** | Single-author bridges are invisible until the person leaves or is unavailable |
| **Target** | All single-author bridges detected with zero false negatives |
| **JTBD trace** | JS-02 (detect team coupling) |

---

## KPI-04: Blast Radius Visibility

**Hypothesis**: If we scan manifests and report dependency coupling, then platform engineers will know the blast radius before making shared library changes.

| Dimension | Value |
|-----------|-------|
| **Who** | Platform engineers managing shared libraries (Tomasz persona) |
| **Does what** | Know how many repos depend on a shared library before updating it |
| **By how much** | From manual grep across repos (30+ minutes) to automatic scan (seconds) |
| **Measured by** | Integration test: planted Cargo.toml path dependency detected with correct consumer count |
| **Baseline** | Manual `grep shared-libs` across repo directories |
| **Target** | All direct dependency consumers detected; blast radius count matches reality |
| **JTBD trace** | JS-03 (detect dependency coupling) |

---

## KPI-05: Collection Resilience

**Hypothesis**: If we implement skip-on-error for repo collection, then one bad repo does not prevent coupling analysis of the rest.

| Dimension | Value |
|-----------|-------|
| **Who** | All coupling analysis users |
| **Does what** | Complete coupling analysis even when some repos fail collection |
| **By how much** | From "one failure kills entire analysis" to "one failure skips one repo" |
| **Measured by** | Integration test: 1 invalid repo among 5 = coupling report with 4 repos (6 pairs) |
| **Baseline** | No multi-repo analysis exists |
| **Target** | Failed repos skipped; valid repos produce complete coupling results |
| **JTBD trace** | JS-01 |

---

## KPI-06: Visualization Assembly Automation

**Hypothesis**: If we provide HTML coupling visualization, then architecture review preparation drops from hours to minutes.

| Dimension | Value |
|-----------|-------|
| **Who** | Engineering leaders preparing architecture reviews (Adriana persona) |
| **Does what** | Generate an interactive coupling visualization from one command |
| **By how much** | From 2+ hours (manual Miro diagrams) to 2 minutes (single command) |
| **Measured by** | HTML file generated with graph, matrix, and filtering capabilities |
| **Baseline** | Manual Miro/Lucidchart diagrams drawn from memory and CI logs |
| **Target** | Complete HTML visualization in one `coupling --html` invocation |
| **JTBD trace** | JS-04 (visualize coupling landscape) |

---

## KPI-07: Programmatic Coupling Data Access

**Hypothesis**: If we provide versioned JSON output for coupling data, then CI/CD pipelines can consume coupling trends without custom parsing.

| Dimension | Value |
|-----------|-------|
| **Who** | DevOps engineers with CI/CD integrations (Tomasz persona) |
| **Does what** | Consume coupling data from a stable JSON schema in CI pipelines |
| **By how much** | Eliminate custom text parsing of CLI output |
| **Measured by** | Schema conformance test: `jq` queries work against coupling.json |
| **Baseline** | No programmatic coupling data output exists |
| **Target** | Stable JSON schema consumable by jq, Grafana, and custom scripts |
| **JTBD trace** | JS-01, JS-03 |

---

## Measurement Plan

| KPI | Release | Measurement Method | Frequency |
|-----|---------|-------------------|-----------|
| KPI-01 | Release 1 | Integration test: coupling score for known-coupled repos | Per CI run |
| KPI-02 | Release 1 | Integration test: accuracy on planted coupling vs independence | Per CI run |
| KPI-03 | Release 2 | Integration test: single-author bridge detection | Per CI run |
| KPI-04 | Release 2 | Integration test: blast radius computation | Per CI run |
| KPI-05 | Release 1 | Integration test: mixed valid/invalid repos | Per CI run |
| KPI-06 | Release 3 | HTML structure validation | Per CI run |
| KPI-07 | Release 2 | Schema conformance test with jq | Per CI run |
