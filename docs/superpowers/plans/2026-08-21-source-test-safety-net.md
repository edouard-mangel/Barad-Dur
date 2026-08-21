# Source/Test Safety Net Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Aggregate the existing per-pair `is_test_pair` badge into a scored Coupling metric that flags source files whose paired test file has stopped co-changing (Crime Scene Ch. 9's safety-net erosion signal).

**Architecture:** Extract the stem-based test-pairing predicate from `scorer/builders/coupling.rs` into `metrics/file_role.rs` as the single source of truth, then add a `test_safety_net.rs` submodule under `src/metrics/coupling/` (the `community.rs`/`inheritance.rs` pattern) computing `(snapshot, thresholds) → MetricValue` from `file_change_pairs` + `commits_by_file` — no new collection.

**Tech Stack:** Rust; existing snapshot data only.

**Spec:** `docs/superpowers/specs/2026-08-18-source-test-coupling-design.md`

## Global Constraints

- Functional paradigm: pure `(snapshot) → MetricValue`, no I/O in metrics (CLAUDE.md).
- TDD: watch every test fail before implementing; per-MR `cargo mutants --in-diff` ≥ 80% kill rate — both-sides boundary tests and exact-value assertions required.
- New tunables follow `#[serde(default)]` + `config::validate()` + default-pinning test (`src/config/thresholds.rs` pattern).
- New match arms in `src/scorer/actions.rs` MUST get pin tests in the same MR (MR !93's gate failure precedent).
- Score bands only via `score_count_bands` / `scorer/types.rs` — never hardcoded.
- Metric count assertion: `compute_coupling` currently returns **9** metrics (`compute_coupling_returns_nine_metrics`); this plan makes it **10**.
- Commits: write message to a file and use `git commit -F <file>` (a hook injects trailers into `-m`); never mention AI in messages.

---

### Task 1: Extract `is_test_pair` to `file_role.rs` (pure refactor + parity tests)

**Files:**
- Modify: `src/metrics/file_role.rs` (add public predicate at the end, before `#[cfg(test)]`)
- Modify: `src/scorer/builders/coupling.rs:11-33` (delete `file_stem`/`is_test_of`/`is_test_pair`, call the moved fn)
- Test: `src/metrics/file_role.rs` (`#[cfg(test)] mod tests`, existing module)

**Interfaces:**
- Produces: `pub fn is_test_pair(a: &Path, b: &Path) -> bool` in `crate::metrics::file_role` — consumed by Task 3 and by `scorer/builders/coupling.rs`.

- [ ] **Step 1: Write the parity tests in `file_role.rs`** — port every existing case from `scorer/builders/coupling.rs` tests (`is_test_pair_detects_suffix_test`, `_dot_test_spec`, `_underscore_test_spec`, `_test_prefix`, `_case_insensitive`, `_rejects_unrelated_pairs`) against the new location, with `Path` arguments:

```rust
#[test]
fn is_test_pair_parity_with_scorer_builder_cases() {
    use std::path::Path;
    let p = |s: &str| Path::new(s);
    assert!(is_test_pair(p("user.go"), p("user_test.go")));
    assert!(is_test_pair(p("parser.ts"), p("parser.spec.ts")));
    assert!(is_test_pair(p("api.py"), p("test_api.py")));
    assert!(is_test_pair(p("Widget.cs"), p("widget_tests.cs")));
    assert!(is_test_pair(p("a/b/mod.rs"), p("a/b/mod_test.rs")));
    assert!(!is_test_pair(p("user.go"), p("order_test.go")));
    assert!(!is_test_pair(p("a.rs"), p("b.rs")));
}
```

- [ ] **Step 2: Run to verify it fails** — `cargo test --lib file_role` → FAIL: `is_test_pair` not found.
- [ ] **Step 3: Move the implementation.** Copy `file_stem`, `is_test_of`, `is_test_pair` bodies verbatim from `scorer/builders/coupling.rs:11-33` into `file_role.rs`; only the outer signature changes to `Path`:

```rust
/// Stem-based source↔test pairing (user.go ↔ user_test.go, parser.ts ↔
/// parser.spec.ts, …). Single source of truth shared by the coupling-pair
/// badge and the safety-net metric — extracted per the M5 precedent so two
/// call sites can't drift on what "a test pair" means.
pub fn is_test_pair(a: &Path, b: &Path) -> bool {
    let (Some(a), Some(b)) = (a.to_str(), b.to_str()) else {
        return false;
    };
    let sa = pair_stem(a).to_lowercase();
    let sb = pair_stem(b).to_lowercase();
    is_test_of(&sa, &sb) || is_test_of(&sb, &sa)
}
```

(keep `is_test_of` and rename the private helper to `pair_stem` to avoid clashing with `std::path::Path::file_stem`).
- [ ] **Step 4: Repoint `scorer/builders/coupling.rs`** — delete its three private fns, call `crate::metrics::file_role::is_test_pair(Path::new(a), Path::new(b))` at the badge site, and move its six predicate tests to `file_role.rs` (delete the originals — no second copy).
- [ ] **Step 5: Run the full suite** — `cargo test` all green; `cargo clippy --all-targets -- -D warnings`; `cargo fmt`.
- [ ] **Step 6: Commit** — `refactor(metrics): extract is_test_pair to file_role as shared predicate`.

### Task 2: Config knob `test_safety_net_min_ratio`

**Files:**
- Modify: `src/config/thresholds.rs` (field + default fn + `Default` impl entry on `CouplingThresholds`)
- Modify: `src/config/mod.rs` (`validate()` + tests)

**Interfaces:**
- Produces: `CouplingThresholds.test_safety_net_min_ratio: f64` (default `0.30`), consumed by Task 3.

- [ ] **Step 1: Write the failing tests in `src/config/mod.rs`** (mirror `decay_min_partners_defaults_and_loads`):

```rust
#[test]
fn test_safety_net_min_ratio_defaults_and_loads() {
    let dir = TempDir::new().unwrap();
    let cfg = load(dir.path()).unwrap();
    assert!((cfg.thresholds.coupling.test_safety_net_min_ratio - 0.30).abs() < f64::EPSILON);
    let cache_dir = dir.path().join(".repository-analysis");
    fs::create_dir_all(&cache_dir).unwrap();
    fs::write(
        cache_dir.join("barad-dur.toml"),
        "[thresholds.coupling]\ntest_safety_net_min_ratio = 0.5\n",
    )
    .unwrap();
    let cfg = load(dir.path()).unwrap();
    assert!((cfg.thresholds.coupling.test_safety_net_min_ratio - 0.5).abs() < f64::EPSILON);
}

#[test]
fn validate_test_safety_net_ratio_out_of_range_errors() {
    for bad in [-0.1_f64, 1.1, f64::NAN] {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.coupling.test_safety_net_min_ratio = bad;
        let err = validate(&cfg).unwrap_err();
        assert!(err.to_string().contains("test_safety_net_min_ratio"), "{bad}");
    }
}
```

- [ ] **Step 2: Run to verify failure** (field missing → compile error is the RED).
- [ ] **Step 3: Implement** — field with `#[serde(default = "default_test_safety_net_min_ratio")]` and doc comment ("expected-tight source↔test co-change floor; distinct from `change_coupling_min_ratio` — the two measure different relationships"); validation clause `if !(0.0..=1.0).contains(&r) { bail!(...) }` (NaN fails the range test — same shape as `change_coupling_min_ratio`'s clause at `src/config/mod.rs:240`).
- [ ] **Step 4: `cargo test --lib config` green.**
- [ ] **Step 5: Commit** — `feat(config): add coupling.test_safety_net_min_ratio threshold`.

### Task 3: `test_safety_net` metric module

**Files:**
- Create: `src/metrics/coupling/test_safety_net.rs`
- Modify: `src/metrics/coupling/mod.rs` (declare module, register 10th metric)
- Modify: `src/metrics/coupling/tests.rs` (`compute_coupling_returns_nine_metrics` → ten)

**Interfaces:**
- Consumes: `file_role::is_test_pair` (Task 1), `test_safety_net_min_ratio` (Task 2), `snapshot.file_change_pairs: Vec<(PathBuf, PathBuf, usize)>`, `snapshot.commits_by_file`, `score_count_bands`.
- Produces: `pub(crate) fn test_safety_net(snapshot: &RepoSnapshot, thresholds: &CouplingThresholds) -> MetricValue` named `"Test safety net"`.

- [ ] **Step 1: Write failing unit tests inside the new module** (`#[cfg(test)] mod tests` in `test_safety_net.rs`), fixtures via `metrics::testutil`. Cover exactly, with both-sides boundaries for the mutation gate:
  - one source + one test candidate, co-change ratio ≥ threshold → not flagged, description `"0 of 1 source/test pairs below 30% co-change"`, score 100;
  - ratio exactly at threshold (0.30) not flagged, one co-change fewer → flagged (both sides of `<`);
  - two candidates (`foo.test.ts` + `foo.spec.ts`), higher-ratio candidate wins (flag only if the *best* erodes);
  - source with commits, candidate exists, zero co-changes → flagged with ratio `0.0`;
  - source with no candidate in `snapshot.files` → absent (skipped, not flagged);
  - source with zero commits → absent;
  - `score_count_bands` boundaries on flag count: 0→100, 1→75, 2→75, 3→50, 5→50, 6→25 (both sides of each edge);
  - no pairs anywhere → `score: None`, description `"No source/test pairs detected by naming convention"`;
  - evidence list: top-10 cap, sorted ascending by ratio then path, entry format `"src/a.rs ↔ tests/a_test.rs — 8% co-change"`.
- [ ] **Step 2: Run to verify failure** (module missing).
- [ ] **Step 3: Implement** the two functions from the spec's Architecture section:

```rust
use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::CouplingThresholds;
use crate::metrics::file_role::{classify, is_test_pair, FileRole};
use crate::metrics::{score_count_bands, MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

struct TestPairing {
    test_path: PathBuf,
    co_change_ratio: f64,
}

/// Strongest (highest co-change ratio) test-file pairing per Source file.
/// Sources with no naming-convention candidate are absent — "no test
/// convention detected", not "coverage is bad" (spec decision 3).
fn strongest_test_pairing(snapshot: &RepoSnapshot) -> HashMap<PathBuf, TestPairing> { ... }

/// Pairs whose best ratio sits below `test_safety_net_min_ratio`: the
/// safety net is eroding (Crime Scene Ch. 9). Count scored via the
/// standard four-band scale; evidence lists the 10 worst pairs.
pub(crate) fn test_safety_net(
    snapshot: &RepoSnapshot,
    thresholds: &CouplingThresholds,
) -> MetricValue { ... }
```

  Ratio formula: `co_changes as f64 / commits_a.min(commits_b).max(1) as f64` (same as `build_coupling_pairs`); candidates found by scanning `snapshot.files` for `classify(path) == FileRole::Test && is_test_pair(source, candidate)`, co-change counts looked up in `snapshot.file_change_pairs` (normalize the lookup to the pair's lexicographic order — that Vec stores each pair once, `a < b`).
- [ ] **Step 4: Register** in `compute_coupling`'s metrics vec after `coupling_reach_trend(reach)`; update the count test to `ten`, run `cargo test --lib coupling` green.
- [ ] **Step 5: Commit** — `feat(coupling): test safety-net metric (Crime Scene Ch. 9)`.

### Task 4: Surfaces — tooltip, action arms (pinned)

**Files:**
- Modify: `src/renderer/templates/chrome.js` (METRIC_TIPS entry after `'Co-change reach trend'`)
- Modify: `src/scorer/actions.rs` (`target_tab_for_metric` + `suggest_action` arms + pin-test entries)

- [ ] **Step 1: Write the failing pin assertions** in the existing `target_tab_for_metric_pins_representative_arms` / `suggest_action_pins_representative_arms` tests:

```rust
assert_eq!(target_tab_for_metric("Test safety net"), (Some("coupling"), None));
assert_eq!(
    suggest_action("Test safety net"),
    "Revive the paired tests of recently-changed source files — start with the lowest co-change pairs"
);
```

- [ ] **Step 2: Verify RED, then add the two arms** and the chrome.js tip: `'Test safety net': 'Source files whose naming-convention-paired test file has stopped co-changing with them (co-change ratio below test_safety_net_min_ratio, default 30%). An eroding safety net: the code moves, its tests don’t (Tornhill, Ch. 9). Scoring: 0 → 100, 1–2 → 75, 3–5 → 50, >5 → 25.'`
- [ ] **Step 3:** `cargo test --lib actions` green + `make report-smoke` clean.
- [ ] **Step 4: Commit** — `feat(coupling): surface test safety net in tooltip and actions`.

### Task 5: Integration test + dogfood + tracker

**Files:**
- Create: `tests/source_test_coupling_walking_skeleton.rs`
- Modify: `docs/crime-scene-book-notes.md` (Ch. 9 row 🟡 → ✅), `README.md` coupling metric list if it enumerates metrics

- [ ] **Step 1: Write the failing E2E test** (fixture-repo style of `tests/trends_walking_skeleton.rs` — reuse its isolated `git()` helper shape with `GIT_CONFIG_GLOBAL=/dev/null` and a single captured `base` instant): a repo where `lib.rs` and `lib_test.rs` co-change 3× early, then `lib.rs` changes 7× more alone → run `analyze --json --no-cache`, assert the Coupling category contains `"Test safety net"` with score 75 (1 eroding pair) and the evidence names the pair. Add the true-negative: a second pair that keeps co-changing must not be listed.
- [ ] **Step 2: Verify RED, then green** (should pass from Tasks 1-4; fix if not).
- [ ] **Step 3: Dogfood** — `cargo run --release -- analyze . --json --no-cache`; confirm barad-dûr's own `src/metrics/team/mod.rs ↔ src/metrics/team/tests.rs`-style pairs score as *not* eroding; record the observed count in the MR description. Also confirm (spec's Interactions section) that the metric produces a value — not a panic or spurious N/A — on a backfilled snapshot: `file_change_pairs`/`commits_by_file` exist there, unlike AST data; a one-assertion extension of the integration test suffices.
- [ ] **Step 4: Update the tracker row and commit** — `docs: close Ch. 9 in the crime-scene tracker`.
- [ ] **Step 5: MR** targeting main; wait for the pipeline to register before `glab mr merge --auto-merge` (the !91 lesson).
