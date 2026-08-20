# Knowledge Loss — Design

**Date:** 2026-08-20
**Status:** Implemented 2026-08-20
**Closes:** the Ch. 13 partial gap in `docs/crime-scene-book-notes.md` ("No dedicated
'ex-developer knowledge loss' view, but the underlying ownership-by-author data needed
to build one is already computed")

## Context

Ch. 13 of *Your Code as a Crime Scene* pairs the knowledge map (main developer per
module — shipped) with a **knowledge-loss view**: recolor the code owned by developers
who have left, exposing the blind spots nobody can answer questions about.

The org-coupling hardening (MR !91) built the missing ingredient without aiming to:
blame lines whose author matches no in-window author now carry the
`UNKNOWN_AUTHOR` sentinel instead of collapsing onto author id 0. A line owned by
`UNKNOWN_AUTHOR` is precisely "written by someone not active in the analysis
window" — inactive or departed. The ownership view already renders these as
"(unattributed)"; dogfooding barad-dûr itself surfaced 14 such files. What's missing
is the *metric*: nothing scores or lists the loss.

## Decisions

1. **"Departed" = not active in the analysis window — a documented proxy.** The book
   tracks a named ex-developer's code; barad-dûr has no roster of who left. The
   sentinel gives a usable, honest proxy: an author with no commit in the window
   cannot answer questions *now*, whether they quit or merely moved teams. The window
   already defines "active" for `contributor_activity`; this reuses that meaning.
   Naming a specific ex-developer (the book's exact analysis) would need a
   `--departed <email>` style input — deferred future work, same posture as the
   org-coupling spec's team-mapping deferral.

2. **New Team metric, `knowledge_loss`, zero config.** The Team category owns
   people-and-process signals and already N/A's without blame. Bands are
   maintainer-authored on the repo-wide unattributed share of blamed lines,
   mirroring `bus_factor`'s percentage bands: `< 10% → 100`, `< 25% → 75`,
   `< 50% → 50`, else `25`. No new config fields; if teams want tuning it can
   graduate to `TeamThresholds` later with the standard default+validate pattern.

3. **Evidence: top affected files, exact shares.** `RawValue::List` of up to 10
   entries, sorted by unattributed share descending then path:
   `src/legacy/parser.rs — 83% unattributed (410 of 494 lines)`.
   Description: `12.0% of blamed lines lack an active author (3421 of 28510)`.

4. **Interactions.** Backfill (ADR-005): blame is always empty there → the existing
   "No blame data available" N/A shape, like every blame-dependent Team metric. The
   `MIN_TEAM_SIZE` gate applies as to all Team metrics (a solo repo's "loss" is not a
   team-coordination signal). No snapshot/cache change — pure `(snapshot) → value`
   over `blame_map`, which already carries the sentinel (CACHE_VERSION 6).

## Testing (TDD)

- Percentage bands: both sides of each boundary (9.9/10.0, 24.9/25.0, 49.9/50.0)
  via constructed blame maps; exact description strings.
- Evidence list: exact entry format, sort order (share desc, then path), top-10 cap.
- Zero unattributed lines → 100 with "0%" description; empty blame → N/A.
- `compute_team` wiring test: seventh metric present in both branches (N/A branch
  gains `na("Knowledge loss")`).
- Walking-skeleton addition is unnecessary: no collector change; unit tests plus the
  existing generic renderer path cover it (the org-coupling skeleton already proves
  Team metrics flow to JSON).
- Tooltip (`chrome.js` METRIC_TIPS) + action mapping (`actions.rs`, both functions —
  now directly pinned by tests after MR !92) wired in the same change.

## Estimated size

**S.** One pure function (~40 LOC) + wiring + ~8 unit tests + tooltip/action strings.
