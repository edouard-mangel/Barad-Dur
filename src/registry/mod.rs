pub mod cache;
pub mod cargo;
pub(crate) mod client;
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

    let entry = fetch_dep_network(dep)?;
    cache.insert(key, entry.clone());
    cache::save(repo_root, cache);

    entry_to_dep_age(dep, &entry)
}

/// Fetch age + vulnerability data from the network only — no cache read or write.
/// Returns `None` on any network or parse error (including timeout).
/// Safe to call from multiple threads simultaneously.
pub fn fetch_dep_network(dep: &LockedDep) -> Option<CacheEntry> {
    let result = match dep.ecosystem {
        Ecosystem::Cargo => cargo::fetch_dates(&dep.name, &dep.version).ok(),
        Ecosystem::Npm => npm::fetch_dates(&dep.name, &dep.version).ok(),
        Ecosystem::Pip => pip::fetch_dates(&dep.name, &dep.version).ok(),
        Ecosystem::Nuget => nuget::fetch_dates(&dep.name, &dep.version).ok(),
    };

    let (current_published, latest_version, latest_published) = result?;

    let vulns =
        osv::fetch_vulns(dep.ecosystem.osv_name(), &dep.name, &dep.version).unwrap_or_default();

    Some(CacheEntry {
        current_published,
        latest_version: Some(latest_version),
        latest_published: Some(latest_published),
        vulnerabilities: vulns,
        cached_at: Utc::now(),
    })
}

pub fn entry_to_dep_age(dep: &LockedDep, entry: &CacheEntry) -> Option<DepAge> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deps::{Ecosystem, Vuln};
    use chrono::{Duration, Utc};
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn make_dep() -> LockedDep {
        LockedDep {
            name: "serde".into(),
            version: "1.0.130".into(),
            ecosystem: Ecosystem::Cargo,
        }
    }

    fn make_entry(
        current_published: Option<chrono::DateTime<Utc>>,
        latest_published: Option<chrono::DateTime<Utc>>,
    ) -> CacheEntry {
        CacheEntry {
            current_published,
            latest_published,
            latest_version: Some("1.0.197".into()),
            vulnerabilities: vec![],
            cached_at: Utc::now(),
        }
    }

    // If the registry returns latest < current (clock skew, yanked release),
    // drift must clamp to zero — a negative drift_years would produce a
    // misleading Fresh tier on what is effectively an unknown state.
    #[test]
    fn drift_clamps_to_zero_when_latest_before_current() {
        let now = Utc::now();
        let entry = make_entry(
            Some(now),
            Some(now - Duration::days(30)), // latest older than current
        );
        let dep_age = entry_to_dep_age(&make_dep(), &entry).unwrap();
        assert_eq!(dep_age.drift_years, 0.0);
        assert_eq!(dep_age.tier, DepTier::Fresh);
    }

    // Both date fields use `?` independently. Removing either guard would
    // cause a panic or wrong result; having separate tests makes the intent clear.
    #[test]
    fn none_when_current_published_missing() {
        let entry = make_entry(None, Some(Utc::now()));
        assert!(entry_to_dep_age(&make_dep(), &entry).is_none());
    }

    #[test]
    fn none_when_latest_published_missing() {
        let entry = make_entry(Some(Utc::now()), None);
        assert!(entry_to_dep_age(&make_dep(), &entry).is_none());
    }

    // Vulnerabilities are security data — they must survive the conversion
    // from CacheEntry to DepAge without being dropped or deduplicated.
    #[test]
    fn vulnerabilities_are_propagated() {
        let vuln = Vuln {
            id: "GHSA-0000-0000-0000".into(),
            description: "test vuln".into(),
            severity: "HIGH".into(),
        };
        let mut entry = make_entry(Some(Utc::now() - Duration::days(365)), Some(Utc::now()));
        entry.vulnerabilities = vec![vuln.clone()];
        let dep_age = entry_to_dep_age(&make_dep(), &entry).unwrap();
        assert_eq!(dep_age.vulnerabilities.len(), 1);
        assert_eq!(dep_age.vulnerabilities[0].id, vuln.id);
    }

    // A fresh cache entry must be returned immediately without attempting
    // any network call. This is the primary reason the cache exists —
    // analysis must work in offline / CI environments once the cache is warm.
    #[test]
    fn fetch_dep_returns_fresh_cached_entry_without_network() {
        let dir = tempdir().unwrap();
        let dep = make_dep();
        let mut cache: DepsCache = HashMap::new();
        let key = cache::cache_key(dep.ecosystem.display_name(), &dep.name, &dep.version);
        cache.insert(
            key,
            make_entry(Some(Utc::now() - Duration::days(365)), Some(Utc::now())),
        );
        // If this reaches the network, it would either succeed (flaky) or fail
        // with an error unrelated to our logic. The cache hit must prevent that.
        let result = fetch_dep(&dep, &mut cache, dir.path());
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "serde");
    }

    // fetch_dep_network is the network-only extraction used by parallel fetch.
    // A dep that cannot possibly exist on any registry must return None (not panic).
    // This tests the function exists and handles errors gracefully without hanging
    // (the 15s timeout in client::http() ensures it terminates).
    #[test]
    #[ignore = "network"]
    fn fetch_dep_network_returns_none_for_nonexistent_package() {
        let dep = LockedDep {
            name: "this-package-does-not-exist-barad-dur-test-xyz".into(),
            version: "0.0.0".into(),
            ecosystem: Ecosystem::Cargo,
        };
        let result = fetch_dep_network(&dep);
        assert!(result.is_none());
    }

    // fetch_dep_network must be a callable function with the right signature.
    // This test verifies the API contract at compile time: it accepts a &LockedDep
    // and returns Option<CacheEntry>.
    #[test]
    fn fetch_dep_network_has_correct_signature() {
        // Just verifying the function is callable with the right types.
        // We pass a dep guaranteed to be in cache so no network is hit.
        // (The return value is unused — compilation is what we're testing here.)
        let _ = fetch_dep_network as fn(&LockedDep) -> Option<CacheEntry>;
    }
}
