# Wave Decisions Summary -- Cross-Repository Coupling Detection DESIGN wave

## Feature
multi-repository-analysis

## Date
2026-03-26

---

## Key Architecture Decisions

### AD-01: Modular monolith with new `coupling/` module (not separate crate)
**Decision**: All coupling code lives in `src/coupling/` within the existing barad-dur crate. Renderers live in new files under `src/renderer/`.
**Rationale**: Solo developer, single repo. Full reuse of Collector, RepoSnapshot, cache, and renderer patterns. Module boundary rules enforce separation without crate-level split.
**ADR**: ADR-008

### AD-02: Skip blame and complexity for coupling snapshots
**Decision**: Coupling snapshot collection uses `skip_blame=true` and skips complexity analysis. Only commits (timestamps, authors) and file tree are needed.
**Rationale**: Blame is the slowest collection phase (60-80% of collection time). Coupling analysis only consumes commit timestamps and author names. Skipping blame reduces per-repo collection from 30-60s to 2-5s for uncached repos.

### AD-03: Lightweight coupling window parser (no new dependency)
**Decision**: Parse `--coupling-window` as `<digits><h|m>` with a hand-rolled parser, converted to `chrono::Duration`.
**Rationale**: A single parse point does not justify adding a crate like `humantime`. The format is constrained.

### AD-04: Minimal inline JS for HTML graph (no D3.js)
**Decision**: R3 HTML visualization uses a lightweight inline force-directed layout in JS (~150-200 lines), not D3.js.
**Rationale**: Graph has max 50 nodes. Existing html.rs uses inline JS. D3.js adds 250KB. Can upgrade to inline D3 later if needed.

### AD-05: Combined score formula (weighted average, temporal-heavy)
**Decision**: Combined coupling score = temporal * 0.50 + team * 0.25 + dependency * 0.25. Only computed when all three dimensions are available (R2+).
**Rationale**: Temporal coupling is the strongest signal of operational coupling (CI breakage correlation). Team and dependency coupling are supporting signals. Weights are configurable for different organizational contexts.

### AD-06: GitLab extensibility via pluggable discovery
**Decision**: CouplingConfig includes a conceptual `source` field. R1-R3 support only local directory scanning. The pipeline accepts `Vec<RepoSnapshot>` regardless of discovery mechanism.
**Rationale**: Designing the pipeline around `Vec<RepoSnapshot>` makes the discovery layer pluggable. A future `--gitlab-group` flag would add a new discovery module without changing the analysis or rendering layers.

### AD-07: No modification to existing types
**Decision**: RepoSnapshot, AnalysisReport, CouplingPair (in scorer.rs), and all other existing types remain unchanged. Coupling introduces its own type hierarchy in `coupling/types.rs`.
**Rationale**: NFR-03 (backward compatibility) and CC-05 (existing RepoSnapshot not modified). Complete isolation prevents regressions in `analyze`, `gate`, and `backfill` commands.

### AD-08: Parallel pair analysis via rayon
**Decision**: Pairwise coupling computation is parallelized using rayon's `par_iter` over the pair iterator.
**Rationale**: 50 repos = 1225 pairs. Each pair comparison is independent (read-only access to snapshots). rayon is already a dependency. Parallelism keeps 1225-pair analysis under 60 seconds (NFR-01).

---

## Open Questions Resolved

| OQ | From DISCUSS | Resolution |
|----|-------------|------------|
| OQ-1 | Coupling window granularity | Support hours (`h`) and minutes (`m`). Default: `24h`. |
| OQ-2 | Parallel pair analysis | Yes, via rayon. |
| OQ-3 | Cross-repo file-level coupling | Out of scope (computationally prohibitive). |
| OQ-4 | Graph library for HTML | Inline lightweight JS, no D3.js. |
| OQ-5 | GitLab group scan extensibility | Pipeline accepts Vec<RepoSnapshot>; discovery is pluggable. |
| OQ-6 | Combined coupling score formula | Weighted average: temporal 50%, team 25%, dependency 25%. |

---

## Quality Gates Checklist

- [x] Requirements traced to components (each FR mapped to a module)
- [x] Component boundaries with clear responsibilities (10 modules documented)
- [x] Technology choices in ADRs with alternatives (ADR-008)
- [x] Quality attributes addressed (correctness, performance, maintainability, testability, backward compat)
- [x] Dependency-inversion compliance (analysis modules depend on types, not on renderers)
- [x] C4 diagrams (L1 System Context + L2 Container, Mermaid)
- [x] Integration patterns specified (reuse of Collector, skip-blame optimization)
- [x] OSS preference validated (zero new dependencies, all existing are OSS)
- [x] AC behavioral, not implementation-coupled
- [x] No external integrations (local tool, no third-party APIs in R1-R3)
- [x] Architectural enforcement tooling recommended (cargo-modules)
- [ ] Peer review completed (pending)

---

## Handoff Package

### To acceptance-designer (DISTILL wave)

| File | Purpose |
|------|---------|
| `architecture-design.md` | Full architecture with C4 diagrams, pipeline design, quality attributes |
| `data-models.md` | All coupling types with field definitions |
| `component-boundaries.md` | Module layout, responsibilities, dependency constraints |
| `technology-stack.md` | Technology decisions with rationale |
| `wave-decisions.md` | This file -- 8 architecture decisions, 6 OQ resolutions |
| `ADR-008` | Why extend CLI vs. separate tool |

### Key points for acceptance-designer
1. All types in `coupling/types.rs` -- acceptance tests should assert on `CouplingReport` structure
2. Pipeline is: discovery -> collection -> analysis -> rendering
3. R1 scope: temporal coupling only, CLI output only
4. `barad-dur analyze` behavior must not change (backward compatibility gate)
5. Coupling pairs sorted by score descending, minimum 3 co-changes to report

### Key points for software-crafter
1. Follow functional paradigm: pure functions, iterator chains, immutable inputs
2. Pattern: `(Vec<RepoSnapshot>, CouplingConfig) -> CouplingReport`
3. Skip blame in coupling collection (`skip_blame=true`)
4. Use `rayon::par_iter` for parallel pair analysis
5. All types derive `Debug, Clone, Serialize`
6. Module dependency rules documented in component-boundaries.md
