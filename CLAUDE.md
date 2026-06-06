# barad-dur — Project Conventions

## Commands

```bash
cargo build --release                          # release build
cargo test                                     # run all tests
RUSTFLAGS=-D warnings cargo test               # as CI runs it (warnings are errors)
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check

make analyze                   # analyze current repo → dashboard/report.json
make html-report               # generate self-contained HTML report → report.html
make dashboard                 # start React dashboard dev server (pnpm, port 5173)
make setup                     # configure git hooks (run once after clone)
```

## Architecture

- `src/cli/mod.rs` — CLI args (clap). Subcommands: `analyze`, `gate`, `backfill`, `init`, `coupling`, `watch`, `contributors`
- `src/main.rs` — entry point; dispatches to `src/cmd/`
- `src/scorer.rs` — `AnalysisReport` + `build_report()`; `HotspotFile`, `CouplingPair`, etc.
- `src/renderer/html/` — 13 JS-generating `.rs` files + CSS; each tab is its own module
- `src/metrics/` — health, team, evolution, hygiene, complexity modules
- `src/collector/` — git snapshot collection
- `dashboard/` — React 19 + Vite + Tailwind 4; drag-and-drop report loader

## Gotchas

- Pre-push hook runs `cargo fmt --check`, `cargo clippy -D warnings`, and `cargo install --path .`. Run `make setup` to activate hooks after cloning.
- HTML renderer embeds all JS/CSS inline; no `innerHTML` allowed (security hook enforces this).

## Development Paradigm

This project follows the **functional programming** paradigm.

Concretely for Rust:
- Prefer pure functions (no side-effects, no mutation of inputs)
- Prefer iterator chains (`.map()`, `.filter()`, `.fold()`) over mutable loops
- Propagate errors with `?` and `Result<T, E>` rather than explicit `match`
- Use immutable bindings by default; add `mut` only when required
- New modules should follow the established `(snapshot) → value` pattern from `scorer.rs` and `metrics/`

## Mutation Testing Strategy

Hybrid — per-feature on push (scoped to files changed in last 25h, kill rate ≥ 80%), full-codebase nightly (scheduled CI job).
