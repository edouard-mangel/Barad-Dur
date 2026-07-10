# Pressman Coupling M6 — Refactoring Actions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Emit a capped, prioritized per-file list of coupling refactoring suggestions (`coupling_actions`) and surface it in the Coupling tab across CLI, HTML, and dashboard.

**Architecture:** A pure generator in `src/scorer/actions.rs` consumes the complete finding set (`all_coupling_findings`) and M5's corroboration signal, groups by file, ranks by worst rung (content≻common≻control) then corroborated-first then count, caps at 10, and emits `ActionItem`s. A shared `gated_barrel_findings`/`all_coupling_findings` helper first replaces four duplicated barrel-gating sites (closing an M4 follow-up).

**Tech Stack:** Rust (scorer/metrics), the report's inline JS templates (`renderer/templates/`), React/TS dashboard (`dashboard/`).

**Spec:** `docs/superpowers/specs/2026-07-09-pressman-coupling-m6-design.md`

## Global Constraints

- **Report language:** the word is **"corroborated"**, never "confirmed".
- **Rank inheritance = worst-rung-wins (ordinal):** a file inherits its most severe kind on the ladder **Content ≻ Common ≻ Control** (severity index Content=0, Common=1, Control=2; lower = worse). Rung is the primary sort key; corroboration and count only break ties *within* a rung, never across.
- **Ordering key (exact):** `worst_rung` asc → `corroborated` (true first) → `count` desc → `path` asc. Take the first **10**.
- **Action text format (exact):** `[Coupling] <file> — <N> finding(s) (worst: <kind>)[, corroborated by change history] — <advice>` where `<kind>` ∈ {content, common, control}.
- **`target_tab = Some("coupling")`, `sort_by = None`** on every coupling action.
- **No new config.** Cap 10 is a constant; advice strings are code.
- **Advice strings are the maintainer decision point** — tests assert a stable *substring* per kind, never the whole sentence, so wording stays editable.
- **HTML:** no `innerHTML` (security hook enforces it); build DOM via the existing template idiom.
- **Detection gate:** a snapshot where the AST pass did not run (`!detection_ran(snapshot)`, e.g. ADR-005 backfill) → empty `coupling_actions`, never fabricated.
- **CI warnings-as-errors:** every commit passes `RUSTFLAGS="-D warnings" cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt -- --check`.
- **Commits:** message is a single line, no AI/Claude/nWave trailer. A PreToolUse hook injects trailers into `git commit -m`; commit via `git commit -F <file>` and verify with `git cat-file -p HEAD | grep -iE "claude|co-authored|generated|nwave"` (must be empty); amend with `-F` if a trailer appears.
- **Untouched:** `top_actions`, gate ratchet verdict, M2 history, M5 scoring, detectors.

---

### Task 1: Extract `gated_barrel_findings` + `all_coupling_findings`; refactor the four sites

Pure refactor closing the M4 "gated_barrel_findings helper" follow-up. Behavior identical, guarded by existing tests + a parity test.

**Files:**
- Modify: `src/metrics/coupling/mod.rs` (`compute_coupling` ~line 13, `pressman_finding_counts` ~line 313)
- Modify: `src/scorer/builders.rs` (`build_hotspots` barrel block ~line 66)
- Modify: `src/cmd/gate.rs` (`ratchet_finding_sets` `with_barrel` closure ~line 147)
- Test: `src/metrics/coupling/tests.rs`

**Interfaces:**
- Produces:
  - `pub(crate) fn gated_barrel_findings(snapshot: &RepoSnapshot, thresholds: &CouplingThresholds) -> Vec<CouplingFinding>` — the config-gated barrel-bypass content findings (empty when `content_barrel_rule` is off).
  - `pub(crate) fn all_coupling_findings(snapshot: &RepoSnapshot, thresholds: &CouplingThresholds) -> Vec<CouplingFinding>` — `snapshot.coupling_findings` (cloned) chained with `gated_barrel_findings`.

- [ ] **Step 1: Write the failing parity test**

Append to `src/metrics/coupling/tests.rs`:

```rust
#[test]
fn all_coupling_findings_equals_findings_plus_gated_barrel() {
    // Barrel-on: a cross-component import bypassing a barrel yields a Content
    // finding via gated_barrel_findings; all_coupling_findings must include it
    // on top of the raw AST findings.
    let mut snapshot = make_snapshot();
    snapshot.files = vec![
        make_file("a/index.ts"),
        make_file("a/impl.ts"),
        make_file("b/main.ts"),
    ];
    snapshot.file_metrics.insert(
        std::path::PathBuf::from("b/main.ts"),
        crate::snapshot::FileComplexity::default(),
    );
    snapshot.import_graph.insert(
        PathBuf::from("b/main.ts"),
        vec![PathBuf::from("a/impl.ts")],
    );
    let th = default_thresholds();

    let gated = gated_barrel_findings(&snapshot, &th);
    let all = all_coupling_findings(&snapshot, &th);
    assert_eq!(all.len(), snapshot.coupling_findings.len() + gated.len());
    // Turning the rule off drops the barrel findings from both.
    let off = CouplingThresholds { content_barrel_rule: false, ..default_thresholds() };
    assert!(gated_barrel_findings(&snapshot, &off).is_empty());
    assert_eq!(
        all_coupling_findings(&snapshot, &off).len(),
        snapshot.coupling_findings.len()
    );
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib all_coupling_findings_equals_findings_plus_gated_barrel`
Expected: FAIL — `cannot find function gated_barrel_findings`.

- [ ] **Step 3: Add the two helpers**

In `src/metrics/coupling/mod.rs`, add near `barrel_bypass_findings`:

```rust
/// The config-gated barrel-bypass content findings: `barrel_bypass_findings`
/// when `content_barrel_rule` is on, empty otherwise. Single definition of
/// the barrel-gating that four call sites previously duplicated.
pub(crate) fn gated_barrel_findings(
    snapshot: &RepoSnapshot,
    thresholds: &CouplingThresholds,
) -> Vec<CouplingFinding> {
    if thresholds.content_barrel_rule {
        barrel_bypass_findings(snapshot, thresholds.component_depth)
    } else {
        Vec::new()
    }
}

/// Every coupling finding for a snapshot: the AST findings in
/// `snapshot.coupling_findings` plus the gated barrel-bypass content findings.
/// The complete set the metric, counts, hotspots, gate ratchet, and M6
/// actions all consume, so none can disagree about what was found.
pub(crate) fn all_coupling_findings(
    snapshot: &RepoSnapshot,
    thresholds: &CouplingThresholds,
) -> Vec<CouplingFinding> {
    snapshot
        .coupling_findings
        .iter()
        .cloned()
        .chain(gated_barrel_findings(snapshot, thresholds))
        .collect()
}
```

- [ ] **Step 4: Refactor the four sites to use them**

`compute_coupling` (needs barrel-only, passed as `extra`):
```rust
    let barrel = gated_barrel_findings(snapshot, thresholds);
```
(replaces the inline `let barrel = if thresholds.content_barrel_rule { … } else { Vec::new() };`)

`pressman_finding_counts` — replace the `count_kind` closure + barrel block with a single per-kind count over the complete set:
```rust
    let findings = all_coupling_findings(snapshot, thresholds);
    let count_kind =
        |kind: CouplingKind| findings.iter().filter(|f| f.kind == kind).count();
    Some(CouplingFindingCounts {
        content: count_kind(CouplingKind::Content),
        common: count_kind(CouplingKind::Common),
        control: count_kind(CouplingKind::Control),
    })
```
(the `detection_ran`/`has_detectable_files` early-return above stays; barrel findings are all `Content`, so `content` still includes them.)

`build_hotspots` (`src/scorer/builders.rs`) — replace the inline barrel block + `.chain(barrel.iter())`:
```rust
    let all_findings = crate::metrics::coupling::all_coupling_findings(snapshot, coupling);
    let finding_counts: HashMap<&Path, (usize, usize, usize)> = all_findings
        .iter()
        .fold(HashMap::new(), |mut acc, f| {
            let entry = acc.entry(f.path.as_path()).or_default();
            match f.kind {
                CouplingKind::Content => entry.0 += 1,
                CouplingKind::Common => entry.1 += 1,
                CouplingKind::Control => entry.2 += 1,
            }
            acc
        });
```
Update the `use crate::metrics::coupling::{…}` line: drop `barrel_bypass_findings` if now unused, add `all_coupling_findings`.

`ratchet_finding_sets` (`src/cmd/gate.rs`) — the `with_barrel` closure IS `all_coupling_findings`:
```rust
pub(crate) fn ratchet_finding_sets(
    coupling_cfg: &crate::config::CouplingThresholds,
    base: &crate::snapshot::RepoSnapshot,
    head: &crate::snapshot::RepoSnapshot,
) -> (Vec<CouplingFinding>, Vec<CouplingFinding>) {
    use crate::metrics::coupling::all_coupling_findings;
    (
        all_coupling_findings(base, coupling_cfg),
        all_coupling_findings(head, coupling_cfg),
    )
}
```
Drop the now-unused `coupling::barrel_bypass_findings` reference if the file no longer uses it elsewhere (leave other `coupling::` uses intact).

- [ ] **Step 5: Run the parity + affected suites**

Run: `cargo test --lib all_coupling_findings_equals_findings_plus_gated_barrel`
Run: `cargo test --lib metrics::coupling && cargo test --lib scorer && cargo test --test gate_milestone_1 --test pressman_coupling_milestone_3`
Expected: PASS (existing coupling, hotspot, and gate-ratchet tests unchanged).

- [ ] **Step 6: Full sweep + commit**

Run: `RUSTFLAGS="-D warnings" cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check`
Expected: PASS.

```bash
printf 'refactor(coupling): extract gated_barrel_findings/all_coupling_findings helpers\n' > /tmp/m6-t1.txt
git add src/metrics/coupling/mod.rs src/metrics/coupling/tests.rs src/scorer/builders.rs src/cmd/gate.rs
git commit -F /tmp/m6-t1.txt
git cat-file -p HEAD | grep -iqE "claude|co-authored|generated|nwave" && git commit --amend -F /tmp/m6-t1.txt || true
```

---

### Task 2: `generate_coupling_actions` + `coupling_actions` report field

The core logic: expose `corroboration_degree`, add the generator, add the field, wire it into `build_report`.

**Files:**
- Modify: `src/metrics/coupling/mod.rs` (make `corroboration_degree` `pub(crate)`)
- Modify: `src/scorer/actions.rs` (new generator + advice constants)
- Modify: `src/scorer/types.rs` (`AnalysisReport.coupling_actions`)
- Modify: `src/scorer.rs` (`build_report` wiring)
- Test: `src/scorer/actions.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `all_coupling_findings`, `corroboration_degree`, `detection_ran` (all `pub(crate)` in `metrics::coupling`).
- Produces: `pub(super) fn generate_coupling_actions(snapshot: &RepoSnapshot, thresholds: &CouplingThresholds) -> Vec<ActionItem>`; `AnalysisReport.coupling_actions: Vec<ActionItem>`.

- [ ] **Step 1: Expose `corroboration_degree`**

In `src/metrics/coupling/mod.rs`, change `fn corroboration_degree(` to `pub(crate) fn corroboration_degree(`.

- [ ] **Step 2: Write the failing generator tests**

In `src/scorer/actions.rs` `#[cfg(test)] mod tests`, add (helpers `make_finding`-style are local; build snapshots via `crate::metrics::testutil`):

```rust
use crate::config::CouplingThresholds;
use crate::snapshot::{CouplingFinding, CouplingKind, RepoSnapshot};
use std::path::PathBuf;

fn snap_with(findings: Vec<CouplingFinding>) -> RepoSnapshot {
    let mut s = crate::metrics::testutil::make_snapshot();
    s.files = vec![crate::metrics::testutil::make_file("src/a.rs")];
    s.file_metrics.insert(
        PathBuf::from("src/a.rs"),
        crate::snapshot::FileComplexity::default(),
    );
    s.coupling_findings = findings;
    s
}
fn finding(path: &str, kind: CouplingKind) -> CouplingFinding {
    CouplingFinding { path: PathBuf::from(path), line: Some(1), kind, evidence: "e".into() }
}

#[test]
fn coupling_actions_empty_when_no_findings() {
    let s = snap_with(vec![]);
    assert!(generate_coupling_actions(&s, &CouplingThresholds::default()).is_empty());
}

#[test]
fn coupling_actions_order_content_common_control() {
    let s = snap_with(vec![
        finding("src/ctrl.rs", CouplingKind::Control),
        finding("src/glob.rs", CouplingKind::Common),
        finding("src/int.rs", CouplingKind::Content),
    ]);
    let acts = generate_coupling_actions(&s, &CouplingThresholds::default());
    let files: Vec<&str> = acts.iter().map(|a| a.text.as_str()).collect();
    assert!(files[0].contains("src/int.rs") && files[0].contains("worst: content"));
    assert!(files[1].contains("src/glob.rs") && files[1].contains("worst: common"));
    assert!(files[2].contains("src/ctrl.rs") && files[2].contains("worst: control"));
    assert_eq!(acts[0].target_tab.as_deref(), Some("coupling"));
    assert!(acts[0].sort_by.is_none());
}

#[test]
fn coupling_actions_worst_rung_wins_for_mixed_file() {
    // One file with a Common AND a Control finding inherits Common (worse).
    let s = snap_with(vec![
        finding("src/mix.rs", CouplingKind::Control),
        finding("src/mix.rs", CouplingKind::Common),
    ]);
    let acts = generate_coupling_actions(&s, &CouplingThresholds::default());
    assert_eq!(acts.len(), 1);
    assert!(acts[0].text.contains("worst: common"));
    assert!(acts[0].text.contains("2 finding(s)"));
}

#[test]
fn coupling_actions_corroborated_first_within_rung() {
    // Two Common files, one corroborated (co-changes cross-boundary).
    let mut s = snap_with(vec![
        finding("src/dormant.rs", CouplingKind::Common),
        finding("src/live.rs", CouplingKind::Common),
    ]);
    s.files.push(crate::metrics::testutil::make_file("src/live.rs"));
    s.files.push(crate::metrics::testutil::make_file("src/dormant.rs"));
    s.file_change_pairs
        .push((PathBuf::from("src/live.rs"), PathBuf::from("tests/x.rs"), 5));
    for f in ["src/live.rs", "tests/x.rs"] {
        s.commits_by_file.insert(
            PathBuf::from(f),
            (0u32..10).map(crate::snapshot::CommitId).collect(),
        );
    }
    let acts = generate_coupling_actions(&s, &CouplingThresholds::default());
    assert!(acts[0].text.contains("src/live.rs"));
    assert!(acts[0].text.contains("corroborated by change history"));
    assert!(acts[1].text.contains("src/dormant.rs"));
    assert!(!acts[1].text.contains("corroborated"));
}

#[test]
fn coupling_actions_capped_at_ten() {
    let findings = (0..15)
        .map(|i| finding(&format!("src/f{i}.rs"), CouplingKind::Control))
        .collect();
    let s = snap_with(findings);
    assert_eq!(generate_coupling_actions(&s, &CouplingThresholds::default()).len(), 10);
}

#[test]
fn coupling_actions_advice_is_kind_specific() {
    for (kind, needle) in [
        (CouplingKind::Content, "public interface"),
        (CouplingKind::Common, "injected state"),
        (CouplingKind::Control, "intent-revealing"),
    ] {
        let s = snap_with(vec![finding("src/a.rs", kind)]);
        let acts = generate_coupling_actions(&s, &CouplingThresholds::default());
        assert!(acts[0].text.contains(needle), "kind {kind:?}: {}", acts[0].text);
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test --lib scorer::actions::tests::coupling_actions_order_content_common_control`
Expected: FAIL — `cannot find function generate_coupling_actions`.

- [ ] **Step 4: Implement the generator**

In `src/scorer/actions.rs`, add near the top of the file (after existing `use`s add `use std::path::Path;` and `use std::collections::HashMap;` if not already imported) and beside `generate_top_actions`:

```rust
const CONTENT_ADVICE: &str =
    "Reaches into another module's internals — import through the module's public interface instead.";
const COMMON_ADVICE: &str =
    "Shared mutable global state — replace it with explicitly passed or injected state.";
const CONTROL_ADVICE: &str =
    "A flag parameter steers this function's control flow — split it into two intent-revealing functions.";

/// Per-file coupling refactoring suggestions, ranked worst-rung-first
/// (Content≻Common≻Control), corroborated-before-dormant within a rung, then
/// higher finding-count first, capped at 10. A file's action speaks to its
/// most severe rung. Empty when detection did not run.
pub(super) fn generate_coupling_actions(
    snapshot: &crate::snapshot::RepoSnapshot,
    thresholds: &crate::config::CouplingThresholds,
) -> Vec<ActionItem> {
    use crate::metrics::coupling::{all_coupling_findings, corroboration_degree, detection_ran};
    use crate::snapshot::CouplingKind;

    if !detection_ran(snapshot) {
        return Vec::new();
    }
    let findings = all_coupling_findings(snapshot, thresholds);
    if findings.is_empty() {
        return Vec::new();
    }
    let corr = corroboration_degree(snapshot, thresholds);

    // severity index: lower = worse. Group by file, tracking worst rung + count.
    let mut by_file: HashMap<&Path, (u8, usize)> = HashMap::new();
    for f in &findings {
        let sev = match f.kind {
            CouplingKind::Content => 0u8,
            CouplingKind::Common => 1,
            CouplingKind::Control => 2,
        };
        let entry = by_file.entry(f.path.as_path()).or_insert((sev, 0));
        entry.0 = entry.0.min(sev);
        entry.1 += 1;
    }

    let mut rows: Vec<(&Path, u8, bool, usize)> = by_file
        .into_iter()
        .map(|(path, (sev, count))| (path, sev, corr.contains_key(path), count))
        .collect();
    // worst rung asc → corroborated first → count desc → path asc.
    rows.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then(b.2.cmp(&a.2))
            .then(b.3.cmp(&a.3))
            .then(a.0.cmp(b.0))
    });

    rows.into_iter()
        .take(10)
        .map(|(path, sev, corroborated, count)| {
            let (kind_label, advice) = match sev {
                0 => ("content", CONTENT_ADVICE),
                1 => ("common", COMMON_ADVICE),
                _ => ("control", CONTROL_ADVICE),
            };
            let corr_note = if corroborated { ", corroborated by change history" } else { "" };
            ActionItem {
                text: format!(
                    "[Coupling] {} — {} finding(s) (worst: {}){} — {}",
                    path.display(),
                    count,
                    kind_label,
                    corr_note,
                    advice
                ),
                target_tab: Some("coupling".to_string()),
                sort_by: None,
            }
        })
        .collect()
}
```

- [ ] **Step 5: Add the field and wire it in**

In `src/scorer/types.rs`, add to `AnalysisReport` after `pub top_actions: Vec<ActionItem>,`:
```rust
    /// Per-file coupling refactoring suggestions (Pressman M6), ranked by
    /// severity rung then corroboration. Surfaced in the Coupling tab.
    pub coupling_actions: Vec<ActionItem>,
```

In `src/scorer.rs` `build_report`, after `let top_actions = generate_top_actions(&categories);` add:
```rust
    let coupling_actions = actions::generate_coupling_actions(snapshot, coupling);
```
and add `coupling_actions,` to the `AnalysisReport { … }` literal. Ensure `generate_coupling_actions` is imported (extend the `use actions::…` line or call `actions::generate_coupling_actions`).

Then run `RUSTFLAGS="-D warnings" cargo build --tests` — the compiler lists every `AnalysisReport { … }` literal missing the field (CLI tests, builder tests, renderer tests). Add `coupling_actions: vec![],` to each such literal. If `src/renderer/json.rs` has an exact-JSON-shape assertion, update its expected value to include the new key.

- [ ] **Step 6: Run generator + report tests**

Run: `cargo test --lib scorer::actions`
Run: `cargo test --lib scorer`
Expected: PASS (all 6 new generator tests + existing scorer tests with the added field).

- [ ] **Step 7: Full sweep + commit**

Run: `RUSTFLAGS="-D warnings" cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check`

```bash
printf 'feat(scorer): per-file coupling refactoring actions (Pressman M6)\n' > /tmp/m6-t2.txt
git add src/metrics/coupling/mod.rs src/scorer/actions.rs src/scorer/types.rs src/scorer.rs
# plus any test files the field addition touched:
git add -u
git commit -F /tmp/m6-t2.txt
git cat-file -p HEAD | grep -iqE "claude|co-authored|generated|nwave" && git commit --amend -F /tmp/m6-t2.txt || true
```

---

### Task 3: CLI renderer — Coupling actions section

**Files:**
- Modify: `src/renderer/cli/mod.rs` (`render_actions_and_footer` ~line 288)
- Test: `src/renderer/cli/mod.rs` tests

**Interfaces:**
- Consumes: `report.coupling_actions: Vec<ActionItem>`.

- [ ] **Step 1: Write the failing test**

In `src/renderer/cli/mod.rs` tests, add (mirror the existing action-rendering test; construct a report with one coupling action):

```rust
#[test]
fn cli_renders_coupling_actions_section() {
    let mut report = make_minimal_report(); // whatever the existing tests use
    report.coupling_actions = vec![ActionItem {
        text: "[Coupling] src/a.rs — 1 finding(s) (worst: common) — Shared mutable global state: replace it with explicitly passed or injected state.".to_string(),
        target_tab: Some("coupling".to_string()),
        sort_by: None,
    }];
    let out = render_actions_and_footer(&report);
    assert!(out.contains("Coupling Actions"));
    assert!(out.contains("[Coupling] src/a.rs"));
}
```
(If the existing tests build the report inline rather than via a helper, follow that pattern; the point is a report whose `coupling_actions` is non-empty.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib renderer::cli::tests::cli_renders_coupling_actions_section`
Expected: FAIL — assertion (`Coupling Actions` not in output).

- [ ] **Step 3: Implement**

In `render_actions_and_footer`, after the `top_actions` block and before the footer `push_str`, add:
```rust
    if !report.coupling_actions.is_empty() {
        out.push_str(&format!(
            "\n{}\n",
            "───────────────────────────────────────────────────".dimmed()
        ));
        out.push_str(&format!("  {}\n", "Coupling Actions:".bold()));
        for (i, action) in report.coupling_actions.iter().enumerate() {
            out.push_str(&format!("  {}. {}\n", i + 1, action.text));
        }
    }
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib renderer::cli`
Expected: PASS.

- [ ] **Step 5: Full sweep + commit**

Run: `RUSTFLAGS="-D warnings" cargo test --lib renderer && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check`

```bash
printf 'feat(cli): render Coupling Actions section (Pressman M6)\n' > /tmp/m6-t3.txt
git add src/renderer/cli/mod.rs
git commit -F /tmp/m6-t3.txt
git cat-file -p HEAD | grep -iqE "claude|co-authored|generated|nwave" && git commit --amend -F /tmp/m6-t3.txt || true
```

---

### Task 4: HTML report — Coupling tab actions panel

The HTML report embeds JS templates via `include_str!`. **Read first**, then mirror the existing pattern — no `innerHTML`.

**Files:**
- Modify: `src/renderer/templates/coupling.js` (the Coupling tab renderer)
- Possibly: `src/renderer/templates/coupling_style.css` (a panel style)
- Test: `src/renderer/html/tests.rs` or `tests_extra.rs` (the full-report render test)

**Read before writing:**
1. `src/renderer/templates/coupling.js` — how it reads `report` fields and builds DOM (it already renders coupling finding counts / badges from M4). Identify the DOM-construction idiom (`document.createElement`, `textContent`, `appendChild`) — **no `innerHTML`**.
2. `src/renderer/html.rs` — how `coupling.js` is embedded and what `report` shape the JS receives (the serialized `AnalysisReport`, so `report.coupling_actions` is available as an array of `{text, target_tab, sort_by}`).
3. The existing full-report HTML render test (`src/renderer/html/tests.rs`) that asserts on rendered content — mirror it.

**Requirements:**
- Render a "Coupling Actions" panel in the Coupling tab listing each `report.coupling_actions[i].text` as a list item, in order. Build nodes with `createElement`/`textContent`/`appendChild` (never `innerHTML`).
- If `report.coupling_actions` is empty or absent, render nothing (no empty panel).
- Match the visual idiom of the existing coupling panels.

- [ ] **Step 1: Read `coupling.js`, `html.rs`, and the render test; identify the panel idiom.**
- [ ] **Step 2: Write/extend a failing HTML render test** asserting the rendered output contains a coupling-actions item text when the report has one (extend the existing full-report fixture test — add a `coupling_actions` entry to its fixture and assert the text appears). Run it; confirm it fails.

Run: `cargo test --test <the html render test> ` or `cargo test --lib renderer::html`
Expected: FAIL (text not present).

- [ ] **Step 3: Implement the panel in `coupling.js`** using the read idiom, no `innerHTML`. Add CSS to `coupling_style.css` only if a new class is needed.
- [ ] **Step 4: Run the render test + the security/innerHTML check.**

Run: `cargo test --lib renderer::html && cargo test --test <html render test>`
Also confirm no `innerHTML` was introduced: `grep -n "innerHTML" src/renderer/templates/coupling.js` → no new hits.
Expected: PASS; grep clean.

- [ ] **Step 5: Full sweep + commit**

Run: `RUSTFLAGS="-D warnings" cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check`

```bash
printf 'feat(html): Coupling Actions panel in the report Coupling tab (Pressman M6)\n' > /tmp/m6-t4.txt
git add src/renderer/templates/coupling.js src/renderer/templates/coupling_style.css src/renderer/html/tests.rs src/renderer/html/tests_extra.rs
git commit -F /tmp/m6-t4.txt
git cat-file -p HEAD | grep -iqE "claude|co-authored|generated|nwave" && git commit --amend -F /tmp/m6-t4.txt || true
```

---

### Task 5: Dashboard — Coupling tab actions panel

React 19 + Vite + Tailwind. **Read first**, mirror the existing coupling-tab rendering.

**Files:**
- Modify: `dashboard/src/types.ts` (add `coupling_actions`)
- Modify: `dashboard/src/pages/Report.tsx` (Coupling tab panel)
- Test: dashboard vitest suite if one covers the Coupling tab (mirror it); otherwise a minimal render test.

**Read before writing:**
1. `dashboard/src/types.ts` — how `top_actions` and `coupling_finding_counts` are typed. Note `top_actions` is typed loosely (`string[]`); inspect how `Report.tsx` actually consumes it to type `coupling_actions` correctly as an array of `{ text: string; target_tab?: string; sort_by?: string }`.
2. `dashboard/src/pages/Report.tsx` — the Coupling tab section (where M4's coupling badges/counts render) — mirror its list/panel idiom.
3. `dashboard/src/lib` report-validation (the `isValidReport`-style guard) — add `coupling_actions` as **optional** so older reports without it still load.

**Requirements:**
- `coupling_actions?: { text: string; target_tab?: string; sort_by?: string }[]` in the report type (optional — old reports lack it).
- A "Coupling Actions" panel in the Coupling tab listing `text` per item, in order; render nothing when the array is empty/absent.
- Match the existing Coupling-tab visual idiom (Tailwind classes used by sibling panels).

- [ ] **Step 1: Read `types.ts`, `Report.tsx` coupling tab, and the validation guard.**
- [ ] **Step 2: Add the optional `coupling_actions` type + guard entry.**
- [ ] **Step 3: Render the panel in the Coupling tab** mirroring the sibling pattern.
- [ ] **Step 4: Verify** — `cd dashboard && pnpm test` (if a Coupling-tab test exists, extend it to assert an action renders) and `pnpm tsc --noEmit` (or the project's typecheck) clean.

Run: `cd dashboard && pnpm test && pnpm exec tsc --noEmit`
Expected: PASS; no TS errors.

- [ ] **Step 5: Commit**

```bash
printf 'feat(dashboard): Coupling Actions panel in the Coupling tab (Pressman M6)\n' > /tmp/m6-t5.txt
git add dashboard/src/types.ts dashboard/src/pages/Report.tsx dashboard/src/lib
git commit -F /tmp/m6-t5.txt
git cat-file -p HEAD | grep -iqE "claude|co-authored|generated|nwave" && git commit --amend -F /tmp/m6-t5.txt || true
```

---

### Task 6: End-to-end integration test + dogfood

Prove the ordered, kind-specific actions surface through the real binary, and confirm barad-dur's own report is sane.

**Files:**
- Create: `tests/pressman_coupling_milestone_6.rs`

**Interfaces:**
- Consumes: the built binary `env!("CARGO_BIN_EXE_barad-dur")`; JSON `report.coupling_actions[] { text }`. `serde_json` + `tempfile` are already dev-dependencies (M5).

- [ ] **Step 1: Write the integration test**

Create `tests/pressman_coupling_milestone_6.rs`:

```rust
//! M6 milestone E2E: a fixture with content, common, and control findings
//! across files produces a `coupling_actions` list ordered content→common→
//! control, each with kind-specific advice, surfaced through `analyze --json`.

use std::process::{Command, Output};

fn fixture_repo() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let git = |args: &[&str]| -> Output {
        let out = Command::new("git").current_dir(dir.path()).args(args).output().expect("git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        out
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "f@e.com"]);
    git(&["config", "user.name", "F"]);
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    // common: static mut
    std::fs::write(dir.path().join("src/globals.rs"), "pub static mut COUNTER: usize = 0;\n").unwrap();
    // control: pub fn with a branched-on bool flag
    std::fs::write(
        dir.path().join("src/flags.rs"),
        "pub fn run(verbose: bool) -> u32 { if verbose { 1 } else { 0 } }\n",
    ).unwrap();
    // content: #[path] attribute import
    std::fs::write(dir.path().join("src/other.rs"), "pub fn h() {}\n").unwrap();
    std::fs::write(
        dir.path().join("src/hack.rs"),
        "#[path = \"other.rs\"]\nmod other;\npub fn g() { other::h() }\n",
    ).unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "fixture"]);
    dir
}

fn coupling_action_texts(report: &serde_json::Value) -> Vec<String> {
    report["coupling_actions"]
        .as_array()
        .expect("coupling_actions array")
        .iter()
        .map(|a| a["text"].as_str().expect("text").to_string())
        .collect()
}

#[test]
fn coupling_actions_surface_ordered_and_kind_specific() {
    let dir = fixture_repo();
    let out = Command::new(env!("CARGO_BIN_EXE_barad-dur"))
        .arg("analyze").arg(dir.path()).args(["--json", "--no-cache"])
        .output().expect("run");
    assert!(out.status.success(), "analyze failed: {}", String::from_utf8_lossy(&out.stderr));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let texts = coupling_action_texts(&report);

    // Exactly the three fixture files, ordered content → common → control.
    let content_i = texts.iter().position(|t| t.contains("worst: content")).expect("content action");
    let common_i = texts.iter().position(|t| t.contains("worst: common")).expect("common action");
    let control_i = texts.iter().position(|t| t.contains("worst: control")).expect("control action");
    assert!(content_i < common_i && common_i < control_i, "order: {texts:?}");
    // Kind-specific advice present.
    assert!(texts[content_i].contains("public interface"));
    assert!(texts[common_i].contains("injected state"));
    assert!(texts[control_i].contains("intent-revealing"));
}
```

- [ ] **Step 2: Run the integration test**

Run: `cargo test --test pressman_coupling_milestone_6`
Expected: PASS. (If a `worst:` kind is missing, dump `report["coupling_actions"]` — the fixture must yield exactly one finding of each kind; a `#[path]` content finding and a branched-bool control finding are the load-bearing pieces.)

- [ ] **Step 3: Dogfood**

Run: `cargo run --release -- analyze . --no-cache --json | python3 -c "import json,sys; r=json.load(sys.stdin); print('\n'.join(a['text'] for a in r['coupling_actions']))"`
Expected: control-advice actions for barad-dur's control-finding files (content/common are 0). Quote the output in the report.

- [ ] **Step 4: Full sweep + commit**

Run: `RUSTFLAGS="-D warnings" cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check`

```bash
printf 'test(coupling): M6 milestone — coupling actions surfaced end-to-end\n' > /tmp/m6-t6.txt
git add tests/pressman_coupling_milestone_6.rs
git commit -F /tmp/m6-t6.txt
git cat-file -p HEAD | grep -iqE "claude|co-authored|generated|nwave" && git commit --amend -F /tmp/m6-t6.txt || true
```

---

## Self-Review

**Spec coverage:**
- Separate `coupling_actions` list, Coupling tab → Task 2 (field/logic) + Tasks 3/4/5 (surfacing).
- One-per-file, cap 10 → Task 2 (`by_file` grouping, `.take(10)`), tested.
- Worst-rung-wins → Task 2 (`entry.0.min(sev)`), tested (`coupling_actions_worst_rung_wins_for_mixed_file`).
- Corroborated-first within rung → Task 2 ordering key, tested.
- Advice text (maintainer-editable, substring-asserted) → Task 2 constants + `coupling_actions_advice_is_kind_specific`.
- DRY refactor (`gated_barrel_findings`/`all_coupling_findings`, 4 sites) → Task 1, parity-tested.
- Corroboration exposure (`pub(crate)`) → Task 2 Step 1.
- Surfacing CLI/HTML/dashboard → Tasks 3/4/5.
- Detection-did-not-run → empty → Task 2 (`detection_ran` guard).
- Integration + dogfood → Task 6.
- No config, `top_actions`/gate/history/M5 untouched → constraints; nothing edits them except the pure-refactor gate site (Task 1, parity-tested).

**Placeholder scan:** Rust tasks (1,2,3,6) carry complete code. Tasks 4/5 are read-then-mirror by necessity (inline JS templates + dashboard TSX not reproduced blind) — each names the exact files to read, the data shape, the no-`innerHTML`/optional-type constraints, and concrete acceptance assertions. No "TBD"/"handle edge cases" placeholders.

**Type consistency:** `gated_barrel_findings`/`all_coupling_findings` (Task 1) → consumed by `generate_coupling_actions` (Task 2). `corroboration_degree` made `pub(crate)` (Task 2 Step 1) before use. `ActionItem { text, target_tab: Some("coupling"), sort_by: None }` consistent across Task 2 and the renderer tasks. `AnalysisReport.coupling_actions: Vec<ActionItem>` (Rust) ↔ `coupling_actions?: {text,...}[]` (optional in dashboard, Task 5). Action text format identical between Task 2 unit tests and Task 6 E2E assertions (`worst: <kind>`, advice substrings).
