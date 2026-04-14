# barad-dur — Project Conventions

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
