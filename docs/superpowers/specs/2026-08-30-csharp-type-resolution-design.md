# C# type resolution — design

## Why

C# is wired into every dispatch table but its import resolution does not
work. `resolve_csharp_import` maps a `using` to a file path:

```rust
using Domain;                  ->  Domain.cs
using Infrastructure.Entities; ->  Infrastructure/Entities.cs
```

A C# `using` imports a **namespace**, not a file. Namespaces span many
files, and C# — unlike Java — does not require them to mirror directories.
Those candidates essentially never exist.

Measured on two real C# repositories (25 and 17 `.cs` files):

```
Afferent coupling      mean 0.0, max 0   -> score 100
Efferent coupling      mean 0.0, max 0   -> score 100
Circular dependencies  0 cycles          -> score 100
```

**Zero edges, three metrics reporting a perfect score.** This is the
false-perfect bug v0.21.0 set out to remove, surviving in a language nobody
re-checked. It is worse than the Kotlin case: Kotlin had no resolver arm,
so `resolves_imports("kt")` was false and the guard caught it. C# *has* an
arm that produces candidates which never match, so the metrics are
considered measured.

`every_language_with_an_import_query_can_resolve_imports` does not catch
this. It checks that both dispatch tables have an entry — **wiring, not
efficacy**. Python, Go and Java have never been checked against real repos
either.

## The general fix, which should come first

C# is the third instance of one bug class, not a one-off:

| language | resolver | result |
|---|---|---|
| Kotlin | none (query only) | zero edges — caught by the wiring guard, fixed |
| C# | present, wrong semantics (`using` is a namespace) | **zero edges, false 100** |
| Go | present, wrong semantics (builds a literal `*.go` path; nothing expands globs) | **zero edges, false 100** |

Python, Go and Java have never been validated against real repositories.

Neither symbol-level nodes nor confidence levels prevent this. Both improve
edge *quality*; the recurring failure is edge *absence* being read as
"measured and clean". An edge that is never created carries no confidence,
and granularity is irrelevant when the count is zero.

**The fix that generalises already exists in this codebase, for calls:**

> `call_resolution_floor` — "when the snapshot-wide call resolution rate
> (resolved + same-file over all edges) falls below this fraction,
> function-hub output is suppressed rather than built on mostly-unresolved
> data. Default 0.5."

The import graph has no equivalent. It should. Track the resolution rate —
specifiers that produced an edge over specifiers extracted — and when it
falls below a floor, report the three import-derived metrics *unscored*
rather than scoring an empty graph.

| fix | catches a wrong resolver | catches a missing one | per-language work |
|---|---|---|---|
| symbol-level nodes | no | no | yes, each |
| confidence levels | no | no | yes, each |
| **import resolution floor** | **yes** | **yes** | **no** |

This is also the honest complement to `has_import_extractable_files`, which
asks *could* this language resolve — a static capability question. The floor
asks *did it, here*, which is what actually matters, and would have flagged
Kotlin, C# and Go without anyone thinking to check them.

Do this before, or independently of, the C# work below.

## Options considered and rejected

**Roslyn.** Would solve it correctly — full symbol binding resolves a type
reference to its declaring file. Rejected: no Rust binding exists and none
can, since Roslyn is a .NET library. The shipped Docker image is
`FROM scratch` (static musl binary + git + CA certs); .NET would add
~70-100 MB for one language, and `dotnet` is not present on dev machines or
in CI, so results would silently depend on the host.

**Namespace index — `using N` yields an edge to every file declaring N.**
Approximate. Measured 27 file-level edges on a 25-file repo. One `using` of
a 12-file namespace produces 12 edges, alone clearing
`god_node_min_degree = 8` and making a file a "structural hub" by syntax
rather than architecture.

**`.csproj` ProjectReference.** Exact, and genuinely good data — the
measured repo has a clean layered graph (Domain -> nothing, Infrastructure
-> Domain, App -> Infrastructure, Tests -> both). But it is *project*
granularity, and projecting it into a file-level graph is multiplicative:
`UnitTests -> Domain, Infrastructure` becomes 6 x 16 = 96 edges from one
line of XML. Total **200 edges across 25 files** — average degree 16,
double the hub floor, so nearly every file becomes a hub. Seven times more
fabricated edges than the approximation it was meant to beat.

Also note C# project references cannot be circular — the compiler forbids
it — so a project-level circular-dependency metric would be trivially
always clean.

`.csproj` data belongs in a **project/module-level dependency view**, which
barad-dur does not have. That is a separate feature, and `Cargo.toml`
workspace members, `package.json` workspaces and Maven modules would feed
the same concept.

## Chosen: type-name index + usage matching

Approximate symbol resolution without Roslyn, following the precedent
already in this codebase — `inheritance.rs` resolves `extends Base` to a
declaring file for TS/JS, and `calls.rs` resolves call targets.

**Extraction, per file, one tree-sitter pass:**

- declared `namespace`
- declared type names: `class`, `interface`, `record`, `struct`, `enum`
- `using` directives
- PascalCase identifiers used in the body

**Collector, repo-level index:** `(namespace, TypeName) -> file`, built from
all files' declarations. Same slot as PSR-4 roots — `RepoImportConfig`,
populated at both collection sites (working tree reads disk, historical
path reads the blob at that commit).

**Resolution:** for file F, for each in-scope namespace N (its usings plus
its own), for each used identifier T, emit an edge to `index[(N, T)]` when
present.

### The integration decision

C# extraction currently emits `using` strings as raw imports. Under this
design it emits **candidate fully-qualified names** instead — the
`using x identifier` cross-product, e.g. `Domain.TrainingRepository`. A file
with 5 usings and 30 PascalCase identifiers yields ~150 candidates, nearly
all of which miss.

Cheap at runtime (hash lookups), but it changes what `raw_imports` *means*
for C#: from "what the file imports" to "what it might reference".

The alternative is a dedicated C# resolution path in the collector,
bypassing `raw_imports` — cleaner semantics, more code, and a second
pipeline to keep working. Preference is the cross-product, because it
reuses `RepoImportConfig`, `resolve_imports` and the existing invariant
test rather than adding a parallel path.

### Expected result

Precise file-level edges with no fan-out: `UserRepository.cs` -> the one
file declaring `TrainingRepository`, not all three files in `Domain`.

### Known limitations to record, not fix

- Two types of the same name in different in-scope namespaces are
  ambiguous; C# resolves by rules we cannot replicate. Emit edges to both
  and document it.
- `global using` (implicit in modern SDK-style projects) is not in the file,
  so its namespaces are not in scope for this analysis.
- Extension methods, generics and aliases (`using X = A.B.C;`) are not
  modelled.
- PascalCase filtering is a heuristic for "looks like a type".

## Follow-ons, from studying graphify's model

graphify builds a **symbol-level** graph — nodes are symbols, linked to
their files by `contains` edges — and derives file-level views by
projection. Every dead end above (namespace fan-out 12 edges, `.csproj`
fan-out 200 edges) was a granularity mismatch: exact data at one
granularity, projected multiplicatively into another. Not flattening avoids
it entirely.

The type-index design below is a partial rediscovery: it resolves straight
to the declaring file, skipping the symbol node. That works for imports but
discards structure that would make call graphs and inheritance composable.

graphify also carries explicit confidence, with a rule against dropping:

```
EXTRACTED  relationship explicit in source          score 1.0
INFERRED   reasonable inference                     0.6-0.95
AMBIGUOUS  uncertain - flag for review, DO NOT OMIT 0.1-0.3
```

That answers the ambiguity limitation below better than "emit both and
document it": emit them marked AMBIGUOUS so downstream can weight or
exclude them, rather than being indistinguishable from certain edges.
Precedent exists here already — `BaseRef::{SameFile, Resolved,
Unresolvable}` and `call_resolution_floor` both encode resolution
confidence. The import graph is the one place with no such notion: an edge
either exists or it does not.

Both are worth doing, and neither is urgent. They make edges better; they
do not stop the next language shipping with zero of them.

## Still open

Whether to fix the false-100 immediately by making C# unscored (small,
honest, no signal) while this is built, or to wait and ship the real
resolution in one change.

**Go is broken today, same class, and is tracked separately.**
