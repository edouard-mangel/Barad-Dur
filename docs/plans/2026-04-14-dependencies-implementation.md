# Dependencies Category Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a Dependencies scoring category that measures dependency freshness (libyear) and known CVEs (OSV) across Cargo/npm/pip/NuGet lock files, activated via `--deps` flag.

**Architecture:** Lock files are discovered and parsed by `src/collector/deps.rs`. Release dates and CVEs are fetched from registries (crates.io/npmjs/pypi/nuget) and the OSV API, with results cached for 7 days in `.repository-analysis/deps-cache.json`. A pure scoring function in `src/metrics/deps.rs` converts per-ecosystem data into a `CategoryResult` (20% of overall score).

**Tech Stack:** `reqwest` (blocking, already in Cargo.toml), `serde_json`, `chrono` — all already present. No new dependencies needed.

---

### Task 1: CLI flag and config weights

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/config.rs`

**Step 1: Write the failing test for the `--deps` flag**

In `src/cli.rs`, inside the existing `#[cfg(test)] mod tests`, add:

```rust
#[test]
fn deps_flag() {
    let args = parse(&["barad-dur", "analyze", ".", "--deps"]);
    assert!(args.deps);
}

#[test]
fn deps_not_in_all_categories() {
    // --deps is opt-in, not included in "all categories" by default
    let args = parse(&["barad-dur", "analyze", "."]);
    assert!(!args.deps);
}
```

**Step 2: Run to verify failure**

```bash
cargo test --lib cli::tests::deps_flag 2>&1 | tail -5
```
Expected: `error[E0609]: no field 'deps'`

**Step 3: Add `--deps` to `AnalyzeArgs`**

In `src/cli.rs`, inside `AnalyzeArgs`, after the `hygiene` field:

```rust
/// Enable dependency analysis (libyear + CVE detection)
///
/// Fetches release dates from crates.io, npmjs, pypi, and nuget, and
/// checks for known CVEs via the OSV API. Results are cached for 7 days.
/// Requires network access on first run.
#[arg(long, help_heading = "Category Filters")]
pub deps: bool,
```

Update `should_run()` to handle `"deps"`:

```rust
pub fn should_run(&self, category: &str) -> bool {
    if self.all_categories() {
        return category != "deps"; // deps always requires explicit --deps
    }
    match category {
        "health" => self.health,
        "team" => self.team,
        "evolution" => self.evolution,
        "hygiene" => self.hygiene,
        "deps" => self.deps,
        _ => false,
    }
}
```

**Step 4: Run to verify tests pass**

```bash
cargo test --lib cli::tests 2>&1 | tail -5
```
Expected: all cli tests pass.

**Step 5: Update `CategoryWeights` in `src/config.rs`**

Add `deps` field:

```rust
pub struct CategoryWeights {
    #[serde(default = "default_health_weight")]
    pub health: u32,
    #[serde(default = "default_team_weight")]
    pub team: u32,
    #[serde(default = "default_evolution_weight")]
    pub evolution: u32,
    #[serde(default = "default_hygiene_weight")]
    pub hygiene: u32,
    #[serde(default = "default_coupling_weight")]
    pub coupling: u32,
    #[serde(default = "default_deps_weight")]
    pub deps: u32,
}
```

Update default weight functions:

```rust
fn default_health_weight() -> u32 { 35 }
fn default_team_weight() -> u32 { 10 }
fn default_evolution_weight() -> u32 { 20 }
fn default_hygiene_weight() -> u32 { 15 }
fn default_coupling_weight() -> u32 { 20 }
fn default_deps_weight() -> u32 { 0 } // 0 = excluded unless --deps is passed
```

Update `Default` impl, `sum()`, and `as_weight_pairs()`:

```rust
pub fn as_weight_pairs(&self) -> Vec<(&'static str, f64)> {
    let s = self.sum() as f64;
    let mut pairs = vec![
        ("Health", self.health as f64 / s),
        ("Team", self.team as f64 / s),
        ("Evolution", self.evolution as f64 / s),
        ("Git Hygiene", self.hygiene as f64 / s),
        ("Coupling", self.coupling as f64 / s),
    ];
    if self.deps > 0 {
        pairs.push(("Dependencies", self.deps as f64 / s));
    }
    pairs
}
```

**Step 6: Verify config compiles**

```bash
cargo check 2>&1 | grep "^error" | head -10
```

**Step 7: Commit**

```bash
rtk git add src/cli.rs src/config.rs
rtk git commit -m "feat(deps): add --deps CLI flag and CategoryWeights.deps field"
```

---

### Task 2: Core data types

**Files:**
- Create: `src/deps.rs`
- Modify: `src/lib.rs` (add `pub mod deps;`)

**Step 1: Create `src/deps.rs` with tests**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Ecosystem {
    Cargo,
    Npm,
    Pip,
    Nuget,
}

impl Ecosystem {
    pub fn osv_name(&self) -> &'static str {
        match self {
            Ecosystem::Cargo => "crates.io",
            Ecosystem::Npm => "npm",
            Ecosystem::Pip => "PyPI",
            Ecosystem::Nuget => "NuGet",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Ecosystem::Cargo => "Cargo",
            Ecosystem::Npm => "npm",
            Ecosystem::Pip => "pip",
            Ecosystem::Nuget => "NuGet",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DepTier {
    Fresh,    // drift < 0.5y
    Aging,    // 0.5y – 2y
    Stale,    // 2y – 5y
    Critical, // > 5y
}

impl DepTier {
    pub fn from_drift(drift_years: f64) -> Self {
        match drift_years {
            d if d < 0.5 => DepTier::Fresh,
            d if d < 2.0 => DepTier::Aging,
            d if d < 5.0 => DepTier::Stale,
            _ => DepTier::Critical,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vuln {
    pub id: String,
    pub severity: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepAge {
    pub name: String,
    pub ecosystem: Ecosystem,
    pub current_version: String,
    pub drift_years: f64,
    pub tier: DepTier,
    pub vulnerabilities: Vec<Vuln>,
}

impl DepAge {
    pub fn is_critical_callout(&self) -> bool {
        self.tier == DepTier::Critical || !self.vulnerabilities.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemReport {
    pub ecosystem: Ecosystem,
    pub total_deps: usize,
    pub mean_drift_years: f64,
    pub total_drift_years: f64,
    pub critical_deps: Vec<DepAge>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dep_tier_boundaries() {
        assert_eq!(DepTier::from_drift(0.0), DepTier::Fresh);
        assert_eq!(DepTier::from_drift(0.49), DepTier::Fresh);
        assert_eq!(DepTier::from_drift(0.5), DepTier::Aging);
        assert_eq!(DepTier::from_drift(1.99), DepTier::Aging);
        assert_eq!(DepTier::from_drift(2.0), DepTier::Stale);
        assert_eq!(DepTier::from_drift(4.99), DepTier::Stale);
        assert_eq!(DepTier::from_drift(5.0), DepTier::Critical);
        assert_eq!(DepTier::from_drift(10.0), DepTier::Critical);
    }

    #[test]
    fn critical_callout_by_drift() {
        let dep = DepAge {
            name: "old-crate".into(),
            ecosystem: Ecosystem::Cargo,
            current_version: "0.1.0".into(),
            drift_years: 6.0,
            tier: DepTier::Critical,
            vulnerabilities: vec![],
        };
        assert!(dep.is_critical_callout());
    }

    #[test]
    fn critical_callout_by_vuln() {
        let dep = DepAge {
            name: "vulnerable-pkg".into(),
            ecosystem: Ecosystem::Npm,
            current_version: "1.0.0".into(),
            drift_years: 0.3,
            tier: DepTier::Fresh,
            vulnerabilities: vec![Vuln {
                id: "CVE-2024-1234".into(),
                severity: "HIGH".into(),
                description: "RCE vulnerability".into(),
            }],
        };
        assert!(dep.is_critical_callout());
    }

    #[test]
    fn ecosystem_osv_names() {
        assert_eq!(Ecosystem::Cargo.osv_name(), "crates.io");
        assert_eq!(Ecosystem::Npm.osv_name(), "npm");
        assert_eq!(Ecosystem::Pip.osv_name(), "PyPI");
        assert_eq!(Ecosystem::Nuget.osv_name(), "NuGet");
    }
}
```

**Step 2: Run tests**

```bash
cargo test --lib deps::tests 2>&1 | tail -5
```
Expected: all 4 tests pass.

**Step 3: Export from lib.rs**

```rust
pub mod deps;
```

**Step 4: Commit**

```bash
rtk git add src/deps.rs src/lib.rs
rtk git commit -m "feat(deps): add core data types — Ecosystem, DepTier, DepAge, EcosystemReport"
```

---

### Task 3: Lock file parsers

**Files:**
- Create: `src/collector/deps.rs`
- Modify: `src/collector/mod.rs`

**Step 1: Create `src/collector/deps.rs`**

```rust
use std::path::Path;
use crate::deps::Ecosystem;

#[derive(Debug, Clone, PartialEq)]
pub struct LockedDep {
    pub name: String,
    pub version: String,
    pub ecosystem: Ecosystem,
}

pub fn collect_locked_deps(repo_root: &Path) -> Vec<LockedDep> {
    let mut deps = Vec::new();
    deps.extend(parse_cargo_lock(repo_root));
    deps.extend(parse_npm_lock(repo_root));
    deps.extend(parse_pip_lock(repo_root));
    deps.extend(parse_nuget_lock(repo_root));
    deps
}

pub fn parse_cargo_lock(repo_root: &Path) -> Vec<LockedDep> {
    let path = repo_root.join("Cargo.lock");
    let Ok(content) = std::fs::read_to_string(&path) else { return vec![] };
    let Ok(table) = content.parse::<toml::Value>() else { return vec![] };
    let Some(packages) = table.get("package").and_then(|v| v.as_array()) else { return vec![] };

    packages.iter().filter_map(|pkg| {
        let name = pkg.get("name")?.as_str()?.to_string();
        let version = pkg.get("version")?.as_str()?.to_string();
        Some(LockedDep { name, version, ecosystem: Ecosystem::Cargo })
    }).collect()
}

pub fn parse_npm_lock(repo_root: &Path) -> Vec<LockedDep> {
    let path = repo_root.join("package-lock.json");
    let Ok(content) = std::fs::read_to_string(&path) else { return vec![] };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else { return vec![] };

    let Some(packages) = json.get("packages").and_then(|v| v.as_object()) else { return vec![] };

    packages.iter().filter_map(|(key, val)| {
        let name = key.strip_prefix("node_modules/")?.to_string();
        if name.is_empty() { return None; }
        let version = val.get("version")?.as_str()?.to_string();
        Some(LockedDep { name, version, ecosystem: Ecosystem::Npm })
    }).collect()
}

pub fn parse_pip_lock(repo_root: &Path) -> Vec<LockedDep> {
    let path = repo_root.join("requirements.txt");
    let Ok(content) = std::fs::read_to_string(&path) else { return vec![] };

    content.lines()
        .filter(|l| !l.trim_start().starts_with('#') && l.contains("=="))
        .filter_map(|l| {
            let mut parts = l.splitn(2, "==");
            let name = parts.next()?.trim().to_string();
            let version = parts.next()?.trim()
                .split(|c: char| !c.is_alphanumeric() && c != '.')
                .next()?.to_string();
            if name.is_empty() || version.is_empty() { return None; }
            Some(LockedDep { name, version, ecosystem: Ecosystem::Pip })
        }).collect()
}

pub fn parse_nuget_lock(repo_root: &Path) -> Vec<LockedDep> {
    let path = repo_root.join("packages.lock.json");
    let Ok(content) = std::fs::read_to_string(&path) else { return vec![] };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else { return vec![] };

    let Some(deps_by_target) = json.get("dependencies").and_then(|v| v.as_object()) else {
        return vec![];
    };

    let mut result = Vec::new();
    for (_target, packages) in deps_by_target {
        if let Some(pkgs) = packages.as_object() {
            for (name, info) in pkgs {
                if let Some(version) = info.get("resolved").and_then(|v| v.as_str()) {
                    result.push(LockedDep {
                        name: name.clone(),
                        version: version.to_string(),
                        ecosystem: Ecosystem::Nuget,
                    });
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parse_cargo_lock_basic() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.lock"), r#"
[[package]]
name = "serde"
version = "1.0.130"

[[package]]
name = "tokio"
version = "1.20.0"
"#).unwrap();
        let deps = parse_cargo_lock(dir.path());
        assert_eq!(deps.len(), 2);
        assert!(deps.iter().any(|d| d.name == "serde" && d.version == "1.0.130"));
        assert!(deps.iter().any(|d| d.name == "tokio" && d.version == "1.20.0"));
        assert!(deps.iter().all(|d| d.ecosystem == Ecosystem::Cargo));
    }

    #[test]
    fn parse_npm_lock_basic() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("package-lock.json"), r#"
{
  "lockfileVersion": 2,
  "packages": {
    "node_modules/lodash": { "version": "4.17.21" },
    "node_modules/react": { "version": "18.2.0" }
  }
}
"#).unwrap();
        let deps = parse_npm_lock(dir.path());
        assert_eq!(deps.len(), 2);
        assert!(deps.iter().any(|d| d.name == "lodash" && d.version == "4.17.21"));
        assert!(deps.iter().all(|d| d.ecosystem == Ecosystem::Npm));
    }

    #[test]
    fn parse_pip_lock_requirements_txt() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("requirements.txt"),
            "requests==2.28.0\nflask==2.3.0\n# comment\n\npytest>=7.0\n"
        ).unwrap();
        let deps = parse_pip_lock(dir.path());
        assert!(deps.iter().any(|d| d.name == "requests" && d.version == "2.28.0"));
        assert!(deps.iter().any(|d| d.name == "flask" && d.version == "2.3.0"));
        // pytest>=7.0 has no pinned version, must NOT appear
        assert!(!deps.iter().any(|d| d.name == "pytest"));
        assert!(deps.iter().all(|d| d.ecosystem == Ecosystem::Pip));
    }

    #[test]
    fn parse_nuget_lock_basic() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("packages.lock.json"), r#"
{
  "dependencies": {
    "net8.0": {
      "Newtonsoft.Json": { "type": "Direct", "resolved": "13.0.3" }
    }
  }
}
"#).unwrap();
        let deps = parse_nuget_lock(dir.path());
        assert!(deps.iter().any(|d| d.name == "Newtonsoft.Json" && d.version == "13.0.3"));
        assert!(deps.iter().all(|d| d.ecosystem == Ecosystem::Nuget));
    }

    #[test]
    fn no_lock_files_returns_empty() {
        let dir = tempdir().unwrap();
        assert!(collect_locked_deps(dir.path()).is_empty());
    }
}
```

**Step 2: Add to `src/collector/mod.rs`**

```rust
pub mod deps;
```

**Step 3: Run tests**

```bash
cargo test --lib collector::deps::tests 2>&1 | tail -5
```
Expected: all 5 pass.

**Step 4: Commit**

```bash
rtk git add src/collector/deps.rs src/collector/mod.rs
rtk git commit -m "feat(deps): lock file parsers for Cargo, npm, pip, NuGet"
```

---

### Task 4: Registry cache (TTL)

**Files:**
- Create: `src/registry/cache.rs`
- Create: `src/registry/mod.rs`
- Modify: `src/lib.rs`

**Step 1: Create `src/registry/cache.rs`**

```rust
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

const CACHE_FILE: &str = ".repository-analysis/deps-cache.json";
const TTL_DAYS: i64 = 7;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub current_published: Option<DateTime<Utc>>,
    pub latest_version: Option<String>,
    pub latest_published: Option<DateTime<Utc>>,
    pub vulnerabilities: Vec<crate::deps::Vuln>,
    pub cached_at: DateTime<Utc>,
}

impl CacheEntry {
    pub fn is_fresh(&self) -> bool {
        Utc::now() - self.cached_at < Duration::days(TTL_DAYS)
    }
}

pub type DepsCache = HashMap<String, CacheEntry>;

pub fn cache_key(ecosystem: &str, name: &str, version: &str) -> String {
    format!("{}:{}:{}", ecosystem.to_lowercase(), name.to_lowercase(), version)
}

pub fn load(repo_root: &Path) -> DepsCache {
    let path = repo_root.join(CACHE_FILE);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(repo_root: &Path, cache: &DepsCache) {
    let path = repo_root.join(CACHE_FILE);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(&path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn cache_key_is_lowercase() {
        assert_eq!(cache_key("Cargo", "Serde", "1.0.0"), "cargo:serde:1.0.0");
        assert_eq!(cache_key("NuGet", "Newtonsoft.Json", "13.0.3"), "nuget:newtonsoft.json:13.0.3");
    }

    #[test]
    fn fresh_entry_within_ttl() {
        let entry = CacheEntry {
            current_published: None, latest_version: None, latest_published: None,
            vulnerabilities: vec![], cached_at: Utc::now(),
        };
        assert!(entry.is_fresh());
    }

    #[test]
    fn stale_entry_beyond_ttl() {
        let entry = CacheEntry {
            current_published: None, latest_version: None, latest_published: None,
            vulnerabilities: vec![], cached_at: Utc::now() - Duration::days(8),
        };
        assert!(!entry.is_fresh());
    }

    #[test]
    fn roundtrip_save_load() {
        let dir = tempdir().unwrap();
        let mut cache: DepsCache = HashMap::new();
        cache.insert("cargo:serde:1.0.130".into(), CacheEntry {
            current_published: Some(Utc::now()), latest_version: Some("1.0.197".into()),
            latest_published: Some(Utc::now()), vulnerabilities: vec![], cached_at: Utc::now(),
        });
        save(dir.path(), &cache);
        let loaded = load(dir.path());
        assert!(loaded.contains_key("cargo:serde:1.0.130"));
    }
}
```

**Step 2: Create `src/registry/mod.rs`**

```rust
pub mod cache;
pub mod cargo;
pub mod npm;
pub mod nuget;
pub mod osv;
pub mod pip;
```

**Step 3: Add to `src/lib.rs`**

```rust
pub mod registry;
```

**Step 4: Run tests**

```bash
cargo test --lib registry::cache::tests 2>&1 | tail -5
```
Expected: all 4 pass.

**Step 5: Commit**

```bash
rtk git add src/registry/ src/lib.rs
rtk git commit -m "feat(deps): registry cache with 7-day TTL"
```

---

### Task 5: Registry fetchers

**Files:**
- Create: `src/registry/cargo.rs`, `npm.rs`, `pip.rs`, `nuget.rs`, `osv.rs`

Network tests are `#[ignore]` — they don't run in CI but can be run manually.

**`src/registry/cargo.rs`:**

```rust
use chrono::{DateTime, Utc};
use anyhow::Result;

pub fn fetch_dates(name: &str, version: &str) -> Result<(Option<DateTime<Utc>>, String, DateTime<Utc>)> {
    let url = format!("https://crates.io/api/v1/crates/{}/versions", name);
    let body: serde_json::Value = reqwest::blocking::Client::new()
        .get(&url)
        .header("User-Agent", "barad-dur (https://lab.frogg.it/Edouard_Mangel/barad-dur)")
        .send()?.json()?;

    let versions = body["versions"].as_array()
        .ok_or_else(|| anyhow::anyhow!("no versions array"))?;

    let current_published = versions.iter()
        .find(|v| v["num"].as_str() == Some(version))
        .and_then(|v| v["created_at"].as_str())
        .and_then(|s| s.parse::<DateTime<Utc>>().ok());

    let latest = versions.iter()
        .find(|v| !v["yanked"].as_bool().unwrap_or(false))
        .ok_or_else(|| anyhow::anyhow!("no non-yanked version"))?;

    let latest_version = latest["num"].as_str().unwrap_or("").to_string();
    let latest_published = latest["created_at"].as_str()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("no created_at"))?;

    Ok((current_published, latest_version, latest_published))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "network"]
    fn fetch_serde_dates() {
        let (current, latest_ver, _) = fetch_dates("serde", "1.0.130").unwrap();
        assert!(current.is_some());
        assert!(!latest_ver.is_empty());
    }
}
```

**`src/registry/npm.rs`:**

```rust
use chrono::{DateTime, Utc};
use anyhow::Result;

pub fn fetch_dates(name: &str, version: &str) -> Result<(Option<DateTime<Utc>>, String, DateTime<Utc>)> {
    let url = format!("https://registry.npmjs.org/{}", name);
    let body: serde_json::Value = reqwest::blocking::get(&url)?.json()?;
    let time = body["time"].as_object().ok_or_else(|| anyhow::anyhow!("no time object"))?;

    let current_published = time.get(version)
        .and_then(|v| v.as_str()).and_then(|s| s.parse().ok());

    let latest_version = body["dist-tags"]["latest"].as_str().unwrap_or("").to_string();
    let latest_published = time.get(&latest_version)
        .and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("no latest date"))?;

    Ok((current_published, latest_version, latest_published))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "network"]
    fn fetch_lodash_dates() {
        let (current, latest_ver, _) = fetch_dates("lodash", "4.17.20").unwrap();
        assert!(current.is_some());
        assert!(!latest_ver.is_empty());
    }
}
```

**`src/registry/pip.rs`:**

```rust
use chrono::{DateTime, Utc};
use anyhow::Result;

pub fn fetch_dates(name: &str, version: &str) -> Result<(Option<DateTime<Utc>>, String, DateTime<Utc>)> {
    let url = format!("https://pypi.org/pypi/{}/json", name);
    let body: serde_json::Value = reqwest::blocking::get(&url)?.json()?;
    let releases = body["releases"].as_object().ok_or_else(|| anyhow::anyhow!("no releases"))?;

    let current_published = releases.get(version)
        .and_then(|v| v.as_array()).and_then(|arr| arr.first())
        .and_then(|f| f["upload_time_iso_8601"].as_str()).and_then(|s| s.parse().ok());

    let latest_version = body["info"]["version"].as_str().unwrap_or("").to_string();
    let latest_published = releases.get(&latest_version)
        .and_then(|v| v.as_array()).and_then(|arr| arr.first())
        .and_then(|f| f["upload_time_iso_8601"].as_str()).and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("no latest date"))?;

    Ok((current_published, latest_version, latest_published))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "network"]
    fn fetch_requests_dates() {
        let (current, latest_ver, _) = fetch_dates("requests", "2.28.0").unwrap();
        assert!(current.is_some());
        assert!(!latest_ver.is_empty());
    }
}
```

**`src/registry/nuget.rs`:**

```rust
use chrono::{DateTime, Utc};
use anyhow::Result;

pub fn fetch_dates(name: &str, _version: &str) -> Result<(Option<DateTime<Utc>>, String, DateTime<Utc>)> {
    let url = format!("https://api.nuget.org/v3/registration5/{}/index.json", name.to_lowercase());
    let body: serde_json::Value = reqwest::blocking::get(&url)?.json()?;
    let items = body["items"].as_array().ok_or_else(|| anyhow::anyhow!("no items"))?;

    let mut current_published: Option<DateTime<Utc>> = None;
    let mut latest_version = String::new();
    let mut latest_published: Option<DateTime<Utc>> = None;

    for page in items {
        if let Some(page_items) = page["items"].as_array() {
            for entry in page_items {
                let catalog = &entry["catalogEntry"];
                let v = catalog["version"].as_str().unwrap_or("");
                let published = catalog["published"].as_str().and_then(|s| s.parse().ok());
                if let Some(pub_date) = published {
                    if latest_published.map_or(true, |lp| pub_date > lp) {
                        latest_version = v.to_string();
                        latest_published = Some(pub_date);
                    }
                }
            }
        }
    }

    Ok((current_published, latest_version, latest_published.ok_or_else(|| anyhow::anyhow!("no date"))?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "network"]
    fn fetch_newtonsoft_dates() {
        let (_, latest_ver, _) = fetch_dates("Newtonsoft.Json", "13.0.1").unwrap();
        assert!(!latest_ver.is_empty());
    }
}
```

**`src/registry/osv.rs`:**

```rust
use anyhow::Result;
use crate::deps::Vuln;

pub fn fetch_vulns(ecosystem_osv_name: &str, name: &str, version: &str) -> Result<Vec<Vuln>> {
    let url = "https://api.osv.dev/v1/query";
    let payload = serde_json::json!({
        "package": { "name": name, "ecosystem": ecosystem_osv_name },
        "version": version
    });

    let response: serde_json::Value = reqwest::blocking::Client::new()
        .post(url).json(&payload).send()?.json()?;

    let vulns = response["vulns"].as_array().cloned().unwrap_or_default();

    Ok(vulns.iter().map(|v| {
        let id = v["id"].as_str().unwrap_or("UNKNOWN").to_string();
        let severity = v["severity"].as_array()
            .and_then(|arr| arr.first())
            .and_then(|s| s["score"].as_str())
            .unwrap_or("UNKNOWN").to_string();
        let description = v["summary"].as_str().unwrap_or("").to_string();
        Vuln { id, severity, description }
    }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "network"]
    fn fetch_known_vulnerable_package() {
        let vulns = fetch_vulns("npm", "lodash", "4.17.15").unwrap();
        assert!(!vulns.is_empty(), "lodash 4.17.15 should have known vulns");
    }

    #[test]
    #[ignore = "network"]
    fn fetch_clean_package_returns_empty() {
        let vulns = fetch_vulns("crates.io", "serde", "1.0.197").unwrap();
        assert!(vulns.is_empty());
    }
}
```

**Step: Verify compiles**

```bash
cargo check 2>&1 | grep "^error" | head -10
```

**Step: Commit**

```bash
rtk git add src/registry/
rtk git commit -m "feat(deps): registry fetchers for crates.io, npm, PyPI, NuGet, OSV"
```

---

### Task 6: Registry dispatcher

**Files:**
- Modify: `src/registry/mod.rs`

Add `fetch_dep` to `src/registry/mod.rs`:

```rust
use std::path::Path;
use chrono::Utc;
use crate::collector::deps::LockedDep;
use crate::deps::{DepAge, DepTier, Ecosystem};
use cache::{CacheEntry, DepsCache};

pub fn fetch_dep(dep: &LockedDep, cache: &mut DepsCache, repo_root: &Path) -> Option<DepAge> {
    let key = cache::cache_key(dep.ecosystem.display_name(), &dep.name, &dep.version);

    if let Some(entry) = cache.get(&key) {
        if entry.is_fresh() {
            return entry_to_dep_age(dep, entry);
        }
    }

    let result = match dep.ecosystem {
        Ecosystem::Cargo => cargo::fetch_dates(&dep.name, &dep.version).ok(),
        Ecosystem::Npm   => npm::fetch_dates(&dep.name, &dep.version).ok(),
        Ecosystem::Pip   => pip::fetch_dates(&dep.name, &dep.version).ok(),
        Ecosystem::Nuget => nuget::fetch_dates(&dep.name, &dep.version).ok(),
    };

    let (current_published, latest_version, latest_published) = result?;

    let vulns = osv::fetch_vulns(dep.ecosystem.osv_name(), &dep.name, &dep.version)
        .unwrap_or_default();

    let entry = CacheEntry {
        current_published, latest_version: Some(latest_version),
        latest_published: Some(latest_published), vulnerabilities: vulns, cached_at: Utc::now(),
    };

    cache.insert(key, entry.clone());
    cache::save(repo_root, cache);
    entry_to_dep_age(dep, &entry)
}

fn entry_to_dep_age(dep: &LockedDep, entry: &CacheEntry) -> Option<DepAge> {
    let current = entry.current_published?;
    let latest = entry.latest_published?;
    let drift_seconds = (latest - current).num_seconds().max(0);
    let drift_years = drift_seconds as f64 / (365.25 * 24.0 * 3600.0);
    Some(DepAge {
        name: dep.name.clone(),
        ecosystem: dep.ecosystem.clone(),
        current_version: dep.version.clone(),
        drift_years,
        tier: DepTier::from_drift(drift_years),
        vulnerabilities: entry.vulnerabilities.clone(),
    })
}
```

**Step: Compile check + commit**

```bash
cargo check 2>&1 | grep "^error" | head -10
rtk git add src/registry/mod.rs
rtk git commit -m "feat(deps): registry dispatcher with cache integration"
```

---

### Task 7: Metric scoring function

**Files:**
- Create: `src/metrics/deps.rs`
- Modify: `src/metrics/mod.rs`

**Step 1: Create `src/metrics/deps.rs`**

```rust
use crate::deps::EcosystemReport;
use crate::metrics::{CategoryResult, MetricValue, RawValue};

pub fn compute_deps(ecosystem_reports: &[EcosystemReport]) -> CategoryResult {
    let metrics: Vec<MetricValue> = ecosystem_reports.iter().map(score_ecosystem).collect();
    CategoryResult { name: "Dependencies".to_string(), score: 0, metrics }.compute_score()
}

fn score_ecosystem(report: &EcosystemReport) -> MetricValue {
    if report.total_deps == 0 {
        return MetricValue {
            name: format!("{} dependencies", report.ecosystem.display_name()),
            description: "No dependencies found".to_string(),
            raw_value: RawValue::Count(0), score: 100,
        };
    }

    let base_score = drift_to_score(report.mean_drift_years);
    let cve_penalty: u32 = report.critical_deps.iter()
        .flat_map(|d| &d.vulnerabilities)
        .filter(|v| v.severity == "HIGH" || v.severity == "CRITICAL")
        .count() as u32 * 5;

    let score = base_score.saturating_sub(cve_penalty);
    let critical_count = report.critical_deps.len();
    let description = if critical_count > 0 {
        format!("{:.1} libyears avg, {} critical callout(s)", report.mean_drift_years, critical_count)
    } else {
        format!("{:.1} libyears avg, all deps healthy", report.mean_drift_years)
    };

    MetricValue {
        name: format!("{} dependencies", report.ecosystem.display_name()),
        description,
        raw_value: RawValue::Float(report.mean_drift_years),
        score,
    }
}

fn drift_to_score(mean_drift: f64) -> u32 {
    match mean_drift {
        d if d < 0.5  => 100,
        d if d < 1.0  => 90,
        d if d < 2.0  => 75,
        d if d < 5.0  => 55,
        d if d < 10.0 => 25,
        _             => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deps::{DepAge, DepTier, Ecosystem, EcosystemReport, Vuln};

    fn make_report(ecosystem: Ecosystem, mean_drift: f64, critical_deps: Vec<DepAge>) -> EcosystemReport {
        EcosystemReport { ecosystem, total_deps: 10, mean_drift_years: mean_drift,
            total_drift_years: mean_drift * 10.0, critical_deps }
    }

    #[test]
    fn fresh_deps_score_100() {
        let metric = score_ecosystem(&make_report(Ecosystem::Cargo, 0.2, vec![]));
        assert_eq!(metric.score, 100);
    }

    #[test]
    fn one_year_drift_scores_75() {
        let metric = score_ecosystem(&make_report(Ecosystem::Npm, 1.5, vec![]));
        assert_eq!(metric.score, 75);
    }

    #[test]
    fn critical_drift_scores_25() {
        let metric = score_ecosystem(&make_report(Ecosystem::Pip, 7.0, vec![]));
        assert_eq!(metric.score, 25);
    }

    #[test]
    fn cve_penalty_applied() {
        let dep = DepAge {
            name: "vuln-pkg".into(), ecosystem: Ecosystem::Npm,
            current_version: "1.0.0".into(), drift_years: 0.3, tier: DepTier::Fresh,
            vulnerabilities: vec![
                Vuln { id: "CVE-1".into(), severity: "HIGH".into(), description: "".into() },
                Vuln { id: "CVE-2".into(), severity: "CRITICAL".into(), description: "".into() },
            ],
        };
        let metric = score_ecosystem(&make_report(Ecosystem::Npm, 0.2, vec![dep]));
        assert_eq!(metric.score, 90); // base=100, penalty=2*5=10
    }

    #[test]
    fn compute_deps_averages_ecosystems() {
        let reports = vec![
            make_report(Ecosystem::Cargo, 0.2, vec![]),  // 100
            make_report(Ecosystem::Npm,   1.5, vec![]),  // 75
        ];
        let result = compute_deps(&reports);
        assert_eq!(result.score, 87); // (100+75)/2
    }
}
```

**Step 2: Add to `src/metrics/mod.rs`**

```rust
pub mod deps;
```

**Step 3: Run tests**

```bash
cargo test --lib metrics::deps::tests 2>&1 | tail -5
```
Expected: all 5 pass.

**Step 4: Commit**

```bash
rtk git add src/metrics/deps.rs src/metrics/mod.rs
rtk git commit -m "feat(deps): scoring function — drift-to-score + CVE penalty"
```

---

### Task 8: Wire into analysis pipeline

**Files:**
- Modify: `src/scorer/types.rs`
- Modify: `src/main.rs`

**Step 1: Add `dep_ecosystem_reports` to `AnalysisReport`**

In `src/scorer/types.rs`, add to `AnalysisReport`:

```rust
pub dep_ecosystem_reports: Vec<crate::deps::EcosystemReport>,
```

Initialize to `vec![]` in `build_report()` in `src/scorer.rs`.

**Step 2: In `src/main.rs`, after `compute_selected_metrics`, add dep collection**

```rust
let dep_reports = if args.deps {
    use barad_dur::collector::deps::collect_locked_deps;
    use barad_dur::registry::{self, cache as reg_cache};

    let locked = collect_locked_deps(&local_path);
    let mut reg_cache = reg_cache::load(&local_path);
    let dep_ages: Vec<_> = locked.iter()
        .filter_map(|dep| registry::fetch_dep(dep, &mut reg_cache, &local_path))
        .collect();
    build_ecosystem_reports(dep_ages)
} else {
    vec![]
};

if args.deps && !dep_reports.is_empty() {
    categories.push(barad_dur::metrics::deps::compute_deps(&dep_reports));
}
```

Add the helper function (outside `run_analyze`):

```rust
fn build_ecosystem_reports(dep_ages: Vec<barad_dur::deps::DepAge>) -> Vec<barad_dur::deps::EcosystemReport> {
    use std::collections::HashMap;
    let mut by_ecosystem: HashMap<String, Vec<barad_dur::deps::DepAge>> = HashMap::new();
    for dep in dep_ages {
        by_ecosystem.entry(dep.ecosystem.display_name().to_string()).or_default().push(dep);
    }
    by_ecosystem.into_values().map(|deps| {
        let total = deps.len();
        let total_drift: f64 = deps.iter().map(|d| d.drift_years).sum();
        let mean_drift = if total > 0 { total_drift / total as f64 } else { 0.0 };
        let critical_deps: Vec<_> = deps.iter().filter(|d| d.is_critical_callout()).cloned().collect();
        barad_dur::deps::EcosystemReport {
            ecosystem: deps[0].ecosystem.clone(),
            total_deps: total, mean_drift_years: mean_drift,
            total_drift_years: total_drift, critical_deps,
        }
    }).collect()
}
```

Set on report after building:
```rust
report.dep_ecosystem_reports = dep_reports;
```

**Step 3: Run all tests**

```bash
cargo test 2>&1 | tail -5
```
Expected: all pass, no regressions.

**Step 4: Commit**

```bash
rtk git add src/scorer/types.rs src/main.rs
rtk git commit -m "feat(deps): wire deps category into analysis pipeline"
```

---

### Task 9: CLI renderer

**Files:**
- Modify: `src/renderer/cli.rs`

Find where categories are rendered. After the categories loop, add a section for critical callouts:

```rust
if !report.dep_ecosystem_reports.is_empty() {
    println!();
    for eco in &report.dep_ecosystem_reports {
        println!(
            "  {} {}: {:.1} libyears avg ({} deps)",
            "▸".bright_blue(),
            eco.ecosystem.display_name(),
            eco.mean_drift_years,
            eco.total_deps
        );
        for dep in &eco.critical_deps {
            let suffix = if dep.vulnerabilities.is_empty() {
                String::new()
            } else {
                format!(" [{} CVE(s)]", dep.vulnerabilities.len())
            };
            println!(
                "    {} {} {} — {:.1}y behind{}",
                "⚠".yellow(), dep.name, dep.current_version,
                dep.drift_years, suffix
            );
        }
    }
}
```

**Step: Commit**

```bash
rtk git add src/renderer/cli.rs
rtk git commit -m "feat(deps): CLI renderer — ecosystem summary + critical callouts"
```

---

### Task 10: HTML renderer — Dependencies tab

**Files:**
- Modify: `src/renderer/html.rs`

The `dep_ecosystem_reports` field is already in `AnalysisReport` and serializes automatically into `window.R`. The HTML tab reads from it using safe DOM construction (no dynamic string concatenation as HTML).

**Step 1: Add tab button** — find where `Overview`, `Hotspots` tab buttons are defined, add:

```html
<button class="tab-btn" data-tab="deps">Dependencies</button>
```

**Step 2: Add tab panel** — after last panel:

```html
<div id="tab-deps" class="tab-panel" style="display:none">
  <div id="deps-content"></div>
</div>
```

**Step 3: Add JS render function using safe DOM methods** — no string interpolation into HTML; use `createElement` + `textContent`:

```javascript
function renderDeps() {
    var container = document.getElementById('deps-content');
    var reports = (window.R.dep_ecosystem_reports || []);
    if (!reports.length) {
        var msg = document.createElement('p');
        msg.className = 'muted';
        msg.textContent = 'No lock files found. Run with --deps to enable dependency analysis.';
        container.appendChild(msg);
        return;
    }
    reports.forEach(function(eco) {
        var card = document.createElement('div');
        card.className = 'card';

        var title = document.createElement('h3');
        title.textContent = eco.ecosystem + ' \u2014 ' + eco.total_deps + ' deps';
        card.appendChild(title);

        var drift = document.createElement('p');
        drift.textContent = 'Mean drift: ' + eco.mean_drift_years.toFixed(1) + ' libyears';
        card.appendChild(drift);

        var criticals = eco.critical_deps || [];
        if (criticals.length) {
            var ul = document.createElement('ul');
            ul.className = 'critical-list';
            criticals.forEach(function(dep) {
                var li = document.createElement('li');
                var label = '\u26A0 ' + dep.name + ' ' + dep.current_version
                    + ' \u2014 ' + dep.drift_years.toFixed(1) + 'y behind';
                if (dep.vulnerabilities.length) {
                    label += ' [' + dep.vulnerabilities.length + ' CVE(s)]';
                }
                li.textContent = label;
                ul.appendChild(li);
            });
            card.appendChild(ul);
        }
        container.appendChild(card);
    });
}
```

Wire into tab switching: call `renderDeps()` when `data-tab="deps"` is activated.

**Step: Commit**

```bash
rtk git add src/renderer/html.rs
rtk git commit -m "feat(deps): HTML Dependencies tab with safe DOM construction"
```

---

### Task 11: Integration test + release

**Files:**
- Modify: `tests/integration_tests.rs`
- Modify: `CHANGELOG.md`
- Modify: `Cargo.toml` (bump to 0.12.0)

**Step 1: Add integration test**

```rust
#[test]
fn analyze_without_deps_flag_omits_deps_category() {
    let output = Command::cargo_bin("barad-dur").unwrap()
        .args(["analyze", ".", "--json"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output().unwrap();

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let categories: Vec<&str> = json["categories"].as_array().unwrap()
        .iter().filter_map(|c| c["name"].as_str()).collect();

    assert!(!categories.contains(&"Dependencies"),
        "Dependencies should not appear without --deps flag");
}
```

**Step 2: Run all tests**

```bash
cargo test 2>&1 | tail -5
```

**Step 3: Prepend to CHANGELOG.md**

```markdown
## [0.12.0] - 2026-04-14

### Added
- New **Dependencies** category (20% weight when active): libyear drift + CVE detection
  - Supports Cargo, npm, pip, NuGet lock files
  - Release dates from crates.io, npmjs.org, pypi.org, nuget.org
  - CVE detection via OSV API (api.osv.dev) — covers all four ecosystems
  - Results cached 7 days in `.repository-analysis/deps-cache.json`
  - Per-ecosystem breakdown with critical callouts (stale >5y or has CVE)
  - Activated via `--deps` flag — offline by default
- New Dependencies tab in HTML report (safe DOM, no external deps)
- Updated category weights: Health 35%, Evolution 20%, Dependencies 20%, Hygiene 15%, Team 10%
```

**Step 4: Bump version**

In `Cargo.toml`:
```toml
version = "0.12.0"
```

**Step 5: Final test run**

```bash
cargo test 2>&1 | tail -5
```

**Step 6: Commit, tag, push**

```bash
rtk git add tests/ CHANGELOG.md Cargo.toml Cargo.lock
rtk git commit -m "feat(deps): complete Dependencies category — libyear + OSV CVE detection (v0.12.0)"
rtk git tag v0.12.0
rtk git push && rtk git push origin v0.12.0
```
