# Dependencies Category — Libyear + CVE Detection

**Date:** 2026-04-14
**Status:** Approved

## Goal

Add a new **Dependencies** scoring category that measures dependency freshness (libyear drift) and known security vulnerabilities (OSV) across all lock files found in the repository.

## Decisions

| Topic | Decision |
|-------|----------|
| Activation | `--deps` flag — offline by default |
| Ecosystems | Cargo, npm, pip, NuGet |
| Network sources | Registry APIs (dates) + OSV API (CVEs) |
| Cache | `.repository-analysis/deps-cache.json`, TTL 7 days per entry |
| Category weight | 20% of overall score |
| Overall weights | Health 35%, Evolution 20%, Dependencies 20%, Hygiene 15%, Team 10% |
| Output granularity | Per-ecosystem aggregates + named critical callouts |

## Data Sources

### Registry APIs (libyear drift)

| Ecosystem | Lock file(s) | API endpoint |
|-----------|-------------|--------------|
| Cargo | `Cargo.lock` | `https://crates.io/api/v1/crates/{name}/versions` |
| npm | `package-lock.json`, `yarn.lock`, `pnpm-lock.yaml` | `https://registry.npmjs.org/{name}` |
| pip | `requirements.txt`, `Pipfile.lock`, `pyproject.toml` | `https://pypi.org/pypi/{name}/json` |
| NuGet | `packages.lock.json`, `*.csproj` | `https://api.nuget.org/v3/registration5/{name}/index.json` |

### OSV API (vulnerabilities)

Single endpoint covering all ecosystems:
```
POST https://api.osv.dev/v1/query
{
  "package": { "name": "<name>", "ecosystem": "<Cargo|npm|PyPI|NuGet>" },
  "version": "<version>"
}
```

## Data Structures

```rust
pub enum Ecosystem { Cargo, Npm, Pip, Nuget }

pub enum DepTier { Fresh, Aging, Stale, Critical }
// Fresh: < 0.5y  |  Aging: 0.5–2y  |  Stale: 2–5y  |  Critical: > 5y

pub struct Vuln {
    pub id: String,           // e.g. "GHSA-xxxx" or "CVE-xxxx"
    pub severity: String,     // "LOW" | "MEDIUM" | "HIGH" | "CRITICAL"
    pub description: String,
}

pub struct DepAge {
    pub name: String,
    pub ecosystem: Ecosystem,
    pub current_version: String,
    pub drift_years: f64,
    pub tier: DepTier,
    pub vulnerabilities: Vec<Vuln>,  // empty if none known
}

pub struct EcosystemReport {
    pub ecosystem: Ecosystem,
    pub total_deps: usize,
    pub mean_drift_years: f64,
    pub total_drift_years: f64,
    pub critical_deps: Vec<DepAge>,  // tier == Critical OR has vulnerabilities
}
```

## Critical Callout Triggers

A dependency is named explicitly in output when either condition is true:
- **Stale**: `drift_years > 5.0`
- **Vulnerable**: at least one entry in `vulnerabilities`

## Scoring

Per-ecosystem score derived from mean drift:

| Mean drift | Base score |
|------------|------------|
| 0y | 100 |
| 0.5y | 90 |
| 1y | 75 |
| 2y | 55 |
| 5y | 25 |
| 10y+ | 0 |

Penalty: each HIGH or CRITICAL severity CVE deducts 5 points (floor 0).

Final Dependencies score = average of per-ecosystem scores (ecosystems with no lock file are skipped).

## Cache Format

File: `.repository-analysis/deps-cache.json`

```json
{
  "cargo:serde:1.0.130": {
    "current_published": "2021-09-14T00:00:00Z",
    "latest_version": "1.0.197",
    "latest_published": "2024-02-22T00:00:00Z",
    "vulnerabilities": [],
    "cached_at": "2026-04-14T10:00:00Z"
  }
}
```

Key format: `{ecosystem}:{name}:{version}` (lowercase).
TTL: 7 days — entries older than 7 days are re-fetched.

## CLI Behaviour

```bash
barad-dur analyze . --deps          # enable dependency analysis
barad-dur analyze . --deps --json   # JSON output includes dep data
barad-dur analyze .                 # no --deps → Dependencies category skipped silently
```

## Rendering

**CLI:** Dependencies section shows per-ecosystem mean drift + critical callout list.
**JSON:** `dep_ecosystem_reports` array + `critical_deps` per ecosystem.
**HTML:** New "Dependencies" tab with per-ecosystem cards and a critical callouts section (only shown when non-empty).

## Module Layout

```
src/
  collector/
    deps.rs        — lock file discovery + parsing
  registry/
    mod.rs         — dispatch to per-ecosystem fetcher + OSV
    cargo.rs
    npm.rs
    pip.rs
    nuget.rs
    osv.rs
    cache.rs       — TTL cache read/write
  metrics/
    deps.rs        — pure scoring function (EcosystemReport → MetricValue)
```
