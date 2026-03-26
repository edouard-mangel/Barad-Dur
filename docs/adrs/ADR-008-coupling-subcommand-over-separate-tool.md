# ADR-008: Extend CLI with coupling subcommand (not a separate tool)

## Status
Accepted

## Date
2026-03-26

## Context

Cross-repository coupling detection is a new capability for barad-dur. The feature requires:
- Git repository validation and data collection
- Commit history analysis across multiple repos
- CLI output formatting
- HTML report generation

Two approaches were considered: (1) add a `coupling` subcommand to the existing `barad-dur` binary, or (2) create a separate tool (e.g., `barad-dur-coupling` or `repo-coupler`).

## Decision

Extend the existing `barad-dur` CLI with a `Commands::Coupling(CouplingArgs)` variant. The coupling pipeline lives in a new `coupling/` module within the same crate.

## Alternatives Considered

### Alternative A: Separate binary (`barad-dur-coupling`)

- **Pros**: Clean separation, independent release cycle, smaller binary per tool
- **Cons**: Cannot reuse Collector, RepoSnapshot, TimeWindow, cache, renderer patterns without extracting a shared library. Duplicates git2/rayon/clap dependencies. Users must install and update two tools. The shared data model (RepoSnapshot) would need to be a third crate.
- **Rejection rationale**: Solo developer, single repo. The overhead of maintaining a multi-crate workspace or separate repo exceeds the benefit. Conway's Law: one developer, one binary.

### Alternative B: Workspace with shared crate

- **Pros**: Code reuse via shared `barad-dur-core` crate, separate binaries
- **Cons**: Workspace setup overhead, separate release management, additional CI configuration. The coupling feature depends heavily on RepoSnapshot which contains git2 types that are not easily extracted.
- **Rejection rationale**: Premature modularization. The coupling module boundary within a single crate provides sufficient separation. Workspace split can happen later if team grows.

## Consequences

### Positive
- Full reuse of Collector, RepoSnapshot, TimeWindow, cache, renderer patterns
- Single `cargo install` for users
- Single binary to distribute
- Coupling module can directly consume `RepoSnapshot.commits` and `RepoSnapshot.authors` without serialization boundaries
- Existing test infrastructure (assert_cmd) works unchanged

### Negative
- Binary size increases (~5-10% estimate, mostly from new coupling logic and HTML template)
- All barad-dur releases include coupling code even when not used
- Risk of coupling between existing analysis and new coupling code (mitigated by strict module boundary rules)

### Risks and Mitigations
- **Module boundary erosion**: Mitigated by documented dependency constraints and recommended `cargo-modules` enforcement
- **CLI surface area growth**: Mitigated by clean subcommand separation (coupling is a distinct top-level command, not flags on analyze)
