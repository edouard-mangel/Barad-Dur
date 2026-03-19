# Outcome KPIs: historical-trends

## Objective

Within 8 weeks of shipping Release 1, engineering leads and senior developers who use barad-dur regularly are able to state the direction of their repo's health with quantitative evidence — and do so in under 30 seconds.

---

## Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| 1 | Users who run analyze 2+ times | Accumulate trend history without manual steps | 100% of repeat runs produce growing trends.json | 0% — no trend data stored today | Integration test pass rate; CI log verification | Leading |
| 2 | Engineering leads after a regular run | Read directional answer (improving/declining/stable) inline in CLI | Time to answer "is it better?" < 30 sec (from command invocation) | ~5 min (manual spreadsheet lookup) | User feedback; timing integration test | Leading |
| 3 | Developers preparing sprint reviews | Produce a shareable before/after trend artifact without manual data assembly | Before/after comparison available in 1 command (`--trend`) in < 60 sec | N/A (no artifact today) | User feedback; manual verification | Leading |
| 4 | CI/CD pipeline consumers | Parse trend data from JSON output without custom diffing scripts | Dashboard integration completable in < 1 hour using published schema | N/A (no machine-readable trend data) | Schema conformance tests; downstream consumer feedback | Leading |
| 5 | All users (guardrail) | Analysis runtime not meaningfully degraded by trend recording | Runtime delta < 0.5 sec per run | Current average: ~11s for this repo | Performance integration test | Guardrail |

---

## Metric Hierarchy

**North Star**: users with 2+ weeks of history answer "is my repo improving?" in under 30 seconds without leaving the terminal.

**Leading Indicators**:
- trends.json accumulation rate (% of repeat runs that produce entries)
- Time to directional answer (measurable in a usability test or via user feedback)
- Schema adoption rate (CI integrations using `--json --trend`)

**Guardrail Metrics** (must NOT degrade):
- Analysis runtime: no increase > 0.5 seconds
- JSON backward compatibility: 0 breaking changes to `--json` without `--trend`
- Exit code reliability: corrupt trends.json must never cause exit code 1

---

## Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|------------|-------------------|-----------|-------|
| trends.json accumulation | Integration tests | CI pass rate on trend recording tests | Each CI run | DESIGN wave |
| Runtime delta | Performance test | Measure before/after trend-enabled run on fixture repo | Each CI run | DESIGN wave |
| JSON backward compat | Contract test | Fixture JSON diff vs baseline | Each CI run | DESIGN wave |
| Directional answer time | User feedback | Qualitative; ask at release | Post-release | Product owner |
| Schema adoption | CI logs / user reports | Qualitative; self-reported | 4 weeks post-release | Product owner |

---

## Hypothesis

We believe that passive trend recording (auto-appended on every `analyze .` run, zero extra steps) for repeat barad-dur users will produce directional health data within 2-4 weeks of adoption.

We will know this is true when a user who has run barad-dur at least 4 times can read their trajectory from the CLI output in under 30 seconds without opening any other file or tool.

---

## Notes

- KPIs 1-4 are leading indicators tied to behavior change, not feature delivery ("shipped US-01" is an output, not an outcome)
- KPI 5 (guardrail) is critical because the primary adoption anxiety is performance cost
- Velocity KPI is intentionally absent at this stage: the feature has no prior users, so baseline trend velocity data does not exist. Add velocity KPI in Release 2 retrospective.
