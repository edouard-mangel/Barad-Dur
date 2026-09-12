# Calculation Boundaries and Analysis Time Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Remove metric dependencies on the report orchestrator and make time-dependent calculations explicit and testable.

**Architecture:** Calculation results live with their domains; score-band policy lives in a small dependency-neutral module. Application boundaries capture reference time and pass it into calculations separately from report/history timestamps.

**Tech Stack:** Existing Rust, Chrono, Serde, calculation and integration tests.

**Spec:** [M06 finding](../../reviews/2026-09-07-structure-maintainability.md#m06--clarify-calculationreport-dependencies-and-analysis-time) and the design below.

## Global constraints

- Prose-only planning; no code, probes, execution, commits, or releases.
- Preserve calculation formulas, score thresholds, JSON/bincode fields, and existing public Rust paths through re-exports where necessary.
- Do not reset caches/history or change historical scoring semantics as incidental refactoring.
- One analysis invocation has an explicit reference time; immutable inputs are not mutated to carry it.
- Keep clock access at application boundaries, not hidden in metrics, builders, global state, or injected per-function clock services.
- M02 owns shared orchestration, M04 owns identity, and M01 owns generated wire declarations.

## Design decisions

### Ownership

Place coupling calculation results with coupling, call-graph results with callgraph, and churn results with churn. Move pure score-band definitions to a small scoring module that imports neither report orchestration nor CLI/I/O modules. The scorer composes those results into reports and can re-export the original names during migration.

Do not create a generic shared-types dumping ground, move every report DTO, or rename serialized fields during the extraction. Preserve M01's generation annotations when types move and check the generated output for unintended drift.

### Reference-time policy

For this maintainability refactor, preserve the existing meaning of wall-clock-based calculations. Capture time once at the application boundary and pass that value through. Do not silently reinterpret historical age as age at the selected commit.

| Context | Calculation reference | Separate metadata policy |
|---|---|---|
| Live analyze/gate | Invocation time captured once | Live report/history generation metadata remains explicit |
| Cached snapshot | Current invocation time, not snapshot creation time | Cache acquisition metadata is retained, not reused as a scoring clock |
| Date-filtered live analysis | Invocation time; selected dates still filter evidence | Do not reinterpret an explicit until filter as a new age-scoring rule |
| Backfill/historical collection | Backfill invocation time, preserving current scoring semantics | Historical history points retain their selected commit timestamp |
| Watch | Fresh reference for each analysis iteration | No process-start time reused indefinitely |
| Corpus runs | Existing invocation policy and pinned inputs | Do not change committed corpus date boundaries or baselines silently |
| Fixed-time calculation tests | Explicit supplied timestamp | No wall-clock dependency in the calculation under test |

This is a conservative planning choice, not a claim that current historical semantics are ideal. As-of-commit historical scoring and globally wall-clock-independent corpus baselines are separate behavioral changes, outside this plan. Fixed snapshot/configuration/reference inputs will be reproducible; a later invocation with a different reference is allowed to produce different age values.

## Task 1: Inventory dependencies and clock roles

**Files:** src/metrics/callgraph.rs, src/metrics/churn.rs, src/metrics/coupling/mod.rs, src/metrics/evolution/mod.rs, src/scorer/types.rs, src/scorer/builders/, src/scorer/audit.rs, src/scorer.rs, src/runner.rs, command/backfill/watch and field-test entry points.

- [ ] List every production metric-to-scorer dependency and classify the imported item as calculation result, score policy, or display-only model.
- [ ] List production clock reads separately from tests; classify each as evidence filtering, calculation reference, acquisition metadata, generation metadata, or history-point time.
- [ ] Record current signed durations, rounding, threshold comparisons, future-dated evidence, and unavailable-data behavior.
- [ ] Enumerate public type paths and serialization/generation attributes before moving definitions.

**Acceptance:** Every dependency and clock has an explicit owner and meaning; no broad search result is mistaken for a complete behavioral audit.

## Task 2: Move neutral types and score policy

**Files:** New src/scoring.rs, src/metrics/coupling/types.rs, src/metrics/callgraph/types.rs, src/metrics/churn/types.rs as needed; src/lib.rs, corresponding metric modules, src/scorer/types.rs and consumers.

**Contract:** Metrics own their calculation outputs; scoring owns pure band thresholds/classification; the report layer consumes them without being imported back by metrics.

- [ ] Move only the identified calculation-owned types and score policy, preserving fields, names, attributes, and default threshold values.
- [ ] Re-export original public Rust names where needed so an ownership change does not force unrelated external API breakage.
- [ ] Update internal imports to the true owners rather than routing metrics through compatibility re-exports in scorer.
- [ ] Verify unchanged serialization and M01-generated declarations; update generator import locations only if the exporter requires it.
- [ ] Keep display-only report structures in scorer and avoid a universal types module.

**Acceptance:** Production metric calculations no longer import scorer; public-path compatibility and serialized shapes remain intact.

## Task 3: Make calculations accept reference time

**Files:** src/metrics/evolution/mod.rs and tests.rs, src/scorer/builders/files.rs, authors.rs, src/scorer/audit.rs, relevant composition functions.

**Contract:** Age/activity calculations receive a concrete immutable timestamp alongside their existing inputs and return the same kind of result without reading the clock.

- [ ] Replace internal current-time reads in code age, file age, author activity, and dead-file calculations with explicit inputs.
- [ ] Preserve duration units, signed values, rounding, inclusivity, and missing-data handling.
- [ ] Keep timestamps used purely as output metadata separate from calculation inputs.
- [ ] Add fixed-time tests immediately before/at/after each relevant threshold, around UTC day boundaries, and for future-dated evidence.

**Acceptance:** Repeating a calculation with the same snapshot, configuration, and reference time yields the same output; test fixtures need not sleep or alter a global clock.

## Task 4: Thread time from application boundaries

**Files:** src/runner.rs, src/cmd/analyze.rs, src/cmd/gate.rs, src/cmd/watch.rs, src/backfill/mod.rs, src/scorer.rs, M02 analysis composition if present.

- [ ] Capture reference time once per invocation and pass it through metrics and report enrichment.
- [ ] Pass explicit generation/history metadata to history construction; retain backfill's selected-commit history timestamp without feeding it into age scoring.
- [ ] Ensure cache hits and misses use the same invocation reference, not different snapshot creation timestamps.
- [ ] Capture fresh time per watch iteration and one consistent time for a backfill invocation.
- [ ] Preserve date-filtering semantics and collection modes; do not add a public time-override flag as incidental plumbing.

**Acceptance:** One analysis cannot use different clocks for its score, file ages, author activity, and audit; historical point placement remains unchanged.

## Task 5: Verify boundaries, parity, and limitations

- [ ] Sweep all production clock reads and metric-to-scorer imports, documenting legitimate boundary/metadata reads that remain.
- [ ] Test same-reference parity for live/cached paths and shared calculations, plus distinct references intentionally changing age results.
- [ ] Verify full reports and history metadata, not only category scores; pay attention to previously inconsistent near-boundary timestamps.
- [ ] During implementation, run full Rust tests, formatting/Clippy, feature-gated generation checks, relevant frontend/HTML tests, and P1/P2 gates.
- [ ] Investigate any corpus movement and document it; do not automatically accept baselines or alter the history schema to conceal differences.
- [ ] Document the preserved historical semantics and the separate future option of as-of-commit scoring.

**Completion:** Dependencies point from report composition toward calculations, and time is visible in calculation inputs without an unapproved scoring-policy migration.

## Sequencing and evidence

Complete the neutral-type and reference-time slices before M02. Its shared analysis entry point then reuses the established owners and time inputs instead of moving types or defining clock policy again. M06 initially threads time through existing command paths; M02 subsequently consolidates that composition.

The review identified current imports and clock reads. No fixed-time parity, public-path compile check, serializer comparison, or corpus result has been produced by this planning-only work. Those remain future checks under the [review process](../../review-process.md).
