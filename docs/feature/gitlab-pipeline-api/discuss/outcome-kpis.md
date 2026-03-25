# Outcome KPIs: gitlab-pipeline-api

## Feature-Level KPIs

### KPI-01: Adoption Rate

- **Who**: DevOps engineers and team leads on Froggit
- **Does what**: Trigger barad-dur analysis from their CI pipelines
- **By how much**: >= 5 projects using the trigger API within 3 months of launch
- **Measured by**: Count of distinct projects triggering the barad-dur pipeline (GitLab pipeline analytics)
- **Baseline**: 0 projects (capability does not exist)

### KPI-02: Trigger Success Rate

- **Who**: CI pipelines triggering barad-dur
- **Does what**: Successfully complete analysis and produce a downloadable artifact
- **By how much**: >= 95% success rate (excluding intentional gate failures)
- **Measured by**: Ratio of successful analyze-api jobs to total triggered jobs (exclude gate-failed as "success with gate fail")
- **Baseline**: N/A (new capability)

### KPI-03: Time to First Analysis

- **Who**: New adopters of the pipeline API
- **Does what**: Go from "never used" to "first successful triggered analysis"
- **By how much**: < 30 minutes from reading docs to first successful trigger
- **Measured by**: Time between first docs page view and first successful pipeline trigger (proxy: support request frequency)
- **Baseline**: N/A (manual process takes ~2 hours including CLI installation)

### KPI-04: Mean Analysis Duration

- **Who**: Triggered pipeline jobs
- **Does what**: Complete full analysis (clone + analyze + artifact save)
- **By how much**: < 10 minutes for repos with < 10,000 commits
- **Measured by**: Average job duration from GitLab CI job metrics
- **Baseline**: Local CLI run ~5-8 minutes (comparable; adds clone overhead)

## Story-Level KPIs

| Story | KPI | Target | Measurement |
|-------|-----|--------|-------------|
| US-01 | Artifact production rate | 100% for valid repos | Job success rate |
| US-02 | First-attempt success | >= 80% | Support requests vs adoptions |
| US-03 | Gate overhead | < 5 seconds added | Job duration delta |
| US-04 | Setup line count | <= 10 lines of CI config | Template usage |
| US-05 | Self-service rate | >= 80% setup without help | Support request ratio |
| US-06 | Branch analysis usage | >= 20% of triggers use non-main | Variable usage stats |
| US-07 | Runtime reduction | 20-40% for 2-category runs | Job duration comparison |
| US-08 | Timeout failures | 0 for < 100K commits with skip-blame | Failure rate |
| US-09 | Concurrent stability | 0 failures at 10 concurrent | Failure rate under load |

## Measurement Infrastructure

| Metric | Source | Collection Method |
|--------|--------|-------------------|
| Trigger count | GitLab pipeline API | Pipeline source = "trigger" filter |
| Job duration | GitLab CI job metrics | Built-in GitLab analytics |
| Success/failure rate | GitLab pipeline status | Built-in pipeline dashboard |
| Artifact download count | GitLab audit log | API access logs (if available) |
| Variable usage | Job logs | Parse trigger variables from log |

## Review Cadence

- **Week 1**: Validate walking skeleton (US-01 + US-02) works E2E
- **Month 1**: Check adoption rate (KPI-01) and success rate (KPI-02)
- **Month 3**: Full KPI review, decide on R3 (robustness) investment
