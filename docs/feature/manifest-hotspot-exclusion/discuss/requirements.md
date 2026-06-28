# Requirements — manifest-hotspot-exclusion

## Problem

Core-ecosystem manifest files (`package.json`, `Cargo.toml`, `go.mod`, `*.csproj`,
`pom.xml`, `build.gradle[.kts]`, `requirements.txt`, `pyproject.toml`) are not in
`DEFAULT_EXCLUDE_PATTERNS`, so they enter the snapshot and surface in hotspots,
complexity, and coupling-churn views. They are declarative config with high edit
frequency (dependency bumps, version edits) but no logic to act on — they are noise
in those surfaces and can crowd out genuine code hotspots.

Lockfiles are already excluded (`exclude.rs:32`); manifests are the remaining gap.

## Scope

### In scope
- Add core-ecosystem manifest globs to `DEFAULT_EXCLUDE_PATTERNS` in
  `src/collector/exclude.rs`.
- Unit tests proving exclusion under defaults and retention when defaults are off.

### Out of scope
- **Opt-out granularity** (re-include manifests without disabling all defaults) —
  OPEN QUESTION for DESIGN (see Constraints).
- **Richer manifest parsing** (direct-vs-transitive deps, version-range risk,
  scripts/engines/workspaces) — separate `Ideas.md` entry.
- **Coupling-pair render-time suppression** — manifests co-changing with everything
  is a render concern, not a snapshot drop; not addressed here.
- Measuring manifest churn as a positive signal — deferred with richer parsing.

## Functional Requirements

- **FR-1**: With `exclude.use_defaults = true`, each core-ecosystem manifest is
  excluded from the analyzed file set, including nested copies in monorepos
  (`**/`-prefixed globs).
- **FR-2**: With `exclude.use_defaults = false` (or `--no-default-excludes`),
  manifests are NOT excluded — identical to the existing all-defaults-off behavior.
- **FR-3**: No manifest file appears in the hotspot ranking when defaults are on.

### Core manifest set (FR-1)
```
**/package.json
**/Cargo.toml
**/go.mod
**/*.csproj
**/pom.xml
**/build.gradle
**/build.gradle.kts
**/requirements.txt
**/pyproject.toml
```

## Non-Functional Requirements

- **NFR-1 (safety invariant)**: The deps/CVE category (`src/collector/deps.rs`) and
  dependency-based coupling (`src/coupling/dependency.rs`) MUST produce identical
  output before and after the change. Both read manifests/lockfiles from disk
  (`repo_root.join(...)`), independent of the snapshot — verified during DISCUSS with
  code citations in `discuss-verification.md`.
- **NFR-2 (consistency)**: New globs follow the existing `**/`-prefix convention used
  by lockfile patterns so nested paths are covered.
- **NFR-3 (paradigm)**: Implementation stays a pure predicate (`is_excluded`) with no
  I/O, per project FP conventions.
- **NFR-4 (no perf regression)**: `is_excluded` already runs a linear glob match per
  file; adding ~9 globs is ~9 extra string comparisons (~µs) per path, dominated by
  existing git/blame/AST work. Target: < 2% analysis-time overhead — effectively
  unmeasurable.

## Constraints

- **C-1 (opt-out granularity — OPEN for DESIGN)**: Today `use_defaults` is
  all-or-nothing; a user wanting manifests in hotspots must disable *all* defaults
  (losing lockfile/generated-dir exclusion). v1 accepts this; DESIGN decides whether
  a finer-grained toggle (e.g. `exclude.manifests`) is warranted.
- **C-2**: `dist/` is intentionally NOT excluded today (published output may be real
  code). Excluding manifests does not contradict this — manifests are config, not
  output.
- **C-3**: `requirements.txt` doubles as the pip "lock" read by `deps.rs`; excluding
  it from the snapshot is safe because that read is from disk (covered by NFR-1).

## Completeness self-check
- Problem, scope (in/out), FRs, NFRs, constraints, and the concrete glob set are all
  specified. The single deferred decision (C-1) is explicitly flagged, not left
  implicit. Completeness estimate: ≥ 0.95.
