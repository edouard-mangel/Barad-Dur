# "Your Code as a Crime Scene" — chapter notes vs. barad-dûr

Reading notes on Adam Tornhill's *Your Code as a Crime Scene* (2015, Pragmatic
Bookshelf), chapter by chapter, cross-referenced against what this repo
actually implements today. Verdicts are based on reading the source in
`src/`, not on the CLAUDE.md description — file/function references below
are the evidence.

Legend: ✅ Implemented · 🟡 Partial · ⬜ Not implemented

---

## Part 1 — Evolving Software

### Chapter 1: Welcome!
Framing chapter. Tornhill's thesis: most of a system's lifetime cost is
*understanding* existing code, not writing new code, so tooling should mine
the evolutionary history in version control (who changed what, how often,
together with what) rather than judge a single static snapshot. States the
three-part structure the book follows: find offending code (hotspots),
evaluate architecture against how it's actually modified (temporal
coupling), and understand the social/organizational forces that shape the
code.

**Verdict:** ✅ This is the mission statement barad-dûr already embodies —
the whole `Collector → RepoSnapshot → Metrics → Scorer` pipeline is built
on mining git history rather than static analysis alone.

### Chapter 2: Code as a Crime Scene
Introduces the **hotspot**: the overlap between *complexity* and *effort*
(change frequency), by analogy to geographical offender profiling (a
criminal's crime-scene locations cluster around a probable home base).
Argues complexity metrics alone are useless without knowing which code
people actually have to touch.

**Verdict:** ✅ `src/metrics/health/complex_hotspots.rs::complex_hotspots` —
files in the top quartile of *both* cyclomatic complexity and commit count,
restricted to production source (`file_role.rs`). Direct implementation of
the core hotspot formula.

### Chapter 3: Creating an Offender Profile
Walks through the mechanics with Code Maat: `git log --numstat` → revision
counts per file (effort) → `cloc`-style LOC (complexity proxy) → merge the
two, sort by revisions then size. Flags the method's limits (what counts as
"hot" is relative, no absolute threshold; commit-style bias).

**Verdict:** ✅ Same pipeline, more rigorous: `collector/gitcli.rs` +
`collector/libgit.rs` gather commits/blame, `metrics/complexity/*`
(tree-sitter AST, not raw LOC) computes real cyclomatic complexity per
file/function, and `complex_hotspots.rs` does the percentile-based
merge — barad-dûr uses statistical percentiles (p75) rather than the
book's plain top-N sort, which directly addresses the "what counts as hot"
limitation the book flags but doesn't solve.

### Chapter 4: Analyze Hotspots in Large-Scale Systems
Scaling hotspot analysis to big codebases needs visualization — the book
uses D3.js circle-packing ("enclosure diagrams") sized by LOC and colored
by change intensity, and discusses reading clusters of nearby hotspots as
signs of low cohesion.

**Verdict:** ✅ `renderer/templates/treemap_layout.js` +
`treemap_ui.js` — a squarified treemap (not circle-packing, but the same
size-by-complexity / color-by-churn idea) rendered in the HTML report's
hotspots tab, with per-file complexity/churn tooltips
(`treemap_ui.js:217-218`). No cluster/cohesion analysis across
neighboring hotspots, though.

### Chapter 5: Judge Hotspots with the Power of Names
A heuristic for triaging hotspots before deep investigation: judge by
module name plus size (does "Configuration.java" at 2,600 LOC still sound
like a config file?). Warns about availability bias skewing which hotspot
gets blamed.

**Verdict:** 🟡 The `god_objects` metric's `god_reason` function
(`src/metrics/health/god_objects.rs:22-52`) reports *why* a file was
flagged (LOC, method count, structural-hub degree) — functionally similar
triage info — but there's no name-based heuristic; the report doesn't flag
"suspiciously generic name + large size" as its own signal.

### Chapter 6: Calculate Complexity Trends from Your Code's Shape
Uses indentation depth as a cheap, language-neutral complexity proxy, and
tracks it across a file's revision history to see whether a hotspot's
complexity is trending up, down, or flat (Lehman's law of increasing
complexity).

**Verdict:** ⬜ Not implemented. `trend.rs` computes *overall report score*
trend across runs (`compute_trend`, velocity, sparkline —
`src/trend.rs:74`), and `backfill/` samples historical commits to build
that series, but there is no per-file/per-hotspot complexity-over-time
series. `max_nesting_depth`/`nesting_variance` exist as single-snapshot
metrics (`biomarkers.rs`) but aren't tracked historically per file.

---

## Part 2 — Dissect Your Architecture

### Chapter 7: Treat Your Code As a Cooperative Witness
Introduces **temporal coupling** (a.k.a. change/logical coupling): files
that change together in commits despite having no explicit code dependency.
Uses eyewitness-memory bias as the framing device for why we need this
objective data instead of trusting intuition about the design.

**Verdict:** ✅ `src/metrics/coupling/mod.rs::change_coupling_smells` +
`corroboration_degree` — cross-component co-change pairs above a
configurable ratio threshold (`qualifying_smell_pairs`), reported as
"change coupling smells" and used to corroborate static Pressman coupling
findings.

### Chapter 8: Detect Architectural Decay
Formalizes the analysis: **sum of coupling** to find architecturally
significant modules, then coupling-degree (percent of shared commits)
between pairs, then trend analysis of a module's coupling over multiple
time windows to catch a hub module accumulating responsibilities
("architectural decay").

**Verdict:** 🟡 Cross-boundary co-change detection exists
(`change_coupling_smells`, community-detection cross-check in
`coupling/community.rs`), and multi-repo temporal coupling exists
(`src/coupling/temporal.rs::analyze_temporal_coupling`, with same-author
weighting and a statistical baseline the book doesn't have). What's
missing is the book's core move for *this* chapter: tracking one file/
module's coupling-partner count growing over successive time windows to
catch decay in progress — barad-dûr's coupling metrics are single-snapshot,
not longitudinal per-file trends (same gap as Chapter 6, at the
architecture level instead of the complexity level).

### Chapter 9: Build a Safety Net for Your Architecture
Applies temporal coupling to the code/test boundary: define transformations
mapping physical directories to logical "Code" and "Test" components,
measure their coupling degree, and track code-growth vs. test-growth ratio
over iterations to catch a runaway "automated-test death march."

**Verdict:** ⬜ Not implemented as a standalone code/test coupling or
growth-ratio metric. `file_role.rs` classifies files into
Source/Test/Config/Docs roles and most metrics filter tests out
(`is_source_file`), but nothing measures the *coupling between* the Source
and Test partitions, nor tracks their relative growth trend over time.

### Chapter 10: Use Beauty as a Guiding Principle
Generalizes Ch. 8–9's technique to arbitrary architectural boundaries: pick
a pattern (Pipes-and-Filters, layered MVC, microservices), define a
transformation mapping files to logical components, then look for
temporal coupling that violates the pattern's promise (a Views layer that's
supposed to be swappable but co-changes with Models 75% of the time).
Frames "beauty" (consistency, no surprises) as the underlying design
value.

**Verdict:** ⬜ Not implemented. There's no user-defined architectural
layer/component mapping and no "coupling that violates this specific
pattern" check. `coupling/community.rs`'s Louvain-style community
detection over the import graph is the closest analog — it discovers
structural clusters automatically rather than checking them against a
declared architecture — but it's used only as corroborating evidence for
change-coupling smells, not as an architecture-conformance check.

---

## Part 3 — Master the Social Aspects of Code

### Chapter 11: Norms, Groups, and False Serial Killers
Social-psychology chapter (pluralistic ignorance, groupthink, the Thomas
Quick case) plus a soft technique: mining commit-message word clouds to
surface a team's real modus operandi (are we mostly "fixing," "adding," or
firefighting?).

**Verdict:** 🟡 `src/metrics/hygiene.rs::firefighting_ratio` and
`commit_message_quality` classify commit messages by reactive-vs-planned
keywords and quality heuristics — same underlying idea (mine commit
messages for team behavior signal) — but no word-cloud/frequency
visualization of commit-message vocabulary exists.

### Chapter 12: Discover Organizational Metrics in Your Codebase
Windows Vista research: number-of-authors-per-module predicts defects
better than any code metric. Introduces **main developer** (most added
lines) and **temporal coupling across organizational boundaries**
(commits grouped by day instead of by exact commit, since different
authors commit independently) to reason about cross-team communication
cost via Conway's law.

**Verdict:** ✅ `src/metrics/health/bus_factor.rs` and
`churn_ownership.rs` both compute single-author-dominance and flag
high-churn single-owner files — the "many/one authors touching a hotspot"
signal is there — and `src/metrics/team/mod.rs::knowledge_distribution`
computes a Gini coefficient of ownership. Closed 2026-08-19 by the
Cross-team coupling Team metric (day-bucketed pairs × primary-author
mismatch); explicit team-mapping config remains deferred future work.

### Chapter 13: Build a Knowledge Map of Your System
Visualizes the **main developer per module** as a color-coded map over the
whole codebase (reusing the Ch. 4 enclosure-diagram layout) — lets you find
who to ask about a given area, and separately visualizes **knowledge loss**
by recoloring only an ex-developer's code to show the blind spot left
behind.

**Verdict:** ✅ `renderer/templates/ownership.js` renders exactly this: a
per-file author-ownership visualization (percentage of lines by
contributor, colored per author, `ownership.js:22-29`), driven by
`snapshot.blame_map` via `author_line_counts`. No dedicated "ex-developer
knowledge loss" view, but the underlying ownership-by-author data needed
to build one is already computed.

### Chapter 14: Dive Deeper with Code Churn
Absolute code churn (lines added/deleted per commit/day) as a process
signal: a steady trickle is healthy, a two-week spike/silence cycle reveals
crunch-driven branch merges, and a rising-churn-near-deadline pattern
predicts a missed deadline. Also uses churn to prioritize among several
temporally-coupled modules (which one actually grew?) and as an alternative
hotspot metric to raw revision counts.

**Verdict:** 🟡 `metrics/evolution/mod.rs::growth_trend` computes net
files/lines added-deleted within the analysis window and
`commit_cadence` computes a coefficient-of-variation "regularity" score —
same churn-pattern spirit — but there's no day-bucketed churn timeline
chart (only `scorer/builders/hotspots.rs::churn_timeline`, which buckets
*commit counts* per file into 12 slices for the hotspot sparkline, not
lines added/deleted) and no explicit "which coupled module actually grew"
prioritization step.

### Chapter 15: Toward the Future
Forward-looking chapter: analyzing non-code artifacts under version control,
Michael Feathers's method-level temporal coupling for finding
Single-Responsibility violations inside a class, developer-network graphs
(who works with whom, colored by team, à la Conway's law), and a wishlist
for tools that go beyond commit-granularity data (in-editor real-time
warnings, "programmers who touched this also touched X").

**Verdict:** ⬜ Not implemented / out of scope. No non-code artifact
analysis, no method-level (sub-file) temporal coupling, no developer
social-graph visualization (the multi-repo `coupling/team.rs` computes
*shared-author* coupling between repos, which is adjacent but operates at
repo granularity, not a per-developer graph), and (by design, given the
CLI/CI batch model) no live in-editor integration.

### Appendix 1: Refactoring Hotspots
A refactoring heuristic for once you've found a hotspot: group its
methods/functions by what they actually do (not alphabetically), let
better names emerge from "wishful thinking" (write the ideal call first,
then make it real), and treat naming as the highest-leverage refactoring
tool.

**Verdict:** 🟡 `scorer/actions.rs::generate_top_actions` +
the `CONTENT_ADVICE`/`COMMON_ADVICE`/`CONTROL_ADVICE`/`INHERITANCE_ADVICE`
constants give generic refactoring guidance per Pressman coupling kind
("split it into two intent-revealing functions", "favor composition over
inheritance") — actionable next steps, in the book's spirit — but nothing
does the book's specific method-grouping-by-task analysis within a
flagged hotspot file.

---

## Gap summary

| Ch. | Topic | Status | What's missing |
|----|-------|--------|-----------------|
| 1 | Philosophy / mission | ✅ | — |
| 2 | Hotspots (complexity × effort) | ✅ | — |
| 3 | Hotspot mining pipeline | ✅ | — |
| 4 | Large-scale hotspot visualization | ✅ | Cluster/cohesion reading of neighboring hotspots |
| 5 | Judge hotspots by name | 🟡 | No name-based triage heuristic (only size/degree reasons) |
| 6 | Per-file complexity trend | ⬜ | No historical complexity-over-time series per file/hotspot |
| 7 | Temporal coupling | ✅ | — |
| 8 | Architectural decay (coupling trend) | 🟡 | No per-module coupling-degree trend across time windows |
| 9 | Code/test coupling safety net | ⬜ | No Source↔Test coupling metric or growth-ratio trend |
| 10 | Architecture-pattern conformance | ⬜ | No user-declared component boundaries / pattern-violation check |
| 11 | Commit-message social signal | 🟡 | Firefighting/quality heuristics exist; no word-cloud/vocabulary view |
| 12 | Organizational metrics (Conway) | ✅ | team-mapping config deferred |
| 13 | Knowledge map | ✅ | No dedicated "ex-developer knowledge loss" view |
| 14 | Code churn deep dive | 🟡 | No lines-added/deleted churn timeline; no churn-based coupling prioritization |
| 15 | Future directions | ⬜ | Non-code artifacts, method-level coupling, developer social graph — none in scope |
| A1 | Refactoring hotspots by name grouping | 🟡 | Generic advice exists; no method-grouping-by-task analysis |

**Biggest structural gap:** the book leans heavily on *trend analysis over
multiple time windows* — per-file complexity trend (Ch. 6), per-module
coupling-degree trend (Ch. 8), code/test growth-ratio trend (Ch. 9), churn
pattern shape (Ch. 14) — and barad-dûr's `trend.rs`/`backfill/` machinery
only tracks the *aggregate report score* across historical commits, not
any of these metrics at per-file or per-pair granularity. The backfill
sampling infrastructure (`src/backfill/sampling.rs`) already walks
historical commits, so extending it to also snapshot a specific file's
complexity or a specific pair's coupling degree at each sample point looks
like the natural next step — it would unlock Ch. 6, 8, and 9's core
techniques without new collection infrastructure.
