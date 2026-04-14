pub mod cache;
pub mod cargo;
pub mod npm;
pub mod nuget;
pub mod osv;
pub mod pip;

use std::path::Path;

use chrono::Utc;

use crate::collector::deps::LockedDep;
use crate::deps::{DepAge, DepTier, Ecosystem};
use cache::{CacheEntry, DepsCache};

/// Fetch (or return cached) age + vulnerability data for a single dependency.
/// Returns `None` if the registry call fails (network unavailable, unknown package).
pub fn fetch_dep(dep: &LockedDep, cache: &mut DepsCache, repo_root: &Path) -> Option<DepAge> {
    let key = cache::cache_key(dep.ecosystem.display_name(), &dep.name, &dep.version);

    // Return cached entry if still fresh
    if let Some(entry) = cache.get(&key) {
        if entry.is_fresh() {
            return entry_to_dep_age(dep, entry);
        }
    }

    // Fetch from the appropriate registry
    let result = match dep.ecosystem {
        Ecosystem::Cargo => cargo::fetch_dates(&dep.name, &dep.version).ok(),
        Ecosystem::Npm => npm::fetch_dates(&dep.name, &dep.version).ok(),
        Ecosystem::Pip => pip::fetch_dates(&dep.name, &dep.version).ok(),
        Ecosystem::Nuget => nuget::fetch_dates(&dep.name, &dep.version).ok(),
    };

    let (current_published, latest_version, latest_published) = result?;

    let vulns =
        osv::fetch_vulns(dep.ecosystem.osv_name(), &dep.name, &dep.version).unwrap_or_default();

    let entry = CacheEntry {
        current_published,
        latest_version: Some(latest_version),
        latest_published: Some(latest_published),
        vulnerabilities: vulns,
        cached_at: Utc::now(),
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
