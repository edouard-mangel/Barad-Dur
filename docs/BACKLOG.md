# Barad-dur Backlog

## v2 — Planned

_Items actively being designed or scheduled for implementation._

(See `docs/plans/` for detailed designs once approved.)

## Performance — Blame Optimization

**Priority**: High (blame is 95% of runtime on large repos)
**Context**: See ADR-001.11 for full performance profile.

### ~~Per-Blob Blame Cache~~ ✓ Done

Implemented in `src/cache/blame.rs`. Blame output cached by blob OID in `.repository-analysis/blame_cache.bin`. `FileEntry.blob_oid` populated from tree walk. Cache is loaded, used, pruned, and saved during each collection cycle in `snapshot_builder.rs`.

### ~~libgit2 In-Process Blame~~ ✗ Investigated, Rejected (2026-04-08)

Investigated replacing `git blame --porcelain` subprocess with `git2::Repository::blame_file`. Benchmark and parity verification both failed:

- **barad-dur** (229 files, ~300 commits, Linux-native): libgit2 was **0.70x** (slower) than subprocess porcelain. 36% of files diverged on timestamps — `git blame` correctly attributes renamed lines to the rename commit, while libgit2 walks to the original commit where the line's pre-rename content was added.
- **FW.Runtime** (8306 non-binary files, 6118 commits, Linux-native): libgit2 processed only 600/8306 files in 47 minutes before being killed — catastrophically slow (>10 hour extrapolation). The subprocess path completes the same repo in seconds. Pathological files with deep edit histories dominate libgit2's runtime.

`BlameOptions::track_copies_same_file(true)` and `use_mailmap(true)` did not close the parity gap. The divergence is a fundamental difference in how libgit2's blame walker traverses parents vs git CLI's blame implementation (which has optimizations libgit2 lacks).

**Conclusion:** The per-blob blame cache already solves the incremental case. For cold runs, the subprocess path remains both faster and more compatible with git CLI semantics. Revisit only if a future version of libgit2 closes the perf gap.

### Selective Blame

For metrics that only need ownership of recently-changed code (churn hotspots), blame only files modified in the time window. Full blame still needed for bus factor and knowledge distribution, but could be deferred or sampled.

---

## Future — Not Yet Scheduled

### Interactive Config Editor

**Priority**: Nice-to-have
**Depends on**: `.barad-dur.toml` config file (v2 infrastructure)

A guided CLI command (`barad-dur init` or `barad-dur config`) that helps users create or edit their `.barad-dur.toml` configuration file interactively. Should cover:

- Architectural grouping: define component mappings (regex → component name) with live preview of how current files would be grouped
- Team mapping: assign authors to teams, with auto-suggestions based on email domains
- Metric thresholds: customize score thresholds and weights
- Validation: warn on invalid regex, unmapped files, unknown authors

Could be a TUI (e.g. `ratatui`) or a simple question-and-answer flow (e.g. `dialoguer`).

### Accessible Colors in HTML Report

**Priority**: Nice-to-have

Audit and update the color palette used in the self-contained HTML report (`src/renderer/html.rs`) for WCAG AA compliance. Targets:

- Score badge colors (red / orange / green) — verify contrast ratios against white/dark backgrounds
- Chart colors (D3 palette) — add patterns or labels as fallback for color-blind users
- Consider a `prefers-color-scheme` media query for dark-mode support

### Coupling Cluster Visualization

**Priority**: Nice-to-have

Add a graphical representation of file coupling in the HTML report to surface clusters of highly coupled files. Currently coupling is shown as a flat ranked list (`src/renderer/html.rs`). Targets:

- Force-directed graph (D3 `forceSimulation`) where nodes are files and edges are coupling pairs weighted by co-change frequency
- Visual clustering makes architectural boundaries (or their absence) immediately apparent
- Filter controls: minimum coupling threshold, show only top-N files

### ~~Exclude Files by Language / File Type~~ ✓ Done (v0.17.0)

`--exclude-ext <EXT>` CLI flag and `[exclude] extensions = [...]` TOML key added in v0.17.0. Supports bare extensions (`jar`), compound extensions (`min.js`), case-insensitive, leading dots normalised. Language-name aliases (e.g. `rust`, `python`) not yet supported — would map to a set of extensions.

### Reconsider Afferent/Efferent Coupling

**Priority**: Nice-to-have

Revisit the current afferent (Ca) / efferent (Ce) coupling metrics and their computation. Consider whether the existing implementation accurately reflects coupling direction, and whether instability (`Ce / (Ca + Ce)`) and abstractness should be surfaced as first-class metrics in the report.

### Detect Architecture Style to Determine Cross-Boundary Coupling

**Priority**: Nice-to-have

Automatically detect the architectural style of the repository (layered, hexagonal, feature-sliced, modular monolith, etc.) by analyzing directory structure and naming conventions. Use the detected style to configure which coupling relationships constitute cross-boundary violations — e.g. infrastructure importing from domain in hexagonal architecture. This would make coupling health scores context-aware rather than topology-agnostic.
