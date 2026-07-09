# Pressman Coupling M4 — Hotspot Cross-Referencing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hotspot rows cross-reference Pressman coupling findings: `HotspotFile` gains per-kind finding counts, the hotspot score gets a configurable multiplier (default 1.25×) when Content/Common findings are present, and both render surfaces (HTML report + dashboard) show a coupling badge. Rationale from the spec: severity × change frequency = actual risk; a `static mut` in a high-churn file outranks a dormant one.

**Architecture:** No new subsystems. The config gains one field (`CouplingThresholds.hotspot_multiplier`); `build_hotspots` in `src/scorer/builders.rs` gains a `&CouplingThresholds` parameter, joins `snapshot.coupling_findings` (plus toggle-gated barrel-bypass findings) per file, and applies the multiplier after normalization; `hotspots.js` and `HotspotsView.tsx` each add one badge column. Spec: `docs/superpowers/specs/2026-07-02-pressman-coupling-design.md` § "M4 — Hotspot cross-referencing" + § "Configuration".

**Tech Stack:** Rust (serde, toml), vanilla-JS report templates (`el()`/`txt()` DOM helpers — no `innerHTML`, security hook enforces), React 19 + vitest for the dashboard.

## Global Constraints

- CI runs `RUSTFLAGS=-D warnings cargo test` — warnings are errors. Run tests that way (`env RUSTFLAGS='-D warnings' cargo test` — the bare `VAR=x cmd` form gets mangled by the rtk hook).
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt -- --check` must pass (pre-push hook enforces).
- Functional paradigm: pure functions, iterator chains, immutable bindings, `?` for errors (project CLAUDE.md). `build_hotspots`'s existing `for f in &mut files` normalization loop is established style — extend it, don't restructure.
- TDD: every behavior change starts with a failing test.
- Commit messages: conventional commits; NEVER mention Claude, AI, or the assistant (no Co-Authored-By).
- Score-band thresholds live in `src/scorer/types.rs` — never re-hardcode 71/41.
- No `innerHTML` anywhere in templates or dashboard (security hook blocks it). Build DOM via the templates' `el()`/`txt()` helpers.
- Branch: `feat/pressman-coupling-m4` off `main`.
- Dashboard commands run from `dashboard/` with `pnpm` (never npm/yarn).

## Design decisions locked for this MR

- **Only Content + Common findings trigger the multiplier** (spec text: "multiplier when Content/Common findings are present"). Control counts are carried and displayed but never multiply — flag params are the least severe rung.
- **Per-file counts mirror `pressman_finding_counts`'s gating**: barrel-bypass findings join the per-file Content count only when `content_barrel_rule` is on. Same consistency argument that motivated `ratchet_finding_sets` in the pre-M4 hygiene MR — two views of "content coupling" must never disagree.
- **Multiplied score caps at 100.** Every consumer assumes the 0–100 domain: `scoreColor(100 - scoreVal)` in hotspots.js, `score > 70 ? red : …` in HotspotsView.tsx, the d3 color scale domain `[0, 50, 100]`.
- **Counts are plain `usize`, not `Option`.** `HistoryCounts` needs `Option` because backfill entries persist and detection may not have run (ADR-005); `HotspotFile` rows are per-report artifacts of live analysis — zero means zero.
- **Dashboard fields are optional in TS** (`content_findings?: number`): the dashboard loads arbitrary (old) report.json files by drag-and-drop; missing fields must render as "no badge", not crash.

## Explicitly deferred (do NOT do these)

- Corroboration of findings against change history — that is M5, a design checkpoint requiring real finding data first.
- Per-finding refactoring actions — M6.
- Making the badge column sortable — the three counts don't reduce to one sort key; YAGNI until asked.
- Scatter-plot/treemap badge markers — spec says "hotspot rows show a coupling badge"; rows only.

---

### Task 1: Config — `hotspot_multiplier` threshold with validation and init template

**Files:**
- Modify: `src/config/thresholds.rs` (struct `CouplingThresholds` ~line 135, its `Default` impl ~line 156)
- Modify: `src/config/mod.rs` (fn `validate` ~line 224; tests module ~line 284, next to `load_coupling_thresholds_from_toml` ~line 469)
- Modify: `src/init.rs` (the `[thresholds.coupling]` template block, after `content_barrel_rule` ~line 207)

**Interfaces:**
- Produces: `CouplingThresholds.hotspot_multiplier: f64` (serde-defaulted to 1.25) — Tasks 2, 3, 6 read it via the `&CouplingThresholds` already flowing into `build_report`.

- [ ] **Step 1: Write the failing tests** (in `src/config/mod.rs` tests module, next to `load_coupling_thresholds_defaults` / `load_coupling_thresholds_from_toml` — copy their structure)

```rust
    #[test]
    fn load_hotspot_multiplier_default() {
        let dir = TempDir::new().unwrap();
        let cfg = load(dir.path()).unwrap();
        assert!((cfg.thresholds.coupling.hotspot_multiplier - 1.25).abs() < f64::EPSILON);
    }

    #[test]
    fn load_hotspot_multiplier_from_toml() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join(".repository-analysis");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(
            cache_dir.join("barad-dur.toml"),
            "[thresholds.coupling]\nhotspot_multiplier = 1.5\n",
        )
        .unwrap();
        let cfg = load(dir.path()).unwrap();
        assert!((cfg.thresholds.coupling.hotspot_multiplier - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn validate_hotspot_multiplier_below_one_errors() {
        let mut cfg = RepoConfig::default();
        cfg.thresholds.coupling.hotspot_multiplier = 0.9;
        assert!(validate(&cfg).is_err(), "a discount multiplier is a config mistake");
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test config:: -- hotspot_multiplier`
Expected: compile error — no field `hotspot_multiplier` on `CouplingThresholds`.

- [ ] **Step 3: Implement**

In `src/config/thresholds.rs`, add to `CouplingThresholds` (after `content_barrel_rule`):

```rust
    /// Hotspot-score multiplier applied when a file carries Content or
    /// Common coupling findings — severity × change frequency = risk.
    /// Control findings never multiply (least severe rung).
    #[serde(default = "default_hotspot_multiplier")]
    pub hotspot_multiplier: f64,
```

Add next to the other default fns:

```rust
fn default_hotspot_multiplier() -> f64 {
    1.25
}
```

Extend the `Default` impl:

```rust
            hotspot_multiplier: default_hotspot_multiplier(),
```

In `src/config/mod.rs::validate`, after the `change_coupling_min_ratio` check:

```rust
    let multiplier = config.thresholds.coupling.hotspot_multiplier;
    if multiplier < 1.0 {
        bail!(
            "thresholds.coupling.hotspot_multiplier must be >= 1.0, got {}",
            multiplier
        );
    }
```

In `src/init.rs`, extend the `[thresholds.coupling]` block (keep the column alignment of the existing lines):

```rust
    out.push_str("hotspot_multiplier        = 1.25\n\n");
```

and remove the `\n\n` from the previous `content_barrel_rule` line (it becomes `"content_barrel_rule       = true\n"`), so the blank line stays after the last key of the section.

- [ ] **Step 4: Run config + init tests**

Run: `cargo test config:: && cargo test init`
Expected: all PASS — `generate_toml_is_valid` parses the new template line against `CouplingThresholds`' serde names; if it fails, the field name is wrong, don't bend the test.

- [ ] **Step 5: Commit**

```bash
git add src/config/thresholds.rs src/config/mod.rs src/init.rs
git commit -m "feat(config): add coupling hotspot_multiplier threshold"
```

---

### Task 2: `HotspotFile` per-kind finding counts

**Files:**
- Modify: `src/scorer/types.rs` (struct `HotspotFile` ~line 9)
- Modify: `src/scorer/builders.rs` (fn `build_hotspots` ~line 40; tests module ~line 439)
- Modify: `src/scorer.rs` (call site ~line 67: `build_hotspots(snapshot)` → pass `coupling`; test `build_hotspots_ranks_by_score` ~line 187)
- Modify: `src/renderer/html/tests.rs` (~lines 236–280: four `HotspotFile` literals gain the three new zero fields)
- Modify: `src/renderer/html/tests_extra.rs` (~line 252: one `HotspotFile` literal, same)

**Interfaces:**
- Consumes: `CouplingThresholds { content_barrel_rule, component_depth, hotspot_multiplier }` (Task 1); `crate::metrics::coupling::barrel_bypass_findings(snapshot, component_depth) -> Vec<CouplingFinding>` (exists, `pub(crate)`); `snapshot.coupling_findings: Vec<CouplingFinding>` where `CouplingFinding { path: PathBuf, kind: CouplingKind, .. }`.
- Produces: `HotspotFile` fields `content_findings: usize`, `common_findings: usize`, `control_findings: usize` (serialized into report JSON — Tasks 4, 5, 6 rely on these exact names); new signature `build_hotspots(snapshot: &RepoSnapshot, coupling: &CouplingThresholds) -> Vec<HotspotFile>`.

- [ ] **Step 1: Write the failing tests** (in `src/scorer/builders.rs` tests module; the coupling tests in `src/metrics/coupling/tests.rs` use `crate::metrics::testutil::{make_snapshot, make_file}` — same helpers work here)

```rust
    #[test]
    fn hotspot_rows_carry_per_kind_finding_counts() {
        use crate::snapshot::{CouplingFinding, CouplingKind};
        let mut snapshot = crate::metrics::testutil::make_snapshot();
        snapshot.files = vec![
            crate::metrics::testutil::make_file("src/dirty.rs"),
            crate::metrics::testutil::make_file("src/clean.rs"),
        ];
        snapshot.coupling_findings = vec![
            CouplingFinding {
                path: "src/dirty.rs".into(),
                line: Some(1),
                kind: CouplingKind::Common,
                evidence: "static mut CACHE: usize = 0;".into(),
            },
            CouplingFinding {
                path: "src/dirty.rs".into(),
                line: Some(9),
                kind: CouplingKind::Control,
                evidence: "pub fn go(fast: bool)".into(),
            },
        ];
        let cfg = crate::config::CouplingThresholds::default();
        let hotspots = build_hotspots(&snapshot, &cfg);
        let dirty = hotspots.iter().find(|h| h.path == "src/dirty.rs").unwrap();
        assert_eq!(
            (dirty.content_findings, dirty.common_findings, dirty.control_findings),
            (0, 1, 1)
        );
        let clean = hotspots.iter().find(|h| h.path == "src/clean.rs").unwrap();
        assert_eq!(
            (clean.content_findings, clean.common_findings, clean.control_findings),
            (0, 0, 0)
        );
    }

    #[test]
    fn hotspot_content_counts_include_barrel_findings_only_when_toggle_on() {
        // Cross-component import bypassing src/a's barrel — the same shape
        // the gate's ratchet_finding_sets tests use.
        let mut snapshot = crate::metrics::testutil::make_snapshot();
        snapshot.files = vec![
            crate::metrics::testutil::make_file("src/a/index.ts"),
            crate::metrics::testutil::make_file("src/a/impl.ts"),
            crate::metrics::testutil::make_file("src/b/user.ts"),
        ];
        snapshot.import_graph.insert(
            "src/b/user.ts".into(),
            vec!["src/a/impl.ts".into()],
        );
        let cfg = crate::config::CouplingThresholds::default();
        assert!(cfg.content_barrel_rule, "default toggle must be on");
        let on = build_hotspots(&snapshot, &cfg);
        let user_on = on.iter().find(|h| h.path == "src/b/user.ts").unwrap();
        assert_eq!(
            user_on.content_findings, 1,
            "barrel bypass joins the importing file's content count"
        );

        let cfg_off = crate::config::CouplingThresholds {
            content_barrel_rule: false,
            ..crate::config::CouplingThresholds::default()
        };
        let off = build_hotspots(&snapshot, &cfg_off);
        let user_off = off.iter().find(|h| h.path == "src/b/user.ts").unwrap();
        assert_eq!(
            user_off.content_findings, 0,
            "toggle off must mirror pressman_finding_counts' gating"
        );
    }
```

Note: if the second test's `content_findings == 1` assertion fails because `barrel_bypass_findings` attributes the finding to a different path than the importing file, read that function's `finding` construction and fix the TEST's expected path — the production rule is "counts land on whatever path the finding carries", not a re-interpretation.

- [ ] **Step 2: Run to verify compile failure**

Run: `cargo test scorer`
Expected: compile errors — `build_hotspots` takes 1 argument, and `HotspotFile` has no field `content_findings`.

- [ ] **Step 3: Implement**

`src/scorer/types.rs` — extend `HotspotFile` (after `hotspot_score`, before `churn_timeline`):

```rust
    /// Pressman coupling findings in this file, per kind. Content includes
    /// barrel-bypass findings when `content_barrel_rule` is on — the same
    /// gating as `pressman_finding_counts`, so the two views never disagree.
    pub content_findings: usize,
    pub common_findings: usize,
    pub control_findings: usize,
```

`src/scorer/builders.rs` — change the signature and add the count join. New imports at the top: extend the existing `use crate::snapshot::RepoSnapshot;` area with:

```rust
use std::path::Path;

use crate::config::CouplingThresholds;
use crate::metrics::coupling::{barrel_bypass_findings, extract_component};
use crate::snapshot::{CouplingKind, RepoSnapshot};
```

(`extract_component` is already imported — merge, don't duplicate. Adjust to the file's existing import grouping.)

```rust
pub(super) fn build_hotspots(
    snapshot: &RepoSnapshot,
    coupling: &CouplingThresholds,
) -> Vec<HotspotFile> {
```

Before the `let mut files: Vec<HotspotFile> = …` block, build the per-file counts:

```rust
    let barrel = if coupling.content_barrel_rule {
        barrel_bypass_findings(snapshot, coupling.component_depth)
    } else {
        Vec::new()
    };
    let finding_counts: HashMap<&Path, (usize, usize, usize)> = snapshot
        .coupling_findings
        .iter()
        .chain(barrel.iter())
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

Inside the per-file `map` closure, before the `HotspotFile {` literal:

```rust
            let (content_findings, common_findings, control_findings) = finding_counts
                .get(f.path.as_path())
                .copied()
                .unwrap_or((0, 0, 0));
```

and add to the literal (after `hotspot_score: 0.0,`):

```rust
                content_findings,
                common_findings,
                control_findings,
```

`src/scorer.rs:67` — the call site becomes:

```rust
    let file_hotspots = build_hotspots(snapshot, coupling);
```

Fix the remaining compile errors mechanically:
- `src/scorer.rs` test `build_hotspots_ranks_by_score` (~line 218) and the four `build_hotspots(&snapshot)` calls in `src/scorer/builders.rs` tests (~lines 823, 845, 866, 890): pass `&crate::config::CouplingThresholds::default()` as the second argument.
- `src/renderer/html/tests.rs` (four literals) and `src/renderer/html/tests_extra.rs` (one literal): add `content_findings: 0, common_findings: 0, control_findings: 0,` to each `HotspotFile` literal. The compiler lists every site.

- [ ] **Step 4: Run the affected suites**

Run: `cargo test scorer && cargo test renderer::html`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add src/scorer/types.rs src/scorer/builders.rs src/scorer.rs src/renderer/html/tests.rs src/renderer/html/tests_extra.rs
git commit -m "feat(hotspots): per-kind coupling finding counts on hotspot rows"
```

---

### Task 3: Content/Common multiplier on the hotspot score

**Files:**
- Modify: `src/scorer/builders.rs` (the normalization loop at the end of `build_hotspots`, ~line 109; tests module)

**Interfaces:**
- Consumes: `coupling.hotspot_multiplier` (Task 1), `HotspotFile.{content_findings,common_findings}` (Task 2)
- Produces: no new names — `hotspot_score` semantics change for flagged files.

- [ ] **Step 1: Write the failing tests** (builders.rs tests module)

```rust
    /// Two files identical in churn/CC/LOC; one carries a Common finding.
    /// Base score for both: cc_norm=1, loc_norm=1, churn=0 →
    /// (0.3 + 0.2) × 100 = 50. Flagged file: 50 × 1.25 = 62.5.
    fn twin_snapshot(kind: crate::snapshot::CouplingKind) -> crate::snapshot::RepoSnapshot {
        use crate::snapshot::{CouplingFinding, FileComplexity};
        let mut snapshot = crate::metrics::testutil::make_snapshot();
        snapshot.files = vec![
            crate::metrics::testutil::make_file("src/flagged.rs"),
            crate::metrics::testutil::make_file("src/plain.rs"),
        ];
        for p in ["src/flagged.rs", "src/plain.rs"] {
            snapshot.file_metrics.insert(
                p.into(),
                FileComplexity {
                    loc: 100,
                    cyclomatic_complexity: 10,
                    ..Default::default()
                },
            );
        }
        snapshot.coupling_findings = vec![CouplingFinding {
            path: "src/flagged.rs".into(),
            line: Some(1),
            kind,
            evidence: "evidence".into(),
        }];
        snapshot
    }

    #[test]
    fn common_finding_multiplies_hotspot_score() {
        let snapshot = twin_snapshot(crate::snapshot::CouplingKind::Common);
        let cfg = crate::config::CouplingThresholds::default();
        let hotspots = build_hotspots(&snapshot, &cfg);
        let flagged = hotspots.iter().find(|h| h.path == "src/flagged.rs").unwrap();
        let plain = hotspots.iter().find(|h| h.path == "src/plain.rs").unwrap();
        assert!((plain.hotspot_score - 50.0).abs() < 1e-9);
        assert!(
            (flagged.hotspot_score - 62.5).abs() < 1e-9,
            "50 × default 1.25 = 62.5, got {}",
            flagged.hotspot_score
        );
    }

    #[test]
    fn control_finding_does_not_multiply_hotspot_score() {
        let snapshot = twin_snapshot(crate::snapshot::CouplingKind::Control);
        let cfg = crate::config::CouplingThresholds::default();
        let hotspots = build_hotspots(&snapshot, &cfg);
        let flagged = hotspots.iter().find(|h| h.path == "src/flagged.rs").unwrap();
        assert!(
            (flagged.hotspot_score - 50.0).abs() < 1e-9,
            "control is the least severe rung — no multiplier (spec)"
        );
    }

    #[test]
    fn multiplied_hotspot_score_caps_at_100() {
        // Base score here is 50 (cc_norm=1, loc_norm=1, churn=0); a large
        // multiplier would push it to 500 — the cap must clamp to 100
        // because every consumer assumes the 0–100 domain.
        let snapshot = twin_snapshot(crate::snapshot::CouplingKind::Common);
        let cfg = crate::config::CouplingThresholds {
            hotspot_multiplier: 10.0,
            ..crate::config::CouplingThresholds::default()
        };
        let hotspots = build_hotspots(&snapshot, &cfg);
        let flagged = hotspots.iter().find(|h| h.path == "src/flagged.rs").unwrap();
        assert!(
            (flagged.hotspot_score - 100.0).abs() < 1e-9,
            "consumers assume 0–100; got {}",
            flagged.hotspot_score
        );
    }
```

(`FileComplexity` derives `Default`; `make_snapshot`/`make_file` come from `crate::metrics::testutil`.)

- [ ] **Step 2: Run to verify the multiplier tests fail**

Run: `cargo test scorer::builders -- multiplies_hotspot cap_at_100 does_not_multiply`
Expected: `common_finding_multiplies_hotspot_score` and `multiplied_hotspot_score_caps_at_100` FAIL (score stays 50); the control-kind negative passes.

- [ ] **Step 3: Implement** — in the normalization loop at the end of `build_hotspots`:

```rust
    for f in &mut files {
        let churn_norm = f.churn_count as f64 / max_churn as f64;
        let cc_norm = f.cyclomatic_complexity as f64 / max_cc as f64;
        let loc_norm = f.loc as f64 / max_loc as f64;
        let base = (churn_norm * 0.5 + cc_norm * 0.3 + loc_norm * 0.2) * 100.0;
        // Content/Common findings multiply risk (severity × change
        // frequency); capped because every consumer assumes 0–100.
        f.hotspot_score = if f.content_findings + f.common_findings > 0 {
            (base * coupling.hotspot_multiplier).min(100.0)
        } else {
            base
        };
    }
```

- [ ] **Step 4: Run the scorer suites**

Run: `cargo test scorer`
Expected: all PASS — including the pre-existing ranking/ordering tests (their snapshots have no findings, so scores are unchanged).

- [ ] **Step 5: Commit**

```bash
git add src/scorer/builders.rs
git commit -m "feat(hotspots): multiply hotspot score for content/common findings"
```

---

### Task 4: HTML report — coupling badge column

**Files:**
- Modify: `src/renderer/templates/hotspots.js` (COL_TIPS ~line 229; header row ~line 240; row builder ~line 264)
- Modify: `src/renderer/html/tests_extra.rs` (the hotspot fixture at ~line 252 + one new assertion)

**Interfaces:**
- Consumes: `content_findings` / `common_findings` / `control_findings` on each entry of the report JSON's `file_hotspots` (Task 2 serialization).
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Write the failing Rust test** (in `src/renderer/html/tests_extra.rs`; change the existing `HotspotFile` fixture's zero counts to `content_findings: 0, common_findings: 2, control_findings: 1,` and add next to the surrounding render assertions)

```rust
    #[test]
    fn hotspot_counts_are_embedded_in_report_json() {
        let report = report_with_hotspots(); // reuse the fixture fn the existing hotspot test uses — match its actual name
        let html = crate::renderer::render_html(&report).unwrap();
        assert!(
            html.contains("\"common_findings\":2"),
            "per-kind counts must reach the embedded report JSON"
        );
        assert!(html.contains("\"control_findings\":1"));
    }
```

(Read the top of `tests_extra.rs` first: reuse its existing report-construction helper and render call verbatim — the names above are descriptive, the file's real helper/render idiom wins.)

- [ ] **Step 2: Run to verify it fails or passes honestly**

Run: `cargo test renderer::html`
Expected: PASSES already if serde flowed the fields through (Task 2 added them to a `Serialize` struct — likely). If it passes: fine, it's a pin, keep it. If it fails: the fields aren't reaching the JSON — stop and find out why before touching JS.

- [ ] **Step 3: Add the column to `hotspots.js`**

COL_TIPS gains (after `LOC:`):

```js
        Coupling: 'Pressman coupling findings in this file: Cn = content, Cm = common, Ct = control. '
          + 'Content and Common findings multiply the Score (configurable hotspot_multiplier, default 1.25).'
```

Header row — next to the existing `trendTh` pattern, before the dismiss `th`:

```js
      var cplTh = el('th');
      cplTh.append(txt('Coupling'), tipIcon(COL_TIPS.Coupling));
```

and add `cplTh` to the `tr.append(…)` list between the LOC th and the dismiss th.

Row builder — after `locCell`, before `dismissCell`:

```js
        var cplCell = el('td');
        var cn = f.content_findings || 0;
        var cm = f.common_findings || 0;
        var ct = f.control_findings || 0;
        if (cn + cm + ct > 0) {
          var labels = [];
          if (cn) labels.push('Cn ' + cn);
          if (cm) labels.push('Cm ' + cm);
          if (ct) labels.push('Ct ' + ct);
          var badge = el('span', {
            style: {
              fontWeight: '600',
              color: (cn + cm > 0) ? '#f87171' : 'rgba(148,163,184,0.7)'
            }
          });
          badge.append(txt(labels.join(' · ')));
          cplCell.append(badge);
        } else {
          cplCell.append(txt('—'));
        }
```

and add `cplCell` to the `row.append(…)` list between `locCell` and `dismissCell`. (The `|| 0` guards are deliberate: a report generated before M4 has no such fields.)

- [ ] **Step 4: Run renderer tests + eyeball the report**

Run: `cargo test renderer && make html-report`
Expected: tests PASS; open `report.html`, Hotspots tab shows the Coupling column — barad-dur's own control findings (6 as of the pre-M4 dogfood) should appear as `Ct n` badges on their files.

- [ ] **Step 5: Commit**

```bash
git add src/renderer/templates/hotspots.js src/renderer/html/tests_extra.rs
git commit -m "feat(report): coupling badge column in hotspot table"
```

---

### Task 5: Dashboard — coupling badge column

**Files:**
- Modify: `dashboard/src/types.ts` (interface `HotspotFile` ~line 25)
- Modify: `dashboard/src/components/HotspotsView.tsx` (table header ~line 150, row ~line 162)
- Modify: `dashboard/src/components/HotspotsView.test.tsx` (new tests)

**Interfaces:**
- Consumes: report JSON `file_hotspots[].{content_findings,common_findings,control_findings}` (Task 2 names, exactly).
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Write the failing tests** (append to `HotspotsView.test.tsx`; the `file()` fixture helper deliberately stays without the new fields — that IS the old-report case)

```tsx
describe('HotspotsView coupling badge', () => {
  it('shows per-kind counts when findings are present', () => {
    const flagged: HotspotFile = { ...file('src/glob.ts', 80), common_findings: 2, control_findings: 1 }
    render(<HotspotsView files={[flagged]} />)
    expect(screen.queryByText('Cm 2 · Ct 1')).not.toBeNull()
  })

  it('renders an em dash for files without findings (and for pre-M4 reports)', () => {
    render(<HotspotsView files={[file('src/clean.ts', 70)]} />)
    // one em dash from Bugs (0 bugs) + one from Coupling
    expect(screen.getAllByText('—').length).toBe(2)
  })
})
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd dashboard && pnpm vitest run HotspotsView`
Expected: TS compile error — `common_findings` not on `HotspotFile` — then, after Step 3's types change alone, badge-text query failure.

- [ ] **Step 3: Implement**

`dashboard/src/types.ts` — extend the interface (after `hotspot_score`):

```ts
  // Per-kind Pressman coupling finding counts. Optional: reports generated
  // before M4 don't carry them.
  content_findings?: number
  common_findings?: number
  control_findings?: number
```

`HotspotsView.tsx` — header row, after the Props th, before the dismiss th:

```tsx
              <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400, color: 'rgba(148,163,184,0.5)' }}>Coupling</th>
```

Row, after the Props td, before the dismiss td (inside the map callback, next to the existing `dir`/`name` consts):

```tsx
              const cn = f.content_findings ?? 0
              const cm = f.common_findings ?? 0
              const ct = f.control_findings ?? 0
              const badge = [cn > 0 && `Cn ${cn}`, cm > 0 && `Cm ${cm}`, ct > 0 && `Ct ${ct}`]
                .filter(Boolean)
                .join(' · ')
```

```tsx
                  <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: cn + cm > 0 ? '#f87171' : 'rgba(148,163,184,0.45)', fontWeight: cn + cm > 0 ? 600 : 400 }}>{badge || '—'}</td>
```

- [ ] **Step 4: Run the dashboard suite**

Run: `cd dashboard && pnpm vitest run`
Expected: all PASS, including the pre-existing dismiss/sort tests (the fixture change is additive-optional).

- [ ] **Step 5: Commit**

```bash
git add dashboard/src/types.ts dashboard/src/components/HotspotsView.tsx dashboard/src/components/HotspotsView.test.tsx
git commit -m "feat(dashboard): coupling badge column in hotspots view"
```

---

### Task 6: M4 milestone integration test

**Files:**
- Create: `tests/pressman_coupling_milestone_4.rs`

**Interfaces:**
- Consumes: public crate API only — `barad_dur::scorer::build_report`, `barad_dur::config::RepoConfig`, `barad_dur::snapshot::{RepoSnapshot, FileEntry, FileComplexity, CouplingFinding, CouplingKind, TimeWindow}` (all `pub`; `FileEntry` construction pattern is in `tests/pressman_coupling_milestone_2.rs`).

- [ ] **Step 1: Write the test file**

```rust
//! M4: hotspot rows cross-reference Pressman findings — per-kind counts,
//! the Content/Common score multiplier, and the JSON contract the HTML
//! report and dashboard read.

use barad_dur::config::RepoConfig;
use barad_dur::scorer;
use barad_dur::snapshot::{
    CouplingFinding, CouplingKind, FileComplexity, FileEntry, RepoSnapshot, TimeWindow,
};
use std::path::PathBuf;

fn synthetic_snapshot() -> RepoSnapshot {
    let mut s = RepoSnapshot::new(
        PathBuf::from("/tmp/m4"),
        "m4".into(),
        "main".into(),
        TimeWindow::default(),
    );
    for p in ["src/flagged.rs", "src/plain.rs"] {
        s.files.push(FileEntry {
            path: PathBuf::from(p),
            size_bytes: 1,
            is_binary: false,
            depth: 2,
            blob_oid: String::new(),
        });
        s.file_metrics.insert(
            PathBuf::from(p),
            FileComplexity {
                loc: 100,
                cyclomatic_complexity: 10,
                ..Default::default()
            },
        );
    }
    s.coupling_findings = vec![CouplingFinding {
        path: PathBuf::from("src/flagged.rs"),
        line: Some(3),
        kind: CouplingKind::Common,
        evidence: "static mut CACHE: usize = 0;".into(),
    }];
    s
}

#[test]
fn report_hotspots_carry_counts_and_multiplied_score() {
    let snapshot = synthetic_snapshot();
    let cfg = RepoConfig::default();
    let report = scorer::build_report(
        &snapshot,
        Vec::new(),
        None,
        &cfg.weights.as_weight_pairs(),
        &cfg.thresholds.coupling,
    );

    let flagged = report
        .file_hotspots
        .iter()
        .find(|h| h.path == "src/flagged.rs")
        .expect("flagged file must be a hotspot row");
    let plain = report
        .file_hotspots
        .iter()
        .find(|h| h.path == "src/plain.rs")
        .expect("plain file must be a hotspot row");

    assert_eq!(
        (flagged.content_findings, flagged.common_findings, flagged.control_findings),
        (0, 1, 0)
    );
    // Identical churn/CC/LOC twins: only the multiplier separates them.
    assert!(
        flagged.hotspot_score > plain.hotspot_score,
        "common finding must raise the hotspot score ({} vs {})",
        flagged.hotspot_score,
        plain.hotspot_score
    );
    let ratio = flagged.hotspot_score / plain.hotspot_score;
    assert!(
        (ratio - cfg.thresholds.coupling.hotspot_multiplier).abs() < 1e-9,
        "score ratio must equal the configured multiplier, got {ratio}"
    );
}

#[test]
fn hotspot_json_contract_for_renderers() {
    let snapshot = synthetic_snapshot();
    let cfg = RepoConfig::default();
    let report = scorer::build_report(
        &snapshot,
        Vec::new(),
        None,
        &cfg.weights.as_weight_pairs(),
        &cfg.thresholds.coupling,
    );
    let json = serde_json::to_value(&report.file_hotspots).unwrap();
    let flagged = json
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["path"] == "src/flagged.rs")
        .unwrap();
    // Exact field names the HTML template and dashboard read.
    assert_eq!(flagged["content_findings"], 0);
    assert_eq!(flagged["common_findings"], 1);
    assert_eq!(flagged["control_findings"], 0);
}
```

(If `FileComplexity`/`CouplingFinding` turn out not to be re-exported at `barad_dur::snapshot`, fix the `use` path to wherever `pub` actually exposes them — `tests/pressman_coupling_milestone_2.rs` imports from `barad_dur::snapshot`, so it likely just works. If `serde_json` isn't a dev-dependency of the crate root, check `Cargo.toml` `[dev-dependencies]` — it's used by existing tests, so it should be.)

- [ ] **Step 2: Run it**

Run: `cargo test --test pressman_coupling_milestone_4`
Expected: both PASS (Tasks 1–3 already landed the behavior; this suite pins the E2E contract through `build_report` + serde).

- [ ] **Step 3: Commit**

```bash
git add tests/pressman_coupling_milestone_4.rs
git commit -m "test(coupling): M4 milestone — hotspot finding counts and multiplier E2E"
```

---

### Task 7: Docs + final sweep

**Files:**
- Modify: `README.md` (Hotspots tab bullet ~line 71; `file_hotspots` JSON table row ~line 503; Unreleased changelog ~line 568)

- [ ] **Step 1: README updates** — read each location first, then:

(a) The Hotspots tab bullet (~line 71) — append to the existing sentence, before the final `; clicking a row…` clause stays intact:

```markdown
- **Hotspots** — scatter plot (complexity vs churn, radius = LOC) with axis ticks + sortable, filterable table (score, CC, churn, bug-fix commits, LOC, per-kind coupling badge); clicking a row or a bubble highlights its counterpart
```

(b) The `file_hotspots` row of the JSON-fields table (~line 503):

```markdown
| `file_hotspots` | array | Files ranked by hotspot score (churn x complexity x LOC), incl. bug-fix commit counts and per-kind coupling finding counts; Content/Common findings multiply the score (`thresholds.coupling.hotspot_multiplier`, default 1.25) |
```

(c) The **Unreleased** changelog bullet (~line 568) — append to the existing comma-separated list:

```markdown
, hotspot–coupling cross-referencing (per-kind finding counts + configurable score multiplier + badge in report and dashboard)
```

- [ ] **Step 2: Full verification sweep**

Run: `env RUSTFLAGS='-D warnings' cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check && cd dashboard && pnpm vitest run && cd ..`
Expected: everything green.

- [ ] **Step 3: Dogfood**

Run: `cargo run --quiet -- analyze . --no-cache -v`
Expected: runs clean. The 6 known control-coupling findings in barad-dur do NOT change any hotspot score (control never multiplies) — if the Coupling/overall score moved vs the pre-M4 baseline, something multiplied that shouldn't have; stop and report.

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: hotspot coupling badge and multiplier"
```

---

## Final verification (after all tasks)

- [ ] `env RUSTFLAGS='-D warnings' cargo test` — full suite green
- [ ] `cargo clippy --all-targets -- -D warnings && cargo fmt -- --check`
- [ ] `cd dashboard && pnpm vitest run` — dashboard suite green
- [ ] `make html-report` — Hotspots tab shows the Coupling column with real `Ct` badges on barad-dur's own flagged files; files without findings show an em dash
- [ ] `make gate-coupling` — ratchet still passes vs origin/main (M4 adds no findings; it only cross-references existing ones)
