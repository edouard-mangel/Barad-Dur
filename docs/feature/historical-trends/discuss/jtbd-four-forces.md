# JTBD Four Forces Analysis — historical-trends

## Framework

For a user to adopt historical trend analysis, Push + Pull must exceed Anxiety + Habit.
Switch happens when: demand-generating forces > demand-reducing forces.

---

## Force 1: Push of Current Situation (demand-generating)

What makes the current single-point analysis painful enough that users seek something more?

**Primary frustrations driving change**:

1. **The score is directionally blind.** Marco sees 58. He cannot say "we improved 13 points this quarter" or "we dropped 7 points since the refactor." Every monthly report exists in isolation.

2. **Justification requires manual archaeology.** Priya wants to prove refactoring impact. She has to dig up an old terminal session, find the pre-refactor output if she saved it, and format it manually. If she didn't save it, the before-state is gone.

3. **Decay is invisible until it's a crisis.** There is no mechanism to notice that the Bus Factor metric has been declining 2 points per month for five months. Each run looks "fine" in isolation; together they spell disaster.

4. **CI integration produces orphan data.** Each pipeline run produces a `report.json` that gets overwritten or archived separately with no connection between runs. The automation exists but the continuity does not.

**Push intensity: HIGH** — All four personas experience the pain regularly. Marco has had an actual incident that could have been prevented by trend visibility.

---

## Force 2: Pull of New Solution (demand-generating)

What makes historical trend analysis attractive?

1. **Instant directional answer.** Running `barad-dur analyze . --trend` and seeing "Overall: 62 (+7 vs last run, +14 vs 3 months ago)" answers the core question without ceremony.

2. **Shareable evidence artifact.** A trend table or chart in HTML or JSON format becomes the artifact that justifies refactoring budget, reassures stakeholders, and creates accountability.

3. **Proactive decay detection.** A trend spanning 6+ months surfaces slow-burn deterioration (e.g., Bus Factor declining steadily) before it reaches crisis level.

4. **Zero-overhead history.** If trend snapshots are stored automatically as a side effect of every analysis run, there is no manual step. Users who run `analyze .` regularly already accumulate a history without thinking about it.

5. **CI-native reporting.** A machine-readable trend JSON schema enables dashboards and alerting that today require custom hacks or external tooling.

**Pull intensity: HIGH** — The pull is strongest for Priya (concrete presentation need) and Marco (directional confidence). Both are immediate and practical.

---

## Force 3: Anxiety of New Solution (demand-reducing)

What could make users reluctant to adopt or trust trend analysis?

1. **Performance anxiety (highest).** "Will running a trend analysis require re-running blame on 20 past commits? That's 20 × 85s = 28 minutes." This is the single biggest adoption barrier. Users have been burned by slow tools before. If trend mode is slow, it will not be used.

2. **Correctness anxiety.** "If I analyze HEAD today and a past commit from before a merge, are the scores comparable? Do different branches, different `--since` windows, different configs produce misleading deltas?"

3. **Storage anxiety.** "Where is this trend data stored? Will it bloat my repo? Will it conflict with .gitignore or end up committed accidentally?"

4. **Schema stability anxiety.** Priya uses `--json` in CI. "If the trend JSON schema changes in v2.1, my dashboard scripts break. When is it safe to depend on this?"

5. **Snapshot availability anxiety.** "I ran barad-dur for the first time today. I have no history. Will the trend feature just silently produce a table with one row?"

**Anxiety intensity: HIGH** — Performance anxiety is the decisive force. The design must address it explicitly and visibly (fast path, cached snapshots, no mandatory re-analysis of old commits).

---

## Force 4: Habit of Present (demand-reducing)

What familiar patterns will resist change?

1. **Single-command muscle memory.** Every barad-dur user types `barad-dur analyze .` (or with a flag). There is no "history" subcommand in existing workflows. A new invocation pattern requires re-learning.

2. **Saving reports manually.** Priya has a folder of saved `report.json` files with dates in filenames. This is her current "trend analysis." It is fragile but familiar.

3. **Reading the top-level score.** Users scan the CLI output for the overall score number and top actions. A trend view requires reading additional context (deltas, direction, timestamps). This is more cognitive load.

4. **Not depending on side effects.** Developers are wary of tools that silently write files as side effects. The existing `.repository-analysis/snapshot.bin` is already accepted; additional trend files may feel intrusive.

**Habit intensity: MEDIUM** — The existing patterns are weak enough that a well-designed zero-configuration default (auto-record after every analysis) can replace them without friction. The key is making trend data appear naturally in existing output rather than requiring a separate mode.

---

## Force Balance Assessment

```
DEMAND-GENERATING                DEMAND-REDUCING
  Push: HIGH    ------+------   Anxiety: HIGH
  Pull: HIGH    ------+           Habit: MEDIUM
                      |
                      v
         Net: Generate > Reduce
         Switch likelihood: HIGH
         Condition: performance anxiety must be explicitly addressed
```

**Switch likelihood: HIGH** — provided the design credibly addresses the performance anxiety.

**Key blocker**: Performance anxiety about re-analyzing past commits. If historical trend data requires running blame on old commits, the feature will be avoided.

**Key enabler**: Zero-configuration auto-recording. If every `analyze .` automatically stores a trend entry as a side effect, adoption requires no new behavior from the user. History builds passively.

**Design implication** (for DESIGN wave): The trend storage architecture must be append-only snapshots stored from current-run data (not re-analysis of past commits). Past commits are never re-analyzed. Trend depth grows naturally over calendar time as users run the tool. This collapses performance anxiety entirely.

---

## Opportunity Assessment by Job Story

| Job Story | Switch Likelihood | Key Design Implication |
|-----------|-----------------|----------------------|
| JS-01 Track direction | HIGH | Show delta vs last run in default CLI output |
| JS-02 Validate refactoring | HIGH | Before/after table; compare named points in trend history |
| JS-03 Detect decay | MEDIUM | Trend over 6+ runs; visual direction indicator |
| JS-04 CI integration | HIGH | Stable JSON trend schema; trend append separate from snapshot |
