# Analysis Orchestration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Share analysis calculations across analyze, gate, and backfill while retaining their intentional policy differences.

**Architecture:** Commands own I/O and collection policy. A pure analysis layer combines explicit selection, configuration, snapshot evidence, and reference time into reusable facts and score summaries. Full report enrichment is a separate consumer.

**Tech Stack:** Existing Rust modules and test tooling; no new runtime framework.

**Spec:** [M02 finding](../../reviews/2026-09-07-structure-maintainability.md#m02--share-analysis-orchestration-and-derived-findings) and the design below.

## Global constraints

- Implementation began 2026-09-12 on `refactor/analysis-orchestration`; tasks 1–4 are on that branch.
- Preserve scores, category order, advice, gate exit behavior, dependency opt-in, history records, and collection modes.
- Do not change report/snapshot serialization or add expensive collection to lightweight backfill.
- Use pure calculations and immutable inputs, not global memoization or a flag-heavy shared command runner.
- M06 owns neutral calculation types and time policy. M04 owns stable IDs. Coordinate their boundaries without implementing either migration twice.

## Design decision and ownership

Plan a shared analysis result containing ordered categories, weighted scores, and reusable evidence requested by its consumers. Report enrichment consumes this result; history and gate need only summaries and relevant evidence.

This is a planning recommendation, not a new product requirement from the user. A smaller category-helper extraction leaves duplicate evidence and display-only work. A universal command runner would mix collection, network, output, and persistence policy.

| Area | Planned responsibility |
|---|---|
| New src/analysis/mod.rs, selection.rs, result.rs | Pure composition, CLI-independent selection, application-level results |
| New src/metrics/coupling/evidence.rs | Shared coupling-domain facts, not presentation advice |
| src/scorer.rs and src/scorer/builders/ | Full display report composition |
| src/cmd/analyze.rs | CLI selection, optional dependency acquisition, remote enrichment, rendering, history persistence |
| src/cmd/gate.rs | Existing five-category policy, trend/ratchet decisions, baseline collection, exit status |
| src/backfill/mod.rs | Existing four-category policy, sampling/deduplication, lightweight collection, dated history persistence |

Evidence belongs to one snapshot and threshold set. Share identical derivations of god-object flags, coupling reach, enabled findings, and corroboration. Do not merge calculations that differ semantically or compute every possible fact for every command.

## Task 1: Characterize command policies

**Files:** Existing command/backfill tests; new tests/analysis_orchestration_walking_skeleton.rs; tests/common/mod.rs if shared setup is needed.

- [x] Record category selection/order for default analyze, each filter, gate, and backfill.
- [x] Cover configured weights, dependency opt-in with available/unavailable evidence, null scores, empty repositories, and missing AST/blame evidence.
- [x] Record history fields, optional counts, timestamps, branch/source markers, and deduplication.
- [x] Cover score, decline, and ratchet gates independently and together, including errors and exit codes.

**Acceptance:** Tests identify intentional differences rather than assuming all commands should produce identical reports.

## Task 2: Introduce selection and score summaries

**Files:** New src/analysis/ files; src/lib.rs, src/cmd/analyze.rs, src/cmd/gate.rs, src/backfill/mod.rs, src/scorer/actions.rs.

**Contract:** Calculation consumes a snapshot, explicit category selection, effective thresholds/weights, already-acquired dependency evidence, and reference time. It returns ordered categories and a weighted summary without I/O.

- [x] Translate CLI flags at command boundaries; remove AnalyzeArgs from reusable metric-selection logic.
- [x] Reuse one calculation/weighting policy while retaining each command's selected set.
- [x] Use M06-owned primitives rather than report DTOs in calculation interfaces.
- [x] Preserve unavailable evidence, unscored results, and measured zero as distinct states.

**Acceptance:** Equivalent calculation inputs yield equivalent results regardless of the calling command; the pure layer imports no CLI arguments.

## Task 3: Share coupling evidence

**Files:** src/metrics/coupling/mod.rs, new evidence.rs, src/scorer/actions.rs, src/scorer/builders/hotspots.rs, src/cmd/gate.rs.

**Contract:** Immutable evidence carries enabled findings and shared derivations for one snapshot/configuration, preserving unavailable-versus-zero distinctions.

- [x] Inventory duplicate barrel, inheritance, corroboration, and reach derivations; consolidate only identical inputs and rules.
- [x] Reuse enabled finding sets for counts and ratchets, including the barrel-bypass toggle.
- [x] Leave advice wording and display shaping in their respective consumers.
- [x] Preserve ordering, deduplication, evidence text, and thresholds; test toggles and missing AST explicitly.
- [x] Keep gate's AST-enabled baseline collection separate from backfill's no-AST/no-blame path.

**Acceptance:** Consumers cannot independently disagree about which rules count as enabled; unrequested report features do not add work to backfill.

## Task 4: Separate enrichment from history and gates

**Files:** src/scorer.rs, src/scorer/builders/, command modules, src/backfill/mod.rs, relevant history/trend tests.

- [x] Make full report composition consume the shared result without changing serialized output.
- [x] Build history from summaries and explicit metadata, retaining every existing field without constructing discarded ownership, age, audit, and other display sections.
- [x] Make gate checks consume only their required summaries/evidence while retaining user-visible text and exit behavior.
- [x] Migrate callers incrementally; remove old orchestration once each path has parity coverage.

**Acceptance:** Analyze produces equivalent reports, gate makes equivalent decisions, and backfill preserves records and collection policy. Category-filtered reports do not gain advice for omitted categories.

## Task 5: Verify parity and dependency boundaries

- [x] Sweep old calculation/report-builder callers, including wrappers such as watch where applicable.
- [x] Verify the pure layer performs no collection, networking, persistence, direct clock reads, or cross-run mutable caching.
- [x] During implementation, run full Rust suites, formatting/Clippy, relevant report checks, and P1/P2 gates.
- [x] Compare recommendation/evidence surfaces and complete history records, not only overall scores. Benchmark before claiming speed improvements.

**Completion:** Multiple command policies share one calculation policy; evidence has explicit ownership; history and gate no longer require full display reports. Evidence: [implementation record](../../reviews/2026-09-12-analysis-orchestration.md).

## Dependencies and verification limits

Complete M06's neutral-type and explicit-reference-time slices before this extraction, then reuse those boundaries. M03 can proceed separately because snapshot shape remains stable. M04 later replaces internal string selection with stable identities.

Existing source inspection establishes duplicate callers, not measured performance or implementation parity. Proposed interfaces and behavior tests remain future execution checks under the [review process](../../review-process.md).
