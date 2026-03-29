# Branching Strategy — release-pipeline

## Model: Trunk-Based Development

Single long-lived branch: `main`. No `develop`, no `release/*`, no `hotfix/*` branches.

### Rationale
- Single maintainer project — branch management overhead not justified
- Short-lived feature branches (< 2 days) merged via MR
- Tags drive releases, not branches

## Branch Protection on `main`

Configure in GitLab: Settings > Repository > Protected Branches

| Rule | Setting |
|---|---|
| Allowed to push | Maintainers only (no direct push by default) |
| Allowed to merge | Maintainers |
| Require MR before merge | Yes |
| Require pipeline to succeed | Yes |
| Require resolved threads | Yes |
| Allow force push | No |

For a single-maintainer project, the "Require MR" rule can be relaxed for minor changes (typos, docs), but all code changes and all CI config changes must go through an MR to ensure pipeline validation before merge.

## MR Requirements

| Gate | Required |
|---|---|
| Pipeline passes (lint + build + test + analysis) | Yes — blocking |
| Coverage does not decrease | Recommended (enforce via `--fail-under 80` in tarpaulin) |
| Mutation kill rate ≥ 80% | Yes — blocking (per-feature mutation job on MR) |
| No new SAST findings (critical/high) | Yes — blocking |
| Secret detection clean | Yes — blocking |
| At least 1 approval | Optional for solo project — enable when collaborators join |

## Fast-Path for Hotfixes

When a critical bug is discovered in a released version:

1. Create `hotfix/vX.Y.Z` branch from the release tag: `git checkout -b hotfix/v1.2.1 v1.2.0`
2. Apply minimal fix
3. Push and create MR targeting `main`
4. Pipeline runs full suite — no gates are skipped
5. After MR merges to `main`, tag the fix: `git tag v1.2.1`
6. The tag pipeline handles binary builds and Docker image

There is no separate hotfix pipeline — the tag pipeline is sufficient. The time cost of full CI is accepted in exchange for release integrity.

## Tag Convention

All releases use semantic versioning with a `v` prefix.

| Pattern | Purpose | Example |
|---|---|---|
| `v{major}.{minor}.{patch}` | Stable release | `v1.2.0` |
| `v{major}.{minor}.{patch}-beta.{n}` | Pre-release (optional) | `v1.2.0-beta.1` |

The existing pipeline rule `$CI_COMMIT_TAG =~ /^v\d+\.\d+\.\d+$/` only matches stable tags. Pre-release tags do not trigger the release pipeline — they are for internal testing only.

### Release Process

```
1. All changes merged to main
2. Confirm pipeline green on main
3. Confirm cargo.toml version matches intended tag
4. git tag v1.2.0
5. git push origin v1.2.0
6. Tag pipeline runs: lint → build → test → analysis → release → docker
7. release-publish creates GitLab Release with binary download links
8. Docker image tagged :v1.2.0 and :latest
```

Step 3 is a manual check. A `cargo-semver-checks` run is already in the `analysis` stage to catch unintentional API breaks before publishing.

## Commit Message Convention

No formal convention enforced by CI today. Recommended practice:

```
<type>(<scope>): <summary>

Types: feat | fix | refactor | test | chore | docs | ci | perf
```

This enables future changelog generation via `git-cliff` or similar if desired.
