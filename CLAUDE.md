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

hybrid — per-feature on push (scoped to files changed in last 25h, kill rate ≥ 80%), full-codebase nightly (scheduled CI job).


## gstate 
use the /browse skill from gstack for all web browsing, never use mcp__claude-in-chrome__* tools, and lists the available skills: /office-hours, /plan-ceo-review, /plan-eng-review, /plan-design-review, /design-consultation, /review, /ship, /land-and-deploy, /canary, /benchmark, /browse, /qa, /qa-only, /design-review, /setup-browser-cookies, /setup-deploy, /retro, /investigate, /document-release, /codex, /cso, /autoplan, /careful, /freeze, /guard, /unfreeze, /gstack-upgrade.
