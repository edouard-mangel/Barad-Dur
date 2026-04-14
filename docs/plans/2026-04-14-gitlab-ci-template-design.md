# GitLab CI Template Design

## Goal

Allow any GitLab project to add a `barad-dur-analyze` job to their pipeline, via two distribution mechanisms:

- **Plain include** — `include: project:` pointing to this repo
- **CI Catalog component** — discoverable in the GitLab CI/CD Catalog, pinned to a version tag

## Files to Create

### `templates/analyze.yml` — CI Catalog component

Uses the `spec.inputs` / `$[[ inputs.xxx ]]` interpolation syntax required by GitLab CI components.

Referenced by users as:
```yaml
include:
  - component: lab.frogg.it/Edouard_Mangel/barad-dur/analyze@~latest
```

### `templates/barad-dur.yml` — Plain includable template

Uses CI variables for configuration (no `spec:` block). Compatible with `include: project:` on any GitLab instance that can reach `lab.frogg.it`.

Referenced by users as:
```yaml
include:
  - project: 'Edouard_Mangel/barad-dur'
    file: '/templates/barad-dur.yml'
```

Both files define a job named `barad-dur-analyze`.

## Job Behaviour

| Concern | Decision |
|---|---|
| Image | `registry.lab.frogg.it/edouard_mangel/barad-dur:latest` |
| `GIT_DEPTH` | `"0"` — barad-dur requires full git history for churn/coupling analysis |
| Outputs | `barad-dur-report.html` + `barad-dur-report.json` |
| Artifact retention | 1 week by default, configurable |
| Artifact upload | `when: always` so reports are available even on quality gate failure |
| Quality gate | Off by default; enabled by setting `min_score` input or `BARAD_DUR_MIN_SCORE` variable |
| Gate command | `barad-dur gate . --min-score <N>` (exits 1 if score below threshold) |

## Configuration Knobs

| Component input | CI variable | Default | Purpose |
|---|---|---|---|
| `stage` | `BARAD_DUR_STAGE` | `test` | Pipeline stage |
| `min_score` | `BARAD_DUR_MIN_SCORE` | `""` (disabled) | Quality gate threshold (0–100) |
| `artifact_expire` | `BARAD_DUR_ARTIFACT_EXPIRE` | `1 week` | Artifact retention |

## CI Catalog Enablement

- Component versioning is driven by git tags (already using `vX.Y.Z` convention)
- Enable "CI/CD Catalog" in GitLab project Settings → General → Visibility
- `~latest` resolves to the most recent tag; users can also pin to `@v0.12.0`

## Out of Scope

- Windows runner support (Docker image is Linux only)
- `--category` filtering exposed as a knob (can be added later via `extra_flags`)
- Publishing to gitlab.com catalog (self-hosted only for now)
