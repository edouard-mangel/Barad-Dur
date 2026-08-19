# Architecture Conformance — Declared Layer Boundaries — Design

**Date:** 2026-08-18
**Status:** Proposed design (gap-analysis Group C)
**Parent:** `docs/crime-scene-book-notes.md` (Ch.10 gap: "no user-declared component
boundaries / pattern-violation check")

## Context

Ch.10's technique checks whether a codebase's *actual* dependency and change
patterns respect a *declared* layered architecture (e.g. "UI must not depend
on persistence"). Barad-dûr currently only ever **infers** structure — files
are bucketed into implicit "components" by directory-depth
(`extract_component`, `src/metrics/coupling/mod.rs:43`, driven by
`CouplingThresholds::component_depth`) and cross-component co-change is
flagged as a smell. There is no way for a user to *assert* the boundaries
they intend and have barad-dûr check reality against that intent. Of the five
gap-analysis groups this was expected to be the largest net-new subsystem.

**A discovery that changes the cost picture:** the collector already builds a
static import graph, `snapshot.import_graph: HashMap<PathBuf, Vec<PathBuf>>`
(populated by the tree-sitter AST pass), currently consumed only for afferent
/ efferent coupling and circular-dependency detection
(`src/metrics/coupling/mod.rs::compute_coupling`, lines 13-39). This is
exactly the "depends on" signal the book's chapter needs — far more direct
than temporal co-change — and it costs nothing new to collect. The net-new
work in this spec is the declaration + validation + classification layer on
top of an existing signal, not a new dependency-graph subsystem.

## Decisions

### 1. Declaration syntax: a new top-level `[architecture]` TOML section

Every other structurally-distinct config concern (`[weights]`, `[thresholds]`,
`[output]`, `[backfill]`) is its own top-level section in
`.repository-analysis/barad-dur.toml`
(`src/config/mod.rs::TomlConfig`). Architecture layers are lists of named
glob groups plus directional rules between them — structurally nothing like
`CouplingThresholds`'s flat scalar fields — so they get their own section
rather than being nested under `thresholds.coupling`:

```toml
# ──────────────────────────────────────────────────
# Architecture boundaries (optional — omit this section entirely
# to leave architecture conformance checking off)
# ──────────────────────────────────────────────────
[[architecture.layers]]
name = "ui"
paths = ["src/renderer/**"]

[[architecture.layers]]
name = "domain"
paths = ["src/metrics/**", "src/scorer/**"]

[[architecture.layers]]
name = "infra"
paths = ["src/collector/**", "src/cache/**"]

# Only listed (from -> to) edges are permitted between two DIFFERENT
# declared layers. Any cross-layer import not listed here is a violation.
# Edges within the same layer are always fine and never need listing.
[[architecture.allow]]
from = "ui"
to = "domain"

[[architecture.allow]]
from = "domain"
to = "infra"
```

Allow-list (whitelist) semantics, not allow/deny mix: simpler to reason
about, matches how layered-architecture linters elsewhere in the industry
usually work, and avoids a second axis of "is this rule even reachable" logic.
An **absent `[architecture]` section (no `layers` entries) is the default and
means the feature is fully inert** — see Decision 3.

### 2. What "depends on" means: reuse `snapshot.import_graph`, not temporal coupling

The static import graph is already computed for every file the AST pass
understands. A cross-layer **import edge** not covered by an `[[architecture.allow]]`
rule is a violation. This is a pure join against data already on the
snapshot — no new collector work, no new AST/dependency-graph machinery.
Cross-repo import analysis (`src/coupling/dependency.rs`) is a different
subsystem (the standalone multi-repo `barad-dur coupling` CLI command) and is
not reused here; it operates on repo-to-repo relationships, not intra-repo
file imports.

Temporal co-change is deliberately **not** the primary signal (unlike the
sibling Group B/D specs) — the import graph is a strictly better match for
"depends on" when it's available. Annotating violations with temporal
corroboration (a file pair that both violates the import rule *and*
co-changes across the boundary) is valuable additive evidence in the same
spirit as M5's `corroboration_degree`, but is explicitly deferred to Future
Work to keep this spec's scope to the core mechanism.

### 3. Opt-in correctness: absent config must never produce a finding

This is a hard correctness requirement, not a nicety: a repo that has never
heard of this feature must see zero behavior change. Mechanism: the
architecture-conformance `MetricValue` is only pushed into `compute_coupling`'s
`metrics` vec when `thresholds.coupling` — or rather the new
`RepoConfig.architecture.layers` — is **non-empty**; when empty, the metric
is omitted from the category entirely (not scored as a vacuous 100, not
included with 0 findings — genuinely absent, the same "not applicable"
treatment the `deps` category already gets when `weights.deps == 0`,
`src/config/mod.rs:54-56`). This composes for free with backfill: historical
snapshots have an empty `import_graph` regardless (ADR-005), so
`architecture_violations` naturally returns empty and the metric stays
omitted there too — no special-casing needed.

### 4. Score placement: a new sub-metric inside the existing Coupling category

Not a new top-level category. Rationale: an architecture violation *is* a
species of improper coupling — conceptually a sibling of
`change_coupling_smells`, which already lives in
`src/metrics/coupling/mod.rs`. A new top-level category would require a new
`weights` field and a config migration path (weights-sum-to-100 validation);
folding it into Coupling avoids that entirely. Scored via the existing
`metrics::score_count_bands` helper (same banding mechanism other
finding-count metrics use), so `scorer/types.rs::score_band`'s SSOT is
untouched.

### 5. Validation (`config::validate`, `src/config/mod.rs`)

- A rule's `from`/`to` referencing an undeclared layer name → `bail!` (same
  style as the existing weight-sum / ratio-range checks).
- A rule with `from == to` → `bail!` ("meaningless: layers may always depend
  on themselves").
- A layer with an empty `name` or empty `paths` → `bail!`.
- **Not** rejected at config-parse time: two layers whose globs can match the
  same file. Glob-overlap detection in general is not statically decidable
  from the pattern strings alone (depends on the actual file tree at analysis
  time). Instead this is handled at analysis time (Decision below) —
  documented in the generated TOML comment so users know overlapping globs
  are tolerated, not silently wrong.

### Ambiguous / unclassified files (analysis-time, not config-time)

- A file matched by **zero** declared layers → not part of the declared
  architecture, excluded from conformance checking. Most of a typical repo
  (tests, docs, build config) will fall here — this is expected, not an
  error.
- A file matched by **more than one** declared layer's globs → ambiguous,
  also excluded from conformance checking (never guessed at), and the
  ambiguous-file count is surfaced in the metric description text so a
  misconfigured overlap is visible rather than silently wrong.

## Architecture

```
snapshot.import_graph ──────────┐
config.architecture.layers ─────┼─► classify_layer(path, layers) -> LayerMatch
config.architecture.allow ──────┘        │
                                          ▼
                     for each (from_file -> to_file) edge:
                       match (classify(from_file), classify(to_file)) {
                         (One(a), One(b)) if a != b && !allowed(a, b) =>
                           ArchitectureViolation { from_file, to_file, a, b }
                         _ => skip   // same layer, unclassified, or ambiguous
                       }
                                          │
                                          ▼
                     architecture_conformance_metric() -> Option<MetricValue>
                       (None when `layers` is empty — Decision 3)
```

New submodule `src/metrics/coupling/architecture.rs`:

```rust
pub(crate) enum LayerMatch<'a> {
    None,
    One(&'a str),
    Ambiguous,
}

pub(crate) fn classify_layer<'a>(path: &Path, layers: &'a [ArchitectureLayer]) -> LayerMatch<'a>

pub(crate) struct ArchitectureViolation {
    pub from: PathBuf,
    pub to: PathBuf,
    pub from_layer: String,
    pub to_layer: String,
}

pub(crate) fn architecture_violations(
    snapshot: &RepoSnapshot,
    layers: &[ArchitectureLayer],
    allowed: &[(String, String)],
) -> Vec<ArchitectureViolation>

fn architecture_conformance_metric(
    snapshot: &RepoSnapshot,
    layers: &[ArchitectureLayer],
    allowed: &[(String, String)],
) -> Option<MetricValue>
```

`compute_coupling` (`src/metrics/coupling/mod.rs:13`) gains one more entry in
its `metrics` vec, built via `Iterator::chain`/`Option`-flattening so an
absent architecture config simply contributes nothing (consistent with the
existing "pure functions, no mutation" style — CLAUDE.md).

## Configuration

New types in `src/config/thresholds.rs` (or a new `src/config/architecture.rs`
if the pair of structs plus their `Vec` fields make `thresholds.rs` unwieldy —
implementer's call, no behavioral difference):

```rust
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ArchitectureConfig {
    #[serde(default)]
    pub layers: Vec<ArchitectureLayer>,
    #[serde(default, rename = "allow")]
    pub allowed: Vec<ArchitectureRule>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArchitectureLayer {
    pub name: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArchitectureRule {
    pub from: String,
    pub to: String,
}
```

`TomlConfig` gains `#[serde(default)] architecture: ArchitectureConfig`;
`"architecture"` is added to `unknown_top_level_keys`'s `KNOWN` list;
`RepoConfig` gains `pub architecture: ArchitectureConfig`, threaded through
`load()` and `merge_with_cli()` the same way `backfill` is today (no CLI
override needed for v1 — this is a declarative, not a per-run, setting).

## Interactions (deliberately untouched)

- **Backfill (ADR-005):** historical snapshots have an empty `import_graph`
  regardless of whether `[architecture]` is configured, so
  `architecture_violations` returns empty and the metric stays omitted —
  identical mechanism to the opt-out case, no special-casing required.
- **Gate ratchet:** a plain finding-count metric inside the existing Coupling
  category; whether it needs enumerating in the gate's per-kind diff list is
  a verification item for the implementation plan, not a design decision
  here (the gate already tolerates new Coupling sub-metrics without special
  wiring, per the M1–M4 Pressman milestones).
- **Corroboration (M5) / community detection:** explicitly not joined in v1
  (Decision 2) — noted as future work.

## Testing strategy (TDD throughout)

- **`classify_layer` unit tests:** single unambiguous match; no match
  (`None`); two overlapping layer globs on one path (`Ambiguous`).
- **`architecture_violations` unit tests:** edge within one layer → no
  violation; cross-layer edge covered by an `allow` rule → no violation;
  cross-layer edge with no matching rule → violation; edge touching an
  unclassified or ambiguous file → skipped, never a violation.
- **`config::validate` unit tests:** rule referencing an undeclared layer →
  error; self-referential rule → error; empty layer name/paths → error;
  default config (`architecture` absent) still validates cleanly.
- **Integration (`architecture_conformance_walking_skeleton.rs`):** fixture
  repo with a `ui/` file importing a `db/` file, three declared layers, only
  `ui→domain` and `domain→db` allowed → assert a violation finding for the
  direct `ui→db` edge. **The single most important test in this suite:** the
  same fixture analyzed with **no** `[architecture]` section produces zero
  architecture-related findings and the metric is absent from the report
  entirely — proves the opt-in guarantee (Decision 3), not just that the
  feature works when enabled.
- **Dogfood:** run against barad-dûr itself with a plausible boundary (e.g.
  `collector` may depend on nothing under `renderer`) as a sanity check that
  the existing layered-pipeline discipline (CLAUDE.md's own architecture
  section) reports clean.

## Risks & mitigations

- **Ambiguous-glob false negatives:** files matched by >1 layer are excluded
  rather than guessed at, and the ambiguous count is surfaced in the metric
  description — visible, not silent.
- **Config footgun (declared layers, forgotten legitimate edge → violation
  spam):** documented via generous inline comments in the TOML template
  (matching the `[weights]` section's existing documentation style);
  `barad-dur init --with-architecture` scaffolding is future work, not MVP.
- **Language coverage:** `import_graph` is only populated for the 8
  AST-supported languages (CLAUDE.md); files outside that set can never
  violate — acceptable, identical to how afferent/efferent coupling already
  behaves for those files today, not a new limitation introduced here.

## Future work (explicitly deferred)

- Temporal corroboration annotation on violations (M5-style additive
  evidence).
- `barad-dur init --with-architecture` layer scaffolding from existing
  directory structure.
- Per-rule severity/weight instead of uniform count-based scoring.
- Cross-repo architecture conformance (out of scope; single-repo only, like
  the rest of `analyze`).

## Estimated implementation size: **M**

New top-level config section (2 small structs + `validate()` rules), one new
pure submodule (`architecture.rs`, ~150–200 lines: `classify_layer`,
`architecture_violations`, the metric wrapper), no new collector work (reuses
`snapshot.import_graph`), ~10–15 unit tests plus one integration fixture.
Larger than the Group E heuristic bundle and likely Group B, but smaller than
initially feared going in — the "big new subsystem" this gap implied turned
out to already exist as `snapshot.import_graph`; the real net-new surface is
declaration, validation, and classification, not dependency analysis itself.
