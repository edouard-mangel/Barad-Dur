# Design: Analysis Exclusion Expansion + Test Pair Indicator

## Overview

Two related improvements to reduce noise in analysis output and surface useful
signal in the coupling tab.

## Feature 1 — Expanded Default Exclusions

### Problem

The current default exclusion list covers lockfiles, translation files, ORM
migrations, and tooling directories, but misses common generated-file patterns
and build output directories. These inflate churn and coupling metrics without
representing real code changes.

### Solution

Expand `DEFAULT_EXCLUDE_EXTENSIONS` and `DEFAULT_EXCLUDE_PATTERNS` in
`src/collector/exclude.rs`.

**New default extensions:**

| Extension | Source |
|-----------|--------|
| `g.cs` | Roslyn / T4 code generation |
| `generated.cs` | Generic C# generation |
| `pb.go`, `pb.h`, `pb.cc`, `pb.swift` | Protocol Buffers |
| `_pb2.py` | protobuf Python |
| `d.ts` | TypeScript declaration files |
| `min.js`, `min.css` | Minified assets |

**New default path patterns (directories only — `dist/` excluded intentionally):**

```
**/node_modules/**
**/vendor/**
**/__pycache__/**
**/*.egg-info/**
**/target/**
**/.next/**
**/.nuxt/**
**/out/**
**/gen/**
**/generated/**
**/.gradle/**
**/.mvn/**
**/build/**
```

### Constraints

- No new CLI flags or config keys — this is a pure expansion of defaults.
- Users who disagree with any pattern opt out via `use_defaults = false` in
  `[exclude]` TOML block, or add `--no-default-excludes` CLI flag (both exist today).
- `dist/` is intentionally excluded from the list — it is meaningful in some
  repos (e.g. published packages).

---

## Feature 2 — Test Pair Indicator in Coupling Tab

### Problem

Temporal coupling pairs often include production-file ↔ test-file pairs (e.g.
`UserService.java` ↔ `UserServiceTest.java`). These are expected co-changes and
not architectural smells, but they appear in the coupling list alongside genuine
unexpected couplings — creating noise for the reader.

### Solution

Detect test pairs at the scorer level and propagate a flag through to the HTML
renderer.

#### Data model

Add `is_test_pair: bool` to `CouplingPair` in `src/scorer/types.rs`.

#### Detection logic (`src/scorer/builders.rs`)

A pure function `is_test_pair(a: &str, b: &str) -> bool` checks whether one
filename is the test counterpart of the other using stem comparison
(case-insensitive, extension-agnostic):

| Convention | Example |
|------------|---------|
| `{stem}Test` / `{stem}Tests` | `UserServiceTest.java` ↔ `UserService.java` |
| `{stem}.test` / `{stem}.spec` | `parser.test.ts` ↔ `parser.ts` |
| `{stem}_test` / `{stem}_spec` | `user_test.go` ↔ `user.go` |
| `test_{stem}` | `test_user.py` ↔ `user.py` |

Matching is done on the filename stem only (not the full path), so
`src/services/UserService.java` ↔ `tests/services/UserServiceTest.java` matches
correctly.

#### HTML renderer (`src/renderer/html/js_coupling.rs`)

When `pair.is_test_pair` is `true`, render a `🧪` badge beside the file pair
row with tooltip: `"Expected coupling — production file and its test file
naturally change together."`

#### JSON output

`is_test_pair` is serialized automatically via serde — no renderer change
needed.

---

## Files to Modify

| File | Change |
|------|--------|
| `src/collector/exclude.rs` | Expand `DEFAULT_EXCLUDE_EXTENSIONS` and `DEFAULT_EXCLUDE_PATTERNS` |
| `src/scorer/types.rs` | Add `is_test_pair: bool` to `CouplingPair` |
| `src/scorer/builders.rs` | Implement `is_test_pair()`, wire into `build_coupling_pairs()` |
| `src/renderer/html/js_coupling.rs` | Render `🧪` badge when `pair.is_test_pair` |
| `src/renderer/html/tests_extra.rs` | Tests for badge rendering |

## Out of Scope

- `.gitignore` parsing (deferred)
- `dist/` exclusion (intentionally omitted)
- Test pair detection in metrics other than coupling pairs
- Test coverage ratio metric
