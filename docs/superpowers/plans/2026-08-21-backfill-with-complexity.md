# Backfill --with-complexity Implementation Plan (Trends M4)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Opt-in per-hotspot complexity history (Crime Scene Ch. 6): `barad-dur backfill --with-complexity` runs the AST pass at each sampled SHA and stores cyclomatic complexity + LOC for the current top hotspots, surfaced as a rising/flat/falling annotation on hotspot rows.

**Architecture:** The `--with-complexity` flag switches backfill's collector call from `collect_snapshot_at` to the already-shipped `collect_snapshot_at_with_ast` (in-process libgit2 blob walk — the gate-ratchet baseline path), records `{path, cyclomatic_complexity, loc}` for the current HEAD's top `hotspot_top_n` files into an additive-optional `HistoryEntry` field, and `analyze` post-processes hotspot rows with a trend direction computed from ≥3 stored points. ADR-005 is amended (addendum), not repealed: default backfill is untouched.

**Tech Stack:** Rust; `git2` blob walk via existing `ast_pass_at`; serde-additive history schema (stays `schema_version: 1`).

**Spec:** `docs/plans/2026-08-20-longitudinal-trends-design.md` § M4 (including its cost model and abort criterion).

## Global Constraints

- **Merge gate from the design:** dogfood timing must show ≤ 10 s per sample on this repo (release mode) or the design's sampling count is revisited **before merge** — measured, not assumed. The flag always prints per-sample timing either way.
- ADR-005 amended via addendum; default backfill behavior byte-identical (no AST, no blame changes, Coupling still excluded).
- `HistoryEntry` changes are additive `Option` + `#[serde(default)]` only — `schema_version` stays 1; old files load into new code (`None`) and new files into old code (field skipped). Snapshot `CACHE_VERSION` untouched.
- Trend is an **annotation, never scored** (risk-5 stance; the M3/corroboration precedent).
- TDD; `cargo mutants --in-diff` ≥ 80%; both-sides boundary tests on every threshold; new actions.rs arms need pin tests in the same MR (only if arms are added — an unscored annotation adds none).
- Commits via `git commit -F <file>`; never mention AI.

---

### Task 1: CLI flag + AST collection + per-sample timing

**Files:**
- Modify: `src/cli/mod.rs` (`BackfillArgs` gains the flag, next to `no_blame`)
- Modify: `src/backfill/mod.rs` (`run()`: collector switch + timing print; note `run(_args, …)` currently ignores its args — this task starts using them)
- Test: `src/backfill/mod.rs` or `src/cli/mod.rs` unit tests (flag parsing), collector-level test exists already (`ast_pass_at` suite)

**Interfaces:**
- Produces: `BackfillArgs.with_complexity: bool`; backfill snapshots carry `file_metrics` when the flag is set.

- [ ] **Step 1: Write the failing CLI test** (pattern: existing `cli::tests` arg tests):

```rust
#[test]
fn backfill_with_complexity_flag_parses() {
    let cli = Cli::parse_from(["barad-dur", "backfill", ".", "--with-complexity"]);
    let Commands::Backfill(args) = cli.command else { panic!() };
    assert!(args.with_complexity);
    let cli = Cli::parse_from(["barad-dur", "backfill", "."]);
    let Commands::Backfill(args) = cli.command else { panic!() };
    assert!(!args.with_complexity, "default off — ADR-005 default path untouched");
}
```

- [ ] **Step 2: Verify RED**, then add the flag:

```rust
/// Also run the AST pass at each sampled commit and store per-hotspot
/// complexity history (slower; see ADR-005 addendum).
#[arg(long)]
pub with_complexity: bool,
```

- [ ] **Step 3: Switch the collector call** in `run()`'s sample loop and time it:

```rust
let t = std::time::Instant::now();
let snapshot = if args.with_complexity {
    Collector::collect_snapshot_at_with_ast(repo_path, sha, &ignore, true)?
} else {
    Collector::collect_snapshot_at(repo_path, sha, &ignore, true)?
};
if args.with_complexity {
    println!("    AST sample took {:.1}s", t.elapsed().as_secs_f32());
}
```

(check `collect_snapshot_at_with_ast`'s exact signature at `src/collector/snapshot_builder.rs:412` and match it — it may differ from `collect_snapshot_at`'s).
- [ ] **Step 4:** `cargo test` green; commit — `feat(backfill): --with-complexity flag runs the AST pass per sample`.

### Task 2: History data model

**Files:**
- Modify: `src/scorer/types.rs` (`HistoryEntry` + new point struct)
- Test: `src/scorer/types.rs` serde tests (an existing history-serde test module pins field presence)

**Interfaces:**
- Produces: `HistoryEntry.hotspot_complexity: Option<Vec<HotspotComplexityPoint>>`; `pub struct HotspotComplexityPoint { pub path: String, pub cyclomatic_complexity: u32, pub loc: usize }` — consumed by Tasks 3 and 4.

- [ ] **Step 1: Write the failing serde tests**: (a) an old-format JSON entry (no field) deserializes with `None`; (b) a populated entry round-trips with the vec sorted by path; (c) `None` serializes to *no key* (`skip_serializing_if`).
- [ ] **Step 2: Verify RED, implement:**

```rust
/// Per-hotspot complexity at this sample (`backfill --with-complexity`
/// only). Additive-optional: absent everywhere else, so schema_version
/// stays 1 (old readers skip it, old entries read as None).
#[serde(default, skip_serializing_if = "Option::is_none")]
pub hotspot_complexity: Option<Vec<HotspotComplexityPoint>>,
```

  `HotspotComplexityPoint` derives `Debug, Clone, PartialEq, Serialize, Deserialize`.
- [ ] **Step 3:** green; commit — `feat(scorer): additive hotspot_complexity field on HistoryEntry`.

### Task 3: Record points for the current top hotspots

**Files:**
- Modify: `src/backfill/mod.rs` (compute the triage set once before the loop; fill the field per sample)

**Interfaces:**
- Consumes: `build_hotspots` ranking at current HEAD, `cfg.thresholds.health.hotspot_top_n` (default 10), Task 2's types.
- Produces: populated `hotspot_complexity` on every `--with-complexity` entry.

- [ ] **Step 1: Write the failing integration-style test** (a `tests/` fixture repo, or extend backfill's unit tests if a fixture helper exists): after `backfill --with-complexity` on a 3-commit repo whose HEAD hotspot is `a.rs`, every written entry has `hotspot_complexity` = points for `a.rs` with the complexity/LOC of *that sample's* tree (assert two samples differ when the file grew between them); without the flag the field is absent.
- [ ] **Step 2: Verify RED, implement.** Before the sample loop: collect the *current* snapshot (the regular cached path used by `analyze`), run `build_hotspots(&snapshot, &cfg.thresholds.coupling, &Default::default())`, keep the top `hotspot_top_n` paths as the triage set (design: "trend the triaged hotspots", reset-on-rename documented). In the loop, when the flag is set:

```rust
entry.hotspot_complexity = Some(
    triage_paths
        .iter()
        .filter_map(|p| snapshot.file_metrics.get(p).map(|m| HotspotComplexityPoint {
            path: p.to_string_lossy().into_owned(),
            cyclomatic_complexity: m.cyclomatic_complexity,
            loc: m.loc,
        }))
        .collect(),
);
```

  (a triaged file absent at an old SHA simply yields no point — the honest short series the design describes; keep the vec sorted by path for deterministic JSON).
- [ ] **Step 3:** green; commit — `feat(backfill): store per-hotspot complexity points per sample`.

### Task 4: `complexity_trend` annotation on hotspot rows

**Files:**
- Modify: `src/scorer/types.rs` (`HotspotFile.complexity_trend: Option<String>` — `#[serde(skip_serializing_if = "Option::is_none")]`)
- Create: `src/trend.rs` addition or small pure helper `hotspot_complexity_direction(points: &[(i64, u32)]) -> Option<&'static str>` (timestamped CC values → `"rising"/"flat"/"falling"`, `None` below 3 points; reuse `DIRECTION_THRESHOLD`'s per-run-slope idea from `src/trend.rs:18`)
- Modify: `src/cmd/analyze.rs` (post-process after `build_report`, the `report.dep_ecosystem_reports = dep_reports;` precedent: load history via `cache::history::load_history`, group points per path, set the field on matching rows)
- Modify: `src/renderer/templates/hotspots.js` + `chrome.js` (badge next to the reach badge; tooltip text names the ≥3-point requirement and the flag)

- [ ] **Step 1: Write the failing pure-helper tests** — both sides of the direction threshold with exact synthetic points (3 points rising, 3 flat, 3 falling, 2 points → `None`), plus a `cmd`-level test that a report built with a seeded history file carries `complexity_trend: Some("rising")` on the matching row and `None` elsewhere.
- [ ] **Step 2: Verify RED, implement**; annotation only — no score change anywhere.
- [ ] **Step 3:** `make report-smoke` clean; commit — `feat(trends): rising/flat/falling complexity annotation on hotspots`.

### Task 5: Dogfood timing gate + ADR addendum + docs

**Files:**
- Modify: `docs/adrs/ADR-005-backfill-skips-complexity-metrics.md` (addendum section)
- Modify: `docs/crime-scene-book-notes.md` (Ch. 6 row), `README.md`/`src/cli` help text if backfill flags are enumerated, `docs/plans/2026-08-20-longitudinal-trends-design.md` M4 revision note

- [ ] **Step 1: Measure** — `cargo build --release && time ./target/release/barad-dur backfill . --with-complexity` on this repo (fresh `.repository-analysis`, default `sample_count = 10`). Record per-sample timings from the printed lines.
- [ ] **Step 2: Apply the abort criterion** — per-sample ≤ 10 s: proceed. Above: STOP, do not merge; take the measured number back to the design's sampling-count discussion and record the ruling in the MR before continuing.
- [ ] **Step 3: Write the ADR-005 addendum**: the flag, the measured numbers, what stays skipped by default, reset-on-rename limitation, Coupling category still excluded.
- [ ] **Step 4: Update tracker/design notes; commit** — `docs: ADR-005 addendum and Ch. 6 tracker update (measured Ns/sample)` with real numbers.
- [ ] **Step 5: MR**; wait for the pipeline to register before `glab mr merge --auto-merge`; include the dogfood timing table in the MR description.
