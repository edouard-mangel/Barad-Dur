# Technology Stack -- Cross-Repository Coupling Detection

## Date
2026-03-26

---

## 1. Existing Dependencies (Reused)

All existing dependencies are already in `Cargo.toml`. No version changes needed.

| Crate | Version | License | Usage in Coupling Feature |
|-------|---------|---------|--------------------------|
| `git2` | 0.19 | MIT + Apache-2.0 | Repo validation in discovery (open repo, check HEAD) |
| `clap` | 4 (derive) | MIT + Apache-2.0 | `CouplingArgs` struct with derive macros |
| `chrono` | 0.4 | MIT + Apache-2.0 | Timestamp comparison for temporal coupling, Duration for coupling window |
| `rayon` | 1 | MIT + Apache-2.0 | Parallel snapshot collection and parallel pair analysis |
| `indicatif` | 0.17 | MIT | Progress bar during collection and pairwise analysis |
| `serde` | 1 (derive) | MIT + Apache-2.0 | Serialization of CouplingReport and all types |
| `serde_json` | 1 | MIT + Apache-2.0 | JSON output (R2), package.json parsing (R2) |
| `colored` | 2 | MPL-2.0 | CLI output coloring |
| `anyhow` | 1 | MIT + Apache-2.0 | Error handling throughout coupling pipeline |
| `toml` | 0.8 | MIT + Apache-2.0 | Cargo.toml parsing for dependency coupling (R2) |
| `bincode` | 1 | MIT | Snapshot cache serialization (reused via existing collector) |

---

## 2. New Dependencies

**None required.**

All coupling feature needs are covered by existing dependencies. Specifically:
- Temporal coupling: `chrono::Duration` for window comparison, standard library binary search
- Team coupling: `String` operations (lowercase, comparison)
- Dependency coupling: `toml` and `serde_json` already available
- CLI output: `colored` already available
- HTML output: string templating (no template engine needed, follows existing html.rs pattern)
- Parallelism: `rayon` already available

---

## 3. Technology Decisions

### 3.1 Coupling Window Parsing

Parse `--coupling-window` value as `<number><unit>` (e.g., `24h`, `30m`). Use a simple hand-rolled parser (no regex crate needed). Convert to `chrono::Duration`.

**Rationale**: Adding a duration parsing crate (e.g., `humantime`) is not justified for a single parse point. The format is constrained to `<digits><h|m>`.

### 3.2 HTML Visualization (R3)

Self-contained HTML with inline JS. Force-directed graph implemented as minimal custom JS (~150-200 lines). No D3.js.

**Rationale**: D3.js minified is ~250KB. For a graph of up to 50 nodes and 1225 edges, a simple spring-layout algorithm suffices. The existing `renderer/html.rs` already uses this pattern (inline JS, no external libraries). If graph quality is insufficient at review time, D3.js can be inlined as a single string in a future iteration.

### 3.3 Manifest Parsing

| Manifest | Parser | Notes |
|----------|--------|-------|
| Cargo.toml | `toml` crate | Parse `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]` sections |
| package.json | `serde_json` | Parse `dependencies` and `devDependencies` objects |
| go.mod | Line parser | `require` block, simple line-by-line parsing |
| requirements.txt | Line parser | One package per line, strip version specifiers |

**Rationale**: No new crate needed. `toml` and `serde_json` are already dependencies. Go.mod and requirements.txt have simple enough formats for line parsing.

---

## 4. Development and Testing Tools

| Tool | Purpose | Status |
|------|---------|--------|
| `cargo test` | Unit and integration tests | Existing |
| `cargo-modules` | Verify module dependency direction (architectural enforcement) | Recommended, not yet added |
| `cargo-mutants` | Mutation testing per feature gate (>=80% kill rate) | Existing project convention |
| `assert_cmd` + `predicates` | CLI integration tests | Existing dev-dependencies |

---

## 5. No New Infrastructure

The coupling feature requires no new infrastructure:
- No databases (coupling results are computed on-the-fly)
- No servers or APIs (CLI tool only)
- No network access (local git repos only; GitLab group scan is future scope)
- No additional build steps (compiles with existing `cargo build`)
