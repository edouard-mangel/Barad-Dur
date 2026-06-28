# Journey: Analyst reviews hotspots on a manifest-heavy repo

**Feature**: manifest-hotspot-exclusion
**Persona**: Tech-lead / engineer running barad-dûr to find refactoring targets
**Depth**: Lightweight (happy path + the one real decision)

## Backbone

```
Run analysis ──▶ Open hotspots ──▶ Interpret list ──▶ Act on real targets
```

## Happy path (after this feature)

| Step | Action | System output | Emotion |
|------|--------|---------------|---------|
| 1 | `barad-dur analyze .` on a JS/TS (or polyglot) repo | Snapshot collected; manifests dropped at collection | Neutral |
| 2 | Open hotspots (CLI / HTML / dashboard) | Top-N list contains only source/logic files | Confidence ↑ |
| 3 | Read the ranked files | No `package.json` / `Cargo.toml` row competing with real code | Trust ↑ |
| 4 | Pick a target to refactor | Targets are actionable code, not config | Momentum |

## Before this feature (the push)

At step 2–3 the analyst sees `package.json` ranked among hotspots. It is high-churn
(dependency bumps, version edits) but it is declarative config — nothing to refactor.
The analyst mentally filters it out every run. Repeated noise erodes trust in the
ranking and risks burying a genuine code hotspot just below the fold.

## Emotional arc

```
confidence
   ^                         ┌──────── trust (after)
   │                    ┌────┘
   │   noise (before) ──┘ . . . . . . . frustration plateau
   └─────────────────────────────────────────▶ steps
        run        open       interpret      act
```

The arc only diverges at **interpret**: removing manifest noise turns a
filter-it-out-myself moment into a trust-the-list moment.

## Error / edge paths

- **`--no-default-excludes` (or `exclude.use_defaults=false`)**: manifests reappear
  in hotspots — expected, consistent with all other defaults being off.
- **Repo with manifests but the deps/coupling features**: those still read manifests
  from disk (`collector/deps.rs`, `coupling/dependency.rs`) — unaffected. The analyst
  sees no regression in the deps category or dependency-coupling pairs.
- **Monorepo with nested manifests**: `**/`-prefixed globs drop nested copies too,
  mirroring the existing lockfile patterns.
