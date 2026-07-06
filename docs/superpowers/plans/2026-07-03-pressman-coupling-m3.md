# Pressman Coupling M3 — Gate Ratchet Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `barad-dur gate --no-new-coupling --baseline-ref <ref>` fails CI when HEAD introduces Pressman coupling findings that the baseline commit didn't have — fail-loud, explicit baseline, thoroughly documented.

**Architecture:** The baseline snapshot is collected at `<ref>` with a **blob-based AST pass** (new opt-in on `collect_snapshot_at`: reads file contents from the git object DB via each `FileEntry.blob_oid`, runs the same `analyse_file` + `extract_coupling_findings` as the live pass — populating `file_metrics` so `detection_ran` holds). Counts come from the existing single source `pressman_finding_counts`. A pure ratchet function diffs per-kind counts and set-diffs findings by `(path, kind, evidence)` (line numbers shift across commits) to name the new ones. Documentation is a first-class deliverable (spec M3 mandates it).

**Tech Stack:** Rust (git2 blob reads, clap), GitLab CI docs. Branch `feat/pressman-coupling-m3` (stacked on M2).

## Global Constraints

- TDD strictly; `RUSTFLAGS="-D warnings" cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt` clean per commit; conventional commits; never mention Claude/AI.
- **Fail loud, fail closed** (spec resolved question 3): `--no-new-coupling` or `--max-new-coupling` without `--baseline-ref` is a usage error; unresolvable ref is a hard error mentioning `GIT_DEPTH: 0` / `git fetch`. No history-file fallback, ever.
- **Spec-mechanism correction (record in the spec, Task 6):** the spec's M3 text says the baseline uses "the same `collect_snapshot_at` machinery backfill uses; cached by commit hash". Two corrections: (a) plain `collect_snapshot_at` skips the AST pass (ADR-005) and thus carries no findings — the ratchet needs the new blob-based AST opt-in; backfill keeps passing `false` (its performance contract is untouched). (b) There is no per-commit snapshot cache, and CI runs on fresh clones where one would not help — the baseline is collected uncached, cost documented (one AST pass over the tree at `<ref>`).
- Baseline uses the **same exclusion policy** as the HEAD analysis: `collect_snapshot_at` already applies `BaradDurIgnore` + defaults; gate builds the matcher exactly like backfill does.
- `--max-new-coupling <n>` = allowed **total** new findings summed across kinds (spec).
- Kill-rate discipline: the CI mutation gate bit M1 for bound-style assertions — every new match/comparison in this plan gets exact-value tests.
- New-finding identity = `(path, kind, evidence)`, NOT line numbers.
- Mutation shards run on files changed in the last 25h — expect `cmd/gate.rs` and `snapshot_builder.rs` scoping; write tests accordingly (exact assertions, both polarity cases).

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/collector/snapshot_builder.rs` | Modify | `collect_snapshot_at` gains `run_ast: bool`; blob-based per-file pass |
| `src/cli/mod.rs` | Modify | `GateArgs` gains `no_new_coupling`, `max_new_coupling`, `baseline_ref` |
| `src/cmd/gate.rs` | Modify | ratchet wiring, pure verdict fn, error copy |
| `src/backfill/mod.rs` | Modify | pass `run_ast: false` (call-site only) |
| `docs/gate-coupling.md` | Create | full ratchet documentation (CI examples) |
| `README.md` | Modify | short gate-ratchet section linking the doc |
| `Makefile` | Modify | `gate-coupling` target |
| `docs/superpowers/specs/2026-07-02-pressman-coupling-design.md` | Modify | mechanism corrections |
| `tests/pressman_coupling_milestone_3.rs` | Create | fixture-repo E2E: fail on new finding, pass with allowance, hard error on bad ref |

---

### Task 1: Blob-based AST pass in `collect_snapshot_at`

**Files:**
- Modify: `src/collector/snapshot_builder.rs:287` (signature + body), `src/backfill/mod.rs:55` (call site adds `false`), the two existing `collect_snapshot_at` tests in snapshot_builder.rs (add `false`)

**Interfaces:**
- Produces: `pub(crate) fn collect_snapshot_at(repo_path: &Path, sha: &str, _skip_blame: bool, ignore: &BaradDurIgnore, run_ast: bool) -> Result<RepoSnapshot>`. With `run_ast: true`, `file_metrics`, `import_graph` (via `resolve_imports`), and `coupling_findings` (sorted by `(path, line)`) are populated from blob contents at `sha`; with `false`, behavior is byte-identical to today.

- [ ] **Step 1: Failing test** — in snapshot_builder.rs tests, extend the existing throwaway-repo helper (`repo with one commit`, ~line 448) or add a second commit containing `static mut CACHE: usize = 0;` in a `.rs` file, then:

```rust
    #[test]
    fn collect_snapshot_at_with_ast_populates_findings() {
        let (dir, head) = make_single_commit_repo_with(&[(
            "src/lib.rs",
            "static mut CACHE: usize = 0;\npub fn f() {}\n",
        )]);
        let ignore = BaradDurIgnore::load(dir.path()).unwrap();
        let snap = Collector::collect_snapshot_at(dir.path(), &head, true, &ignore, true).unwrap();
        assert!(!snap.file_metrics.is_empty(), "AST pass must populate file_metrics");
        assert_eq!(snap.coupling_findings.len(), 1);
        assert_eq!(snap.coupling_findings[0].kind, crate::snapshot::CouplingKind::Common);
    }

    #[test]
    fn collect_snapshot_at_without_ast_stays_empty() {
        let (dir, head) = make_single_commit_repo_with(&[(
            "src/lib.rs",
            "static mut CACHE: usize = 0;\n",
        )]);
        let ignore = BaradDurIgnore::load(dir.path()).unwrap();
        let snap = Collector::collect_snapshot_at(dir.path(), &head, true, &ignore, false).unwrap();
        assert!(snap.file_metrics.is_empty(), "ADR-005 contract unchanged");
        assert!(snap.coupling_findings.is_empty());
    }
```

Write `make_single_commit_repo_with(files: &[(&str, &str)]) -> (tempfile::TempDir, String)` by generalizing the existing fixture helper at ~line 448 (git init, write files, add, commit via git2 or std::process git — match whatever the existing helper does; refactor it to delegate to the new one rather than duplicating).

- [ ] **Step 2: RED** — `cargo test --lib collect_snapshot_at` fails to compile (arity).

- [ ] **Step 3: Implement** — inside `collect_snapshot_at`, after `files` is built:

```rust
        let (file_metrics, import_graph, coupling_findings) = if run_ast {
            ast_pass_at(&repo, &files)?
        } else {
            (HashMap::new(), HashMap::new(), Vec::new())
        };
```

and the new helper in the same file:

```rust
/// AST pass over blob contents at a historical commit — the object-DB
/// equivalent of `collect_file_metrics_with_progress` (which reads the
/// working tree). Used by the gate ratchet's baseline collection; backfill
/// keeps this off per ADR-005.
fn ast_pass_at(
    repo: &git2::Repository,
    files: &[FileEntry],
) -> Result<(
    HashMap<PathBuf, FileComplexity>,
    HashMap<PathBuf, Vec<PathBuf>>,
    Vec<CouplingFinding>,
)> {
    let mut file_metrics = HashMap::new();
    let mut raw_imports: RawImports = HashMap::new();
    let mut coupling_findings = Vec::new();
    for entry in files.iter().filter(|f| !f.is_binary) {
        let oid = match git2::Oid::from_str(&entry.blob_oid) {
            Ok(o) => o,
            Err(_) => continue,
        };
        let Ok(blob) = repo.find_blob(oid) else { continue };
        let Ok(content) = std::str::from_utf8(blob.content()) else {
            continue;
        };
        file_metrics.insert(entry.path.clone(), complexity::analyse_file(&entry.path, content));
        let imports = complexity::extract_file_imports(&entry.path, content);
        if !imports.is_empty() {
            raw_imports.insert(entry.path.clone(), imports);
        }
        coupling_findings.extend(complexity::extract_coupling_findings(&entry.path, content));
    }
    coupling_findings.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
    let import_graph = resolve_imports(&raw_imports, files);
    Ok((file_metrics, import_graph, coupling_findings))
}
```

Wire the three values into the `RepoSnapshot` literal (replacing the empty ones). Update backfill's call site and the two existing tests with `false`. Note the pass is sequential (no rayon) — baseline trees are collected once per gate run; keep it simple, note it in the doc comment.

- [ ] **Step 4:** `RUSTFLAGS="-D warnings" cargo test --lib` green.
- [ ] **Step 5:** Commit `feat(collector): opt-in blob-based AST pass for historical snapshots`.

---

### Task 2: CLI flags

**Files:**
- Modify: `src/cli/mod.rs:106-121` (`GateArgs`)

**Interfaces:**
- Produces on `GateArgs`:

```rust
    /// Fail if HEAD introduces Pressman coupling findings absent at --baseline-ref
    ///
    /// Requires --baseline-ref. Recommended CI baseline: the MR merge base
    /// ($CI_MERGE_REQUEST_DIFF_BASE_SHA on GitLab) so the gate measures what
    /// this branch adds, immune to the target branch moving.
    #[arg(long, requires = "baseline_ref")]
    pub no_new_coupling: bool,

    /// Allow up to N new coupling findings in total (summed across kinds)
    ///
    /// Implies the ratchet check; requires --baseline-ref. Useful mid-cleanup.
    #[arg(long, value_name = "N", requires = "baseline_ref")]
    pub max_new_coupling: Option<usize>,

    /// Git ref to compare coupling findings against (e.g. the MR merge base)
    ///
    /// The ref's tree is analyzed with the same detectors and exclusion rules
    /// as HEAD. Shallow clones must fetch it first (GIT_DEPTH: 0 on GitLab).
    #[arg(long, value_name = "REF")]
    pub baseline_ref: Option<String>,
```

- [ ] **Step 1: Failing test** — clap validation unit tests in `src/cli/mod.rs`'s test module (follow existing arg-test style there, or add one):

```rust
    #[test]
    fn gate_ratchet_requires_baseline_ref() {
        use clap::Parser;
        let err = Cli::try_parse_from(["barad-dur", "gate", "--no-new-coupling"]).unwrap_err();
        assert!(err.to_string().contains("--baseline-ref"));
        assert!(Cli::try_parse_from([
            "barad-dur", "gate", "--no-new-coupling", "--baseline-ref", "origin/main"
        ])
        .is_ok());
        assert!(Cli::try_parse_from([
            "barad-dur", "gate", "--max-new-coupling", "3", "--baseline-ref", "abc123"
        ])
        .is_ok());
    }
```

- [ ] **Step 2-4:** RED (fields missing) → implement → green (`cargo test --lib cli`).
- [ ] **Step 5:** Commit `feat(cli): gate ratchet flags --no-new-coupling/--max-new-coupling/--baseline-ref`.

---

### Task 3: Pure ratchet verdict

**Files:**
- Modify: `src/cmd/gate.rs` (new pure functions + unit tests in its existing test module)

**Interfaces:**
- Consumes: `CouplingFindingCounts`, `CouplingFinding`, `pressman_finding_counts` (pub(crate) — cmd is in-crate).
- Produces:

```rust
pub(crate) struct RatchetVerdict {
    pub failed: bool,
    /// (kind label, baseline count, head count) for every kind that increased.
    pub increases: Vec<(&'static str, usize, usize)>,
    /// Findings present at HEAD but not at baseline, identity (path, kind, evidence).
    pub new_findings: Vec<CouplingFinding>,
    pub total_new: usize,
}

pub(crate) fn ratchet_verdict(
    baseline: &CouplingFindingCounts,
    head: &CouplingFindingCounts,
    baseline_findings: &[CouplingFinding],
    head_findings: &[CouplingFinding],
    max_new: usize, // 0 for --no-new-coupling
) -> RatchetVerdict
```

Semantics: `total_new = new_findings.len()` (set difference by `(path, kind, evidence)`); `failed = total_new > max_new`; `increases` lists kinds where `head > baseline` count. Note counts and set-diff can disagree when findings move between files while totals stay flat — the set diff is the source of truth for `failed`; counts are display context.

- [ ] **Step 1: Failing tests** (exact assertions — mutation gate):

```rust
    fn finding(path: &str, kind: CouplingKind, evidence: &str) -> CouplingFinding {
        CouplingFinding { path: path.into(), line: Some(1), kind, evidence: evidence.into() }
    }

    #[test]
    fn ratchet_passes_when_head_equals_baseline() {
        let f = vec![finding("a.rs", CouplingKind::Common, "static mut X")];
        let c = CouplingFindingCounts { content: 0, common: 1, control: 0 };
        let v = ratchet_verdict(&c, &c, &f, &f, 0);
        assert!(!v.failed);
        assert_eq!(v.total_new, 0);
        assert!(v.increases.is_empty());
    }

    #[test]
    fn ratchet_fails_on_one_new_finding_and_names_it() {
        let base = vec![finding("a.rs", CouplingKind::Common, "static mut X")];
        let head = vec![
            finding("a.rs", CouplingKind::Common, "static mut X"),
            finding("b.rs", CouplingKind::Content, "#[path = \"../x.rs\"]"),
        ];
        let cb = CouplingFindingCounts { content: 0, common: 1, control: 0 };
        let ch = CouplingFindingCounts { content: 1, common: 1, control: 0 };
        let v = ratchet_verdict(&cb, &ch, &base, &head, 0);
        assert!(v.failed);
        assert_eq!(v.total_new, 1);
        assert_eq!(v.new_findings[0].path, std::path::PathBuf::from("b.rs"));
        assert_eq!(v.increases, vec![("content", 0, 1)]);
    }

    #[test]
    fn ratchet_allowance_admits_exactly_n() {
        let base: Vec<CouplingFinding> = vec![];
        let head = vec![
            finding("a.rs", CouplingKind::Control, "pub fn f(flag: bool)"),
            finding("b.rs", CouplingKind::Control, "pub fn g(flag: bool)"),
        ];
        let cb = CouplingFindingCounts { content: 0, common: 0, control: 0 };
        let ch = CouplingFindingCounts { content: 0, common: 0, control: 2 };
        assert!(!ratchet_verdict(&cb, &ch, &base, &head, 2).failed, "n == allowance passes");
        assert!(ratchet_verdict(&cb, &ch, &base, &head, 1).failed, "n > allowance fails");
    }

    #[test]
    fn ratchet_ignores_line_number_shifts() {
        let mut moved = finding("a.rs", CouplingKind::Common, "static mut X");
        moved.line = Some(99);
        let base = vec![finding("a.rs", CouplingKind::Common, "static mut X")];
        let c = CouplingFindingCounts { content: 0, common: 1, control: 0 };
        let v = ratchet_verdict(&c, &c, &base, &[moved], 0);
        assert!(!v.failed, "same finding at a different line is not new");
    }

    #[test]
    fn ratchet_removed_findings_do_not_mask_new_ones() {
        // one removed + one added elsewhere: counts flat, set diff catches it
        let base = vec![finding("a.rs", CouplingKind::Common, "static mut X")];
        let head = vec![finding("b.rs", CouplingKind::Common, "static mut Y")];
        let c = CouplingFindingCounts { content: 0, common: 1, control: 0 };
        let v = ratchet_verdict(&c, &c, &base, &head, 0);
        assert!(v.failed, "moved-plus-renamed global is a new finding even with flat counts");
        assert_eq!(v.total_new, 1);
    }
```

- [ ] **Step 2: RED** → **Step 3: Implement:**

```rust
pub(crate) fn ratchet_verdict(
    baseline: &CouplingFindingCounts,
    head: &CouplingFindingCounts,
    baseline_findings: &[CouplingFinding],
    head_findings: &[CouplingFinding],
    max_new: usize,
) -> RatchetVerdict {
    let key = |f: &CouplingFinding| (f.path.clone(), f.kind, f.evidence.clone());
    let base_keys: std::collections::HashSet<_> = baseline_findings.iter().map(key).collect();
    let new_findings: Vec<CouplingFinding> = head_findings
        .iter()
        .filter(|f| !base_keys.contains(&key(f)))
        .cloned()
        .collect();
    let increases: Vec<(&'static str, usize, usize)> = [
        ("content", baseline.content, head.content),
        ("common", baseline.common, head.common),
        ("control", baseline.control, head.control),
    ]
    .into_iter()
    .filter(|(_, b, h)| h > b)
    .collect();
    let total_new = new_findings.len();
    RatchetVerdict { failed: total_new > max_new, increases, new_findings, total_new }
}
```

(`CouplingKind` derives Hash? Check — it derives `PartialEq, Eq` but maybe not `Hash`; if not, add `Hash` to its derive list in snapshot/mod.rs — additive, harmless.)

- [ ] **Step 4:** green + clippy + fmt. **Step 5:** Commit `feat(gate): pure ratchet verdict over baseline/head findings`.

---

### Task 4: Wire into `run_gate`

**Files:**
- Modify: `src/cmd/gate.rs` (`run_gate`)

**Interfaces:**
- Consumes: Tasks 1-3; `BaradDurIgnore::load` (backfill's pattern); `pressman_finding_counts(snapshot, &cfg.thresholds.coupling)`.
- Produces: after the existing score/trend checks, when `args.no_new_coupling || args.max_new_coupling.is_some()`:

```rust
    let ratchet_failed = if args.no_new_coupling || args.max_new_coupling.is_some() {
        let baseline_ref = args
            .baseline_ref
            .as_deref()
            .expect("clap `requires` guarantees baseline_ref");
        let sha = collector.resolve_ref(baseline_ref)?; // see below
        let ignore = crate::collector::ignore_file::BaradDurIgnore::load(&local_path)?;
        let base_snapshot = Collector::collect_snapshot_at(&local_path, &sha, true, &ignore, true)?;
        let base_counts = coupling::pressman_finding_counts(&base_snapshot, &cfg.thresholds.coupling)
            .unwrap_or(CouplingFindingCounts { content: 0, common: 0, control: 0 });
        let head_counts = report.coupling_finding_counts.unwrap_or(CouplingFindingCounts {
            content: 0,
            common: 0,
            control: 0,
        });
        let base_barrel = coupling::barrel_bypass_findings(&base_snapshot, cfg.thresholds.coupling.component_depth);
        let head_barrel = coupling::barrel_bypass_findings(&snapshot, cfg.thresholds.coupling.component_depth);
        let base_findings: Vec<_> = base_snapshot.coupling_findings.iter().cloned().chain(base_barrel).collect();
        let head_findings: Vec<_> = snapshot.coupling_findings.iter().cloned().chain(head_barrel).collect();
        let verdict = ratchet_verdict(
            &base_counts,
            &head_counts,
            &base_findings,
            &head_findings,
            args.max_new_coupling.unwrap_or(0),
        );
        print_ratchet(&verdict, baseline_ref, args.max_new_coupling.unwrap_or(0));
        verdict.failed
    } else {
        false
    };
```

and fold `ratchet_failed` into the exit code. `print_ratchet` prints PASS/FAIL, the per-kind increases, and each new finding as `path:line — evidence` (line may be None for barrel findings). Barrel findings respect `cfg.thresholds.coupling.content_barrel_rule` — chain them only when the toggle is on (match `pressman_finding_counts`'s behavior exactly, or the counts and the set will disagree).

`resolve_ref`: add a small helper on `Collector` (or a free fn in gate.rs using git2 directly):

```rust
fn resolve_baseline_ref(repo_path: &Path, r: &str) -> anyhow::Result<String> {
    let repo = git2::Repository::discover(repo_path)?;
    let obj = repo.revparse_single(r).map_err(|e| {
        anyhow::anyhow!(
            "cannot resolve baseline ref '{r}': {e}. On CI, shallow clones hide history — \
             set GIT_DEPTH: 0 (GitLab) or fetch the ref first (git fetch origin {r})."
        )
    })?;
    Ok(obj.peel_to_commit()?.id().to_string())
}
```

- [ ] **Step 1: Failing tests** — unit-test `print_ratchet` output shape and the exit-code fold with synthetic verdicts (existing gate test style); the E2E lives in Task 5.
- [ ] **Step 2-4:** RED → implement → `RUSTFLAGS="-D warnings" cargo test --lib` green.
- [ ] **Step 5:** Commit `feat(gate): coupling ratchet against an explicit baseline ref`.

---

### Task 5: Milestone integration test

**Files:**
- Create: `tests/pressman_coupling_milestone_3.rs`

Because `run_gate` and `collect_snapshot_at` are crate-internal, the E2E drives the **binary**: build a fixture repo with two commits (base: clean `lib.rs`; head: adds `static mut CACHE: usize = 0;`), then run the installed-from-source binary via `cargo run --quiet -- gate <fixture> --no-new-coupling --baseline-ref <base-sha>` using `std::process::Command` (`CARGO_BIN_EXE_barad-dur` env — the standard cargo integration-test binary path) and assert:

1. exit code non-zero, stdout mentions `static mut CACHE` and `common`;
2. same invocation with `--max-new-coupling 1` → exit code 0;
3. `--baseline-ref does-not-exist` → non-zero, stderr/stdout mentions `GIT_DEPTH` hint;
4. `--no-new-coupling` without `--baseline-ref` → clap usage error (non-zero, message names `--baseline-ref`).

Fixture setup via `std::process::Command` git calls in a `tempfile::TempDir` (init, config user, write, add, commit ×2, capture `rev-parse HEAD~1`). Set `min_score` to 0 (`--min-score 0`) so the score gate never interferes with the ratchet assertion.

- [ ] Write test → run → full sweep (`RUSTFLAGS="-D warnings" cargo test`, clippy, fmt) → commit `test(coupling): M3 milestone — gate ratchet E2E on fixture repo`.

---

### Task 6: Documentation package (spec deliverable — not optional)

**Files:**
- Create: `docs/gate-coupling.md` — what the ratchet guarantees; why the baseline is explicit (fail-loud rationale, rejected history-file alternative); merge-base recommendation; GitLab CI job example:

```yaml
coupling-gate:
  stage: test
  variables:
    GIT_DEPTH: 0
  script:
    - barad-dur gate . --min-score 0 --no-new-coupling
      --baseline-ref "$CI_MERGE_REQUEST_DIFF_BASE_SHA"
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
```

  plus a local-usage section (`--baseline-ref origin/main`), the `--max-new-coupling` cleanup mode, cost note (one uncached AST pass at the ref), and the error-message catalogue.
- Modify: `README.md` — short "Coupling ratchet" subsection under the gate docs pointing to `docs/gate-coupling.md`.
- Modify: `Makefile` — `gate-coupling:` target running `cargo run --quiet -- gate . --no-new-coupling --baseline-ref origin/main`.
- Modify: spec M3 section — mechanism corrections from Global Constraints (blob-based AST opt-in; no per-commit cache), and delete the stale "cached by commit hash" parenthetical.
- [ ] Write all four → `cargo run -- gate . --min-score 0 --no-new-coupling --baseline-ref HEAD~1` smoke run quoted in the report → commit `docs(gate): coupling ratchet documentation, CI example, make target`.

---

## Post-plan notes

- **Not in M3:** hotspot cross-referencing (M4), corroboration (M5 checkpoint), actions (M6), and the M2-deferred decision on counts-in-filtered-runs — the gate always computes coupling itself, so filtered-run history entries never feed the ratchet (note this in docs/gate-coupling.md).
- The M3 hygiene ledger (detector false-negative inventory from the MR reviews) is deliberately NOT bundled into this plan — separate small MR after M3, so the ratchet doesn't wait on detector tuning.
- If MR !49/!50 merge before execution completes, rebase this branch onto main and retarget.
