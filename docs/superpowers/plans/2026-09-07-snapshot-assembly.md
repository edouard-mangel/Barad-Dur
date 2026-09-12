# Snapshot Assembly Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Give source-analysis data one named aggregation/resolution path and one final snapshot assembly path.

**Architecture:** Named raw and resolved intermediate records replace positional tuples. Live and historical readers remain explicit adapters; shared pure helpers aggregate, resolve, and assemble their results.

**Tech Stack:** Existing Rust collector, Rayon, tree-sitter extraction, git2, and snapshot model.

**Spec:** [M03 finding](../../reviews/2026-09-07-structure-maintainability.md#m03--unify-snapshot-assembly-with-named-intermediate-structures) and the design below.

## Global constraints

- Prose-only planning; no implementation or probes.
- Preserve serialized RepoSnapshot layout, bincode cache compatibility, exclusions, ordering, and unreadable-content behavior.
- Historical source and manifest content comes from the selected commit; current ignore policy remains current where that is the existing behavior.
- Retain live parallelism, historical collection modes, and backfill's no-AST/no-blame policy.
- Do not redesign parsers, import semantics, cache invalidation, or the public collector API as incidental cleanup.

## Design decision

Replace RawAstOutput and AstParts with named intermediate records covering their existing fields. Keep these collector-internal, outside the serialized snapshot model. A new source-assembly module owns aggregation and resolution; snapshot_builder keeps collection orchestration and final assembly.

Prefer concrete data records and the existing resolver helpers over a generic reader framework. Disk/blob access and package-manifest provenance stay visible at the adapters. Share processing after reading, not the policy deciding what or where to read.

## Task 1: Characterize live and historical behavior

**Files:** src/collector/snapshot_builder.rs tests, tests/collector_tests.rs, relevant backfill and coupling suites; new tests/snapshot_assembly_walking_skeleton.rs.

- [ ] Build equivalent-content cases for working-tree and historical collection; compare the applicable analysis channels rather than timestamps or path-specific metadata.
- [ ] Include imports, re-exports, classes/inheritance, call records, file metrics, and any other current tuple fields.
- [ ] Cover binary/non-UTF-8 content, unreadable files, invalid/missing blobs, empty input, duplicate references, and unresolved imports.
- [ ] Include a historical manifest that differs from the working tree and changed current ignore rules.
- [ ] Capture current ordering and index relationships before restructuring.

**Acceptance:** Tests expose provenance and collection-mode differences; they do not incorrectly require AST-less backfill to equal AST-enabled collection.

## Task 2: Replace positional intermediates

**Files:** src/collector/snapshot_builder.rs; new src/collector/source_assembly.rs; module registration in src/collector/mod.rs.

**Contract:** Raw data contains collected but unresolved channels; resolved data contains exactly the channels needed for final snapshot assembly. Neither is serialized.

- [ ] Inventory every tuple position and give it a domain-meaningful field name.
- [ ] Update producers, destructuring, defaults, and tests together, keeping behavior unchanged.
- [ ] Make skipped AST collection explicit through the existing mode and empty intermediate result; do not reinterpret it as measured absence of findings.
- [ ] Keep each field's ownership and conversion visible; avoid an opaque catch-all payload.

**Acceptance:** No positional RawAstOutput/AstParts plumbing remains and no serialized snapshot field changes.

## Task 3: Consolidate aggregation and resolution

**Files:** New source_assembly.rs, snapshot_builder.rs, existing resolver modules only where signatures must accept named data.

**Contract:** Aggregation consumes per-file SourceAnalysis results; resolution consumes aggregated raw channels, included files, and explicit manifest/resolver context.

- [ ] Reuse the existing single-parse analyse_source flow; preserve which files are read and parsed.
- [ ] Route live and historical outputs through one aggregation policy while retaining live parallel collection.
- [ ] Share raw import, class, re-export, and call resolution using existing resolve_against_files behavior.
- [ ] Require historical manifest input from the selected tree, never a hidden disk fallback.
- [ ] Preserve deduplication, error handling, and deterministic ordering; do not introduce nondeterministic container traversal into output.

**Acceptance:** Adding a new source-analysis channel has one aggregation/resolution path, with adapters responsible only for acquisition/provenance.

## Task 4: Unify final construction and indexing

**Files:** src/collector/snapshot_builder.rs; src/snapshot/mod.rs only if internal helper access is necessary, not to change the serialized model.

**Contract:** Final assembly consumes complete repository metadata, commit/author/file data, explicit blame/AST results, and acquisition metadata, then builds indexes from that final data once.

- [ ] Replace duplicated RepoSnapshot construction with one internal assembly path.
- [ ] Preserve branch, name, path, time-window, created-at, manifest, and collection-state provenance supplied by each caller.
- [ ] Build indexes after core data is final; test that referenced files, commits, and authors resolve consistently.
- [ ] Retain existing public entry points for lightweight and AST-enabled historical collection.

**Acceptance:** One final construction path serves both adapters without altering the cache format or historical source provenance.

## Task 5: Complete behavior and cache review

- [ ] Sweep every acquisition/assembly caller, including live cache misses, cache hits, backfill, and gate baselines.
- [ ] Test cold versus warm collection output and disk/blob parity where collection modes match.
- [ ] Check relevant collector, backfill, coupling, and call-graph integration suites; run the full Rust suite and formatting/Clippy.
- [ ] Complete P1 for field propagation, provenance, ordering, and indexing, followed by required P2 corpus/audit checks.
- [ ] Retain existing compatibility checks; if serialized changes become unavoidable, stop that expansion for a separate design instead of silently invalidating caches.

**Completion:** Intermediate data is named, processing and final construction are shared, acquisition policy remains explicit, and no externally observable behavior changes.

## Dependencies and evidence

This plan can ship independently of M01/M02 because RepoSnapshot remains stable. M06 later changes calculation inputs, not collection provenance. Tuple definitions and duplicated assembly locations were inspected; new helper signatures, full parity, and packaging/cache behavior have not been tested during planning.

Future verification follows the [review process](../../review-process.md); this document is not execution evidence.
