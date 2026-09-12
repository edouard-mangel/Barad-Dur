# Kotlin import extraction and wildcard lookup

Worktree: `.worktrees/kotlin-wildcard-resolution`; base: `cbadbe0`.
Reference: https://github.com/ktorio/ktor at `88c0026f6fee2af55d96a20620f2e700b912b334` (shallow checkout; source-only measurements).

## P0 evidence and scope

The grammar probe used the pinned tree-sitter-kotlin-ng 1.1.0 and tree-sitter 0.27.0. For `import io.ktor.http.*`, the parse was:

```text
import "import io.ktor.http.*"
 import "import"
 qualified_identifier "io.ktor.http"
  identifier "io"
  . "."
  identifier "ktor"
  . "."
  identifier "http"
 . "."
 * "*"
```

`analyse_source` returned `[]` for that wildcard followed by `import io.ktor.http.HttpStatusCode`. The previous query `(import (identifier) @path)` misses the qualified child and can capture an alias instead. The new regression failed with actual `["Status"]` versus expected `["io.ktor.http.*", "io.ktor.http.HttpStatusCode", "Foo"]`.

A text census of Ktor found 3,129 tracked files, 2,593 Kotlin files, 15,190 import lines and 9,446 wildcard imports. The benchmark uses production tree-sitter extraction to establish the actual workload, rather than relying on these textual counts.

Fix extraction first, capturing the optional anonymous `*` separately so aliases and comments cannot corrupt the package path. Then build a directory index over known Kotlin files once per resolution batch. Both graph resolution and single-target resolution must reuse that index.

Ktor uses roots such as `ktor-http/common/src/`, outside the resolver's existing conventions. This work preserves candidate-root semantics; inferring cross-module or multiplatform dependencies without Gradle configuration is outside this performance fix. Report unresolved imports honestly and test populated graphs explicitly.

## Invariants this change introduces or preserves

- Named and wildcard imports retain the full imported path; aliases are not paths.
- Wildcard lookups visit candidate directories and their direct Kotlin members, not the full repository per import.
- Only included `.kt`/`.kts` files can become wildcard targets; no self edges, descendants, or unrelated modules.
- Multiple candidate roots may overlap; targets remain unique and sorted.
- A single-target caller returns a path only for exactly one distinct target.
- Snapshot caches and score history computed using the broken extraction must be invalidated.

## Validation

Failing extraction test, focused Kotlin and cross-language extraction tests, benchmark before/after with the same extracted imports (index construction included), full Rust suite, formatting, clippy, P1 call-site sweep, field regression/determinism and audit. Store measured results in the review note. Do not change corpus baselines implicitly.
