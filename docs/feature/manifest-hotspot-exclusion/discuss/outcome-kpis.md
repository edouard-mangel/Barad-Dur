# Outcome KPIs — manifest-hotspot-exclusion

Outcome-based, measurable targets. These validate the *effect*, not just the code.

| KPI | Baseline (before) | Target (after) | How to measure |
|-----|-------------------|----------------|----------------|
| K-1: Manifest files in top-20 hotspots on a JS/TS repo | ≥ 1 (typically `package.json`) | 0 | Run `analyze` on a manifest-bearing repo; inspect hotspots |
| K-2: deps category output delta | n/a | 0 (identical) | Diff deps category JSON before/after on same repo |
| K-3: dependency-coupling pairs delta | n/a | 0 (identical) | Diff coupling pairs before/after on same repo |
| K-4: Manifest exclusion under defaults-off | n/a | Manifests present (unchanged behavior) | AC-4 test |

## Leading indicator
- All `is_excluded_*` manifest tests (AC-1..4) pass under
  `RUSTFLAGS=-D warnings cargo test`.

## Guardrail (must-not-regress)
- K-2 and K-3 are guardrail KPIs: if either shows a non-zero delta, the safety
  invariant (NFR-1) is violated and the change must be revisited before merge.

## Dogfood check
- Run `make analyze` on barad-dûr itself (a Rust repo with `Cargo.toml`): confirm
  `Cargo.toml` disappears from its own hotspots while `Cargo.lock` stays excluded as
  before.
