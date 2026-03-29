# Observability Design — release-pipeline

## Context

barad-dur has no production server. Observability is entirely CI/CD pipeline observability — build health, test quality, release integrity, and distribution success. GitLab CI built-in features provide the full observability surface.

## Test Report Integration

### Coverage Widget (already implemented)
`cargo-tarpaulin` emits `coverage/cobertura.xml`. Surfaced via:
```yaml
artifacts:
  reports:
    coverage_report:
      coverage_format: cobertura
      path: coverage/cobertura.xml
```
GitLab renders a coverage percentage badge and diff annotation on MRs showing which lines are newly uncovered.

### JUnit XML (gap — not yet implemented)
`cargo test` does not emit structured test results. To add:

Option A — `cargo-nextest`:
```bash
cargo install cargo-nextest --locked
cargo nextest run --profile ci
```
With a `nextest.toml` profile:
```toml
[profile.ci]
status-level = "all"
final-status-level = "all"
junit = { path = "test-results/junit.xml" }
```
Then in `.gitlab-ci.yml`:
```yaml
artifacts:
  reports:
    junit: test-results/junit.xml
```

Option B — `cargo test` + `cargo2junit`:
```bash
cargo test -- -Z unstable-options --format json | cargo2junit > junit.xml
```
Requires nightly toolchain — not preferred for a stable Rust project.

**Recommendation**: cargo-nextest. Faster than `cargo test`, native JUnit output, stable toolchain.

**Value delivered**: MR diff view shows individual test pass/fail. Flaky test detection via GitLab's test analytics. Historical test duration trends.

## Coverage Visualization

GitLab parses the `coverage:` regex line from job output:
```yaml
coverage: '/^\d+.\d+% coverage/'
```
This populates the pipeline coverage badge. The Cobertura report enables line-level coverage overlay in the MR diff.

**Current gap**: coverage threshold enforcement. Tarpaulin exits 0 regardless of coverage percentage. To enforce a minimum:
```bash
cargo tarpaulin --engine llvm --out xml --output-dir coverage/ --fail-under 80
```
`--fail-under 80` makes the job fail if coverage drops below 80%, blocking merge.

## Pipeline Failure Alerting

GitLab CI provides built-in email notification on pipeline failure (Settings > Notifications). For a single-maintainer project this is sufficient.

For the `mutation-nightly` job specifically, since it runs on schedule and failures are silent until manually checked:
- Enable "Pipeline failed" email notifications for the project
- Alternatively, use a GitLab webhook → self-hosted endpoint or ntfy.sh for push notification on schedule pipeline failure

No external alerting infrastructure is required.

## Job Duration Tracking

GitLab CI records job duration for every run. Accessible via:
- Pipeline list view (duration column)
- `GET /projects/:id/jobs` API

Jobs with known duration baselines to track:

| Job | Expected duration | Alert threshold |
|---|---|---|
| `build` | ~3 min | >10 min (cache miss) |
| `test` | ~5 min | >20 min |
| `coverage` | ~8 min (tarpaulin install) | >30 min |
| `mutation` (per-feature) | 5–20 min (diff-scoped) | >45 min |
| `mutation-nightly` | 2–6 hours (full codebase) | inform only |
| `release-linux` | ~5 min | >20 min |
| `release-windows` | ~8 min | >30 min |

Long-running jobs (`coverage`, `mutation`) have `timeout:` set to prevent runaway execution consuming runner capacity.

## Self-Analysis as Operational Health Metric

The `self-analysis` job runs barad-dur on its own repository on every `main` push. The resulting `barad-dur-report.html` published to GitLab Pages acts as a living dashboard of the codebase's own health metrics (complexity, coupling, team ownership, evolution).

URL: `https://edouard_mangel.lab.frogg.it/barad-dur/` (GitLab Pages endpoint)

This provides a continuous health signal without external monitoring infrastructure.

## Artifact-Based Audit Trail

| Signal | Source | Retention |
|---|---|---|
| Test pass/fail | JUnit XML (proposed) | GitLab pipeline history |
| Coverage trend | Cobertura XML | 1 week per run |
| CVE status | cargo-audit output + trivy scan | job trace |
| Mutation kill rate | `mutants.log` + `mutants.out/` | 1 week |
| Binary artifact checksums | GitLab Package Registry | permanent (on tag) |
| Docker image layers | GitLab Container Registry | until manually cleaned |
| Codebase health report | barad-dur self-analysis | 1 month |
