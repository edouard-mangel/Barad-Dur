//! Acquiring dependency evidence for `analyze --deps`: look up each locked
//! package's age and known vulnerabilities (registry responses are cached),
//! report progress on stderr, and group the result into per-ecosystem reports.
//! The grouping itself is `crate::deps::build_ecosystem_reports`.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::cli::AnalyzeArgs;
use crate::deps::EcosystemReport;

pub(super) fn load_dep_reports(
    args: &AnalyzeArgs,
    local_path: &Path,
    show_progress: bool,
) -> Vec<EcosystemReport> {
    if !args.deps {
        return vec![];
    }

    use crate::collector::deps::collect_locked_deps;
    use crate::registry;
    use crate::registry::cache as reg_cache;

    let locked = collect_locked_deps(local_path);
    if locked.is_empty() {
        return vec![];
    }

    let mut dep_cache = reg_cache::load(local_path);

    let (cached, uncached) = registry::partition_cached(&locked, &dep_cache);

    if show_progress {
        eprintln!("{}", deps_progress_start(cached.len(), uncached.len()));
    }

    let uncached_count = uncached.len();
    let counter = AtomicUsize::new(0);

    let dep_ages = registry::resolve_dep_ages(
        &locked,
        &uncached,
        &mut dep_cache,
        make_progress_fetch(
            &registry::fetch_dep_network,
            &counter,
            uncached_count,
            show_progress,
        ),
    );

    reg_cache::save(local_path, &dep_cache);

    crate::deps::build_ecosystem_reports(dep_ages)
}

fn deps_progress_start(cached: usize, uncached: usize) -> String {
    if uncached == 0 {
        format!("Deps: {} packages (all cached)", cached)
    } else {
        format!(
            "Deps: {} cached, fetching {} from registry (timeout {}s/pkg)…",
            cached,
            uncached,
            crate::registry::client::TIMEOUT_SECS,
        )
    }
}

fn deps_progress_tick(n: usize, total: usize) -> String {
    format!("  [{}/{}] fetched", n, total)
}

/// The progress line for the `n`th of `total` fetches, if one is due: every
/// tenth fetch, and always the last, so the `[N/N]` completion line appears.
fn tick_message(n: usize, total: usize) -> Option<String> {
    (n.is_multiple_of(10) || n == total).then(|| deps_progress_tick(n, total))
}

/// Wraps `fetch_fn` so the progress counter increments for every attempt —
/// including failures — ensuring the [N/N] completion tick always fires.
fn make_progress_fetch<'a, F>(
    fetch_fn: &'a F,
    counter: &'a AtomicUsize,
    uncached_count: usize,
    show_progress: bool,
) -> impl Fn(&crate::collector::deps::LockedDep) -> Option<crate::registry::cache::CacheEntry> + Sync + 'a
where
    F: Fn(&crate::collector::deps::LockedDep) -> Option<crate::registry::cache::CacheEntry> + Sync,
{
    move |dep| {
        let result = fetch_fn(dep);
        if show_progress && uncached_count > 0 {
            let n = counter.fetch_add(1, Ordering::Relaxed) + 1;
            if let Some(message) = tick_message(n, uncached_count) {
                eprintln!("{message}");
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::deps::LockedDep;
    use crate::deps::Ecosystem;
    use crate::registry::cache::CacheEntry;
    use chrono::{Duration, Utc};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn make_locked(name: &str) -> LockedDep {
        LockedDep {
            name: name.into(),
            version: "1.0.0".into(),
            ecosystem: Ecosystem::Cargo,
        }
    }

    fn make_entry() -> CacheEntry {
        CacheEntry {
            current_published: Some(Utc::now() - Duration::days(365)),
            latest_published: Some(Utc::now()),
            latest_version: Some("1.0.0".into()),
            vulnerabilities: vec![],
            cached_at: Utc::now(),
        }
    }

    #[test]
    fn progress_start_all_cached() {
        let msg = deps_progress_start(42, 0);
        assert!(msg.contains("42") && msg.contains("cached"), "{msg}");
        // "all cached" distinguishes this branch from the fetching branch,
        // catching the `== with !=` mutation that swaps them.
        assert!(
            msg.contains("all cached"),
            "expected 'all cached' in: {msg}"
        );
    }

    #[test]
    fn progress_start_some_uncached() {
        let msg = deps_progress_start(10, 30);
        assert!(msg.contains("10") && msg.contains("30"), "{msg}");
    }

    #[test]
    fn progress_start_embeds_timeout_constant() {
        let msg = deps_progress_start(0, 1);
        let expected = crate::registry::client::TIMEOUT_SECS.to_string();
        assert!(msg.contains(&expected), "{msg}");
    }

    #[test]
    fn progress_tick_shows_n_of_total() {
        let msg = deps_progress_tick(20, 100);
        assert!(msg.contains("20") && msg.contains("100"), "{msg}");
    }

    // The progress counter must increment for every dep attempt, including failures.
    // If it only increments on success, the [N/N] completion tick is never printed
    // when any dep fails to fetch (network error, timeout, unknown package).
    #[test]
    fn make_progress_fetch_increments_counter_on_failure() {
        let dep = make_locked("broken");
        let counter = AtomicUsize::new(0);
        let fetch_fn = |_: &LockedDep| -> Option<CacheEntry> { None };

        let wrapped = make_progress_fetch(&fetch_fn, &counter, 3, true);
        let result = wrapped(&dep);

        assert!(result.is_none());
        assert_eq!(
            counter.load(Ordering::Relaxed),
            1,
            "counter must increment even when fetch returns None"
        );
    }

    #[test]
    fn make_progress_fetch_increments_counter_on_success() {
        let dep = make_locked("ok");
        let counter = AtomicUsize::new(0);
        let fetch_fn = |_: &LockedDep| -> Option<CacheEntry> { Some(make_entry()) };

        let wrapped = make_progress_fetch(&fetch_fn, &counter, 3, true);
        let result = wrapped(&dep);

        assert!(result.is_some());
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn a_tick_is_due_every_tenth_fetch_and_at_the_last_one() {
        // 25 to fetch: ticks at 10, 20, and the final 25; nothing in between.
        for n in [10, 20, 25] {
            assert_eq!(
                tick_message(n, 25),
                Some(format!("  [{n}/25] fetched")),
                "n = {n}"
            );
        }
        for n in [1, 9, 11, 19, 21, 24] {
            assert_eq!(tick_message(n, 25), None, "n = {n}");
        }
    }

    #[test]
    fn the_counter_is_left_alone_when_progress_is_off() {
        let counter = AtomicUsize::new(0);
        let fetch_fn = |_: &LockedDep| -> Option<CacheEntry> { None };
        let wrapped = make_progress_fetch(&fetch_fn, &counter, 3, false);
        wrapped(&make_locked("quiet"));
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn the_counter_is_left_alone_when_nothing_was_uncached() {
        let counter = AtomicUsize::new(0);
        let fetch_fn = |_: &LockedDep| -> Option<CacheEntry> { None };
        let wrapped = make_progress_fetch(&fetch_fn, &counter, 0, true);
        wrapped(&make_locked("cached"));
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    // When all uncached deps fail, counter still reaches uncached_count.
    #[test]
    fn make_progress_fetch_counter_reaches_total_despite_all_failures() {
        let counter = AtomicUsize::new(0);
        let uncached_count = 3;
        let fetch_fn = |_: &LockedDep| -> Option<CacheEntry> { None };

        for _ in 0..uncached_count {
            let dep = make_locked("fail");
            let wrapped = make_progress_fetch(&fetch_fn, &counter, uncached_count, true);
            wrapped(&dep);
        }

        assert_eq!(counter.load(Ordering::Relaxed), uncached_count);
    }
}
