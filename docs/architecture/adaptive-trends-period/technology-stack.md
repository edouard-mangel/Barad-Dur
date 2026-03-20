# Technology Stack — adaptive-trends-period (backfill)

**Date**: 2026-03-19
**Author**: Morgan (Solution Architect)

The backfill feature requires no new crate dependencies. Every technology decision extends an existing dependency already present in `Cargo.toml`.

---

## Stack Summary

| Concern | Technology | License | Role in backfill |
|---|---|---|---|
| Language | Rust (stable) | MIT / Apache 2.0 | All implementation |
| CLI parsing | clap 4.x | MIT / Apache 2.0 | New `Backfill` subcommand and `BackfillArgs` |
| Git object access | git2 (libgit2 binding) | MIT | SHA-targeted commit and tree traversal |
| Git blame | process::Command (std) | Rust stdlib | `git blame <sha> -- <file>` invocation |
| Serialization | serde + serde_json | MIT / Apache 2.0 | `source` field on `HistoryEntry`; `BackfillConfig` deserialization |
| Error handling | anyhow | MIT / Apache 2.0 | `Result<BackfillSummary>` propagation; warn-and-continue |
| Config format | toml (serde_toml) | MIT / Apache 2.0 | `[backfill]` section in `barad-dur.toml` |

---

## Technology Decisions

### git2 for `collect_commits_at` and `collect_files_at`

**Rationale**: `git2` is already a direct dependency used in `src/collector/libgit.rs` for `collect_commits` and `collect_files`. The SHA-targeted variants are straightforward extensions: replacing `revwalk.push_head()` with `revwalk.push(sha_oid)`, and replacing `repo.head()` with `repo.find_commit(sha_oid)?.tree()`. No new crate is needed; the libgit2 API directly supports object lookup by OID.

**Alternative considered**: `process::Command` calling `git log` and `git ls-tree`. Rejected because git2 is already present and provides typed, in-process access to git objects without subprocess overhead. Subprocess parsing would add fragility for a concern that git2 handles natively.

**License**: MIT (git2 crate) + libgit2 (GPL-2.0 with linking exception, permitting use as a library without GPL propagation).

### `process::Command` for blame at historical SHA

**Rationale**: The existing `src/collector/gitcli.rs` already drives git blame via `process::Command`, parsing the output line-by-line. Adding `at_rev: Option<&str>` and conditionally inserting `<sha>` before `--` in the command arguments is a minimal, low-risk change.

**Alternative considered**: `git2::Repository::blame_file()` with `BlameOptions`. The git2 blame API is lower-level (returns a `Blame` object requiring manual hunk iteration) and the existing CLI-based approach is simpler to extend for the `--rev` case. Switching to the git2 API would require rewriting the blame parsing logic, adding risk without benefit. The subprocess approach is well-tested in the existing codebase.

**Alternative considered**: `git show <sha>:<file>` per-file to reconstruct historical content, then blame in-memory. Rejected: introduces significant I/O overhead (one subprocess per file per SHA), and reading historical file content is specifically the approach ruled out by D-04 (non-destructive) and ADR-005.

### serde `skip_serializing_if` for `source`

**Rationale**: The `source: Option<String>` field must not appear in JSON output for live `analyze` entries. `#[serde(skip_serializing_if = "Option::is_none")]` achieves this without a custom serializer. The serde crate is already a dependency. Existing `trends.json` files without the field deserialize cleanly because serde fills absent optional fields with `None`.

### No new dependencies

Adding a crate dependency to a Rust project increases compile times, supply-chain attack surface, and maintenance burden. Every concern in the backfill feature is addressed by extending existing code paths:

- Commit traversal at a SHA: git2 (existing)
- File listing at a SHA: git2 (existing)
- Blame at a SHA: process::Command (existing)
- Config loading: serde + toml (existing)
- History writes: existing `append_if_new_head`
- Error handling: anyhow (existing)
- Pure sampling arithmetic: Rust std (no crate needed)
