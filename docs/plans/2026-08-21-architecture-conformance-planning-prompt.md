# Architecture conformance (Ch. 10) — planning prompt

Not a design doc — a self-contained prompt for handing to another agent (e.g.
Fable) to produce the actual design for architecture-pattern conformance
checking, the last unimplemented *Your Code as a Crime Scene* chapter in
`docs/crime-scene-book-notes.md` (Ch. 10, "Use Beauty as a Guiding
Principle"). Everything else in the tracker is ✅ as of 2026-08-21.

Paste the block below as-is into a fresh agent session — it assumes no prior
context.

---

```
I need a detailed design document (design + testing strategy, not code yet) for adding architecture-conformance checking to barad-dûr, a Rust CLI repository-health analyzer. This is a real feature for an existing, mature codebase — study the architecture below before proposing anything.

## What the feature is

Tornhill's Ch. 10 generalizes temporal-coupling analysis to *declared* architectural boundaries: the user names their intended pattern (layered, pipes-and-filters, hexagonal, feature modules…), provides a transformation mapping files to logical components, and the tool flags temporal coupling that violates the pattern's promise — a Views layer that is supposed to be swappable but co-changes with Models 75% of the time, two "independent" services that always move together. The value over what barad-dûr already has is the DECLARED part: today every boundary the tool reasons about is inferred (top-level directory prefix or Louvain communities), so it can say "these co-change across directories" but never "this violates the architecture you claim to have".

## Architecture you must work within

Pipeline: `CLI (clap) → Collector (git2 + git CLI) → RepoSnapshot → Metrics → Scorer → Renderer`. Read these before designing anything:

- `src/metrics/coupling/mod.rs` — the Coupling category (10 metrics as of the safety-net MR). `extract_component(path, depth)` + `CouplingThresholds.component_depth` (default 2) is the current "component" notion: a path-prefix truncation. `qualifying_smell_pairs` is the single source of truth for "a meaningful cross-boundary co-change" (ratio ≥ change_coupling_min_ratio, both files have history, different components). Any conformance check MUST reuse or extend this pairing, not invent a fourth co-change definition (the repo has consolidated these twice already — M5's extraction and the trends-M3 review both exist because copies drifted).
- `src/metrics/coupling/community.rs` — Louvain-style community detection over the import graph, currently used only as corroborating evidence ("these two files also sit in different import communities"). The obvious Ch. 10 play: DISCOVERED communities vs DECLARED components — a declared component whose files scatter across many import communities, or a community straddling two declared components, is a conformance finding with two independent evidence sources (imports + co-change).
- `src/collector/exclude.rs` + `src/collector/ignore_file.rs` — the `.baraddurignore` precedent: repo-root dotfile, full gitignore semantics via the `ignore` crate, bytes fingerprinted into cache staleness (`src/cache/staleness.rs`). The declared-architecture input needs a home: a new dotfile (`.baraddurarch`?), a section in `.repository-analysis/barad-dur.toml`, or something else. Study how `barad-dur init` generates the TOML and how exclusion deliberately stayed OUT of the TOML (file-pattern inputs live in dotfiles with gitignore semantics; scalar knobs live in TOML) before choosing — and note the mapping must be shareable/reviewable in the repo, which argues for a checked-in file.
- `src/config/thresholds.rs` + `src/config/mod.rs::validate()` — every tunable follows #[serde(default)] + validate() + a default-pinning test.
- `src/snapshot/mod.rs` — `file_change_pairs: Vec<(PathBuf, PathBuf, usize)>` (lexicographically normalized pairs), `import_graph`, `known_paths()`. Everything a conformance check needs is already collected; this feature should require NO new collector work. If your design wants per-commit windowing, `Commit.files_changed` + timestamps are in every snapshot (see metrics/coupling's growing_coupling_reach for the metric-time windowing pattern).
- `src/scorer.rs::build_report` + `src/renderer/` — how findings surface: category metrics (MetricValue with score + RawValue::List evidence), per-row structures on the report (CouplingPair.cross_boundary), and the HTML tabs (templates in src/renderer/templates/, one JS file per tab, report-smoke jsdom test clicks every tab).

## Project conventions (non-negotiable, from CLAUDE.md)

- Functional paradigm: pure `(snapshot, config) → MetricValue`, no I/O in metrics — so the declared mapping must be parsed at collection/config time and travel on config or snapshot, never read from disk inside a metric.
- TDD; per-MR `cargo mutants --in-diff` ≥ 80% kill rate — testing strategy must name both-sides boundary tests and exact-value assertions.
- Annotation-before-scoring doctrine (established across M5 corroboration, call-graph, trends M1–M3): new signals ship unscored (`score: None`, excluded from category averages) until dogfooding justifies bands. State explicitly whether conformance violations should EVER score, given they depend on user-declared intent being current.
- New actions.rs match arms need pin tests in the same MR (a mutation-gate failure taught this).
- Spec → plan → TDD → MR; specs in docs/superpowers/specs/, plans in docs/superpowers/plans/.

## Design questions you must answer head-on

1. **Mapping format.** What does the user write? Candidate shape: ordered `component-name: glob` rules (first match wins, gitignore-style globs via the `ignore`/`globset` crates already in-tree), plus optional pattern declarations (`layers: [ui, domain, data]` with allowed-dependency direction, or `independent: [svc-a, svc-b]`). Decide the minimal v1: is declaring components + "these should not co-change" enough, or does v1 need dependency-direction rules? Justify against Tornhill's actual Ch. 10 method (his transformations are exactly "regex → logical name" files).
2. **What is a violation, exactly?** Candidates: (a) cross-declared-component pairs from qualifying_smell_pairs (co-change evidence); (b) import edges crossing a forbidden direction (static evidence); (c) declared component vs Louvain community mismatch (structural drift evidence). Pick which ship in v1 and how they combine — the corroboration precedent (co-change corroborated by imports weighs more) is the house style.
3. **Relationship to the existing cross-component machinery.** `change_coupling_smells` already flags cross-component co-change using path-prefix components. Does the declared mapping REPLACE the prefix heuristic when present (component_depth becomes the fallback), or run as a parallel metric? Beware: replacing changes existing scores for users who add a mapping — is that the point, or a surprise?
4. **Staleness and honesty.** A declared architecture can be aspirational or rotten. The tool must not present conformance as ground truth: consider surfacing "N% of files matched no declared component" prominently (unmapped-file ratio as a first-class output, maybe the gate for scoring at all), and never scoring when coverage is below a floor.
5. **Cache interaction.** If the mapping lives in a repo file, must its bytes join the exclusion fingerprint in cache/staleness.rs? (Answer is probably yes if anything derived from it is stored in the snapshot, and no if it's applied purely at metric time from config — prefer the design that keeps it OUT of the snapshot cache.)
6. **Gate integration.** Should `barad-dur gate` be able to fail a pipeline on new conformance violations (the Pressman ratchet precedent in src/cmd/gate.rs), and if so in v1 or later?

## Known risks

1. **Config UX is the feature.** If declaring the mapping is tedious, nobody does it and the feature is dead code. Consider `barad-dur init --arch` scaffolding a mapping from the top-level directory layout as a starting point.
2. **False authority.** A violation report against a stale declaration misleads more than the inferred heuristics it replaces. The unmapped-ratio guard (question 4) and unscored-first doctrine are the mitigations — say how they interact.
3. **Scope creep toward a full architecture DSL.** Tornhill's transformations are deliberately dumb (regex → name). Resist dependency-rule languages until the dumb version proves insufficient; state what v2+ would add and why v1 excludes it.

## What I want back

A written design document covering: the mapping file format and location with a worked example for barad-dûr itself (its own CLAUDE.md architecture section names the intended components — use them); the v1 violation definition and which evidence sources combine; replace-vs-parallel decision for the existing prefix heuristic; the unmapped-ratio honesty mechanism; scoring stance per the annotation-first doctrine; cache/staleness impact; milestone split (walking skeleton first — the `<feature>_walking_skeleton.rs` then `<feature>_milestone_N.rs` test convention); a TDD/mutation plan naming boundary tests; and a final section on what v1 deliberately does not do.
```
