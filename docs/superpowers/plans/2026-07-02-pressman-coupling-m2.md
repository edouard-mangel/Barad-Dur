# Pressman Coupling M2 — Trend Counts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Record per-kind Pressman finding counts (content/common/control) in history entries so trends show raw magnitude, not just band-quantized scores — recording them only when detection actually ran.

**Architecture:** A pure `pressman_finding_counts(snapshot, thresholds) -> Option<CouplingFindingCounts>` in `metrics/coupling` is the single source for counts; `build_report` embeds it in `AnalysisReport` (dashboard/JSON get it free via serde); `build_history_entry` copies it into three `Option<usize>` fields on `HistoryCounts`. `None` = detection didn't run (backfill/ADR-005), distinct from 0 = clean. The HTML report's Trends tab tooltip surfaces the counts.

**Tech Stack:** Rust (serde), vanilla JS template (trends.js). Branch: `feat/pressman-coupling-m2` (stacked on M1).

## Global Constraints

- TDD strictly; `RUSTFLAGS="-D warnings" cargo test --lib`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt` clean before every commit; conventional commits, never mention Claude/AI.
- Metrics stay pure `(snapshot, thresholds) → value`; no I/O.
- **None vs 0 semantics is the heart of M2:** `Option` fields are `None` when the AST pass didn't run (`snapshot.file_metrics.is_empty()`, the ADR-005 backfill path) or no detectable-language files exist; `Some(0)` means detection ran and found nothing. Never conflate them.
- `trends.json` backward compatibility: new `HistoryCounts` fields use `#[serde(default, skip_serializing_if = "Option::is_none")]` so old files load and new files stay compact.
- Counts must exactly equal what the three metrics report (including barrel findings in Content when `content_barrel_rule` is on).
- Facts discovered during recon (trust these, don't re-derive): backfill (`src/backfill/mod.rs:55-67`) computes only health/team/evolution/hygiene categories — no coupling — and uses `collect_snapshot_at`, which leaves `file_metrics`, `import_graph`, and `coupling_findings` empty. `build_history_entry` (`src/scorer.rs:20-53`) already skips unscored metrics. The React dashboard has no history view; the "history view" is the HTML report's Trends tab (`src/renderer/templates/trends.js`, tooltip at lines 166-168).

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/metrics/coupling/mod.rs` | Modify | `detection_ran` gate; `pressman_finding_counts` |
| `src/metrics/coupling/tests.rs` | Modify | helper gains file_metrics; new tests |
| `src/scorer/types.rs` | Modify | `CouplingFindingCounts`; report field; `HistoryCounts` fields |
| `src/scorer.rs` | Modify | `build_report` takes `&CouplingThresholds`; computes counts; `build_history_entry` copies them |
| `src/cmd/analyze.rs`, `src/cmd/gate.rs`, `src/backfill/mod.rs` | Modify | call-site signature update |
| `src/renderer/templates/trends.js` | Modify | tooltip shows counts when present |
| `docs/superpowers/specs/2026-07-02-pressman-coupling-design.md` | Modify | correct the "backfill retroactive" claim |
| `tests/pressman_coupling_milestone_2.rs` | Create | E2E: live counts Some+consistent; backfill-style None+unscored |

---

### Task 1: Detection-ran gate on Pressman metrics

**Files:**
- Modify: `src/metrics/coupling/mod.rs` (`pressman_metric`), `src/metrics/coupling/tests.rs`

**Interfaces:**
- Produces: `pub(crate) fn detection_ran(snapshot: &RepoSnapshot) -> bool` (`!snapshot.file_metrics.is_empty()`), used by `pressman_metric` now and `pressman_finding_counts` in Task 2.

- [ ] **Step 1: Write the failing test** (in `src/metrics/coupling/tests.rs`):

```rust
#[test]
fn pressman_metrics_unscored_when_detection_did_not_run() {
    // Backfill-style snapshot (ADR-005): files listed but no AST pass ran,
    // so file_metrics is empty. Empty findings must NOT read as "clean".
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![crate::metrics::testutil::make_file("src/a.rs")];
    // file_metrics deliberately left empty
    let result = compute_coupling(&snapshot, &CouplingThresholds::default());
    for name in ["Content coupling", "Common coupling", "Control coupling"] {
        let m = result.metrics.iter().find(|m| m.name == name).unwrap();
        assert_eq!(m.score, None, "{name} must be unscored when the AST pass didn't run");
    }
}
```

- [ ] **Step 2: Run** `cargo test --lib metrics::coupling` — the new test FAILS (metrics score 100 today). Existing M1 tests built via `snapshot_with_findings` will start failing at Step 3, so fix the helper in the same step.

- [ ] **Step 3: Implement:**

In `src/metrics/coupling/mod.rs`:

```rust
/// True when the collector's AST pass actually ran on this snapshot.
/// `collect_snapshot_at` (ADR-005, backfill) skips it, leaving
/// `file_metrics` empty — an empty findings list there means "not
/// collected", never "clean".
pub(crate) fn detection_ran(snapshot: &RepoSnapshot) -> bool {
    !snapshot.file_metrics.is_empty()
}
```

In `pressman_metric`, replace the single gate with:

```rust
    if !detection_ran(snapshot) {
        return MetricValue {
            name: name.to_string(),
            description: "Coupling detection did not run (no parsed files)".to_string(),
            raw_value: RawValue::Count(0),
            score: None,
        };
    }
    if !has_detectable_files(snapshot) {
        // existing unscored branch, unchanged
```

In `src/metrics/coupling/tests.rs`, make `snapshot_with_findings` mark detection as ran:

```rust
fn snapshot_with_findings(findings: Vec<CouplingFinding>) -> RepoSnapshot {
    let mut s = crate::metrics::testutil::make_snapshot();
    s.files = vec![crate::metrics::testutil::make_file("src/a.rs")];
    s.file_metrics.insert(
        PathBuf::from("src/a.rs"),
        crate::snapshot::FileComplexity::default(),
    );
    s.coupling_findings = findings;
    s
}
```

Apply the same one-line `file_metrics.insert` to any other test that calls `compute_coupling` on a hand-built snapshot (the barrel-toggle test builds its own: insert an entry for `"app/main.ts"`). Tests calling `barrel_bypass_findings` directly are unaffected.

- [ ] **Step 4: Run** `RUSTFLAGS="-D warnings" cargo test --lib` — all pass (including `pressman_metrics_unscored_without_detectable_files`, whose `.py`-only snapshot must now ALSO get a `file_metrics` entry for `main.py` so it exercises the detectable-language branch, not the detection-ran branch — update it).

- [ ] **Step 5: Commit**

```bash
cargo fmt && git add src/metrics/coupling/
git commit -m "fix(metrics): pressman metrics unscored when AST pass did not run"
```

---

### Task 2: `CouplingFindingCounts` in the report

**Files:**
- Modify: `src/scorer/types.rs` (new type + report field), `src/scorer.rs` (`build_report` signature + computation), `src/metrics/coupling/mod.rs` (+ tests.rs), `src/cmd/analyze.rs:95-101`, `src/cmd/gate.rs:51-57`, `src/backfill/mod.rs:61-67`

**Interfaces:**
- Consumes: Task 1's `detection_ran`; existing `has_detectable_files`, `barrel_bypass_findings`, `CouplingThresholds`.
- Produces:
  - `scorer::CouplingFindingCounts { content: usize, common: usize, control: usize }` (derives `Debug, Clone, Copy, PartialEq, Serialize`)
  - `AnalysisReport.coupling_finding_counts: Option<CouplingFindingCounts>` with `#[serde(skip_serializing_if = "Option::is_none")]`
  - `metrics::coupling::pressman_finding_counts(snapshot: &RepoSnapshot, thresholds: &CouplingThresholds) -> Option<CouplingFindingCounts>`
  - `build_report(snapshot, categories, remote_meta, weights, coupling: &CouplingThresholds) -> AnalysisReport` — **signature change**: the old `component_depth: usize` parameter is replaced by the whole thresholds struct (`coupling.component_depth` used internally for `build_coupling_pairs`).

- [ ] **Step 1: Write the failing tests** (in `src/metrics/coupling/tests.rs`):

```rust
#[test]
fn finding_counts_match_metrics_including_barrel() {
    let mut snapshot = snapshot_with_findings(vec![
        make_finding(CouplingKind::Common),
        make_finding(CouplingKind::Control),
        make_finding(CouplingKind::Control),
    ]);
    snapshot.files.extend([
        crate::metrics::testutil::make_file("app/main.ts"),
        crate::metrics::testutil::make_file("lib/index.ts"),
        crate::metrics::testutil::make_file("lib/impl.ts"),
    ]);
    snapshot.import_graph.insert(
        PathBuf::from("app/main.ts"),
        vec![PathBuf::from("lib/impl.ts")],
    );
    let thresholds = CouplingThresholds { component_depth: 1, ..Default::default() };
    let counts = pressman_finding_counts(&snapshot, &thresholds).expect("detection ran");
    assert_eq!(counts.content, 1, "barrel bypass counted into content");
    assert_eq!(counts.common, 1);
    assert_eq!(counts.control, 2);

    let off = CouplingThresholds { component_depth: 1, content_barrel_rule: false, ..Default::default() };
    assert_eq!(pressman_finding_counts(&snapshot, &off).unwrap().content, 0);
}

#[test]
fn finding_counts_none_when_detection_did_not_run() {
    let mut snapshot = crate::metrics::testutil::make_snapshot();
    snapshot.files = vec![crate::metrics::testutil::make_file("src/a.rs")];
    assert!(pressman_finding_counts(&snapshot, &CouplingThresholds::default()).is_none());
}
```

And in `src/scorer.rs` tests:

```rust
#[test]
fn report_embeds_finding_counts_when_detection_ran() {
    let mut snapshot = RepoSnapshot::new(
        std::path::PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    snapshot.files = vec![crate::metrics::testutil::make_file("src/a.rs")];
    snapshot.file_metrics.insert(
        std::path::PathBuf::from("src/a.rs"),
        crate::snapshot::FileComplexity::default(),
    );
    let report = build_report(
        &snapshot,
        vec![make_category("Health", 80)],
        None,
        WEIGHTS,
        &crate::config::CouplingThresholds::default(),
    );
    assert_eq!(
        report.coupling_finding_counts,
        Some(crate::scorer::CouplingFindingCounts { content: 0, common: 0, control: 0 })
    );
}

#[test]
fn report_finding_counts_none_for_backfill_style_snapshot() {
    let snapshot = RepoSnapshot::new(
        std::path::PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    let report = build_report(
        &snapshot,
        vec![make_category("Health", 80)],
        None,
        WEIGHTS,
        &crate::config::CouplingThresholds::default(),
    );
    assert_eq!(report.coupling_finding_counts, None);
}
```

(`crate::metrics::testutil` is `#[cfg(test)]`-public within the crate; if scorer tests can't see it, inline a tiny `FileEntry` literal instead — same shape as `testutil::make_file`.)

- [ ] **Step 2: Run** — compile errors (type/function missing). Expected RED.

- [ ] **Step 3: Implement.**

`src/scorer/types.rs` (near `HistoryCounts`):

```rust
/// Per-kind Pressman coupling finding counts for one analysis run.
/// `None` on the report means detection did not run (e.g. backfill's
/// ADR-005 snapshot) — distinct from all-zero, which means "clean".
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct CouplingFindingCounts {
    pub content: usize,
    pub common: usize,
    pub control: usize,
}
```

`AnalysisReport` gains (after `import_cycles`):

```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coupling_finding_counts: Option<CouplingFindingCounts>,
```

`src/metrics/coupling/mod.rs`:

```rust
/// Single source of truth for per-kind finding counts. Must equal what the
/// three Pressman metrics report (Content includes barrel-bypass findings
/// when the rule is enabled). `None` when detection did not run or no
/// detectable-language files exist.
pub(crate) fn pressman_finding_counts(
    snapshot: &RepoSnapshot,
    thresholds: &CouplingThresholds,
) -> Option<CouplingFindingCounts> {
    if !detection_ran(snapshot) || !has_detectable_files(snapshot) {
        return None;
    }
    let count_kind = |kind: CouplingKind| {
        snapshot.coupling_findings.iter().filter(|f| f.kind == kind).count()
    };
    let barrel = if thresholds.content_barrel_rule {
        barrel_bypass_findings(snapshot, thresholds.component_depth).len()
    } else {
        0
    };
    Some(CouplingFindingCounts {
        content: count_kind(CouplingKind::Content) + barrel,
        common: count_kind(CouplingKind::Common),
        control: count_kind(CouplingKind::Control),
    })
}
```

(add `use crate::scorer::CouplingFindingCounts;` — no cycle: `metrics` already sits beside `scorer` in the crate; if an import cycle DOES bite, move the struct to `src/metrics/mod.rs` and re-export from scorer — note which you did in the report.)

`src/scorer.rs` — change the signature and body:

```rust
pub fn build_report(
    snapshot: &RepoSnapshot,
    categories: Vec<CategoryResult>,
    remote_meta: Option<RemoteMeta>,
    weights: &[(&str, f64)],
    coupling: &crate::config::CouplingThresholds,
) -> AnalysisReport {
    ...
    let coupling_pairs = build_coupling_pairs(snapshot, coupling.component_depth);
    ...
    let coupling_finding_counts =
        crate::metrics::coupling::pressman_finding_counts(snapshot, coupling);
    AnalysisReport {
        ...
        coupling_finding_counts,
        ...
    }
}
```

Update the call sites — `src/cmd/analyze.rs` and `src/cmd/gate.rs` and `src/backfill/mod.rs` pass `&cfg.thresholds.coupling` instead of `cfg.thresholds.coupling.component_depth`; the ~7 `build_report(..., WEIGHTS, 2)` test call sites in `src/scorer.rs` pass `&crate::config::CouplingThresholds::default()` (default `component_depth` is 2, so behavior is identical). The compiler enumerates every remaining site, including `AnalysisReport` struct literals in `src/cmd/gate.rs` tests and renderer tests — add `coupling_finding_counts: None,` there.

- [ ] **Step 4: Run** `RUSTFLAGS="-D warnings" cargo test --lib` → PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt && git add -A src/
git commit -m "feat(scorer): embed pressman finding counts in the analysis report"
```

---

### Task 3: History counts

**Files:**
- Modify: `src/scorer/types.rs` (`HistoryCounts`), `src/scorer.rs` (`build_history_entry`), `src/cache/history.rs` (compat test)

**Interfaces:**
- Produces: `HistoryCounts` gains
  ```rust
      #[serde(default, skip_serializing_if = "Option::is_none")]
      pub content_coupling: Option<usize>,
      #[serde(default, skip_serializing_if = "Option::is_none")]
      pub common_coupling: Option<usize>,
      #[serde(default, skip_serializing_if = "Option::is_none")]
      pub control_coupling: Option<usize>,
  ```
  populated by `build_history_entry` from `report.coupling_finding_counts`.

- [ ] **Step 1: Failing tests** (in `src/scorer.rs` tests):

First extract Task 2's report construction into two test helpers in `src/scorer.rs`'s tests module (refactor `report_embeds_finding_counts_when_detection_ran` and `report_finding_counts_none_for_backfill_style_snapshot` to use them — same assertions, no duplication):

```rust
fn report_with_detection() -> AnalysisReport {
    let mut snapshot = RepoSnapshot::new(
        std::path::PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    snapshot.files = vec![crate::metrics::testutil::make_file("src/a.rs")];
    snapshot.file_metrics.insert(
        std::path::PathBuf::from("src/a.rs"),
        crate::snapshot::FileComplexity::default(),
    );
    build_report(
        &snapshot,
        vec![make_category("Health", 80)],
        None,
        WEIGHTS,
        &crate::config::CouplingThresholds::default(),
    )
}

fn report_without_detection() -> AnalysisReport {
    let snapshot = RepoSnapshot::new(
        std::path::PathBuf::from("/tmp"),
        "test".into(),
        "main".into(),
        TimeWindow::default(),
    );
    build_report(
        &snapshot,
        vec![make_category("Health", 80)],
        None,
        WEIGHTS,
        &crate::config::CouplingThresholds::default(),
    )
}
```

Then the new tests:

```rust
#[test]
fn history_entry_carries_finding_counts() {
    let entry = build_history_entry(&report_with_detection(), "abc123", None);
    assert_eq!(entry.counts.content_coupling, Some(0));
    assert_eq!(entry.counts.common_coupling, Some(0));
    assert_eq!(entry.counts.control_coupling, Some(0));
}

#[test]
fn history_entry_counts_none_without_detection() {
    let entry = build_history_entry(
        &report_without_detection(),
        "abc123",
        Some("backfill".into()),
    );
    assert_eq!(entry.counts.content_coupling, None);
    assert_eq!(entry.counts.common_coupling, None);
    assert_eq!(entry.counts.control_coupling, None);
}
```

And in `src/cache/history.rs` tests (backward compat — follow the existing alias-test pattern there):

```rust
#[test]
fn old_trends_json_without_coupling_counts_still_loads() {
    // Serialize an entry built from HistoryCounts::default() minus the new
    // fields by writing a raw JSON string with only commits/files/authors,
    // deserialize, and assert the Option fields default to None.
    let raw = r#"[{"timestamp":"2026-01-01T00:00:00Z","head":"abc","overall_score":80,
        "category_scores":{},"metrics":{},"counts":{"commits":1,"files":2,"authors":3},
        "branch":"main","schema_version":1}]"#;
    let entries: Vec<HistoryEntry> = serde_json::from_str(raw).unwrap();
    assert_eq!(entries[0].counts.content_coupling, None);
    assert_eq!(entries[0].counts.commits, 1);
}
```

- [ ] **Step 2: Run** — RED (missing fields).

- [ ] **Step 3: Implement** — add the three fields to `HistoryCounts` (types.rs), and in `build_history_entry`:

```rust
        counts: HistoryCounts {
            commits: report.total_commits,
            files: report.total_files,
            authors: report.total_authors,
            content_coupling: report.coupling_finding_counts.map(|c| c.content),
            common_coupling: report.coupling_finding_counts.map(|c| c.common),
            control_coupling: report.coupling_finding_counts.map(|c| c.control),
        },
```

Existing `HistoryCounts { commits: .., files: .., authors: .. }` literals in tests (trend.rs:237, cache/history.rs:129, renderer/html/tests.rs:606, tests_extra.rs:53) compile again by appending `..Default::default()` (the struct derives `Default`).

- [ ] **Step 4: Run** `RUSTFLAGS="-D warnings" cargo test --lib` → PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt && git add src/scorer/ src/scorer.rs src/cache/ src/trend.rs src/renderer/
git commit -m "feat(trend): record pressman finding counts in history entries"
```

---

### Task 4: Trends tab tooltip + spec correction

**Files:**
- Modify: `src/renderer/templates/trends.js:164-169` (tooltip), `docs/superpowers/specs/2026-07-02-pressman-coupling-design.md` (M2 section)

**Interfaces:** none new — JS reads the serde-serialized `entry.counts`.

- [ ] **Step 1: Implement the tooltip** — in the tooltip string builder (current lines 166-168), append after the authors line, matching the file's ES5 string-concatenation style:

```js
+ (entry.counts.content_coupling != null
    ? 'coupling findings: ' + entry.counts.content_coupling + ' content, '
      + entry.counts.common_coupling + ' common, '
      + entry.counts.control_coupling + ' control\n'
    : '')
```

(`!= null` deliberately catches both `undefined` — old entries — and `null`; absent keys from `skip_serializing_if` read as `undefined`.)

- [ ] **Step 2: Verify by generating a report** — `cargo run -- analyze . --no-cache --html -o /tmp/claude-1000/-home-edouard-WS-barad-dur/cbf486e0-67c0-483f-8491-2c4d15a04d4d/scratchpad/report.html` and grep the output for `coupling findings:` (present in the embedded trends.js) and for `content_coupling` in the embedded history JSON (present once at least one analyze wrote a new-format entry — the run itself appends one). Quote both grep hits in your report.

- [ ] **Step 3: Correct the spec** — in the design doc's M2 section, replace the sentence "Backfill makes history retroactive automatically since it reuses the same pipeline." with:

```markdown
Backfill entries carry **no** Pressman data: ADR-005's historical snapshots
skip the AST pass (empty `file_metrics`), so their metrics render unscored
and their counts serialize as absent (`None`) — never as fake zeros or
perfect scores. Retroactive coupling trends would require historical AST
collection and are explicitly deferred.
```

- [ ] **Step 4: Run** `RUSTFLAGS="-D warnings" cargo test --lib` (renderer tests embed trends.js via include_str! — they catch JS-side syntax only indirectly; the grep in Step 2 is the real check) → PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt && git add src/renderer/templates/trends.js docs/superpowers/specs/
git commit -m "feat(renderer): show pressman finding counts in trends tooltip"
```

---

### Task 5: Milestone integration test

**Files:**
- Create: `tests/pressman_coupling_milestone_2.rs`

**Interfaces:**
- Consumes public lib API: `Collector::{open, collect_snapshot, collect_snapshot_at}`, `metrics::coupling::compute_coupling`, `scorer::{build_report, build_history_entry}`, `config::CouplingThresholds`.

- [ ] **Step 1: Write the test:**

```rust
//! M2: finding counts flow into reports and history entries — and are
//! honestly absent when detection did not run (ADR-005 backfill path).

use barad_dur::collector::Collector;
use barad_dur::config::CouplingThresholds;
use barad_dur::metrics::{coupling, evolution, health, hygiene, team};
use barad_dur::scorer;
use barad_dur::snapshot::TimeWindow;
use std::path::PathBuf;

fn test_repo_path() -> PathBuf {
    std::env::var("BARAD_DUR_TEST_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

const WEIGHTS: &[(&str, f64)] = &[
    ("Health", 0.25),
    ("Team", 0.10),
    ("Evolution", 0.25),
    ("Git Hygiene", 0.20),
    ("Coupling", 0.20),
];

#[test]
fn live_analysis_records_finding_counts_in_history_entry() {
    let Ok(collector) = Collector::open(&test_repo_path(), TimeWindow::default()) else {
        return;
    };
    let snapshot = collector.collect_snapshot().expect("snapshot");
    let thresholds = CouplingThresholds::default();
    let default_cfg = barad_dur::config::Config::default();
    let categories = vec![
        health::compute_health(&snapshot, &default_cfg.thresholds.health),
        team::compute_team(&snapshot, &default_cfg.thresholds.team),
        evolution::compute_evolution(&snapshot, &default_cfg.thresholds.evolution),
        hygiene::compute_hygiene(&snapshot, &default_cfg.thresholds.hygiene),
        coupling::compute_coupling(&snapshot, &thresholds),
    ];
    let report = scorer::build_report(&snapshot, categories, None, WEIGHTS, &thresholds);

    let counts = report
        .coupling_finding_counts
        .expect("live analysis must produce counts");
    let entry = scorer::build_history_entry(&report, "test-head", None);
    assert_eq!(entry.counts.content_coupling, Some(counts.content));
    assert_eq!(entry.counts.common_coupling, Some(counts.common));
    assert_eq!(entry.counts.control_coupling, Some(counts.control));
}

#[test]
fn backfill_style_snapshot_records_no_counts_and_unscored_metrics() {
    let Ok(collector) = Collector::open(&test_repo_path(), TimeWindow::default()) else {
        return;
    };
    let head = collector.head_commit_hash().expect("head");
    let snapshot = Collector::collect_snapshot_at(&test_repo_path(), &head, true)
        .expect("historical snapshot");

    // Pressman metrics must be unscored (detection didn't run)…
    let cat = coupling::compute_coupling(&snapshot, &CouplingThresholds::default());
    for name in ["Content coupling", "Common coupling", "Control coupling"] {
        let m = cat.metrics.iter().find(|m| m.name == name).unwrap();
        assert_eq!(m.score, None, "{name} must be unscored on ADR-005 snapshots");
    }

    // …and the report/history carry no counts (mirroring backfill's category list).
    let default_cfg = barad_dur::config::Config::default();
    let categories = vec![
        health::compute_health(&snapshot, &default_cfg.thresholds.health),
        team::compute_team(&snapshot, &default_cfg.thresholds.team),
        evolution::compute_evolution(&snapshot, &default_cfg.thresholds.evolution),
        hygiene::compute_hygiene(&snapshot, &default_cfg.thresholds.hygiene),
    ];
    let report = scorer::build_report(
        &snapshot,
        categories,
        None,
        WEIGHTS,
        &CouplingThresholds::default(),
    );
    assert_eq!(report.coupling_finding_counts, None);
    let entry = scorer::build_history_entry(&report, &head, Some("backfill".into()));
    assert_eq!(entry.counts.content_coupling, None);
}
```

Adjust to reality if `Config::default()`/threshold field names differ (check `src/config/mod.rs` — backfill/mod.rs:55-67 shows the exact pattern to copy) and if `pressman_finding_counts`/`compute_coupling` visibility needs a `pub` bump for the integration test (unit-crate `pub(crate)` items aren't visible from `tests/`; `compute_coupling` is already `pub`; the test above deliberately avoids `pressman_finding_counts`, using only `pub` API).

- [ ] **Step 2: Run** `cargo test --test pressman_coupling_milestone_2` → PASS.

- [ ] **Step 3: Full sweep** — `RUSTFLAGS="-D warnings" cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt -- --check` → all green.

- [ ] **Step 4: Commit**

```bash
git add tests/pressman_coupling_milestone_2.rs
git commit -m "test(coupling): M2 milestone — finding counts in reports and history"
```

---

## Post-plan notes

- **Not in M2:** count-delta rendering in the CLI trend section or `TrendDelta` (the raw counts ride inside `TrendSummary.history` already; render work is pulled by demand, not pushed). No React-dashboard work (it has no history view). Gate ratchet is M3 and does NOT use these history counts (spec resolved question 3: explicit `--baseline-ref`).
- The M1 branch is the base; if MR !49 changes under review, rebase this branch before merging.
