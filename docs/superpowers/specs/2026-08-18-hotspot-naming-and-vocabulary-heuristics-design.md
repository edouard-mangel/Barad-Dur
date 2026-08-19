# Hotspot Naming & Vocabulary Heuristics — Design

**Date:** 2026-08-18
**Status:** Draft design, pending user review
**Parent:** `docs/crime-scene-book-notes.md` — Group E (Ch. 5, Ch. 11, Appendix 1)

## Context

`docs/crime-scene-book-notes.md` groups five gaps against *Your Code as a
Crime Scene*. This spec covers **Group E**, three small partial gaps bundled
because none of them needs new collector or config surface — each is a pure
join over data barad-dûr already collects:

1. **Ch. 5 — judge hotspots by name.** A hotspot's *name itself* is a cheap
   triage signal: a generically-named file (`Manager`, `Helper`, `Util`) that
   has grown large is more suspicious than a domain-named one of the same
   size, because a generic name is itself evidence the file has no single
   responsibility. Today `god_objects`' `god_reason` (`src/metrics/health/
   god_objects.rs:22-52`) reports *why* a file was flagged (LOC, method
   count, structural-hub degree) but never looks at the name.
2. **Ch. 11 — commit-message vocabulary as a social signal.**
   `firefighting_ratio` (`src/metrics/hygiene.rs:264-318`) already scans
   commit messages for a fixed 4-word revert/hotfix/emergency/rollback list.
   The gap is a *broader* friction-vocabulary signal (hacks, workarounds,
   temporary fixes) — a different word list, same "reactive work" spirit,
   not a general-purpose word-cloud.
3. **Appendix 1 — refactor hotspots by method grouping.** `generate_
   coupling_actions` (`src/scorer/actions.rs:85-155`) already gives generic,
   kind-level advice for Pressman coupling findings (`CONTENT_ADVICE`,
   `COMMON_ADVICE`, etc.). The gap is file-specific: group a god-object
   file's *own* method names by shared prefix to suggest where the
   split lines actually are.

All three are **advisory annotations on existing findings** — none adds a
new score dimension, a new config threshold, or new collector work. That is
what makes this group cheap, and it is a deliberate scope constraint, not an
oversight: the point of Group E is quick, low-risk wins while the more
expensive groups (trend infrastructure, source/test coupling, architecture
conformance) get their own specs.

## Decisions

1. **Name-smell list (Ch. 5):** a small, defensible stem list — `manager`,
   `helper`, `util`, `utils`, `handler`, `processor`, `service`, `common`,
   `base`, `misc` — matched case-insensitively against a file's basename
   stem (extension stripped), mirroring `file_role.rs`'s `stem_lower` +
   `TEST_STEM_NAMES` pattern exactly. Rejected: a longer taxonomy (`Data`,
   `Info`, `Object`, generic OO smells) — the book's own example
   (`Configuration.java`) is about words that read as *legitimately narrow*
   when small and *suspicious* only once the file has also grown large, so a
   short, high-precision list beats a long, noisy one.
2. **Name-smell is annotation-only, never a standalone finding.** It only
   fires on files `god_objects` *already* flagged (LOC/method-bloat or
   structural-hub reasons) — it adds a name-based reason to the existing
   `god_reason` string, it does not independently flag files, and it does
   not change `god_objects`' score. Rejected: a standalone "suspicious name"
   metric — the book frames naming as a *triage aid* for hotspots you
   already found, not a new source of hotspots, and a standalone metric
   would need its own score/threshold design (bigger scope, defeats the
   point of this being the cheap group).
3. **Friction-vocabulary list (Ch. 11):** `hack`, `workaround`, `kludge`,
   `temporary`, `fixme`, `sorry` — distinct from `FIREFIGHTING_KEYWORDS`
   (`revert`, `hotfix`, `emergency`, `rollback`), which signals *reactive
   incident response*; this list signals *known technical debt admitted in
   the moment*. Both are legitimate, non-overlapping social signals worth
   tracking separately rather than merging into one bigger list — merging
   would blur two different meanings (firefighting = something broke and we
   scrambled; friction language = we knew this wasn't right but shipped it
   anyway).
4. **Friction vocabulary is repo-wide, not per-file (Ch. 11).** Mirrors
   `firefighting_ratio` exactly: percentage of non-merge, in-window commits
   whose message contains a friction word. Rejected: per-file word
   attribution (joining commit messages to `files_changed`) — the book's
   word-cloud technique is presented as a team/process-level read ("are we
   mostly fixing, adding, or firefighting"), and `firefighting_ratio`
   already established repo-wide as this codebase's answer to that framing;
   a per-file variant is a bigger, separate design question (which files get
   which words, how ties are broken, how it interacts with the existing
   per-file `god_reason`/hotspot annotation surface) that belongs in its own
   spec if wanted later, not bundled here.
5. **Method-grouping algorithm (Appendix 1):** strip a fixed verb-prefix set
   (`get_`, `set_`, `handle_`, `validate_`, `build_`, `compute_`, `parse_`,
   `render_`, `is_`, `has_`) from each function name in a file's already-
   collected `FileComplexity.functions: Vec<FunctionMetrics>`
   (`src/snapshot/mod.rs:125-129`, populated by the existing tree-sitter
   pass — **no new AST/collector work**), group by the surviving prefix
   token, and keep only groups with ≥2 members. Rejected: longest-common-
   prefix clustering across arbitrary names — more general but produces
   messy, hard-to-name groups on real code (e.g. `parse_json`/`parse_xml`
   sharing `parse_` is meaningful; `parse_json`/`park_ranger` sharing `par`
   is noise); a fixed, curated verb list gives predictable, explainable
   group names at the cost of missing verbs outside the list, which is the
   right tradeoff for advisory text nobody is required to act on.
6. **Method-grouping fires only on already-flagged god-object files**, same
   annotation-only philosophy as decision 2. Requires extracting the file-
   selection half of `god_objects()` into its own reusable pure function
   (see Architecture §3) so the action generator and the metric agree on
   *which files* are god objects — the same "one definition, not two
   divergent thresholds" rule this codebase already follows for corroborated
   coupling findings (`corroboration_degree`, M5 design).
7. **No score impact anywhere in this spec.** All three items only add text
   to existing `MetricValue`/`ActionItem` strings. This is the concrete
   reason Group E is the cheapest of the five: no band recalibration, no
   dogfood-score risk, no config migration.

## Architecture

### 1. Name-smell annotation (Ch. 5)

New module `src/metrics/name_smell.rs`, same shape as `file_role.rs`:

```rust
const SMELLY_NAME_STEMS: &[&str] = &[
    "manager", "helper", "util", "utils", "handler",
    "processor", "service", "common", "base", "misc",
];

/// True if the file's basename stem (extension stripped) contains a
/// generic, responsibility-agnostic word — a name-based hotspot triage
/// signal, not a standalone judgment.
pub fn has_smelly_name(path: &Path) -> bool {
    let stem = stem_lower(path); // relocate file_role.rs's stem_lower to a
                                  // small shared `path_util` helper, or
                                  // duplicate the ~10-line function — see
                                  // Risks for the tradeoff.
    SMELLY_NAME_STEMS.iter().any(|s| stem.contains(s))
}
```

`god_objects.rs::god_reason` gains one line: after building `reasons`, if
`!reasons.is_empty() && has_smelly_name(path)`, push `"generic name
suggests broad responsibility"`. Signature grows by one `&Path` parameter
threaded from the existing call site.

### 2. Friction-vocabulary metric (Ch. 11)

New sibling function in `src/metrics/hygiene.rs`, copy-shaped from
`firefighting_ratio`:

```rust
const FRICTION_KEYWORDS: &[&str] =
    &["hack", "workaround", "kludge", "temporary", "fixme", "sorry"];

fn friction_language_ratio(
    snapshot: &RepoSnapshot,
    _thresholds: &HygieneThresholds,
) -> MetricValue { /* identical shape to firefighting_ratio, new keyword list */ }
```

Registered as a fifth metric in `compute_hygiene()`'s `metrics` vec. Same
N/A-on-empty-window handling, same score-banding style (lower percentage =
higher score) as `firefighting_ratio` — copy its band cutoffs verbatim
(`<2%→90, <5%→75, <10%→55, <20%→35, else 20`) so the two metrics read
consistently side by side in the Git Hygiene category.

### 3. Method-grouping refactor suggestion (Appendix 1)

Step A — extract the god-object file selection that today lives inline in
`god_objects()` (`src/metrics/health/god_objects.rs:77-88`) into:

```rust
/// Files flagged as god objects, with their reason — the single
/// definition `god_objects()` and any downstream action generator share.
pub(crate) fn god_object_files(
    snapshot: &RepoSnapshot,
    thresholds: &HealthThresholds,
) -> Vec<(PathBuf, String)>
```

`god_objects()` is refactored to build its `gods: Vec<String>` display list
from this function's output (pure refactor, behavior identical, guarded by
its existing tests — same shape as the M5 design's `corroboration_degree`
extraction).

Step B — new pure function, e.g. in `src/scorer/actions.rs` next to the
other action generators:

```rust
const GROUPING_PREFIXES: &[&str] = &[
    "get_", "set_", "handle_", "validate_", "build_",
    "compute_", "parse_", "render_", "is_", "has_",
];

/// Group a file's function names by a known verb prefix; only groups with
/// ≥2 members are returned (a lone `handle_x` isn't a split boundary).
fn group_methods_by_prefix(functions: &[FunctionMetrics]) -> Vec<(&'static str, Vec<&str>)>
```

Step C — `generate_refactoring_actions(snapshot, health_thresholds) ->
Vec<ActionItem>`, mirroring `generate_coupling_actions`'s shape exactly: for
each `god_object_files()` entry, look up `snapshot.file_metrics[path]
.functions`, run `group_methods_by_prefix`, and if it returns ≥1 group,
emit one `ActionItem` per file:

```
[Health] src/metrics/health/god_objects.rs — 512 loc, 22 public methods —
consider splitting by responsibility: has_* (4), is_* (3)
```

Files with zero qualifying groups get no action (silently — this is
advisory, not a requirement to always say something). Wired into
`build_report()` in `src/scorer.rs` alongside the existing two `generate_*`
calls (line ~67-68), appended to the same actions list.

## Interactions (deliberately untouched)

- **God Objects score / band:** unchanged — `god_object_files` is a pure
  extraction of existing logic, `god_objects()`'s score computation is
  untouched.
- **Git Hygiene score:** `compute_hygiene()`'s `CategoryResult::compute_score`
  now averages 5 metrics instead of 4 — this *does* shift the category score
  slightly wherever `friction_language_ratio`'s band differs from the
  category average. This is an accepted, expected consequence of adding a
  real metric (not a bug) — flagged explicitly here so it isn't mistaken for
  scope creep during review. If the team wants zero score movement, gate the
  new metric behind a `HygieneThresholds` flag (see Configuration) that
  defaults it out of the score average — deferred unless requested.
- **Actions list:** `generate_refactoring_actions`'s output is additive to
  the existing `top_actions`/`coupling_actions` lists in `build_report` — no
  reordering of the other two.

## Configuration

None required for v1. All three lists (`SMELLY_NAME_STEMS`,
`FRICTION_KEYWORDS`, `GROUPING_PREFIXES`) are hardcoded constants, matching
how `FIREFIGHTING_KEYWORDS` and `SUSPICIOUS_PATTERNS` are hardcoded today
rather than config-driven — this codebase's existing convention for small
curated word/pattern lists (only thresholds and toggles go in
`Thresholds`/`*Thresholds` structs). If a team wants the friction-language
metric excluded from scoring (see Interactions), that would be the one
config addition worth considering, deferred to implementation time based on
whether dogfooding shows it's needed.

## Testing strategy (TDD throughout)

- **`has_smelly_name`:** unit tests mirroring `file_role.rs`'s test style —
  `"src/UserManager.rs"` → true, `"src/user_service.py"` → true,
  `"src/main.rs"` → false, case-insensitivity (`"Helper.ts"` → true).
- **`god_reason` integration:** a synthetic `FileComplexity` over the
  existing LOC/method-bloat threshold, at a smelly-named path → reasons
  string contains the new note; same file at a non-smelly path → note
  absent; a file with *no* other reason (small, unnamed-smelly) → still no
  reason (name alone never triggers a flag) — this is the test that proves
  decision 2 (annotation-only, not a standalone trigger).
- **`friction_language_ratio`:** copy `firefighting_ratio`'s existing test
  suite shape (`*_detects_reactive_commits`, `*_ignores_merge_commits`,
  `*_all_keywords_detected`, `*_zero_percent_scores_highest`, `*_returns_na_
  when_no_commits_in_window`, `*_is_case_insensitive`) against the new
  keyword list — five to six tests, same fixtures pattern.
- **`god_object_files` extraction parity:** a test asserting `god_objects()`'s
  displayed reason strings are unchanged before/after the refactor (same
  regression-guard pattern as M5's corroboration-predicate extraction).
- **`group_methods_by_prefix`:** unit tests — a list with 4 `handle_*` + 3
  `validate_*` + 1 `parse_foo` → two groups (`handle_`, `validate_`), the
  lone `parse_foo` excluded (group size 1); a list with no matching prefixes
  → empty; case/ordering determinism (sort group keys, sort names within a
  group) so action text is stable across runs.
- **`generate_refactoring_actions` integration:** a fixture `RepoSnapshot`
  with one god-object-flagged file whose functions cluster into two groups
  → one `ActionItem` with both group names and counts in the text; a
  god-object file with no clustering functions → no action emitted for it;
  a non-god-object file with clustering names → no action (proves decision
  6, the shared-selection-function gate).
- **Dogfood:** run `cargo run -- analyze . -v` after implementation and
  visually confirm the new hygiene metric and any refactoring actions read
  sensibly against barad-dûr's own repo — no assertion, just a sanity pass
  (the same "dogfood" step the M5 design used).

## Risks & mitigations

- **`stem_lower` duplication:** `file_role.rs`'s `stem_lower` is private
  (`fn`, not `pub`). Either relocate it to a small shared helper (e.g.
  `src/metrics/path_util.rs`, used by both `file_role.rs` and the new
  `name_smell.rs`) or accept a ~10-line duplicated copy. Recommend
  relocating — it's a pure, already-tested function, and a second copy would
  be exactly the kind of drift this codebase's "one definition" convention
  (decision 6, M5 precedent) argues against. This is the one place this spec
  touches a file outside its own new code; call it out explicitly in the
  implementation PR description.
- **False positives on legitimately-named files:** `"common"` and `"base"`
  are common *legitimate* names for small, cohesive shared modules (this
  repo has `src/metrics/testutil.rs`-style helpers). Mitigated by decision 2
  — the annotation only ever appears on files *already* flagged as god
  objects by size/degree, so a small, well-scoped `common.rs` never gets
  flagged at all.
- **Hygiene score movement:** addressed explicitly in Interactions; not
  hidden.
- **Verb-prefix list staleness:** `GROUPING_PREFIXES` is Rust/general-
  purpose-verb-shaped and will under-group in codebases with different
  naming conventions (e.g. heavy `on_`/`did_`/`will_` event-handler style).
  Accepted for v1 — advisory text degrading gracefully to "no suggestion" on
  a miss is a safe failure mode, unlike a false accusation.

## Future work (explicitly deferred)

- Per-file friction-vocabulary attribution (decision 4) as its own spec, if
  a per-file view proves more useful than the repo-wide ratio in practice.
- A `HygieneThresholds` toggle to exclude `friction_language_ratio` from the
  category score average, if dogfooding shows unwanted score movement.
- Extending `group_methods_by_prefix` beyond a fixed verb list once real
  usage shows which additional prefixes recur across dogfooded repos.

---

**Estimated implementation size: S.** Three independent, additive-only pure
functions plus one pre-existing-logic extraction (`god_object_files`); zero
new config surface (constants only); zero new collector/AST work (method
names, commit messages, and file paths are all already collected); ~15-18
new unit/integration tests total, each following an existing sibling test's
exact shape. The cheapest of the five gap-group specs — confirmed, not just
assumed: every other group needs either new storage (Group A), a new
cross-file pairing algorithm (Group B), an entirely new config subsystem
(Group C), or a new ownership×coupling join with its own threshold design
(Group D); this group needs none of that.
