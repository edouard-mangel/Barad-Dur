# ADR-001: Barad-dur Architecture Decision Record

**Status:** Accepted
**Date:** 2026-03-06
**Context:** Decisions made during the implementation of Barad-dur v1.0

---

## ADR-001.1: Layered Architecture with Shared Data Model

**Decision:** Use a layered architecture (Collector → Snapshot → Metrics → Scorer → Renderer) with `RepoSnapshot` as the shared data model passed between layers.

**Rationale:**
- Clean separation of concerns: each layer only depends on the snapshot, not on other layers
- Easy to add new metric categories without changing existing code
- Snapshot can be cached independently of metric computation
- Each metric function is a pure function `(snapshot) → MetricValue`, making testing trivial

**Alternatives considered:**
- (A) Pipeline architecture with streams — rejected because git data needs random access patterns
- (B) Plugin architecture — overkill for v1, revisit for v2

---

## ADR-001.2: Library + Binary Pattern

**Decision:** Use `lib.rs` + `main.rs` dual-target so all logic lives in the library crate.

**Rationale:**
- Enables `cargo test --lib` for fast unit testing without building integration tests
- Library can be consumed as a dependency by other tools
- `main.rs` is a thin CLI wrapper (~100 lines)

---

## ADR-001.3: git2 (libgit2) Primary + Git CLI Fallback

**Decision:** Use `git2` for commit walking and file tree enumeration. Use `git blame --porcelain` via CLI for blame data.

**Rationale:**
- `git2` provides fast, in-process access to commits and diffs
- `git2`'s blame API is significantly slower than the CLI for large files
- Porcelain format provides structured, parseable output with explicit field markers
- Blame collection uses `rayon` for parallelism across files

**Trade-off:** Depends on `git` being available in PATH for blame. Acceptable since target users are developers who always have git installed.

---

## ADR-001.4: Bincode for Snapshot Cache

**Decision:** Use `bincode` binary serialization for the snapshot cache stored at `.barad-dur/snapshot.bin`.

**Rationale:**
- Much faster than JSON for large snapshots (commits × files × blame lines)
- Compact on disk (~10x smaller than equivalent JSON)
- Automatic Derive via Serde — no custom serialization code needed
- Staleness detection via simple HEAD hash comparison (O(1) check)

**Trade-off:** Cache is not human-readable. Acceptable because cache is an optimization detail, not a user-facing artifact.

---

## ADR-001.5: Metric Scoring System (0-100)

**Decision:** Each metric produces a score from 0 to 100. Category scores are averages of their metrics. Overall score is a weighted average of categories.

**Weights:**
| Category | Weight | Rationale |
|----------|--------|-----------|
| Health | 30% | Core project sustainability |
| Team | 30% | People are the most important factor |
| Evolution | 20% | Growth patterns matter but less urgently |
| Git Hygiene | 20% | Important but least business-critical |

**Rationale:**
- Uniform 0-100 scale makes scores comparable across metrics
- Weighted categories reflect the relative importance of each dimension
- Top Actions generated from the 3 lowest-scoring metrics provide actionable output

---

## ADR-001.6: Time Window Default (6 Months)

**Decision:** Default analysis window is the last 6 months, configurable via `--since`, `--until`, or `--all`.

**Rationale:**
- 6 months captures recent activity patterns without being overwhelmed by ancient history
- Short enough to reflect current team dynamics, long enough for meaningful trends
- `--all` flag enables full history analysis when needed (e.g., for code age metrics)
- Time parsing supports relative specs (`3months`, `30days`) and ISO dates (`2024-01-01`)

---

## ADR-001.7: 17 Metrics Across 4 Categories

**Decision:** Implement 17 specific metrics organized into 4 categories.

### Health (5 metrics)
| Metric | Algorithm | Score Mapping |
|--------|-----------|---------------|
| Bus factor | Min authors for 50% file blame coverage | 1→20, 2→50, 3→75, 4+→100 |
| Churn hotspots | Top 5% files by commit frequency | >60% concentration→30, >40%→60, else→90 |
| Temporal coupling | File pairs with >70% co-change ratio | 0→100, 1-3→75, 4-8→50, 9+→25 |
| Stale code | Files with zero commits in window | >50%→25, >30%→50, >10%→75, else→100 |
| File complexity | Files >50KB or depth >5 | 0→100, 1-3→80, 4-8→60, 9+→40 |

### Team (5 metrics)
| Metric | Algorithm | Score Mapping |
|--------|-----------|---------------|
| Knowledge distribution | Gini coefficient of blame lines per author | >0.7→20, >0.5→50, >0.3→75, else→100 |
| Contributor activity | % of authors with commits in window | <30%→25, <50%→50, <70%→75, else→100 |
| Ownership clarity | % of files with >50% blame to one author | >80%→90, >60%→75, >40%→60, else→40 |
| Collaboration patterns | Directories where >80% blame is one author | Silo % thresholds |
| Merge patterns | Merge commit count and frequency | Context-dependent |

### Evolution (4 metrics)
| Metric | Algorithm | Score Mapping |
|--------|-----------|---------------|
| Growth trend | Net file count change in window | >50% change→40, >20%→65, else→90 |
| Refactoring ratio | Modification vs addition-only commits | Healthy balance→90 |
| Code age | Median blame line timestamp | 3-12 months→90 (sweet spot) |
| Commit cadence | Daily commit variance (coefficient of variation) | Regular→90, irregular→50 |

### Git Hygiene (3 metrics)
| Metric | Algorithm | Score Mapping |
|--------|-----------|---------------|
| Commit message quality | Length >10, capitalized, not "wip"/"fix" | >80%→90, <40%→30 |
| History cleanliness | Octopus merges + empty messages | Issues + merge % |
| Gitignore coverage | Tracked files matching suspicious patterns | 0→100, 1-2→70, 3-5→45, 6+→20 |

---

## ADR-001.8: Output Modes

**Decision:** Two output modes: colored CLI (default) and JSON (via `--json`).

**Rationale:**
- CLI output with colored score bars for human consumption
- JSON for CI/CD integration and programmatic consumption
- `--pretty` flag for human-readable JSON (debugging/exploration)
- `-o` flag for file output (both modes)
- Verbosity levels: default (categories only), `-v` (with metric details)

---

## ADR-001.9: Error Handling Strategy

**Decision:** Use `anyhow::Result` for error propagation with contextual error messages.

**Rationale:**
- `anyhow` provides clean error chaining without boilerplate
- Error messages are user-friendly ("'path' is not a git repository")
- Warnings go to stderr, data to stdout (proper Unix convention)
- Corrupt cache is silently deleted and rebuilt (graceful degradation)

---

## ADR-001.10: No AST Analysis in v1

**Decision:** v1 uses only git metadata (commits, blame, file tree). No source code parsing.

**Rationale:**
- Git metadata is language-agnostic — works for any project
- Keeps scope manageable for initial release
- File complexity uses size and depth as proxies (surprisingly effective)
- v2 will add tree-sitter for language-aware metrics (cyclomatic complexity, coupling, etc.)

---

## Test Coverage Summary

| Module | Unit Tests | Integration Tests |
|--------|-----------|------------------|
| snapshot | 10 | — |
| cli | 13 | — |
| collector (libgit) | — | 14 |
| collector (gitcli) | 2 | — |
| cache | 8 | — |
| metrics/health | 5 | — |
| metrics/team | 5 | — |
| metrics/evolution | 4 | — |
| metrics/hygiene | 3 | — |
| scorer | 5 | — |
| renderer/cli | 5 | — |
| renderer/json | 4 | — |
| end-to-end | — | 8 |
| **Total** | **64** | **22** |

---

## v2 Roadmap Considerations

1. **AST analysis via tree-sitter** — cyclomatic complexity, function length, import graph
2. **PR/merge request analysis** — review turnaround, approval patterns, comment density
3. **Historical trend tracking** — score over time via cached snapshots
4. **Configuration file** — `.barad-dur.toml` for custom thresholds and weights
5. **GitHub/GitLab integration** — fetch PR data via API
