# HTML Report Modules Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Make embedded report script dependencies explicit while retaining a single offline HTML artifact.

**Architecture:** Maintain JavaScript modules with explicit imports and one initialization entry point. An explicit developer build produces a committed single-script bundle embedded by Rust. Default Cargo builds consume that bundle without Node.

**Tech Stack:** Existing JavaScript, Vite/pnpm tooling, Rust HTML renderer, jsdom smoke checks, and GitLab CI.

**Spec:** [M05 finding](../../reviews/2026-09-07-structure-maintainability.md#m05--make-html-script-dependencies-explicit) and the design below.

## Global constraints

- Planning only; no modules, build configuration, generated bundle, or probes are implemented here.
- Preserve offline HTML delivery, every tab, cross-tab links, hash state, quick-open, sorting, filtering, and theme behavior.
- Preserve script-safe JSON escaping and DOM construction without innerHTML.
- No Node requirement for ordinary Cargo builds, installation from source, or published crate consumers.
- No new frontend framework, runtime CDN dependency, runtime module loading, or unrelated visual redesign.

## Design decision

Use explicit modules and a committed bundle, reusing the existing Vite toolchain through a separate report-only build configuration. The report build must not invoke the React dashboard entry point or emit external runtime assets. Its output is a single classic script with a private outer scope so opening the HTML from disk does not require module fetching.

Keeping concatenation with stronger comments would retain hidden dependencies. Generating the bundle during Cargo build would impose Node on users. Committing a reproducible bundle adds a generated-file workflow but preserves the current distribution contract.

The exact pinned Vite configuration and packaging behavior are unverified implementation-readiness checks; this plan makes no untested API signature claims.

## Task 1: Inventory shared state and observable behavior

**Files:** src/renderer/html.rs, src/renderer/templates/, scripts/report-smoke.mjs, existing HTML renderer tests.

- [ ] Inventory the current concatenation list and each fragment's imported helpers, report data, mutable state, events, and initialization assumptions.
- [ ] Identify actual dependency cycles, especially navigation, tab activation, overview widgets, tooltips, and quick-open.
- [ ] Record every tab and navigation route, empty/unscored states, sorting/filtering behavior, and keyboard/theme interactions.
- [ ] Preserve representative report fixtures for full and sparse data, malicious display strings, and optional report sections.

**Acceptance:** The refactor has a dependency inventory and observable behavior checklist, not just a file-splitting target.

## Task 2: Establish the bundle and distribution contract

**Files:** New dashboard/vite.report.config.ts, scripts/report-bundle-check.mjs, src/renderer/templates/report_entry.js, src/renderer/templates/generated/report.js; dashboard/package.json, Makefile, Cargo.toml packaging rules if necessary.

**Contract:** Explicit generation produces one deterministic embedded script. Non-mutating checking rebuilds elsewhere and compares the complete expected output set.

- [ ] Define a report-only entry/output, fixed build settings, and lockfile-pinned tooling using the existing dependency workflow.
- [ ] Exclude runtime imports, external chunks, external source maps, network assets, and environment-dependent paths/timestamps.
- [ ] Commit the bundle and ensure it is included in source/crate packages without running Node from build.rs or normal startup.
- [ ] Detect edited, missing, and extra generated artifacts; checking must not rewrite the committed output.
- [ ] Keep generated output inspectable enough for review and include required dependency notices if any are bundled.

**Acceptance:** A packaged default Cargo build can render a report using only committed assets. Dashboard-only development remains separately invocable.

## Task 3: Extract pure helpers and explicit state owners

**Files:** Existing shared.js, navigation.js, chrome.js, overview_widgets.js, treemap_layout.js; new formatting.js and report_context.js where needed.

**Contract:** Pure helpers take explicit inputs; navigation owns tab/hash/quick-open state; the entry point creates per-report context and connects consumers.

- [ ] Move formatting and layout helpers behind explicit exports without changing numeric/text conventions.
- [ ] Move shared mutable state to a named per-report owner, not a new module-global singleton.
- [ ] Have initialization read embedded report data once; tabs receive their required data and callbacks rather than reading arbitrary globals.
- [ ] Break cycles with explicit callbacks or data inputs, not a general event bus/service locator.
- [ ] Make event binding and any cleanup/reinitialization responsibility explicit.

**Acceptance:** Helper tests require no full-report DOM; navigation tests can exercise state changes independently of tab internals.

## Task 4: Convert tabs and switch the renderer

**Files:** Existing overview, hotspots, coupling, graph, ownership, age, treemap UI, trends, dependencies, audit, and authors scripts; report_entry.js; src/renderer/html.rs.

- [ ] Convert each tab into a module with explicit data/helper/navigation dependencies and registration through the entry point.
- [ ] Preserve initialization order where behavior requires it; replace accidental ordering with declared dependencies.
- [ ] Switch Rust embedding from manual fragment concatenation to the committed bundle only when equivalent behavior is covered.
- [ ] Remove the shared.js/authors.js split closure and obsolete concatenation constants. Retain source modules as maintained inputs.
- [ ] Keep CSS and embedded-data escaping unchanged; check that the bundle itself contains no script-terminating sequence that could break its HTML container.

**Acceptance:** The generated report contains one self-sufficient script, with no missing tab or hidden dependency on the old concatenation order.

## Task 5: Enforce module and artifact verification

**Files:** scripts/report-smoke.mjs, new dashboard/vitest.report.config.ts and report-module tests, .gitlab-ci.yml, Makefile, README.md.

- [ ] Test pure helpers and navigation state independently using a dedicated report-module test configuration.
- [ ] Run every-tab smoke against the actual Rust-rendered bundled artifact, not only against source modules.
- [ ] Cover cross-tab drill-through, hash restoration, quick-open, sorting, theme changes, sparse data, and script-injection-sensitive strings.
- [ ] Inspect offline output for external assets/imports and verify packaged build/install behavior without Node available.
- [ ] Add reproducibility checks using the repository's exact merge-result checkout policy; run full relevant Rust/frontend suites and P1/P2 gates during implementation.

**Completion:** Source dependencies are explicit, helper/state behavior is independently testable, and distributed HTML remains offline and self-contained.

## Dependencies and evidence

M04 may change tooltip lookup keys; carry that behavior into the modules without duplicating the migration. M01's React decoder remains a different loading boundary and is not reused blindly inside already-generated HTML.

The current manual concatenation and split closure were inspected. Bundle configuration, determinism, browser behavior, and package contents have not been probed for this planning-only work. Verify them before accepting the implementation under the [review process](../../review-process.md).
