# Ideas

Backlog of metric/feature ideas not yet scheduled. Each entry: the concept, why it
might help, the known caveats, and where it would hook into the existing pipeline.

## Issue-rework metric (defect recidivism via commit-message issue keys)

**Concept.** Parse issue-tracker keys (e.g. `PROJ-123`, `#456`) out of commit
messages, group commits by key, and surface work items that accumulated many
follow-up *bugfix* commits after their initial implementation. Aggregate into a
hygiene-style signal: how much of delivery is rework vs. first-time-right.

**Why it might help.** An issue that keeps getting re-touched with `fix:` commits is
a strong "this was hard / under-tested / firefought" marker — i.e. defect
recidivism / rework. Complements the existing reactive-work signal by attributing
churn to specific work items instead of counting it repo-wide.

**Where it hooks in.**
- Sibling to `firefighting_ratio` in `src/metrics/hygiene.rs:264` — that metric
  already iterates `snapshot.commits`, reads `c.message`, and keyword-matches over
  `snapshot.time_window`. Same plumbing; add a regex for the issue key + a
  group-by-key fold.
- Register in `src/scorer.rs` → `build_report()` like any other metric.
- Pure `(snapshot) -> MetricValue`, TDD via `src/metrics/testutil.rs` (tests first).

**Caveats (load-bearing).**
- **Coverage dependence.** Only works if commits consistently cite issue keys. Many
  teams put keys only in branch names or MR titles. Low coverage → silent
  under-reporting, which is worse than no metric. Needs a guard: report
  issue-reference coverage % as its own value and only emit the rework score when
  coverage clears a threshold (mirror how `firefighting_ratio` returns `N/A` for an
  empty window).
- **"Bugfix" detection.** Distinguishing a fix-commit from a feature-commit on the
  same issue needs conventional-commit `fix:` parsing or keyword heuristics.
  Without commit-type discipline you can't tell "many commits = big feature" from
  "many commits = buggy."
- **Tracker-key format varies** (Jira `ABC-123`, GitHub/GitLab `#123`). Make the
  pattern configurable via `.repository-analysis/barad-dur.toml`
  (`src/config/`) rather than hardcoding one convention.

**Open question for when this is picked up.** Target audience — disciplined teams
only (score when coverage high, else N/A), any-repo best-effort (always report
coverage, gate the score), or tuned to our own conventions. This decides how hard
the coverage guard has to work.

## Exclude manifest files from hotspots/churn surfaces

**Concept.** Manifest files (`package.json`, `Cargo.toml`, `go.mod`, `*.csproj`,
`pom.xml`, `build.gradle`, `requirements.txt`) currently flow into the snapshot and
surface as hotspots, even though they are declarative config, not logic. Frequent
version bumps / dependency edits inflate their churn and pollute the hotspot,
complexity, and coupling surfaces. Drop them from the snapshot by default.

**State of play (verified).**
- Lockfiles are *already* excluded — `src/collector/exclude.rs:32` lists
  `**/package-lock.json`, `**/pnpm-lock.yaml`, `**/Cargo.lock`, `**/go.sum`,
  `**/*.lock`, etc. They cannot reach hotspots with defaults on. (If one is ever
  seen, suspect stale snapshot cache or `--no-default-excludes`.)
- Manifests are *not* excluded — there is no `package.json`/`Cargo.toml`/etc.
  pattern in `DEFAULT_EXCLUDE_PATTERNS`. This is the real gap.

**Where it hooks in.** Add the manifest globs to `DEFAULT_EXCLUDE_PATTERNS` in
`src/collector/exclude.rs`. Exclusion is applied once at snapshot construction
(`src/collector/snapshot_builder.rs:96`), dropping the file from `files` before any
metric runs. TDD: extend the `is_excluded_*` test block in `exclude.rs`.

**Why a global snapshot drop is safe (verified).** The two features that consume
manifests both read them *directly from disk* (`repo_root.join(...)`), not from the
snapshot:
- `src/coupling/dependency.rs:302` — `parse_package_json` reads `package.json` from
  disk for dependency-based coupling.
- `src/collector/deps.rs` — reads *lockfiles* (not `package.json`) from disk for the
  deps/CVE category.
So excluding manifests from the snapshot removes them from churn/hotspot/complexity
surfaces only; dependency coupling and the deps category are unaffected.

**Caveats / open questions.**
- Manifest *churn* is itself a real signal (dependency volatility) — a plain
  exclude throws it away rather than measuring it. See the next idea.
- `dist/` is intentionally NOT excluded today (see `exclude.rs` test at the bottom);
  stay consistent with that "published output can be real code" stance — manifests
  are config, so excluding them does not contradict it.
- Decide whether to also exclude from *coupling pairs* surfacing (manifests co-change
  with everything) — that is a separate render-time concern, not a snapshot drop.

## Richer manifest parsing (mine the latent signal in package.json et al.)

**Concept.** Today `package.json` is read for one thing only —
`src/coupling/dependency.rs:90` pulls dependency *names* from `dependencies` +
`devDependencies` and discards everything else. Manifests are information-rich;
extract more to feed the deps/hygiene categories.

**Latent signals worth mining.**
- Direct vs. transitive dep count (manifest gives *direct* deps; lockfile gives the
  full transitive tree) → dependency-bloat ratio.
- Declared version ranges (`^`, `~`, `*`, `latest`, pinned) → reproducibility /
  supply-chain risk; loose ranges are a hygiene red flag.
- `scripts` presence (`test` / `lint` / `typecheck`) → quality-tooling signal;
  absence is itself telling.
- `engines` → declared runtime constraints, maturity marker.
- `license`, `private` → compliance, publishability.
- `workspaces` → monorepo topology, could inform real module boundaries.
- git history of the manifest → dependency volatility over time (the legitimate
  signal currently hiding inside the hotspot noise the previous idea removes).

**Where it hooks in.** Extend the disk-reading manifest parsers in
`src/coupling/dependency.rs` and/or add a manifest collector feeding a new
`src/metrics/` function (pure `(snapshot) -> MetricValue`, or a disk-read collector
like `collector/deps.rs`). Register in `src/scorer.rs` → `build_report()`.

**Relationship to the exclude idea.** Complementary, not conflicting: exclude the
manifest from churn/hotspot surfaces (idea above) *and* mine its contents + history
for real signal here. "Exclude from hotspots" ≠ "ignore the file."

**Caveat.** Per-ecosystem manifest formats differ (`package.json` JSON,
`Cargo.toml`/`pyproject.toml` TOML, `pom.xml` XML, `build.gradle` Groovy/Kotlin).
Scope to the highest-value ecosystems first rather than boiling the ocean.
