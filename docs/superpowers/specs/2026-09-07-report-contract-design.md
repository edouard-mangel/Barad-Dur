# M01 — Rust-generated report types and dashboard decoder

Date: 2026-09-07

Status: design decisions recorded; [prose-only plan](../plans/2026-09-07-report-contract.md) complete. Production implementation is not authorized.

Source: [M01 finding](../../reviews/2026-09-07-structure-maintainability.md#m01--establish-a-reliable-dashboard-report-boundary).

## Decisions

The user selected TypeScript types generated from Rust with a separate runtime decoder, and explicitly requires no backward compatibility for older report JSON. Planning contains no implementation code or further probes.

Rust defines the current wire contract. Generate its complete AnalysisReport type graph through an optional development feature and commit the declarations. Proposed ts-rs generation is not part of ordinary Cargo builds; dashboard-only builds need no Rust toolchain.

The decoder constructs a validated projection of consumed fields, derived from generated types. It does not claim to validate every unused report section or maintain another handwritten wire model.

## Current-format policy

- Require current consumed fields, including thresholds, hotspot roles/finding counts, and structured actions.
- Preserve null scores and fields deliberately omitted by the current serializer.
- Reject primitive raw values, plain-text actions, and absent required fields instead of adding compatibility defaults.
- Validate nested consumed fields and return precise error paths.
- Accept unrelated additive fields, including the unconsumed dynamic trend envelope.
- An older file with an already-valid current shape can load; this is shape validation, not a producer-age restriction.
- Guide users to regenerate incompatible JSON with the installed CLI; never delete or migrate the original report.

Snapshot caches and trend histories are separate formats. This decision does not authorize resetting them.

## Data, display, and loading boundaries

Generation describes enum tags, lowercase file roles, timestamps, nullable fields, and deliberate omissions. Integer declarations use numbers because the current JSON protocol does; this does not promise lossless arbitrary 64-bit precision.

The decoder validates identity/counts, categories/metrics, actions, nullable remote metadata, hotspot/coupling rows, ownership, ages, and effective thresholds. Raw percentages need not fall between zero and one hundred; scores do. Optional navigation fields may be absent, but malformed present values are rejected.

Formatting handles every raw variant explicitly. Thresholds are explicit inputs to all score helpers/components, including TopActions and radar effects. Null remains visually unscored; hotspot risk is a separate quantity.

Upload and session restoration share parsing, decoding, and storage error handling. Validate before storing, revalidate on restoration, preserve rejected data, and show actionable errors.

## Generation and delivery

Generation is explicit. Checking compares complete generated file sets and contents without modifying them; additions, removals, and edits all count as drift. Deterministic producer-owned fixtures verify actual serialization against declarations and feed frontend tests.

Rust and Node CI requirements remain explicit and use the exact merge-result checkout convention. Documentation explains regeneration and that self-contained HTML retains its embedded renderer.

## Invariants

1. Current wire declarations derive from Rust and have reproducible drift checks.
2. Generation changes neither JSON/bincode representation nor ordinary build requirements.
3. Components receive validated data, not unchecked JSON.
4. Null scores remain distinct from zero; raw tags retain their meaning.
5. Report order cannot affect thresholds or colors.
6. Upload and restoration enforce the same current-only policy.
7. Rejection never destroys reports or resets caches/history.
8. Unconsumed additive fields do not break the projection.

## Existing evidence and future checks

Before the user clarified the planning-only boundary, isolated scratch work with ts-rs 12.0.1 exported 35 declarations: 26 scorer types, 3 metric types, FileRole, and 5 dependency types. Generated declarations and a decoder sketch passed isolated strict TypeScript checking; the scratch package also built with generation disabled.

Those checks showed that deliberately omitted fields need explicit generator annotations and history deserialization aliases produce known diagnostics without changing current output names. They did not establish application compilation, fixture determinism, browser behavior, packaging, or CI correctness. No further probes were performed for this prose-only revision.

Future implementation must verify the remaining claims and complete the full-suite, P1, and P2 gates. Earlier API references: [ts-rs TS](https://docs.rs/ts-rs/12.0.1/ts_rs/trait.TS.html) and [ts-rs Config](https://docs.rs/ts-rs/12.0.1/ts_rs/struct.Config.html).
