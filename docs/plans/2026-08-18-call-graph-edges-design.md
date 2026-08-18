# Call-graph edges — design & testing plan

Response to `2026-08-18-call-graph-edges-planning-prompt.md`. Design + testing
strategy for function-level call-graph extraction; no code. All file references
verified against HEAD (`72cf6c5`).

**TL;DR recommendation** (details in §9): build a strictly scoped version —
TS/JS first, Rust second, *annotation-first* (no new score-bearing metric until
dogfooding shows the resolution rate earns trust). The honest value is
function-granularity attribution and liveness confirmation, not new coupling
discovery. If the roadmap currently favors breadth (existing metrics on more
languages) over depth, defer the whole feature; nothing here rots.

---

## 1. Proposed approach

### 1.1 Where it slots in

Extraction rides the existing single tree-sitter parse in
`src/metrics/complexity/mod.rs::analyse_source()` — one new field on
`SourceAnalysis`, one new tree-level extractor per language (mirroring
`class_records_from_tree`), gated per-language exactly like the inheritance
extractor is today:

```rust
pub struct SourceAnalysis {
    // …existing fields…
    pub call_sites: Vec<RawCallSite>,   // new
}
```

No second parse pass, ever. The per-language `*_CALLS` query lives in
`queries.rs` next to the existing `*_FUNCTIONS` queries, with the same
query-validity test pattern.

Resolution happens in the collector's phase-5 resolution step
(`snapshot_builder.rs::resolve_class_records` precedent): raw specifiers →
repo paths via the existing `import_resolver::resolve_specifier`, then
aggregation into counted edges. Everything downstream is pure
`(snapshot) → value` metric code.

### 1.2 What counts as a call, and what we refuse to guess

Extraction only claims what a syntax tree can actually know. Per call
expression, classify the callee:

| Callee shape | Example | Classification |
|---|---|---|
| Bare identifier bound by an import | `import { f } from './x'; f()` | `Specifier { specifier, name }` (aliases unwrapped, same as `RawBaseRef`) |
| Bare identifier, not import-bound | `f()` with local `function f` | `SameFile(name)` |
| Qualifier is an import binding | Rust `helpers::run()` with `use crate::helpers;` — Phase 2 | `Specifier` |
| Method on a value | `obj.method()`, `self.f()`, `this.f()` | `Unresolved { name: "method" }` |
| Computed / dynamic | `fns[k]()`, `(cond ? a : b)()` | `Unresolved { name: "<dynamic>" }` |
| Callback *reference* (not a call) | `arr.map(f)` | **not extracted** — no call edge exists at this site |

The last row is a deliberate stance on risk #1: a function passed as a value
is not a call we can attribute, and inventing an edge there is exactly the
false confidence the prompt warns about. It under-counts; it never fabricates.

`Unresolved` is *counted*, not discarded — unlike `BaseRef::Unresolvable`
(which only terminates a chain), an unresolved call retains its name and
participates in the per-file resolution-rate accounting (§4). "We saw 40
calls, resolved 28" is itself a trust signal the output must carry.

Caller attribution: each call site is attributed to its innermost enclosing
function declaration (matching what the `*_FUNCTIONS` query captures);
top-level / module-init calls attribute to the sentinel caller `"<toplevel>"`.
Nested functions attribute to the innermost, consistent with how
`FunctionMetrics` already treats nested declarations as their own entries.

### 1.3 Re-export (barrel) following

A callee resolved to a barrel file must be chased to the declaring file, or
function-level in-degree silently accretes on `index.ts`. The chase logic is
`metrics/coupling/inheritance.rs::resolve_key` — named hop, star hop,
cycle-cut. Plan: extract `resolve_key` into a shared helper
(`metrics/coupling/reexport.rs` or similar) parameterized over the record
index, used by both DIT and call-edge lookup. This is metric-time resolution
(like DIT), so the stored snapshot keeps the barrel-level `Resolved` target
and the knob-free chase stays live without re-collection — same rationale as
the existing "depth is computed at metric time" comment on `ClassRecord`.

## 2. Data model

### 2.1 Snapshot addition

```rust
/// One aggregated caller→callee edge in a file. Produced by the collector's
/// AST pass; HEAD-only (ADR-005 — absent from backfill snapshots).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallRecord {
    /// File containing the call site(s).
    pub path: PathBuf,
    /// Enclosing function name, or "<toplevel>".
    pub caller: String,
    pub callee: CalleeRef,
    /// Number of call sites aggregated onto this edge (the weight).
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CalleeRef {
    /// Callee assumed declared in the same file.
    SameFile(String),
    /// Callee bound by an import resolved to a project-local file
    /// (possibly a barrel — chased at metric time).
    Resolved { path: PathBuf, name: String },
    /// A real call whose target static analysis cannot name a file for:
    /// external package, method on a value, dynamic dispatch. Name kept
    /// for honest accounting; "<dynamic>" for computed callees.
    Unresolved { name: String },
}

// RepoSnapshot:
pub call_records: Vec<CallRecord>,
```

`CACHE_VERSION` in `src/cache/storage.rs`: 4 → 5. Old caches deserialize-fail
and re-collect; no migration code (established pattern).

### 2.2 Identity scheme: `(path, name)`, deliberately

No synthetic function ids. Function identity is `(PathBuf, String)` — the
same key the DIT resolver uses for classes and the only key `FunctionMetrics`
supports (it has `name`, no id). Consequences accepted and documented:

- **Overload/same-name collision** (two `impl` blocks with a `fn new`, TS
  method `render` on two classes in one file): their incoming edges merge.
  This *over*-groups within one file but never invents a cross-file edge.
  Mirrors the documented anonymous-class collapse in `inheritance.rs`
  ("a known bounded limitation; disambiguate by line if it ever matters").
- **No byte-offset/line in the key**: line numbers churn on every unrelated
  edit; a cache keyed on them would be misleadingly precise. Declaration line
  is available for *display* by joining against `FunctionMetrics` order, not
  for identity.

### 2.3 Cost class

- Extraction: one additional query execution per parsed file — O(AST nodes)
  per file, same class as the existing complexity/import queries riding the
  same tree. Expected well under 10% on top of the current AST pass (the
  parse dominates; measure on dogfood before/after with `-v` timing).
- Aggregation: O(call-sites) hashing into edges.
- Resolution: O(edges) hash lookups against the known-file set.
- Snapshot growth: O(distinct caller→callee pairs), not O(call-sites) —
  counts absorb repetition. For a barad-dûr-sized repo, thousands of edges,
  tens of KB of bincode. Memory/cache impact is minor; the bound worth
  stating is that pathological generated files are already excluded by the
  default exclusion layers before the AST pass runs.

## 3. Language rollout

**Phase 1: TS/JS. Phase 2: Rust. Nothing else committed.**

The inheritance resolver being TS/JS-only is not an accident to route
around — it is the scoping decision that made M7 shippable, and the same
logic applies with more force here:

- TS/JS is the only language where the full chain already exists:
  import-binding extraction (`import_bindings`), alias unwrapping, specifier
  resolution (`resolve_specifier`), and re-export chasing. Call extraction
  for TS/JS is a new query plus classification; resolution is reuse. It is
  also the ecosystem where barrels make file-granularity most misleading —
  the barrel-bypass liveness use case (§5.3) is TS/JS-specific.
- Rust is Phase 2 for dogfooding: barad-dûr analyzes itself in CI, and a
  function-hub signal the maintainers can eyeball against their own
  `scorer.rs`/`metrics/` is the fastest way to calibrate trust before any
  score depends on it. Rust's resolvable subset is honest and useful: bare
  calls and `path::to::fn()` calls whose head segment is `use`-bound (the
  import graph already resolves Rust `use` targets); every method call is
  `Unresolved` — which the resolution-rate accounting will visibly say.
- Python/Go/Java/C#/Kotlin: explicitly deferred. Python duck typing and the
  JVM/CLR languages' method-call-dominant style would push resolution rates
  low enough that the metric would mostly report its own uncertainty.
  Deferral is recorded in the metric output itself: files in unsupported
  languages contribute no records, and the metric guards report "no call
  data" rather than a fake 100 (the `detection_ran` / `score: None` pattern
  in `coupling/mod.rs`).

## 4. Representing and surfacing uncertainty

Three-state `CalleeRef` (§2.1) is the representation. Surfacing rules:

1. **Every consumer that shows an in/out-degree must show its basis.**
   Evidence strings name resolved counts explicitly, e.g.
   `build_report — function hub: 23 resolved incoming calls (7.2x median)`.
2. **Per-snapshot resolution rate** — `resolved_edges / (resolved_edges +
   unresolved_edges)` — computed once, carried in the metric description
   ("call resolution 71% — method/dynamic calls unresolved by design").
3. **Trust floor.** New threshold `call_resolution_floor` (default `0.5`).
   If the snapshot-wide resolution rate is below it, function-hub analysis
   returns `score: None` with description "call resolution below trust floor
   (41% < 50%)" — the same honest-degradation shape as "Coupling detection
   did not run". A metric built on mostly-unresolved data must say so, not
   guess (risk #1 head-on).
4. **Liveness annotation is one-directional.** A barrel-bypass finding with a
   matching resolved call edge gains "— live: called N time(s)"; absence of
   an edge adds *nothing* (never "dead" — an unresolved or callback-style use
   is invisible to us). Under-claiming is the only safe direction; identical
   philosophy to corroboration ("additive evidence, never folded into the
   score" — `cross_community_smell_count`).

## 5. Metrics: extended vs new

### 5.1 Extended — `god_objects` (health): dominant-function attribution

No score change. When a file is flagged as a structural hub, and the call
graph attributes ≥ `god_function_dominance` (default `0.6`, fraction) of its
resolved incoming references to one function, the reason string gains
`— driven by fn 'build_report' (18 of 23 resolved incoming calls)`. This
delivers the #1 promised value (which function *is* the hub) with zero risk
to existing scores, and its absence (low resolution, no dominant function)
degrades to today's output exactly.

### 5.2 New (later milestone, config-gated) — `god functions` (health)

Function-granularity analog of `is_structural_hub`, same shape:

```
health.god_function_min_in_degree      usize, default 8
health.god_function_degree_multiplier  f64,   default 4.0
health.call_resolution_floor           f64,   default 0.5
health.god_function_dominance          f64,   default 0.6   (used by §5.1)
```

Degree = resolved incoming call edges (distinct caller functions, not raw
count — a hot loop calling once per iteration is one caller); median taken
over all functions in source files. Validation in `config/mod.rs::validate()`
follows the `god_node_degree_multiplier` precedent exactly: multiplier
finite, > 0, ≤ 1e6; `min_in_degree` ≥ 1 (0 flags everything); floor and
dominance in `[0.0, 1.0]`, NaN rejected. Defaults and every rejection case
pinned by unit tests (existing pattern in `config` tests).

This metric ships **unscored first** (`score: None`, list-only evidence) and
only gains a score band after ≥ one release of dogfood observation — the
prompt's risk #1 makes "observe before scoring" a design requirement, not
caution theater.

### 5.3 Extended — barrel-bypass Content findings (coupling): liveness

Per §4.4. Matching rule: finding `source imports target directly` is live iff
some `CallRecord` in `source` has `Resolved { path: target, .. }` (post
barrel-chase). Purely additive to the evidence string; count and score
untouched.

### 5.4 Extended — afferent/efferent coupling (coupling): weighted display

Descriptions gain call-weighted medians alongside the existing binary-import
medians ("median weighted: 4.2 calls/edge") — display only. Re-scoring
coupling on weights would change every user's numbers on heuristic data;
that's a separate, later decision with its own evidence. Honesty about risk
#2: the *relationships* are already in `import_graph`; weight refines, it
does not discover.

## 6. ADR-005 boundary: preserved

Call-graph data is HEAD-only. `collect_snapshot_at()` (backfill) leaves
`call_records` empty exactly as it leaves `file_metrics` empty; every
consumer distinguishes "not collected" from "no calls" via the established
`detection_ran`-style guard (an empty `file_metrics` already marks a backfill
snapshot; call metrics reuse that same guard rather than inventing a second
sentinel). No trend lines, no backfill visibility, no ADR revision. Re-parsing
AST per sampled commit remains rejected for the same D-07 cost reasons —
and call extraction only *increases* that cost, strengthening ADR-005's
conclusion. If historical call graphs are ever wanted, that is the ADR's own
"future `--with-complexity` flag" discussion, a separate decision.

## 7. Renderer / output surface

- JSON report: new top-level `call_graph` section — resolution rate, edge
  counts by state, top function hubs (path, name, resolved in-degree). All
  consumers read `score_thresholds` as today; no new hardcoded bands.
- CLI/HTML: milestone-gated. M1–M2 expose JSON only; the HTML report gains a
  function-hub list inside the existing health tab later (template file per
  tab pattern in `renderer/templates/`, `include_str!` embedding, no
  `innerHTML`).

## 8. TDD & mutation-gate plan

TDD is mandatory: every item below is written and watched failing before its
implementation. The ≥ 80% `cargo mutants --in-diff` bar shapes the *style* of
assertion, per lesson already encoded in this repo's tests:

**Assertion rules (mutation-hardening):**
- Never assert only `!is_empty()` — pin exact record vectors, exact counts
  (`count: 2`, not `count >= 1`; kills `+= 1`→`= 1` and swap mutants).
- Every threshold gets both-sides boundary tests (`7 not flagged / 8 flagged`
  at the floor; multiplier at exactly `median * 4.0` vs just above — the
  `god_objects_boundary_*` pattern; kills `>`→`>=` mutants).
- Evidence strings asserted exactly (kills format-string and argument-order
  mutants) — the `"class C extends B → A (depth 2)"` pattern.
- Determinism: sorted-output test for every list derived from a HashMap
  (the `god_objects_list_is_sorted_for_determinism` pattern).

**Per milestone (integration suites follow `<feature>_walking_skeleton.rs` /
`<feature>_milestone_N.rs` naming):**

1. **M1 — TS/JS extraction (walking skeleton).**
   Query-validity tests for `JS_CALLS`/TS in `queries.rs` (existing pattern).
   Unit tests per syntactic form, one exact-expectation test each: bare local
   call → `SameFile`; imported call → `Specifier` with alias unwrapped;
   default-import call; method call → `Unresolved{name}`; computed call →
   `Unresolved{"<dynamic>"}`; `arr.map(f)` → **no record** (the refusal is a
   test, or a mutant deleting the guard survives); nested-function caller
   attribution; top-level → `"<toplevel>"`; two identical calls → one record
   `count: 2`. Plus the `analyse_source_matches_the_four_individual_extractions`
   consistency test extended to five channels. Integration:
   `call_graph_walking_skeleton.rs` — analyze `BARAD_DUR_TEST_REPO`, assert
   `call_records` in JSON output with the exact expected shape for a pinned
   fixture file.
2. **M2 — resolution + uncertainty accounting.** Specifier→path resolution
   (resolves / external stays `Unresolved` / unknown file `Unresolved`);
   barrel chase: named hop, aliased hop, star hop, barrel-of-barrels,
   cyclic barrels terminate (port the six `inheritance.rs` re-export tests to
   the shared helper — refactor under green); resolution-rate arithmetic
   pinned at exact fractions incl. 0-edge and all-resolved cases; trust-floor
   boundary both sides (0.49 → `score: None` with exact description, 0.50 →
   data present).
3. **M3 — god-object attribution (§5.1).** Dominance boundary both sides at
   0.6; low-resolution snapshot degrades to byte-identical current reason
   string; exact reason-string pinning.
4. **M4 — barrel-bypass liveness (§5.3).** Edge present → exact "— live:
   called 2 time(s)" suffix; edge absent → finding byte-identical to today
   (a test that today's string is *unchanged* kills any mutant inverting the
   annotation guard); resolved-to-barrel edge chases before matching.
5. **M5 — Rust extraction.** Same form-by-form suite: bare call, `use`-bound
   path call, method call → `Unresolved`, macro invocations excluded
   (documented: `format!` et al. are not call edges), `<toplevel>` for
   const/static initializers. Dogfood integration assertion on a real
   `src/scorer.rs` edge.
6. **M6 — config + new metric (§5.2).** Default-pinning test per new field;
   `validate()` rejection test per degenerate value (NaN, 0, negative,
   > 1e6, out-of-[0,1]); metric boundary tests cloned from the
   `god_objects` suite shape.

Each MR is one milestone, so `--in-diff` scopes the gate to code whose tests
were purpose-built above; the shard-timeout/aggregate-gate CI structure is
unchanged.

## 9. Risks vs payoff — and whether to build it now

| # | Risk | Position |
|---|---|---|
| 1 | Heuristic resolution → false confidence | Contained by design, not solved: refusal to fabricate edges (callbacks, dynamic), counted `Unresolved`, trust floor gating any score, annotation-only until dogfooded. The residual risk is users reading "resolved in-degree" as ground truth anyway — mitigated by resolution rate appearing in every surface, but not eliminable. |
| 2 | Marginal new information | Conceded up front: in statically-scoped languages the *relationships* are already in `import_graph`. This design claims only granularity (which function), weight (display-only), and liveness (one-directional). §5 deliberately adds **zero** score changes at launch. If that payoff list looks thin to the maintainer, that is a fair reason to defer — see below. |
| 3 | Language scope | TS/JS then Rust, others explicitly deferred with the metric reporting absence honestly. The inheritance precedent's scoping is followed, not routed around. |
| 4 | Cache & perf | O(call-sites) extraction on the existing parse, O(edges) storage, `CACHE_VERSION` 5, measured on dogfood at M1 with a stated abort criterion: if the AST pass slows > 15%, extraction gets a config gate before M2 proceeds. |
| 5 | Identity drift | Real and unresolved. This is one deliberate step toward code intelligence; the mitigation is that everything here remains snapshot-in, value-out, HEAD-only, zero new dependencies (no graph crate — edge lists + hash maps, like Louvain). The line this design refuses to cross: no symbol tables, no type inference, no per-line churn-sensitive ids. If a future feature needs those, that is the moment to say no. |

**Recommendation.** Build M1–M4 (TS/JS, annotation-first) *if* the current
priority is deepening trust in the existing coupling/hub signals — liveness
confirmation and function attribution make findings users already see more
actionable, which fits the tool's "trustworthy signal" identity. Hold M5–M6
until M1–M4 dogfood data shows the resolution rate on real repos clears the
trust floor comfortably.

Defer entirely if the near-term roadmap values breadth instead (e.g.
extending inheritance resolution to more languages, or registry/deps depth) —
risk #2 means nothing here is time-sensitive, and the planning prompt itself
is the durable artifact.

What would need to be true to green-light score-bearing god-functions (M6):
dogfood resolution rate ≥ ~70% sustained; the M3 attribution strings judged
accurate by maintainers on their own codebase; and at least one real user
request for function-level flagging — absent that, the annotation tier is
the whole feature.
