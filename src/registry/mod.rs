pub mod cache;
pub mod cargo;
pub(crate) mod client;
pub mod npm;
pub mod nuget;
pub mod osv;
pub mod pip;

use chrono::Utc;
use rayon::prelude::*;

use crate::collector::deps::LockedDep;
use crate::deps::{DepAge, DepTier, Ecosystem};
use cache::{CacheEntry, DepsCache};

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

/// Split `locked` into (cached, uncached) based on whether a fresh cache entry exists.
pub fn partition_cached<'a>(
    locked: &'a [LockedDep],
    cache: &DepsCache,
) -> (Vec<&'a LockedDep>, Vec<&'a LockedDep>) {
    locked.iter().partition(|dep| {
        let key = cache::cache_key(dep.ecosystem.display_name(), &dep.name, &dep.version);
        cache.get(&key).is_some_and(|e| e.is_fresh())
    })
}

/// Parallel-fetch `uncached` deps via `fetch_fn`, update `cache`, then resolve
/// all of `all` from the (now updated) cache.
/// Deps where `fetch_fn` returns `None` are silently skipped.
pub fn resolve_dep_ages(
    all: &[LockedDep],
    uncached: &[&LockedDep],
    cache: &mut DepsCache,
    fetch_fn: impl Fn(&LockedDep) -> Option<CacheEntry> + Sync,
) -> Vec<DepAge> {
    if !uncached.is_empty() {
        let fetched: Vec<_> = uncached
            .par_iter()
            .filter_map(|dep| {
                let entry = fetch_fn(dep)?;
                let key = cache::cache_key(dep.ecosystem.display_name(), &dep.name, &dep.version);
                Some((key, entry))
            })
            .collect();

        for (key, entry) in fetched {
            cache.insert(key, entry);
        }
    }

    all.iter()
        .filter_map(|dep| {
            let key = cache::cache_key(dep.ecosystem.display_name(), &dep.name, &dep.version);
            cache
                .get(&key)
                .and_then(|entry| entry_to_dep_age(dep, entry))
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deps::{Ecosystem, Vuln};
    use chrono::{Duration, Utc};
    use std::collections::HashMap;

    fn make_dep() -> LockedDep {
        LockedDep {
            name: "serde".into(),
            version: "1.0.130".into(),
            ecosystem: Ecosystem::Cargo,
        }
    }

    fn make_locked(name: &str, version: &str) -> LockedDep {
        LockedDep {
            name: name.into(),
            version: version.into(),
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

    fn make_fresh_entry() -> CacheEntry {
        make_entry(Some(Utc::now() - Duration::days(365)), Some(Utc::now()))
    }

    // If the registry returns latest < current (clock skew, yanked release),
    // drift must clamp to zero — a negative drift_years would produce a
    // misleading Fresh tier on what is effectively an unknown state.
    #[test]
    fn drift_clamps_to_zero_when_latest_before_current() {
        let now = Utc::now();
        let entry = make_entry(Some(now), Some(now - Duration::days(30)));
        let dep_age = entry_to_dep_age(&make_dep(), &entry).unwrap();
        assert_eq!(dep_age.drift_years, 0.0);
        assert_eq!(dep_age.tier, DepTier::Fresh);
    }

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

    #[test]
    #[ignore = "network"]
    fn fetch_dep_network_returns_none_for_nonexistent_package() {
        let dep = LockedDep {
            name: "this-package-does-not-exist-barad-dur-test-xyz".into(),
            version: "0.0.0".into(),
            ecosystem: Ecosystem::Cargo,
        };
        assert!(fetch_dep_network(&dep).is_none());
    }

    #[test]
    fn fetch_dep_network_has_correct_signature() {
        let _ = fetch_dep_network as fn(&LockedDep) -> Option<CacheEntry>;
    }

    #[test]
    fn partition_cached_splits_correctly() {
        let fresh = make_locked("cached-dep", "1.0.0");
        let stale = make_locked("uncached-dep", "2.0.0");
        let mut cache: DepsCache = HashMap::new();
        cache.insert(
            cache::cache_key("cargo", "cached-dep", "1.0.0"),
            make_fresh_entry(),
        );

        let locked = [fresh, stale];
        let (cached, uncached) = partition_cached(&locked, &cache);

        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].name, "cached-dep");
        assert_eq!(uncached.len(), 1);
        assert_eq!(uncached[0].name, "uncached-dep");
    }

    #[test]
    fn resolve_dep_ages_returns_cached_deps_without_calling_fetch_fn() {
        let dep = make_locked("serde", "1.0.0");
        let mut cache: DepsCache = HashMap::new();
        cache.insert(
            cache::cache_key("cargo", "serde", "1.0.0"),
            make_fresh_entry(),
        );

        let locked = [dep];
        let (_, uncached) = partition_cached(&locked, &cache);
        let result = resolve_dep_ages(&locked, &uncached, &mut cache, |_| {
            panic!("fetch_fn must not be called for cached deps");
        });

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "serde");
    }

    #[test]
    fn resolve_dep_ages_fetches_uncached_dep_via_fetch_fn() {
        let dep = make_locked("new-dep", "2.0.0");
        let mut cache: DepsCache = HashMap::new();

        let locked = [dep];
        let (_, uncached) = partition_cached(&locked, &cache);
        let result = resolve_dep_ages(&locked, &uncached, &mut cache, |_| Some(make_fresh_entry()));

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "new-dep");
        assert!(cache.contains_key(&cache::cache_key("cargo", "new-dep", "2.0.0")));
    }

    #[test]
    fn resolve_dep_ages_skips_dep_when_fetch_fn_returns_none() {
        let dep = make_locked("broken-dep", "0.1.0");
        let mut cache: DepsCache = HashMap::new();

        let locked = [dep];
        let (_, uncached) = partition_cached(&locked, &cache);
        let result = resolve_dep_ages(&locked, &uncached, &mut cache, |_| None);

        assert!(result.is_empty());
    }

    #[test]
    fn resolve_dep_ages_resolves_cached_and_skips_failed_uncached() {
        let cached = make_locked("serde", "1.0.0");
        let uncached_dep = make_locked("broken", "0.0.1");
        let mut cache: DepsCache = HashMap::new();
        cache.insert(
            cache::cache_key("cargo", "serde", "1.0.0"),
            make_fresh_entry(),
        );

        let locked = [cached, uncached_dep];
        let (_, uncached) = partition_cached(&locked, &cache);
        let result = resolve_dep_ages(&locked, &uncached, &mut cache, |_| None);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "serde");
    }
}
