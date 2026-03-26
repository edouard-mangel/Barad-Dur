# Prioritization -- Cross-Repository Coupling Detection

## MoSCoW Classification

### Must Have (Walking Skeleton + Release 1)

| Story | Rationale | JTBD Trace | Effort |
|-------|-----------|------------|--------|
| US-01: Coupling subcommand discovers repos | Core entry point; scans root dir for git repos | JS-01 | 1-2 days |
| US-02: Snapshot collection with progress | Data acquisition; reuses existing Collector pipeline | JS-01 | 1-2 days |
| US-03: Temporal coupling analysis | Core coupling algorithm; correlates commit timestamps across repo pairs | JS-01 | 2-3 days |
| US-04: CLI output with ranked pairs | Core value delivery; ranked coupling pairs answer "which repos are too coupled?" | JS-01 | 1 day |

### Should Have (Release 2)

| Story | Rationale | JTBD Trace | Effort |
|-------|-----------|------------|--------|
| US-05: Team coupling (shared authors) | Surfaces knowledge bottlenecks; leverages same git data | JS-02 | 2 days |
| US-06: Dependency coupling (manifest scanning) | Maps blast radius for shared library changes | JS-03 | 2-3 days |
| US-07: JSON coupling output | Enables CI integration; stable schema for downstream consumers | JS-01, JS-03 | 1 day |

### Could Have (Release 3)

| Story | Rationale | JTBD Trace | Effort |
|-------|-----------|------------|--------|
| US-08: HTML coupling visualization | Interactive graph + matrix for architecture reviews | JS-04 | 3-4 days |
| US-09: Dimension filtering in HTML | Toggle temporal/team/dependency edges independently | JS-04 | 1-2 days |

### Will Not Have (deferred)

| Idea | Rationale for deferral |
|------|----------------------|
| GitLab group scan as input source | Future enhancement; local directory covers the immediate need |
| API contract detection (proto files, OpenAPI) | Requires deep parsing; start with manifest files |
| Historical coupling trends | Requires storing coupling snapshots over time; no storage model yet |
| Coupling-based refactoring suggestions | Requires architectural knowledge beyond coupling data |
| Portfolio health dashboard (original broad scope) | Replaced by coupling-focused feature per user clarification |

---

## Priority Order Within Releases

### Release 1 (Must Have)

```
US-01 (discover repos)
  |
  v
US-02 (collect snapshots)
  |
  v
US-03 (temporal coupling analysis)
  |
  v
US-04 (CLI output)
```

Sequential dependency: each story feeds into the next. US-01 produces discovered repos, US-02 collects snapshots, US-03 computes coupling, US-04 renders output.

### Release 2 (Should Have)

```
US-05 (team coupling)  -- depends on US-02 (snapshots)
US-06 (dependency coupling)  -- depends on US-01 (discovered repos)
US-07 (JSON output)  -- depends on US-03/US-04 (coupling pairs struct)
```

US-05 and US-06 are independent of each other and can be developed in parallel. US-07 depends on the coupling pair data structure being stable.

### Release 3 (Could Have)

```
US-08 (HTML visualization)  -- depends on US-03/US-05/US-06 (all coupling dimensions)
US-09 (dimension filtering)  -- depends on US-08
```

---

## Outcome Impact Assessment

| Release | Key Outcome | Impact |
|---------|-------------|--------|
| Release 1 | Adriana can see temporal coupling between repo pairs from one command | HIGH: Replaces weeks of manual CI log correlation |
| Release 2 | Tomasz sees dependency blast radius; Yuki sees she is a single-author bridge | HIGH: Surfaces hidden risks that are currently invisible |
| Release 3 | Adriana has shareable coupling visualization for architecture review | MEDIUM: Presentation quality; political value for decoupling investment |

---

## Risk-Based Ordering

| Risk | Impact | Mitigation | Story |
|------|--------|------------|-------|
| Temporal coupling algorithm produces too many false positives | Users ignore the tool | Configurable window + confidence indicator + significance threshold (US-03) | HIGH |
| Author matching fails across email configs | Team coupling data is unreliable | Name-based normalization + configurable mailmap (US-05) | MEDIUM |
| Manifest scanning misses dependencies | Incomplete blast radius picture | Start with Cargo.toml; document limitations; allow custom patterns (US-06) | MEDIUM |
| HTML graph unreadable at 24+ repos | Users revert to manual diagrams | Threshold filtering + matrix as primary view (US-08) | LOW (deferred to R3) |
| Performance degrades at 50+ repos (1225 pairs) | Tool unusable for large portfolios | Parallel analysis via rayon; efficient pairwise comparison (US-03) | MEDIUM |
