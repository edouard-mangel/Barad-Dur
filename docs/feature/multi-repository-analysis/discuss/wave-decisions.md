# Wave Decisions Summary -- Cross-Repository Coupling Detection DISCUSS wave

## Feature
multi-repository-analysis: detect temporal, team, and dependency coupling between repositories so teams can identify which repos are too tightly bound and take decoupling action.

## Date
2026-03-25

---

## Key Decisions Made in This Wave

### D-01: `coupling` subcommand (not `dashboard --coupling`)

**Decision**: Cross-repo coupling analysis is a new `coupling` subcommand, not a flag on the existing `dashboard` or `analyze` commands.

**Rationale**: Coupling analysis has fundamentally different semantics from single-repo health scoring. It operates on a root directory (not individual repo paths), produces pairwise output (not per-repo scores), and consumes RepoSnapshot data differently (commit timestamps for correlation, not Health/Team/Evolution/Hygiene categories). A separate subcommand avoids overloading `analyze` or the deferred `dashboard` command.

**Alternatives rejected**: `analyze --coupling` (muddies single-repo semantics), `dashboard --coupling` (dashboard feature does not exist yet), `barad-dur compare` (too generic).

---

### D-02: Root directory input (not explicit repo paths)

**Decision**: The coupling subcommand takes a single root directory and scans first-level subdirectories for git repos. It does not accept explicit individual repo paths.

**Rationale**: The user's stated workflow is "I have repos under `/home/edouard/WS/` and want to scan them all." A root directory scan eliminates the need to list repos individually. First-level-only scanning is predictable and avoids recursion complexity.

**Trade-off**: Users with repos in non-standard layouts (nested subdirectories) must restructure or use symlinks. This is acceptable because the common workspace layout has repos as direct children.

---

### D-03: Three coupling dimensions (temporal, team, dependency)

**Decision**: The feature detects three coupling dimensions, delivered incrementally: temporal coupling (R1), team coupling (R2), and dependency coupling (R2).

**Rationale**: The user explicitly requested "ALL coupling dimensions: temporal, dependency, team." Each dimension reveals a different type of coupling invisible from the others. Temporal coupling uses existing git log data. Team coupling uses existing author data. Dependency coupling requires new manifest scanning code.

**Trade-off**: R1 delivers only temporal coupling. Users wanting team or dependency coupling must wait for R2. This is acceptable because temporal coupling is the most commonly felt pain (CI breakage correlation).

---

### D-04: Configurable coupling window (default 24h)

**Decision**: The time gap threshold for temporal co-change detection is configurable via `--coupling-window` with a default of 24 hours.

**Rationale**: 24 hours captures most same-day and next-morning correlations without being so wide that unrelated commits are matched. Different organizations have different commit rhythms (a startup might need 8h, a large enterprise might need 48h).

**Alternatives rejected**: Fixed window (not configurable -- too rigid), per-pair window (too complex for first release).

---

### D-05: Coupling score uses min(commits) as denominator

**Decision**: Temporal coupling score = co_changes / min(commits_A, commits_B) * 100.

**Rationale**: Using the minimum of the two repos' commit counts avoids penalizing repos with very different activity levels. If repo A has 10 commits and repo B has 100, and all 10 of A's commits coincide with B commits, the coupling score should be high (100%), not diluted (10%). The minimum ensures the less-active repo's commit frequency is the reference.

**Trade-off**: A very active repo that coincidentally has commits near a less-active repo's few commits could show high coupling. The confidence indicator (co-change count) mitigates this by showing sample size.

---

### D-06: Author matching by display name (not email)

**Decision**: Team coupling matches authors by lowercase display name, not email address.

**Rationale**: Developers frequently use different email addresses across repos (work vs personal, company email changes). The git display name is more stable. The four forces analysis identified "author email mismatch" as a key anxiety. Name-based matching addresses this directly.

**Trade-off**: Name collisions (two different "David Kim" across repos) could produce false positives. A future `.coupling-mailmap` config can override matches. For most organizations, display name collisions are rare.

---

### D-07: Both CLI and HTML output (not one or the other)

**Decision**: The feature supports both CLI output (ranked coupling pairs) and HTML output (interactive graph + matrix). CLI is default (R1); HTML is R3.

**Rationale**: The user explicitly requested "BOTH CLI (ranked coupling pairs) and HTML (interactive graph/matrix)." CLI serves the day-to-day investigation use case. HTML serves the architecture review and CTO presentation use case.

**Trade-off**: HTML requires a new renderer with graph visualization, which is the most complex piece. Deferring to R3 allows R1 and R2 to ship without the visualization overhead.

---

### D-08: Three-release delivery strategy

**Decision**: The feature is split into 3 releases:
- Release 1: Temporal coupling detection + CLI output
- Release 2: Team coupling + dependency coupling + JSON output
- Release 3: HTML visualization with interactive graph + matrix

**Rationale**: Elephant Carpaccio principle. Each release delivers a verifiable user outcome. Release 1 alone answers "which repos are temporally coupled?" Release 2 adds dimensions and programmatic output. Release 3 adds presentation quality.

**Trade-off**: Users wanting the full coupling picture (all 3 dimensions + visualization) must wait for all 3 releases. Each release is independently valuable.

---

## Stories Produced

| Story | Release | MoSCoW | JTBD Trace |
|-------|---------|--------|------------|
| US-01: Coupling subcommand discovers repos | Release 1 | Must Have | JS-01 |
| US-02: Snapshot collection with progress | Release 1 | Must Have | JS-01 |
| US-03: Temporal coupling analysis | Release 1 | Must Have | JS-01 |
| US-04: CLI output with ranked pairs | Release 1 | Must Have | JS-01 |
| US-05: Team coupling (shared authors) | Release 2 | Should Have | JS-02 |
| US-06: Dependency coupling (manifests) | Release 2 | Should Have | JS-03 |
| US-07: JSON coupling output | Release 2 | Should Have | JS-01, JS-03 |
| US-08: HTML coupling visualization | Release 3 | Could Have | JS-04 |
| US-09: Dimension filtering in HTML | Release 3 | Could Have | JS-04 |

---

## Open Questions for DESIGN Wave

1. **Coupling window granularity**: Should the coupling window support sub-hour granularity (e.g., `--coupling-window 4h`)? Or is hour-level sufficient? The DESIGN wave should define the parsing format.

2. **Parallel pair analysis**: Should pairwise temporal coupling computation be parallelized (rayon)? 50 repos = 1225 pairs. Each pair comparison is O(N log N). The DESIGN wave should benchmark and decide if parallelism is needed for R1.

3. **Cross-repo file-level coupling**: The current design detects coupling at the REPO level. Should there be an option to drill down to file-level coupling across repos (which files in repo A change when files in repo B change)? This is N^2 at file level and may be prohibitive. The DESIGN wave should assess feasibility.

4. **Graph library for HTML**: The HTML visualization needs a force-directed graph. Options: (a) inline D3.js (large but capable), (b) minimal custom JS (smaller but limited), (c) SVG-only static layout. The DESIGN wave should choose based on the self-containment constraint.

5. **GitLab group scan**: The user mentioned GitLab group scanning as a future input source. Should the coupling subcommand design accommodate `barad-dur coupling --gitlab-group acme/platform` as a future extension? The DESIGN wave should plan for this in the CLI args structure.

6. **Combined coupling score formula**: R2 introduces three dimensions. How should the combined score be computed? Options: weighted average (temporal 50%, team 25%, dependency 25%), maximum of all dimensions, or geometric mean. The DESIGN wave should decide based on which formula best reflects "overall coupling risk."

---

## Handoff Package

Files produced in this wave:

| File | Purpose |
|------|---------|
| `jtbd-job-stories.md` | 4 job stories: temporal, team, dependency, visualization |
| `jtbd-four-forces.md` | Forces analysis per job with design implications |
| `jtbd-opportunity-scores.md` | 18 outcomes scored; opportunity landscape; tier prioritization |
| `journey-multi-repo-visual.md` | ASCII mockups for all 5 journey steps + error paths + HTML concept |
| `journey-multi-repo.yaml` | Structured journey schema with integration points and constraints |
| `journey-multi-repo.feature` | Gherkin scenarios covering happy paths, errors, and NFRs |
| `shared-artifacts-registry.md` | 11 shared artifacts with sources, consumers, and integration risks |
| `story-map.md` | Backbone, walking skeleton, 3 release slices, scope assessment |
| `prioritization.md` | MoSCoW classification, priority order, risk-based ordering |
| `requirements.md` | 13 FRs, 6 NFRs, 5 business rules, out of scope, dependencies |
| `user-stories.md` | 9 LeanUX stories with full template (problem/examples/UAT/AC/KPIs) |
| `acceptance-criteria.md` | Consolidated AC across all stories + cross-cutting criteria |
| `outcome-kpis.md` | 7 KPIs with measurement plan and hypotheses |
| `dor-checklist.md` | DoR validation for all 9 stories -- all PASSED |
| `wave-decisions.md` | This file -- 8 key decisions and 6 open questions |

### Handoff to DESIGN wave (solution-architect)

All 9 stories are DoR PASSED. The DESIGN wave should:

1. Begin with Release 1 walking skeleton (US-01 + US-02 + US-03 + US-04)
2. Address the 6 open questions above before finalizing architecture
3. Reference the existing intra-repo coupling implementation (`file_change_pairs` in `snapshot.rs`, `temporal_coupling()` in `health.rs`) as a conceptual starting point for the cross-repo algorithm
4. Follow the functional Rust paradigm: pure functions, iterator chains, `(snapshot) -> value` pattern
5. Note that RepoSnapshot already contains `commits` (with `committed_date`), `authors`, `commits_by_author`, and `commits_by_file` -- all needed for coupling analysis

### Handoff to acceptance-designer (DISTILL wave)

Journey schema (`journey-multi-repo.yaml`), Gherkin scenarios (`journey-multi-repo.feature`), integration points from shared artifacts registry, and outcome KPIs are ready for test scenario refinement.

---

## Changed Assumptions

This section documents the scope pivot from the original "portfolio health dashboard" framing to the current "cross-repository coupling detection" focus.

### Original Assumption (pre-pivot)
The feature was framed as a **multi-repository portfolio health dashboard**: run `barad-dur dashboard <paths>` to see ranked health scores (Health/Team/Evolution/Hygiene) across all repos. The focus was on aggregating existing per-repo analysis into a combined view.

### Pivot Trigger
The user clarified: "I need to detect wrong coupling between different repositories, so I can detect which repositories are too coupled." The real need is coupling detection, not health aggregation.

### What Changed

| Aspect | Before (Dashboard) | After (Coupling Detection) |
|--------|-------------------|---------------------------|
| Primary job | Rank repos by health score | Detect coupling between repo pairs |
| Subcommand | `barad-dur dashboard <paths>` | `barad-dur coupling <root-dir>` |
| Input | Explicit repo paths or glob | Root directory (scan for repos) |
| Output unit | Per-repo score | Per-pair coupling score |
| Dimensions | Health, Team, Evolution, Hygiene | Temporal, Team, Dependency |
| CLI output | Ranked table of repos | Ranked table of coupling pairs |
| HTML output | Portfolio dashboard with heatmap | Coupling graph + matrix |
| Underlying data | Existing AnalysisReport per repo | Commit timestamps, authors, manifests across repos |
| Personas | Same 3 personas | Same 3 personas, reframed around coupling pain |
| Release count | 3 releases (CLI, JSON+config, HTML) | 3 releases (temporal CLI, team+deps+JSON, HTML viz) |

### What Stayed the Same
- Functional Rust paradigm (pure functions, iterator chains)
- Reuse of existing Collector -> Snapshot pipeline for git data collection
- Skip-on-error resilience pattern
- Progress bar pattern for long-running analysis
- Self-contained HTML output pattern
- 3-release incremental delivery strategy
- Same 3 personas (Adriana, Tomasz, Yuki) with reframed motivations

### Impact on Deferred Features
The original "portfolio health dashboard" is NOT cancelled -- it is deferred. It remains a valid future feature that can be built independently of coupling detection. The coupling subcommand and a future dashboard subcommand can coexist.
