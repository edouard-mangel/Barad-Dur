# Call-graph edges — planning prompt

Not a design doc — a self-contained prompt for handing to another agent (e.g.
Fable) to produce the actual design/implementation plan for function-level
call-graph extraction. Captures the context, architecture, and constraints
discussed while scoping the community-detection + structural-hub feature
(!81-!84), where call-graph edges were deliberately deferred as the harder,
third piece of graphify-inspired structural analysis.

Paste the block below as-is into a fresh agent session — it assumes no prior
context.

---

```
I need a detailed implementation plan (design + testing strategy, not code yet) for adding function-level call-graph extraction to barad-dûr, a Rust CLI repository-health analyzer. This is a real feature for an existing, mature codebase — study the architecture below before proposing anything.

## What the feature is

Barad-dûr already builds a file-level `import_graph: HashMap<PathBuf, Vec<PathBuf>>` from tree-sitter parsing, and uses it for afferent/efferent coupling, circular-dependency detection, Louvain-based community detection, and a "structural hub" signal on the god-objects metric (a file is flagged if its import-graph degree is a repo-median-relative outlier). All of that is file-granularity.

This feature adds **function-level call edges** — who calls whom, resolved across files where possible — to enable:
1. Function-granularity hub/god-node detection (today a whole file gets flagged even if only one function in it is the actual hub)
2. Weighted coupling — call frequency, not just binary import presence
3. Confirming existing barrel-bypass content-coupling findings are "live" (the bypassed import is actually called) rather than dead/unused

## Architecture you must work within

Pipeline: `CLI (clap) → Collector (git2 + git CLI) → RepoSnapshot → Metrics → Scorer → Renderer`. Read these files (or their equivalents if paths have shifted) before designing anything:

- `src/metrics/complexity/mod.rs` — `analyse_source()` / `SourceAnalysis`: everything is extracted from ONE tree-sitter parse per file. Any new extraction must slot into this single-parse pattern, not add a second parse pass.
- `src/metrics/complexity/queries.rs` — per-language tree-sitter query strings. Note that a `*_FUNCTIONS` query (function *declarations*) already exists for every supported language (Rust, JS/TS, Python, Go, Java, C#, Kotlin) — the missing piece is call *expressions* and cross-file *resolution*, not declaration discovery.
- `src/metrics/complexity/inheritance.rs` — the closest existing precedent for this exact problem class: resolving a name reference (`extends Base`) to its cross-file definition, with an explicit `BaseRef::{SameFile, Resolved{path,name}, Unresolvable}` enum that terminates gracefully on anything it can't resolve (external packages, re-export chains, ambiguity). **Load-bearing detail: this resolver is only wired up for TS/JS (`Language::JsTs`), not Rust or any of the other 6 languages**, even though it's a simpler problem than call resolution. That scoping decision is a real constraint you should reason about, not route around.
- `src/metrics/coupling/mod.rs` — `afferent_coupling`/`efferent_coupling` (median-relative scoring over `import_graph`), `community.rs` (hand-rolled single-level Louvain modularity, no external graph crate — this codebase deliberately avoids heavy dependencies).
- `src/metrics/health/god_objects.rs` — the existing structural-hub signal this feature would extend to function granularity: `is_structural_hub(degree, median_degree, thresholds)`, gated by a repo-median multiplier + absolute floor, both configurable.
- `src/snapshot/mod.rs` — `RepoSnapshot` (the cached, serialized data model — bincode, versioned via `CACHE_VERSION` in `src/cache/storage.rs`; any schema change is a cache-breaking change and must bump that constant), `FunctionMetrics{name, loc, cyclomatic_complexity, max_nesting_depth}` (already exists per-function, no unique id currently).
- `src/config/thresholds.rs` + `src/config/mod.rs::validate()` — the pattern for any new tunable: a `#[serde(default = "default_x")]` field with a `default_x()` fn, wired into `validate()` if it has a degenerate/dangerous value, with unit tests pinning both the default and the rejection cases.
- **`docs/adrs/ADR-005-backfill-skips-complexity-metrics.md`** — critical constraint: ALL AST-based analysis (import graph, coupling findings, complexity) is current-state-only, computed once against HEAD per `analyze`/`gate` invocation. It is explicitly excluded from `backfill` (the historical-trend sampler, which only recomputes Health/Team/Evolution/Hygiene across sampled past commits) because re-parsing AST at every sampled commit was judged too expensive. Your plan should state explicitly whether it preserves this boundary (call-graph data is HEAD-only, invisible to `backfill`/trends) or proposes revisiting ADR-005 — and if the latter, treat that as a separate, expensive sub-decision, not a given.

## Project conventions (non-negotiable, from CLAUDE.md)

- Functional programming paradigm: pure functions, no mutation of inputs, iterator chains over mutable loops, immutable bindings by default. New modules follow the `(snapshot) → value` pattern already established.
- **TDD is mandatory** — tests written and watched-failing before implementation, no exceptions.
- Mutation testing gate: `cargo mutants --in-diff` on the MR diff must hit ≥80% kill rate to merge. Your plan needs an explicit testing strategy per language/resolution-path that can realistically clear this bar, not just "add tests."
- `cargo fmt` / `cargo clippy --all-targets -- -D warnings` enforced in CI and via a pre-push hook.

## Known risks you must address head-on in the plan, not gloss over

1. **Static call resolution without a type-checker is heuristic, not ground truth.** Dynamic dispatch (trait objects, interfaces), Python duck typing, JS/TS structural typing, and callbacks-as-values all defeat naive resolution. A metric built on incomplete call data risks false confidence — worse than not having the metric, given this tool's whole value proposition is being a trustworthy signal. Your plan needs a concrete stance: how do you represent "resolved" vs "unresolved but real" vs "genuinely doesn't exist" calls, and how does that show up in output (see `BaseRef::Unresolvable` and `CouplingFinding.evidence` for existing precedent on being honest about uncertainty)?
2. **Marginal new information in statically-scoped languages** — you generally can't call a function without importing it, so `import_graph` already captures most of the *relationship*. The real incremental value is weight/frequency and function-level granularity, not discovering brand-new coupling. Your plan should be honest about where the value actually is, not oversell it.
3. **Language scope** — given the inheritance-resolution precedent only covers TS/JS, propose a concrete rollout order (which 1-2 languages first, and why) rather than committing to all 8 at once. Justify the choice against where barad-dûr's own dogfooding/user base would see the most value.
4. **Cache and performance cost** — new `RepoSnapshot` fields, a `CACHE_VERSION` bump, and added AST-pass work on every `--no-cache` collection. Quantify or at least reason about the cost class (is this O(files), O(functions), O(functions × call-sites)?).
5. **Identity drift** — this pushes toward code-intelligence/IDE territory rather than barad-dûr's current "fast git-history-centric health scorer" identity. Your plan should note this tension explicitly, even if it doesn't resolve it.

## What I want back

A written design document covering: proposed approach and data model (what gets added to `RepoSnapshot`, how it's shaped, id scheme if any), language rollout scope and order with justification, how resolution uncertainty is represented and surfaced, which existing metrics get extended vs. what new metric/threshold config is added (name, default, validation), the ADR-005 boundary decision, a concrete TDD/testing plan capable of clearing the mutation-testing gate, and a clear-eyed final section weighing the 5 risks above against the payoff — including whether you'd actually recommend building this now, or what would need to be true first.
```
