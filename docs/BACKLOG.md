# Barad-dur Backlog

## v2 — Planned

_Items actively being designed or scheduled for implementation._

(See `docs/plans/` for detailed designs once approved.)

## Performance — Blame Optimization

**Priority**: High (blame is 95% of runtime on large repos)
**Context**: See ADR-001.11 for full performance profile.

### ~~Per-Blob Blame Cache~~ ✓ Done

Implemented in `src/cache/blame.rs`. Blame output cached by blob OID in `.repository-analysis/blame_cache.bin`. `FileEntry.blob_oid` populated from tree walk. Cache is loaded, used, pruned, and saved during each collection cycle in `snapshot_builder.rs`.

### ~~libgit2 In-Process Blame~~ ✗ Investigated, Rejected (2026-04-08)

Investigated replacing `git blame --porcelain` subprocess with `git2::Repository::blame_file`. Benchmark and parity verification both failed:

- **barad-dur** (229 files, ~300 commits, Linux-native): libgit2 was **0.70x** (slower) than subprocess porcelain. 36% of files diverged on timestamps — `git blame` correctly attributes renamed lines to the rename commit, while libgit2 walks to the original commit where the line's pre-rename content was added.
- **FW.Runtime** (8306 non-binary files, 6118 commits, Linux-native): libgit2 processed only 600/8306 files in 47 minutes before being killed — catastrophically slow (>10 hour extrapolation). The subprocess path completes the same repo in seconds. Pathological files with deep edit histories dominate libgit2's runtime.

`BlameOptions::track_copies_same_file(true)` and `use_mailmap(true)` did not close the parity gap. The divergence is a fundamental difference in how libgit2's blame walker traverses parents vs git CLI's blame implementation (which has optimizations libgit2 lacks).

**Conclusion:** The per-blob blame cache already solves the incremental case. For cold runs, the subprocess path remains both faster and more compatible with git CLI semantics. Revisit only if a future version of libgit2 closes the perf gap.

### ~~Selective Blame~~ ✓ Done

`snapshot_builder.rs` already blames only files modified in the time window (`changed_paths` set, Phase 3). Per-blob cache covers unchanged files on subsequent runs. Known gap: cold first run leaves bus factor / knowledge distribution with partial coverage (only recently-changed files) until the cache warms up.

---

## Future — Not Yet Scheduled

### Interactive Config Editor

**Priority**: Nice-to-have
**Depends on**: `.barad-dur.toml` config file (v2 infrastructure)

A guided CLI command (`barad-dur init` or `barad-dur config`) that helps users create or edit their `.barad-dur.toml` configuration file interactively. Should cover:

- Architectural grouping: define component mappings (regex → component name) with live preview of how current files would be grouped
- Team mapping: assign authors to teams, with auto-suggestions based on email domains
- Metric thresholds: customize score thresholds and weights
- Validation: warn on invalid regex, unmapped files, unknown authors

Could be a TUI (e.g. `ratatui`) or a simple question-and-answer flow (e.g. `dialoguer`).

### ~~Accessible Colors in HTML Report~~ ✓ Done (MR !12)

CSS custom property tokens (`--c-good`, `--c-warn`, `--c-danger`, etc.) with a `body.cbf` override block. All semantic hex colors in JS converted to `var(--c-*)`. Toggle button in the page header persists choice in `localStorage`.

### Coupling Cluster Visualization

**Priority**: Nice-to-have

Add a graphical representation of file coupling in the HTML report to surface clusters of highly coupled files. Currently coupling is shown as a flat ranked list (`src/renderer/html.rs`). Targets:

- Force-directed graph (D3 `forceSimulation`) where nodes are files and edges are coupling pairs weighted by co-change frequency
- Visual clustering makes architectural boundaries (or their absence) immediately apparent
- Filter controls: minimum coupling threshold, show only top-N files

### ~~Exclude Files by Language / File Type~~ ✓ Done (v0.17.0)

`--exclude-ext <EXT>` CLI flag and `[exclude] extensions = [...]` TOML key added in v0.17.0. Supports bare extensions (`jar`), compound extensions (`min.js`), case-insensitive, leading dots normalised. Language-name aliases (e.g. `rust`, `python`) not yet supported — would map to a set of extensions.

### Reconsider Afferent/Efferent Coupling

**Priority**: Nice-to-have

Revisit the current afferent (Ca) / efferent (Ce) coupling metrics and their computation. Consider whether the existing implementation accurately reflects coupling direction, and whether instability (`Ce / (Ca + Ce)`) and abstractness should be surfaced as first-class metrics in the report.

### Detect Architecture Style to Determine Cross-Boundary Coupling

**Priority**: Nice-to-have

Automatically detect the architectural style of the repository (layered, hexagonal, feature-sliced, modular monolith, etc.) by analyzing directory structure and naming conventions. Use the detected style to configure which coupling relationships constitute cross-boundary violations — e.g. infrastructure importing from domain in hexagonal architecture. This would make coupling health scores context-aware rather than topology-agnostic.

### Import Extraction for PHP, Ruby, and C/C++

**Priority**: Medium
**Context**: MR !107 follow-up — see `has_import_extractable_files` in `src/metrics/coupling/mod.rs`

Import extraction is a two-stage pipeline, and the two stages currently
disagree about which languages are supported:

| stage | location | languages |
|---|---|---|
| specifier extraction | `import_query`, `complexity/lang_dispatch.rs` | Rust, JS/TS, Python, Go, Java, C#, Kotlin, PHP |
| path resolution | `candidates_for`, `collector/import_resolver.rs` | Rust, JS/TS, Python, Go, Java, C#, Kotlin, PHP |

Everything else — Ruby, C/C++, Swift, Scala — falls to
`Language::Generic` and never produces an import edge. Kotlin used to
behave identically (query but no resolver arm); that gap is closed, and
`every_language_with_an_import_query_can_resolve_imports` now pins the
two stages together so they cannot drift apart again.

**Consequence today.** Repositories in those languages have an empty
import graph, so afferent coupling, efferent coupling and circular
dependencies all report *unscored* rather than a fabricated 100, and
change-coupling smells scores from co-change alone with the note "no
import data to corroborate against". The signal is honest but absent:
three of ten coupling metrics contribute nothing to the category score.

**Work, per language:**

1. Add the tree-sitter grammar to `Cargo.toml`
2. Extend `Language` + `detect_language` in `complexity/fallback.rs`
3. Add a `*_IMPORTS` query in `complexity/queries.rs`, with a validity
   test alongside the existing ones (`queries.rs:375+`)
4. Wire `grammar_for` + `import_query` in `complexity/lang_dispatch.rs`
5. Add a resolver arm to `candidates_for` in `collector/import_resolver.rs`

Step 5 is the one that is not boilerplate — resolution semantics differ
sharply: PHP `use`/`require` against PSR-4 autoload roots in
`composer.json`, Ruby `require_relative` (path-based) vs `require`
(load-path-based), C/C++ `#include "..."` vs `<...>` against include
directories. Each needs its own candidate-path rules, and each should
land with the language it serves rather than as one shared change.

**PHP is done** — `resolve_php_import` plus `collector/composer.rs`. PHP
namespaces map to directories only because `composer.json` says so, so the
PSR-4 roots are parsed from every manifest in the tree (`autoload` and
`autoload-dev`) and rebased onto each manifest's own directory. A namespace
resolves through the longest matching prefix; `require`/`include` resolve
their string literal against the requiring file's directory, which is what
`__DIR__ . '/x.php'` means. Not handled: Laravel path helpers
(`base_path()`), `require $var`, PSR-0, and the `classmap`/`files` autoload
sections.

**Kotlin is done** — `resolve_kotlin_import`, modelled on
`resolve_java_import`: dotted package paths, also tried under
`src/main/kotlin/`, plus the parent of the dotted path because Kotlin
routinely imports a member or top-level function rather than a type.
Afferent, efferent and circular dependencies are now scored for Kotlin
repositories instead of unscored.

Not handled: Kotlin Multiplatform source roots
(`src/commonMain/kotlin/` and friends) and wildcard imports — both worth
revisiting against a real MPP repository rather than guessed at. Kotlin
also does not enforce package/directory correspondence, so a
non-conventional layout yields no edge (never a wrong one).

Adding any of these requires no change in `src/metrics/coupling/` —
`has_import_extractable_files` reads both dispatch tables directly, so a
newly supported language starts being scored on its own.

### Structural-hub calibration — measured, and what is still open

**Priority**: Low (recorded so the numbers are not re-derived)
**Context**: measured 2026-08-29 while investigating why PHP support moved
god objects on a Laravel monorepo.

Import-graph degree (incoming + outgoing) across source files, five real
repositories:

| repo | lang | n | med | p75 | p90 | p99 | max | @8 | @12 | @16 | @20 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Apios-Web | PHP (Laravel) | 1248 | 1 | 3 | 7 | 32 | 123 | 9.5% | 5.3% | 4.4% | 2.6% |
| mautic | PHP (Symfony) | 4672 | 2 | 5 | 10 | 39 | 325 | 14.6% | 7.9% | 4.6% | 3.1% |
| barad-dur | Rust | 139 | 1 | 3 | 5 | 22 | 61 | 5.8% | 4.3% | 2.9% | 1.4% |
| ihexa | TS | 138 | 1 | 2 | 4 | 16 | 31 | 4.3% | 1.4% | 1.4% | 0.7% |
| the-unit-question | TS | 218 | 0 | 5 | 7 | 11 | 11 | 8.3% | 0% | 0% | 0% |

**What this settled.** `god_node_min_degree = 8` lands near p90 in every
repository measured, so "degree >= 8" is roughly "top decile" regardless of
language. PHP flags more (9.5%, 14.6%) because PHP files genuinely are more
connected — median 1-2 against 0-1, p99 32-39 against 11-22 — not because
the floor is miscalibrated for PHP. The floor was left at 8.

It also showed the median multiplier never bound, which is why it was
replaced by a p90 term (see CHANGELOG).

**Still open.**

- The floor of 8 is justified by "it happens to sit near p90 in five
  repositories", which is an observation, not a derivation. A larger corpus
  might show it drifting.
- `the-unit-question` is the shape neither term handles well: median 0,
  p75 5, and *everything* above 12 vanishes (18 files at >=8, none at >=12).
  A bimodal graph where a single scalar threshold is a poor fit.
- Nothing here measures whether flagged files are *actually* god objects.
  Every number above is distributional; none is validated against a human
  judgement of the code.

### Import resolution floor — stop scoring an empty graph

**Priority**: High (three languages affected today)
**Design**: `docs/superpowers/specs/2026-08-30-csharp-type-resolution-design.md`

Three languages ship a resolver that produces **zero edges**, so afferent
coupling, efferent coupling and circular dependencies score a perfect 100
on repositories nobody can measure:

| language | resolver | why it yields nothing |
|---|---|---|
| C# | present | `using` names a *namespace*, not a file; `Domain.cs` never exists |
| Go | present | builds a literal `*.go` path; nothing expands globs |
| Kotlin | was missing | fixed in v0.22.0 |

`has_import_extractable_files` asks whether a language *could* resolve — a
static capability check — so a wrong resolver passes it. Measured on two
real C# repos: mean degree 0.0, max 0, three metrics at 100.

The fix already exists in this codebase for calls: `call_resolution_floor`
suppresses function-hub output when the snapshot-wide call resolution rate
falls below a fraction. Do the same for imports — track specifiers that
produced an edge over specifiers extracted, and report the import metrics
*unscored* below the floor.

This catches both a wrong resolver and a missing one, for every language
including ones not yet written, with no per-language work.

### Go import resolution is broken

**Priority**: Medium
**Depends on**: nothing; independent of the floor above

```rust
fn resolve_go_import(raw: &str, source: &Path) -> Vec<PathBuf> {
    vec![base.join(last).join("*.go")]   // literal "*.go", never matches
}
```

A Go `import` names a *package* — a directory of files — not a file. The
resolver builds a path ending in the literal string `*.go` and
`resolve_single_import` compares it against real paths; no glob expansion
exists anywhere in the collector. Zero edges, always.

Same class as C#, and the same granularity question: a package maps to many
files, so resolving it needs either a symbol-level model or a deliberate
choice about fan-out. Not yet measured against a real Go repository —
do that first, as was done for PHP and C#.
