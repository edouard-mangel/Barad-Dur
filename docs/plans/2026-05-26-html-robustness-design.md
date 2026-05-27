# HTML Report Robustness — Design

## Context

Three issues were identified in the HTML report pipeline and surrounding infrastructure, approved for implementation in this order:

1. **A — Production error hardening**: Two panic-capable code paths in non-test code
2. **B — HTML empty-state completeness**: Missing empty-state coverage for several tabs + styling gap
3. **C — File splits**: Three oversized files that conflate concerns

---

## Approach A — Production error hardening

### Problem

`registry/client.rs:15` — `http_with_timeout()` calls `.expect("failed to build HTTP client")`. The reqwest client builder can fail (e.g., bad TLS config), and `.expect()` panics. This is in a library function called from production code, not a test-only path.

`src/init.rs` (prompt / version-check path) — I/O operations (`stdin().read_line()`) panic via `.unwrap()` or equivalent; a broken terminal causes a hard crash instead of a graceful degradation.

### Solution

- Change `http_with_timeout()` signature to `-> Result<reqwest::blocking::Client, reqwest::Error>`
- Update `http()` (the `OnceLock`-based singleton) to propagate or log errors
- Update all call sites to handle `Result`
- In `init.rs`, replace panicking I/O with graceful fallback (skip version check on I/O failure, log to stderr)

### Design constraints

- TDD: tests first
- No change to public `http()` API contract if call sites already handle errors; otherwise propagate `Result` up the call chain
- The `OnceLock` singleton path is trickier — `get_or_init` does not support fallible init; may need to switch to `OnceLock<Option<Client>>` or `OnceLock<Result<Client, ...>>`

---

## Approach B — HTML empty-state completeness

### Problem

The `safeRender` error boundary (added in this session) wraps all tab builders, but its error div has no dedicated CSS class, making it hard to style or test. Also, several tabs have no test coverage for the empty-state path (when the report field is `null` / empty array).

Tabs and their empty-state status:
| Tab | Empty-state code | Test |
|-----|-----------------|------|
| Treemap | ✅ "No hotspot data" | ❌ |
| Authors | ❌ missing `.no-data` class | ❌ |
| Coupling | ✅ "No coupling data" | ❌ |
| Ownership | ✅ "No ownership data" | ❌ |
| Deps | ✅ "No dependency data" | ❌ |
| Audit | ✅ "No audit data" | ❌ |

### Solution

1. Add `tab-error` CSS class to the `safeRender` error div (already planned)
2. Fix the Authors tab to render a `.no-data` div when `R.git_authors` is empty
3. Add 6 tests in `tests_extra.rs` — one per tab — asserting the empty-state string is present when the corresponding field is absent/empty

---

## Approach C — File splits

### Problem

Three files have grown beyond a single responsibility:

| File | Size | Issue |
|------|------|-------|
| `src/renderer/html/js_treemap.rs` | ~200 lines | Mixes layout algorithm with UI rendering |
| `src/scorer/audit.rs` | ~300+ lines | Mixes parsing, scoring, and aggregation |
| `src/cli.rs` | ~150+ lines | Mixes flag definitions with subcommand dispatch |

### Solution

**`js_treemap.rs`** → split into:
- `js_treemap_layout.rs` — squarify algorithm, coordinate math
- `js_treemap_ui.rs` — DOM construction, tooltip, color mapping

**`scorer/audit.rs`** → split into a `scorer/audit/` directory:
- `mod.rs` — re-exports, `AuditReport` struct
- `parse.rs` — raw manifest parsing
- `score.rs` — per-dependency scoring logic
- `aggregate.rs` — roll-up across the report

**`src/cli.rs`** → split into:
- `src/cli.rs` — top-level `Cli` struct + subcommand enum
- `src/cli/analyze.rs` — `AnalyzeArgs` struct + all analysis flags

### Design constraints

- No behavior change; pure refactor
- All existing tests must pass after split
- Module re-exports keep public API identical
