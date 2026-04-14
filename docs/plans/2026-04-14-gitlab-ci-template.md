# GitLab CI Template Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Provide a reusable GitLab CI job that any project can include to run barad-dur analysis, available as both a plain `include: project:` template and a GitLab CI Catalog component.

**Architecture:** Two files in `templates/`: `barad-dur.yml` (plain include, CI variables) and `analyze.yml` (CI Catalog component, `spec.inputs`). Both download the binary from the Generic Package Registry and run two separate analysis passes (JSON then HTML) since `--json` and `--html` are mutually exclusive flags. Quality gate is opt-in via `BARAD_DUR_MIN_SCORE` / `min_score` input.

**Tech Stack:** GitLab CI YAML, `glab ci lint` for validation, shell (`alpine:3.21`), barad-dur Generic Package Registry.

---

## Context: What Already Exists

`templates/analyze.yml` exists but has two issues to fix:
1. Uses `--html --json` combined — these flags are mutually exclusive (error: `--json and --html are mutually exclusive`)
2. Missing quality gate support

**Plan:** rename/repurpose the existing file and create the catalog component.

---

## Task 1: Fix and rename `templates/analyze.yml` → `templates/barad-dur.yml`

This becomes the **plain include** template — no `spec:` block, configured via CI variables.

**Files:**
- Rename: `templates/analyze.yml` → `templates/barad-dur.yml`

**Step 1: Create `templates/barad-dur.yml`**

```yaml
# barad-dur analysis template — plain include version
#
# Add to your .gitlab-ci.yml:
#
#   include:
#     - project: Edouard_Mangel/barad-dur
#       ref: main          # or pin to a tag: ref: v0.12.0
#       file: templates/barad-dur.yml
#
# Configure via CI/CD variables (project Settings → CI/CD → Variables):
#
#   BARAD_DUR_MIN_SCORE  Quality gate threshold 0–100. Unset = disabled.
#   BARAD_DUR_VERSION    Pin a release tag, e.g. "v0.12.0". Unset = latest.
#   BARAD_DUR_FLAGS      Extra flags for `barad-dur analyze`, e.g. "--skip-blame".

barad-dur-analyze:
  stage: test
  image: alpine:3.21
  variables:
    GIT_DEPTH: "0"
    BARAD_DUR_FLAGS: ""
    BARAD_DUR_VERSION: ""
    BARAD_DUR_REGISTRY: "https://lab.frogg.it/api/v4/projects/Edouard_Mangel%2Fbarad-dur"
  before_script:
    - apk add --no-cache curl git
    - |
      if [ -z "$BARAD_DUR_VERSION" ]; then
        BARAD_DUR_VERSION=$(curl -sf --header "JOB-TOKEN: ${CI_JOB_TOKEN}" \
          "${BARAD_DUR_REGISTRY}/releases/permalink/latest" \
          | grep -o '"tag_name":"[^"]*"' | cut -d'"' -f4)
      fi
    - echo "Downloading barad-dur ${BARAD_DUR_VERSION}"
    - |
      curl -fL --header "JOB-TOKEN: ${CI_JOB_TOKEN}" \
        "${BARAD_DUR_REGISTRY}/packages/generic/barad-dur/${BARAD_DUR_VERSION}/barad-dur-linux-x86_64" \
        -o /usr/local/bin/barad-dur
    - chmod +x /usr/local/bin/barad-dur
    - barad-dur --version
  script:
    - barad-dur analyze . --json $BARAD_DUR_FLAGS > barad-dur-report.json
    - barad-dur analyze . --html $BARAD_DUR_FLAGS -o barad-dur-report.html
    - |
      if [ -n "${BARAD_DUR_MIN_SCORE:-}" ]; then
        echo "Running quality gate (threshold: ${BARAD_DUR_MIN_SCORE})..."
        barad-dur gate . --min-score "${BARAD_DUR_MIN_SCORE}"
      fi
  artifacts:
    name: barad-dur-report
    paths:
      - barad-dur-report.json
      - barad-dur-report.html
    expose_as: "barad-dur report"
    expire_in: 1 month
    when: always
  rules:
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
    - if: $CI_MERGE_REQUEST_IID
```

**Step 2: Delete the old file**

```bash
rtk git rm templates/analyze.yml
```

**Step 3: Validate**

```bash
# Create a minimal test file that includes the template
cat > /tmp/test-include.yml << 'EOF'
include:
  - local: templates/barad-dur.yml

stages:
  - test
EOF
glab ci lint /tmp/test-include.yml
```

Expected: `✓ Pipeline is valid`

> Note: `glab ci lint` validates against the remote GitLab instance, so this must be run from inside the repo directory (where `glab` knows the remote). If lint reports an error about the `include:` source not being reachable locally, that's expected for project includes — the job YAML structure is what matters.

**Step 4: Commit**

```bash
rtk git add templates/barad-dur.yml && rtk git rm templates/analyze.yml
rtk git commit -m "feat(ci-template): add plain-include template with quality gate support"
```

---

## Task 2: Create `templates/analyze.yml` — CI Catalog component

This is the **CI Catalog component** version, using `spec.inputs` and `$[[ inputs.xxx ]]` interpolation.

**Files:**
- Create: `templates/analyze.yml`

**Step 1: Create `templates/analyze.yml`**

```yaml
spec:
  inputs:
    min_score:
      default: ""
      description: "Quality gate threshold 0–100. Empty string = disabled."
    version:
      default: ""
      description: "Pin a barad-dur release tag, e.g. 'v0.12.0'. Empty = latest."
    flags:
      default: ""
      description: "Extra flags passed to `barad-dur analyze`, e.g. '--skip-blame'."
---
# barad-dur CI Catalog component
#
# Usage:
#   include:
#     - component: lab.frogg.it/Edouard_Mangel/barad-dur/analyze@~latest

barad-dur-analyze:
  stage: test
  image: alpine:3.21
  variables:
    GIT_DEPTH: "0"
    BARAD_DUR_REGISTRY: "https://lab.frogg.it/api/v4/projects/Edouard_Mangel%2Fbarad-dur"
  before_script:
    - apk add --no-cache curl git
    - |
      VERSION="${[[ inputs.version ]]}"
      if [ -z "$VERSION" ]; then
        VERSION=$(curl -sf --header "JOB-TOKEN: ${CI_JOB_TOKEN}" \
          "${BARAD_DUR_REGISTRY}/releases/permalink/latest" \
          | grep -o '"tag_name":"[^"]*"' | cut -d'"' -f4)
      fi
    - echo "Downloading barad-dur ${VERSION}"
    - |
      curl -fL --header "JOB-TOKEN: ${CI_JOB_TOKEN}" \
        "${BARAD_DUR_REGISTRY}/packages/generic/barad-dur/${VERSION}/barad-dur-linux-x86_64" \
        -o /usr/local/bin/barad-dur
    - chmod +x /usr/local/bin/barad-dur
    - barad-dur --version
  script:
    - barad-dur analyze . --json $[[ inputs.flags ]] > barad-dur-report.json
    - barad-dur analyze . --html $[[ inputs.flags ]] -o barad-dur-report.html
    - |
      if [ -n "$[[ inputs.min_score ]]" ]; then
        echo "Running quality gate (threshold: $[[ inputs.min_score ]])..."
        barad-dur gate . --min-score "$[[ inputs.min_score ]]"
      fi
  artifacts:
    name: barad-dur-report
    paths:
      - barad-dur-report.json
      - barad-dur-report.html
    expose_as: "barad-dur report"
    expire_in: 1 month
    when: always
  rules:
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
    - if: $CI_MERGE_REQUEST_IID
```

**Step 2: Validate**

```bash
glab ci lint templates/analyze.yml
```

Expected: `✓ Pipeline is valid` (or a note about `spec:` being a component-only key — that's fine as long as the job YAML is structurally valid).

**Step 3: Commit**

```bash
rtk git add templates/analyze.yml
rtk git commit -m "feat(ci-template): add CI Catalog component (spec.inputs)"
```

---

## Task 3: Enable CI/CD Catalog (manual step — cannot be automated)

This requires a human in the GitLab UI:

1. Go to `lab.frogg.it/Edouard_Mangel/barad-dur` → Settings → General → Visibility, project features, permissions
2. Enable **CI/CD Catalog** toggle
3. Push a version tag (e.g. `v0.12.0` if not yet pushed) — the catalog entry is created on tag push

> The tag `v0.12.0` is noted as pending in project memory. Pushing it will both publish the binaries to the package registry AND register the component version in the catalog.

---

## Task 4: Smoke-test the plain include from SightKick

Verify the template works end-to-end by checking if `CI_JOB_TOKEN` auth against the package registry works cross-project on `lab.frogg.it`.

**Step 1: Check if `BARAD_DUR_REGISTRY` is accessible from another project's pipeline**

From the SightKick project, the API endpoint `https://lab.frogg.it/api/v4/projects/Edouard_Mangel%2Fbarad-dur/releases/permalink/latest` must return the latest release. This works if `barad-dur` is a public project (which it is, based on the public GitLab Pages URL).

**Step 2: Check public package download without token (optional fallback)**

```bash
# Test that the binary is publicly downloadable (no JOB-TOKEN needed)
curl -fL "https://lab.frogg.it/api/v4/projects/Edouard_Mangel%2Fbarad-dur/packages/generic/barad-dur/v0.12.0/barad-dur-linux-x86_64" -o /tmp/barad-dur-test
chmod +x /tmp/barad-dur-test
/tmp/barad-dur-test --version
```

If this works without a token, the template is fully portable to any GitLab instance. If it requires a token, document that `lab.frogg.it` must be reachable from the consuming project's runners.

**Step 3: Commit any fixes**

```bash
rtk git commit -m "fix(ci-template): ..."
```
