# Design Self-Review — Historical Trends

Reviewer: solution-architect-reviewer (Atlas)
Artifact: docs/feature/historical-trends/design/ + docs/adrs/ADR-002/003/004
Iteration: 1
Date: 2026-03-18

```yaml
review_id: "arch_rev_20260318_historical_trends"
reviewer: "solution-architect-reviewer"
artifact: "docs/feature/historical-trends/design/architecture-design.md, docs/adrs/ADR-002/003/004.md"
iteration: 1

strengths:
  - "Pre-existing infrastructure discovery: correctly identified that cache::history already implements the core storage contract, avoiding reimplementation"
  - "ADR-002 documents 4 rejected alternatives (SQLite, Bincode, MessagePack, CSV) with concrete rejection rationale including CI cross-compilation impact of rusqlite"
  - "ADR-003 correctly identifies NFR-03 backward-compat violation risk and resolves it without modifying AnalysisReport"
  - "ADR-004 chose linear regression over EWMA/simple-delta with clear interpretability rationale"
  - "DA-03 HTML tab decision prevents CLI-flag coupling from leaking into the rendering artefact"
  - "Zero new dependencies — validated against Cargo.toml implicitly through technology-stack.md"
  - "Component boundaries document explicitly lists untouched files — implementer has clear scope"
  - "Pipeline execution order is explicit with the step-7-before-step-9 ordering constraint documented"

issues_identified:
  architectural_bias:
    - issue: "No bias detected. Technology choices are constraint-driven; NDJSON chosen because implementation already exists, not by preference."
      severity: "low"
      location: "ADR-002"
      recommendation: "No action required."

  decision_quality:
    - issue: "ADR-002 does not address the history.json → trends.json rename migration for existing users who already have a .repository-analysis/history.json. The consequence section mentions it briefly but does not specify the migration behaviour."
      severity: "medium"
      location: "ADR-002 Consequences"
      recommendation: "Add a migration note: on first run after upgrade, if history.json exists and trends.json does not, copy history.json to trends.json. This prevents losing existing trend data for early adopters."

  completeness_gaps:
    - issue: "Security: the trend file is written by the binary and read back. No sanitisation concern for the NDJSON since it is only produced by barad-dur itself (not user-supplied). Correctly assessed as no threat."
      severity: "low"
      location: "architecture-design.md"
      recommendation: "No action required. Note added for completeness."
    - issue: "The data-models.md defines the sparkline Unicode fallback for non-Unicode terminals but does not specify which environment variable triggers it."
      severity: "low"
      location: "data-models.md"
      recommendation: "Clarify: check TERM=dumb OR NO_COLOR env var is set OR stdout is not a TTY (std::io::stdout().is_terminal()). Software-crafter should use the last condition as the primary gate since it is already used for colored output suppression in the existing code."

  implementation_feasibility:
    - issue: "The renderer::cli::render signature change adds two parameters (trend: Option<&TrendSummary>, show_full_history: bool). All existing test call sites in renderer/cli.rs will need updating. This is straightforward but must not be overlooked."
      severity: "low"
      location: "component-boundaries.md"
      recommendation: "Note for software-crafter: update all make_report() test helpers to pass None/false for the new parameters in existing tests. No existing test logic changes."

  priority_validation:
    q1_largest_bottleneck:
      evidence: "Pre-existing cache::history infrastructure already exists. The bottleneck is surface-area (renderers + new computation module), not storage. Design correctly minimises new code."
      assessment: "YES"
    q2_simple_alternatives:
      evidence: "Each ADR lists 2-4 alternatives. Strategy 1/3/4 considered for injection. SQLite/Bincode/MessagePack/CSV considered for storage."
      assessment: "ADEQUATE"
    q3_constraint_prioritization:
      evidence: "NFR-03 backward compat and D-02 no-git-calls are both addressed as first-class constraints. Feature work does not proceed past them."
      assessment: "CORRECT"
    q4_data_justified:
      evidence: "File size estimate (8.8 MB ceiling), velocity window size rationale, 0.5 stability threshold — all documented. Performance claim (≤0.5s overhead) justified by access pattern analysis (O(N) scan, bounded N)."
      assessment: "JUSTIFIED"

approval_status: "conditionally_approved"
critical_issues_count: 0
high_issues_count: 0
medium_issues_count: 1
low_issues_count: 3
```

## Revisions made in response to review

### Medium issue: history.json → trends.json migration

Addressed in component-boundaries.md and ADR-002. The migration rule:

> On first run after upgrade: if `.repository-analysis/history.json` exists AND `.repository-analysis/trends.json` does not exist, `cache::history` copies `history.json` to `trends.json` before proceeding. The original `history.json` is left in place (non-destructive). This is a one-time forward migration that preserves existing trend data for users who had the pre-release `cache::history` implementation.

This is added to ADR-002 Consequences and noted in component-boundaries.md under `src/cache/history.rs — MODIFIED`.

### Low issues: no action required

The sparkline terminal detection clarification (use `std::io::stdout().is_terminal()`) is implementation detail owned by the software-crafter; it is documented in data-models.md as guidance. The test helper note is a reminder, not an architectural concern.

## Final approval status: APPROVED

All critical and high issues: 0. One medium issue addressed via migration rule. Design is ready for handoff to acceptance-designer.
