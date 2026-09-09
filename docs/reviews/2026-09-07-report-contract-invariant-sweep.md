# Report contract implementation review

Date: 2026-09-07

## P0 evidence

The implementation pinned and inspected `ts-rs` 12.0.1 before adding derives.

| Claim | Probe | Evidence |
|---|---|---|
| Recursive programmatic export is available | `cargo info ts-rs@12.0.1`; inspect the downloaded crate's `TS` trait | `TS::export_all(&Config)` recursively visits dependencies and returns `Result<(), ExportError>`. |
| Output and integer representation are configurable | Inspect `Config` in the downloaded 12.0.1 source | `Config::with_out_dir` selects the base directory and `Config::with_large_int("number")` maps `i64`/`u64` to TypeScript numbers. |
| Serde tags and chrono are supported | `cargo info ts-rs@12.0.1`; inspect crate feature/API documentation | Default `serde-compat` handles enum representation; `chrono-impl` emits date-times as strings. |
| Omitted optionals need explicit treatment | Inspect the 12.0.1 Serde compatibility notes and generated scratch output | `skip_serializing_if` alone does not remove `null`; `#[ts(optional)]` is required for serializer-omitted values. The generated declarations are covered by `report_type_generation`. |

## P1 invariant sweep

| Invariant | Call sites found | Verdict |
|---|---|---|
| Rust owns every generated wire declaration | `src/scorer/types.rs`, `src/metrics/mod.rs`, `src/metrics/file_role.rs`, `src/deps.rs`; recursive root in `src/report_contract.rs`; explicit examples in `examples/` | Complete reachable graph is exported; checking compares full file names and bytes. Ordinary builds do not enable `export-types`. |
| Dashboard components receive a validated projection | Upload: `pages/Landing.tsx` → `storeUploadedReport`; restoration: `pages/Report.tsx` → `restoreReport`; both converge on `parseReportText` → `decodeReport` | No component receives unchecked parsed JSON. `JSON.parse` exists only in `report/storage.ts`. |
| Null scores remain distinct from zero | `decode.ts`; `ScoreGauge.tsx`; `RadarChart.tsx`; `CategoryCard.tsx`; `MetricRow.tsx` | Decoder retains null. Components use neutral color/dash behavior; only chart/bar geometry uses zero for an empty visual track. |
| Every raw-value tag has one formatter | `report/format.ts`; sole display call in `MetricRow.tsx` | All six generated tags are handled explicitly and covered by literal expectations. |
| Score bands use the current report's thresholds | `ScoreGauge.tsx`, `RadarChart.tsx`, `CategoryCard.tsx`, `MetricRow.tsx`, `ScoreBar.tsx`, `TopActions.tsx`; values supplied by `Report.tsx` | Every score helper requires thresholds. Radar's effect depends on thresholds. No mutable or default band state remains. |
| Hotspot/coupling risk is separate from score bands | `HotspotsView.tsx::riskColor`; coupling percentage colors in `CouplingView.tsx` | Retained as domain-specific risk scales; they do not import report score formatting. |
| Upload and restoration enforce one current-only policy | `report/storage.ts`, `Landing.tsx`, `Report.tsx` | Both paths parse unknown input and call the same decoder. Legacy string actions, missing roles/counts, and missing thresholds are rejected. |
| Rejection preserves user data | `storeUploadedReport` validates before `setItem`; `restoreReport` never removes storage; router error state carries guidance | Rejected uploads cannot replace a valid saved report, and rejected restored text remains stored. Covered in storage and router tests. |
| Additive unconsumed data remains compatible | Projection construction in `decode.ts` | Unknown top-level and nested unconsumed fields are ignored; the fixture test adds an unknown field and decodes successfully. |

Searches also found no remaining imports of the deleted `dashboard/src/types.ts`,
no `ActionItem | string` compatibility union, no optional report thresholds, and
no hotspot role/finding-count fallbacks.

## P2 evidence

- `make field-test`: `field test clean across 11 repositories` (regression and
  determinism; no baseline was updated).
- `make field-audit`: five rotation recommendations emitted for `barad-dur`.
  The completed worksheet is
  `field-test/audit/2026-09-08-report-contract.md`.
- Safe failures: none. True failures: none. Two pre-existing Actionable
  failures are unchanged from the 2026-09-07 audit and are unrelated to this
  report-boundary refactor.
