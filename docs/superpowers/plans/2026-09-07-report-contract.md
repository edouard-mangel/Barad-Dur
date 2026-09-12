# Report Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Make current Rust reports load predictably through generated types and a separate runtime decoder.

**Architecture:** Rust owns the wire contract. Committed generated declarations describe it; a decoder constructs the validated projection used by the dashboard. Formatting and thresholds belong to each report.

**Tech Stack:** Rust, Serde, proposed ts-rs 12.0.1, existing TypeScript, React, Vite, Vitest, pnpm, and GitLab CI.

**Spec:** [M01 design](../specs/2026-09-07-report-contract-design.md).

## Global constraints

- Planning only: no implementation code, execution, commits, or releases are authorized.
- User decisions: Rust-generated TypeScript types, separate runtime decoder, and no backward compatibility for older report JSON.
- Preserve the existing Rust JSON representation, valid null scores, and deliberate current field omissions.
- Do not migrate or delete reports, snapshot caches, or trend histories.
- Ordinary Cargo builds require neither Node nor generation; dashboard-only builds use committed declarations without Rust.
- Use report-provided thresholds, never mutable global state or missing-field defaults.

## Design and boundaries

Generate the complete type graph reachable from AnalysisReport. Derive a DashboardReport projection from those declarations and validate every field the dashboard consumes. Decoder success must not assert that unused report sections were validated.

Require current identity/counts, nullable scores, categories/metrics, structured actions, thresholds, hotspot roles and finding counts, coupling pairs/actions, ownership, ages, and nullable remote metadata. Accept unrelated additive fields, including the current dynamic trend envelope. Validate present optional fields instead of converting malformed data to absence.

An older file can load only if its consumed shape already matches the current contract. Do not add producer-version rejection, a legacy adapter, or primitive raw-value conversion. Incompatible input receives a field-specific error and instructions to regenerate JSON with the installed CLI.

## Task 1: Reproducible type generation

**Files:** Cargo.toml, Cargo.lock, src/scorer/types.rs, src/metrics/mod.rs, src/metrics/file_role.rs, src/deps.rs; proposed examples/export_report_types.rs and dashboard/src/report/generated/.

**Contract:** An explicit exporter consumes Rust report types and produces committed declarations. Checking compares complete file sets and contents without rewriting them.

- [ ] Add an opt-in export-types feature and gated exporter; keep ordinary startup/build scripts uninvolved.
- [ ] Annotate reachable serialized types without moving ownership or changing Serde attributes. Distinguish omitted optional fields from required nullable fields.
- [ ] Map existing JSON integers to TypeScript numbers; do not promise arbitrary 64-bit precision.
- [ ] Fix generator configuration and output location. Detect added, missing, and edited declarations.
- [ ] Cover all three drift cases and non-mutating checking. Remove obsolete generated artifacts only in an explicit reviewed change.

**Acceptance:** Declarations cover every raw-value variant, lowercase file roles, timestamps, nullable scores, and deliberately omitted fields. Normal builds remain independent of generation.

## Task 2: Producer-owned contract fixtures

**Files:** Proposed src/scorer/report_contract_tests.rs, tests/fixtures/report-contract/current.ts, dashboard/src/report/contract.test.ts; test registration in src/scorer.rs.

**Contract:** One deterministic fixture comes from actual Rust serialization, is checked against generated TypeScript, and is shared by frontend tests.

- [ ] Construct the fixture inside the crate, respecting non-exhaustive types.
- [ ] Include all six raw variants, null scores, structured actions with present/absent navigation fields, and nonempty hotspot, coupling, ownership, and age rows.
- [ ] Fix timestamps and inputs; do not rely solely on the changing dogfood report.
- [ ] Separate explicit fixture updates from normal comparisons. Initial generation must work before the fixture exists.
- [ ] Ensure serialization drift fails Rust comparison and declaration drift fails frontend type checking.

**Acceptance:** Producer and consumer use the same fixture; normal tests never rewrite expected output.

## Task 3: Decode current report data

**Files:** Proposed dashboard/src/report/model.ts, readers.ts, decode.ts, and decode.test.ts.

**Contract:** Unknown input becomes either a constructed validated projection or an error containing the failing field path and explanation.

- [ ] Derive projection types from generated declarations, not another handwritten wire interface.
- [ ] Validate strings, arrays, finite numbers, signed integers where appropriate, nonnegative counts, and nullable integral scores from zero to one hundred.
- [ ] Require exactly one recognized raw-value tag and validate its payload. Raw percentages require finiteness, not an assumed zero-to-one-hundred range.
- [ ] Require valid report thresholds with warning below good; keep hotspot risk semantics separate.
- [ ] Reject primitive values, string actions, missing required fields, malformed nested rows, unusable dates, and malformed present optional fields.
- [ ] Accept null scores, deliberate current omissions, and unrelated extra fields. Test exact error paths, including array indexes.

**Acceptance:** Components never receive unchecked parsed JSON or data accepted through an assertion alone.

## Task 4: Formatting and report-specific thresholds

**Files:** Proposed dashboard/src/report/format.ts and format.test.ts; ScoreGauge, RadarChart, CategoryCard, MetricRow, ScoreBar, TopActions, HotspotsView, CouplingView, and their tests.

**Contract:** Pure formatters consume tagged values or explicit thresholds; score components receive the current report's thresholds as required inputs.

- [ ] Format every raw variant explicitly, retaining ordinary numeric conventions and the empty-list marker.
- [ ] Remove global threshold state; update every score consumer, including TopActions and radar effect dependencies.
- [ ] Keep null scores visually neutral instead of converting them to a scored zero.
- [ ] Remove missing-role, finding-count, string-action, and coupling-action compatibility defaults.
- [ ] Replace legacy-rendering expectations with decoder-rejection coverage; preserve sorting, dismissal, badges, filters, and navigation.
- [ ] Rerender the same score with different explicit thresholds and check that colors follow the new report.

**Acceptance:** No raw value displays as an object string; report order cannot affect colors; hotspot risk bands retain their separate meaning.

## Task 5: Unified upload and session restoration

**Files:** Proposed dashboard/src/report/storage.ts and storage.test.ts, dashboard/src/pages/ReportLoading.test.tsx; Landing.tsx, Report.tsx, and obsolete dashboard/src/types.ts.

**Contract:** Both paths parse unknown JSON and use the same decoder. A small storage adapter owns the session key and storage errors.

- [ ] Validate before storing uploads; store original valid JSON and revalidate on restoration.
- [ ] Distinguish decoding, JSON syntax, file-read, unavailable-storage, and quota errors.
- [ ] Return invalid restored data to the upload flow with regeneration guidance visible; preserve rejected data.
- [ ] Cover valid upload/restoration, missing session data, corrupt stored JSON, incompatible shapes, and browser-storage access failures.
- [ ] Exercise one complete upload/router flow. Remove the old guard, setter, wire interfaces, and temporary import shim after all callers migrate.

**Acceptance:** Loading paths enforce one policy; rejected uploads cannot overwrite valid saved reports.

## Task 6: Delivery checks and documentation

**Files:** Makefile, dashboard/package.json as needed, .gitlab-ci.yml, README.md, CHANGELOG.md.

- [ ] Document explicit generation, drift-check, and fixture-update operations.
- [ ] Add Rust-capable contract checks and Node-capable dashboard checks using the existing exact merge-result checkout policy.
- [ ] Include exporter example tests explicitly; do not assume normal Rust tests run their harness.
- [ ] Explain current-only JSON loading and regeneration; existing self-contained HTML retains its embedded renderer.
- [ ] Verify packaging/builds do not invoke generation and committed declarations accompany dashboard source.

**Acceptance:** Contract drift blocks delivery without adding a new end-user toolchain requirement.

## Verification and completion

Future implementation runs dashboard tests/build, full default and export-feature Rust suites, exporter checks, formatting/Clippy, relevant report smoke, and required P1/P2 gates in the [review process](../../review-process.md). Record evidence rather than blanket pass claims.

P1 enumerates score consumers, formatters, loading paths, legacy fallbacks, and generation checks. M04 adds identities through this contract; M06 owns Rust type placement.

Earlier isolated checks are summarized in the design. They do not prove application, browser, packaging, or CI correctness. No new probes or application tests are included in this planning delivery.
