# User Stories — manifest-hotspot-exclusion

> JTBD was skipped (motivation is singular and clear). Implicit job:
> "When I scan a repo's hotspots, I want to see only real code, so I can target
> refactoring without mentally filtering out config files."

## US-1 — Exclude core manifests by default
**As** an engineer analyzing a repo
**I want** core-ecosystem manifests excluded from analysis by default
**So that** hotspots/complexity/coupling reflect code, not config churn.

- Priority: Must (S1)
- Maps to: FR-1, FR-3
- AC: AC-1, AC-2, AC-3

## US-2 — Preserve opt-out via existing toggle
**As** a power user who wants to inspect everything
**I want** `--no-default-excludes` to still surface manifests
**So that** disabling defaults behaves consistently across all default patterns.

- Priority: Must (S1)
- Maps to: FR-2
- AC: AC-4

## US-3 — No regression in dependency features
**As** a maintainer relying on the deps/CVE and coupling outputs
**I want** those outputs unchanged after manifests leave the snapshot
**So that** the cleanup carries no hidden cost.

- Priority: Must (S1)
- Maps to: NFR-1
- AC: AC-5

## US-4 — (DEFERRED) Re-include manifests without losing other defaults
**As** a user who values manifest churn visibility
**I want** to keep manifests in hotspots while still excluding lockfiles/generated dirs
**So that** I don't trade one kind of noise removal for another.

- Priority: Could (S2) — **deferred, open question for DESIGN (C-1)**
- Maps to: C-1
- AC: TBD in DESIGN
