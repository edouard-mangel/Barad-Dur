# Pressman Coupling M7 — Inheritance Coupling — Design

**Date:** 2026-07-15 (design started 2026-07-10)
**Status:** Approved design, pending implementation plan
**Parent design:** `2026-07-02-pressman-coupling-design.md` (§ Future work)

## Goal

Add a fourth rung to the Pressman coupling ladder: **inheritance coupling** —
deep class-inheritance chains, where a class depends on the implementation of
every ancestor above it. A new `CouplingKind::Inheritance` variant with a
TS/JS detector, severity banding, a config knob, and automatic participation
in everything M1–M6 built: corroboration, hotspot badges, the gate ratchet,
trend counts, and refactoring actions.

## Decisions

Locked during design review (2026-07-10 → 2026-07-15):

1. **Deep chains only: DIT ≥ 2, project-local.** A class is flagged when its
   Depth of Inheritance Tree over *project-local* edges reaches
   `inheritance_min_depth` (default 2). `class B extends A` alone is idiomatic
   OO, not a defect; the rung targets chains where a change to a root ripples
   through grandchildren. Bases outside the repo (framework classes, npm
   packages) terminate the chain — external depth is not counted. Rejected:
   flagging every cross-file `extends` (depth 1) — too noisy to act on, and
   it would punish ordinary subclassing.
2. **TS/JS only; Rust emits zero findings.** Rust has no implementation
   inheritance — traits are interface inheritance, and trait-object indirection
   is not the ripple hazard this rung measures. Zero-findings-until-supported
   matches the parent design's precedent for the other five languages.
   Rejected: modeling Rust `impl Trait for` chains (measures the wrong thing).
3. **Ladder position: Content ≻ Common ≻ Inheritance ≻ Control.** Deep
   inheritance is implementation dependence on code you don't own locally —
   worse than a flag parameter, better than shared mutable state. The enum
   variant is declared between `Common` and `Control` to keep the
   "ordered worst → least severe" doc contract true. Rejected: below Control
   (understates the fragile-base-class ripple) and above Common (a deep
   hierarchy is at least visible in the type system; a mutated global is not).
4. **Config knob `inheritance_min_depth`** in `[thresholds.coupling]`,
   default `2`; `0` disables the rule; `1` is **rejected by validation**
   (it would contradict decision 1 by flagging all cross-file inheritance).
   Follows the `corroboration_weight` validation precedent.
5. **Approach A: collector records class facts, metric computes the
   hierarchy.** The existing tree-sitter pass emits flat per-class records;
   depth resolution is a pure metric-time computation. Consequence: changing
   `inheritance_min_depth` takes effect without `--no-cache`, and the
   collector stays a fact-recorder. Rejected: resolving depth at collection
   time (bakes a threshold into the cache) and a full import-graph-only
   heuristic (the existing `import_graph` has no symbol information —
   file-level edges cannot see which class extends which).
6. **Gate needs no `Option`.** The gate recomputes baseline counts live from
   the base-SHA snapshot with the current binary (`src/cmd/gate.rs`), so both
   sides of the ratchet always know about the new rung —
   `CouplingFindingCounts` gains a plain `inheritance: usize`. Old-data
   compatibility lives only where old data lives: trend history
   (`HistoryCounts`) gains `inheritance_coupling: Option<usize>` following the
   existing per-kind `#[serde(default)]` pattern, so pre-M7 entries read back
   as `None` ("not measured"), never `Some(0)` ("measured clean").
7. **Every qualifying class is flagged independently.** In a chain
   `C → B → A` with threshold 2, `C` is a finding (depth 2) and `B` is not
   (depth 1). A deeper chain yields one finding per class at/above the
   threshold — matching how the other rungs count per-occurrence, and giving
   the count-based bands a meaningful signal. Rejected: one finding per chain
   root (hides how wide the deep tail is).

## Architecture

Same pipeline split as M1: facts in the **collector**, judgment **pure** in
`metrics/coupling`.

```
Collector (existing tree-sitter TS/JS pass, zero extra parses)
    → per-class records (name, extends-target)
    → snapshot_builder: resolve import specifiers → PathBuf
        → RepoSnapshot.class_records: Vec<ClassRecord>   [CACHE_VERSION 1 → 2]
            → metrics/coupling: memoized-DFS depth → CouplingFinding
                → all_coupling_findings (M6 seam) → actions/gate/hotspots/trend
```

### Data model (`src/snapshot/`)

```rust
/// One `class … extends …` site in a TS/JS file.
pub struct ClassRecord {
    pub path: PathBuf,
    pub line: usize,
    pub class_name: String,
    pub base: BaseRef,
}

pub enum BaseRef {
    /// Base class declared in the same file.
    SameFile(String),
    /// Base imported from a resolved project-local file.
    Resolved { path: PathBuf, name: String },
    /// Imported but the specifier didn't resolve to a project file
    /// (npm package, framework), or a non-identifier extends expression
    /// (`extends mixin(Base)`); terminates depth counting.
    Unresolvable,
}

pub enum CouplingKind { Content, Common, Inheritance, Control } // worst → least
```

- Extraction: the TS/JS tree-sitter query captures `class_declaration` /
  `class` expression nodes with a heritage clause. An identifier base that
  matches an import binding becomes a specifier; specifier → `PathBuf`
  resolution happens in `snapshot_builder`, reusing the import-resolver logic
  that builds `import_graph`. Classes without `extends` produce no record.
- `CACHE_VERSION` bumps 1 → 2: appending a snapshot field can decode from an
  old cache as structurally valid but silently empty — the explicit bump
  forces re-collection.
- `CouplingKind` serializes by variant name, so JSON reports are additive;
  bincode compatibility is covered by the version bump.

### Depth computation (`src/metrics/coupling/`)

Pure, memoized DFS over `class_records`, keyed by `(path, class_name)`:

- `SameFile`/`Resolved` edges follow to the ancestor's record; a missing
  record, `Unresolvable`, or an external base terminates the chain (depth
  stops there).
- Cycles (`A extends B extends A`, possible in erroneous code) are cut with an
  in-progress marker — a cycle member's depth counts only its acyclic prefix.
- A class with depth ≥ `inheritance_min_depth` yields a `CouplingFinding` with
  the class's real declaration line and evidence like
  `class C extends B → A (depth 2)`.
- Findings flow through `all_coupling_findings`, so M5 corroboration
  (path-based, kind-agnostic) and M6 actions pick them up with no new wiring.

## Severity, scoring & config

- **Bands** (`score_pressman`, maintainer-authored like the existing three;
  between Common's harshness and Control's leniency — the floor never
  triggers the ≤ 25 category cap on its own):

  | count | 0 | 1–2 | 3–6 | > 6 |
  |-------|---|-----|-----|-----|
  | score | 100 | 70 | 55 | 40 |

- New `pressman_metric` row: label **"Inheritance coupling"**, deep-class-
  hierarchy description. Corroboration weighting applies unchanged.
- M6 severity index becomes Content = 0, Common = 1, Inheritance = 2,
  Control = 3; ordering/advice inherit automatically.
- Config: `inheritance_min_depth = 2` in `CouplingThresholds`, the TOML
  template, and `barad-dur init`; validation per decision 4. Because depth is
  metric-time, the knob is live without `--no-cache`.
- Gate ratchet: fourth tuple `("inheritance", baseline.inheritance,
  head.inheritance)` — no skip logic (decision 6).

## Renderers & dashboard

M6's design keeps this section thin: action text is baked in Rust and both
HTML and dashboard render it verbatim, and metric rows flow automatically.

- **Action arm** (`scorer/actions.rs`): label `inheritance`, advice
  *"Deep inheritance chain: favor composition over inheritance, or flatten
  the hierarchy."* — CLI/HTML/dashboard inherit it via `text`.
- **Hotspot badges:** `HotspotFile` gains `inheritance_findings: usize`
  (builder counting tuple becomes a 4-tuple); both badge renderers
  (`templates/hotspots.js`, `HotspotsView.tsx`) add an `Ih n` label next to
  `Cn/Cm/Ct`, guarded (`|| 0` / `?? 0`) for old reports.
- **Deliberate non-changes:** the red badge highlight and the hotspot-score
  boost both stay `content + common > 0` — per the ladder, inheritance
  renders muted like Control and does not inflate hotspot ranking.
- **Dashboard types:** `inheritance_findings?: number`, optional like its
  siblings.
- **JSON:** additive only — new count field, new variant name; no renderer
  reads `coupling_finding_counts` (gate + JSON consumers only).

## Testing strategy

- **Extraction unit tests** (`metrics/complexity/pressman.rs`): same-file
  extends → `SameFile`; imported base → specifier; `extends mixin(Base)` /
  computed expression → `Unresolvable`; no-extends class → no record;
  `export default class extends X` captured; Rust file → zero records.
- **Depth & findings** (testutil snapshots): depth 0/1 not flagged; depth 2
  flagged with real line + chain evidence; unresolvable/external bases
  terminate; `A ↔ B` cycle cut without hang; diamond hierarchies exercise
  memoization; knob `0` disables, `3` raises the bar.
- **Integration arms**, one small test each: band boundaries (0→100, 1→70,
  2→70, 3→55, 6→55, 7→40 — exact boundaries kill `cargo mutants` band
  mutants); corroboration weight applies; severity order + advice arm
  (extends the actions table test); hotspot `inheritance_findings` counting +
  boost stays untriggered; ratchet reports an `("inheritance", …)` increase;
  config validation rejects `1`, accepts `0` and `2`; `HistoryCounts` old
  JSON → `None`.
- **E2E (`tests/pressman_coupling_milestone_7.rs`)**, M6 fixture shape:
  `a.ts` (class A), `b.ts` (B extends imported A), `c.ts` (C extends imported
  B), plus a Rust trait impl. Assert via `analyze --json`: exactly one
  finding, kind `Inheritance`, correct line, chain evidence;
  `coupling_finding_counts.inheritance == 1`; Rust contributed nothing;
  an inheritance action ordered after common, before control. A second
  warm-cache run asserts the finding persists — pinning that the
  `CACHE_VERSION` bump actually re-collects instead of serving a
  pre-M7-shaped snapshot.
- **Dashboard:** `HotspotsView.test.tsx` — `inheritance_findings: 2` renders
  `Ih 2`; an old report without the field renders unchanged.
- **Mutation gate:** per-MR `cargo mutants --in-diff` ≥ 80% applies; the
  boundary-value tests above are designed to kill band/comparison mutants.

## Risks & mitigations

- **Silent cache no-op** (the subtlest failure mode): bincode can decode an
  old snapshot as valid-but-empty if the field addition is append-only.
  Mitigated by the explicit `CACHE_VERSION` bump plus the warm-cache E2E
  assertion.
- **Import-resolution gaps:** TS/JS specifier resolution (extensions, index
  files, re-exports) is heuristic. An unresolved base degrades gracefully —
  it terminates the chain (under-count), never fabricates depth (over-count).
  Barrel re-exports of base classes are a known under-count, acceptable for
  v1 and listed as future work.
- **Enum-variant insertion:** inserting `Inheritance` mid-enum touches every
  exhaustive match. Rust makes this compiler-guided (each missing arm is a
  build error); JSON is name-based; bincode is version-bumped.
- **Band bikeshedding:** band values are a marked maintainer decision point,
  editable without touching logic; tests assert the agreed boundaries so a
  deliberate change is a conscious test edit.

## Future work (explicitly deferred)

- Other OO languages (Python/Java/C#/Kotlin) — same `ClassRecord` shape, per-
  language heritage queries; zero findings until then.
- Barrel/re-export–aware base resolution (removes the known under-count).
- Method-level override density (distinguishing deep-but-passive hierarchies
  from deep-and-overriding ones).
