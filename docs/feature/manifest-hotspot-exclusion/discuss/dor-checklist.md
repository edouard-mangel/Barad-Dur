# Definition of Ready — manifest-hotspot-exclusion

9-item DoR. Each validated with evidence before DESIGN handoff.

| # | DoR item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem clearly stated | ✅ | requirements.md → Problem; manifests pollute hotspots, lockfiles already handled |
| 2 | User/value articulated | ✅ | user-stories.md US-1..3; implicit job stated |
| 3 | Scope bounded (in/out) | ✅ | requirements.md → Scope; 3 explicit out-of-scope items |
| 4 | Acceptance criteria testable | ✅ | acceptance-criteria.md AC-1..5, all boolean/equality assertions |
| 5 | Dependencies identified | ✅ | None blocking; S1 independent of S2/S3 (story-map.md) |
| 6 | Constraints captured | ✅ | requirements.md C-1 (open), C-2, C-3 |
| 7 | Measurable outcome defined | ✅ | outcome-kpis.md K-1..4 with baselines/targets |
| 8 | Technical feasibility confirmed | ✅ | Hook point `is_excluded`/`DEFAULT_EXCLUDE_PATTERNS` exists + tested; safety invariant (NFR-1) verified with code citations in **discuss-verification.md** (deps.rs:11-107 disk reads; coupling/dependency.rs:301-304 disk reads; exclusion applied at snapshot_builder.rs:~96) |
| 9 | Open questions flagged, not hidden | ✅ | C-1 opt-out granularity explicitly deferred to DESIGN |

## Verdict
**READY** for DESIGN. One open question (C-1) is explicitly carried forward as a
design decision, not a blocker — the v1 scope (S1) is fully specified and feasible
without resolving it.
