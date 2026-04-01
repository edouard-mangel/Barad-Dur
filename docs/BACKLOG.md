# Barad-dur Backlog

## v2 — Planned

_Items actively being designed or scheduled for implementation._

(See `docs/plans/` for detailed designs once approved.)

## Performance — Blame Optimization

**Priority**: High (blame is 95% of runtime on large repos)
**Context**: See ADR-001.11 for full performance profile.

### ~~Per-Blob Blame Cache~~ ✓ Done

Implemented in `src/cache/blame.rs`. Blame output cached by blob OID in `.repository-analysis/blame_cache.bin`. `FileEntry.blob_oid` populated from tree walk. Cache is loaded, used, pruned, and saved during each collection cycle in `snapshot_builder.rs`.

### libgit2 In-Process Blame

Replace `git blame --porcelain` subprocess spawning with libgit2's `Blame::new()` API. Eliminates ~8k fork/exec calls. Trade-off: libgit2 blame can be slower per-file for files with very long histories, but removes the process spawn overhead that dominates wall-clock time on multi-core machines.

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
