# Contributor Deduplication

## Problem

The same person committing under two different emails (e.g. `alice@old.com` and `alice@company.com`) appears as two separate contributors throughout the report — inflating contributor counts, splitting ownership and bus-factor metrics, and distorting knowledge distribution scores.

Git's `.mailmap` mechanism already resolves this at the commit level, but blame data bypasses it entirely: `git blame --porcelain` returns raw pre-mailmap emails, which the blame parser fails to match against the mailmap-resolved author list, silently falling back to author ID 0.

## Goals

1. Make `.mailmap` the single source of truth for identity resolution across the entire pipeline (commits + blame).
2. Provide a `barad-dur contributors` subcommand that detects suspected duplicates and generates ready-to-use `.mailmap` entries.

---

## Part 1 — Fix the Mailmap Gap in Blame

### Root cause

`collect_commits()` in `src/collector/libgit.rs` applies mailmap via `resolve_author()`, which maps raw emails to canonical ones. The resolved `AuthorId` is stored, but the original raw email is discarded. Later, `collect_blame()` in `src/collector/gitcli.rs` parses `author-mail` from `git blame --porcelain` output — a raw email — and looks it up in `email_to_id`. If the raw email differs from the canonical one, the lookup fails and blame lines are misattributed to author 0.

### Fix

During commit traversal, record both the raw (pre-mailmap) email and the resolved `AuthorId` in a second map: `raw_email_to_id: HashMap<String, AuthorId>`. Store it in `CommitCollection` alongside the existing `authors` vec. The blame parser consults `raw_email_to_id` first, falling back to `email_to_id`, so aliases resolve transparently.

### Files

| File | Change |
|------|--------|
| `src/collector/types.rs` | Add `raw_email_to_id: HashMap<String, AuthorId>` to `CommitCollection` |
| `src/collector/libgit.rs` | In `collect_commits_from_revwalk()`, record raw email → AuthorId before mailmap resolution |
| `src/collector/gitcli.rs` | Accept `raw_email_to_id`; try it before `email_to_id` in blame line parsing |
| `src/collector/snapshot_builder.rs` | Thread `raw_email_to_id` from commit collection to blame collection |

---

## Part 2 — `barad-dur contributors` Subcommand

### Behaviour

Scans commit history and groups authors by display name (case-insensitive). Any group with more than one distinct email is flagged as a suspected duplicate. Outputs the canonical form and suggested `.mailmap` entries.

```
Suspected duplicates:

  Alice Smith
    alice@company.com   42 commits  last: 2026-03-15
    alice@old.com        8 commits  last: 2024-11-20

  Suggested .mailmap entry:
    Alice Smith <alice@company.com> <alice@old.com>

Run with --write to append to .mailmap
```

The canonical email is chosen as the one with the most recent commit. All other emails for that name become aliases.

**`--write` flag:** appends generated entries to `.mailmap` in the repo root (creates the file if absent). Entries already present are skipped.

### Detection heuristic

Exact display name match, case-insensitive. No fuzzy matching in v1 — keeps false positives low.

### Files

| File | Change |
|------|--------|
| `src/cli.rs` | Add `Contributors { write: bool }` variant to the `Command` enum |
| `src/contributors.rs` | New module: `detect_duplicates()`, `format_report()`, `write_mailmap()` |
| `src/main.rs` | Dispatch `Command::Contributors` to the new module |

---

## Testing

- Unit tests in `src/contributors.rs`: grouping logic, canonical email selection, mailmap entry formatting, skip-if-present logic.
- Integration test: repo with two commits from different emails but same display name → `contributors` command reports one suspected duplicate with correct suggested entry.
- Existing blame tests should continue to pass unchanged (the fallback chain is additive).
