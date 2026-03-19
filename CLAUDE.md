# barad-dur — Project Conventions

## Development Paradigm

This project follows the **functional programming** paradigm.
Use @nw-functional-software-crafter for implementation.

Concretely for Rust:
- Prefer pure functions (no side-effects, no mutation of inputs)
- Prefer iterator chains (`.map()`, `.filter()`, `.fold()`) over mutable loops
- Propagate errors with `?` and `Result<T, E>` rather than explicit `match`
- Use immutable bindings by default; add `mut` only when required
- New modules should follow the established `(snapshot) → value` pattern from `scorer.rs` and `metrics/`

## Mutation Testing Strategy

per-feature — gate ≥80% kill rate before merge.
