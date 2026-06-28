# DISCUSS Decisions — manifest-hotspot-exclusion

## Key Decisions
- [D1] Manifest scope = **core ecosystems** (package.json, Cargo.toml, go.mod,
  *.csproj, pom.xml, build.gradle[.kts], requirements.txt, pyproject.toml):
  matches the languages barad-dûr already parses; avoids over-broad globs.
  (see: requirements.md → Core manifest set)
- [D2] Opt-out granularity = **deferred to DESIGN** as open question C-1. v1 accepts
  the existing all-or-nothing `use_defaults`. (see: requirements.md → C-1)
- [D3] No walking skeleton — pipeline already wired end-to-end; smallest valuable
  slice is the whole feature. (see: story-map.md → Walking skeleton)
- [D4] JTBD skipped — singular, clear motivation; would add ceremony. (see:
  user-stories.md preamble)
- [D5] Out of scope: richer manifest parsing + coupling-pair render suppression +
  churn-as-signal. (see: requirements.md → Out of scope)

## Requirements Summary
- Primary need: hotspots/complexity/coupling should reflect real code, not manifest
  config churn. Manifests are excluded at snapshot construction (like lockfiles
  already are), behind the existing `use_defaults` toggle.
- Walking skeleton scope: n/a (single slice S1).
- Feature type: cross-cutting (collection + all downstream metric surfaces).

## Constraints Established
- C-1 (OPEN → DESIGN): opt-out is all-or-nothing today; decide if a dedicated
  `exclude.manifests` toggle is warranted.
- C-2: consistent with `dist/` NOT being excluded — manifests are config, not output.
- C-3: `requirements.txt` doubles as pip "lock"; safe to exclude because deps reads
  it from disk.
- NFR-1 (safety invariant): deps/CVE and dependency-coupling outputs MUST be
  unchanged; both read from disk, verified.

## Upstream Changes
- None. DISCOVER was skipped; no prior assumptions to revise. Evidence base is the
  `Ideas.md` entry + codebase verification performed during DISCUSS.

## Handoff
- To: nw-solution-architect (DESIGN). Key artifacts to read: requirements.md,
  acceptance-criteria.md, story-map.md, outcome-kpis.md, discuss-verification.md.
- Carry-forward decision: resolve C-1 (opt-out granularity).
- Safety invariant (NFR-1) is verified with code citations in
  discuss-verification.md — DESIGN must preserve the disk-read independence.
