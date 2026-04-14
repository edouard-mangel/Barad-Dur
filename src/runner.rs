//! Shared infrastructure used by cmd handlers: snapshot resolution, caching,
//! browser opening, and time-window helpers.

use anyhow::{bail, Result};
use std::path::Path;

use crate::cache;
use crate::cli::AnalyzeArgs;
use crate::collector::Collector;
use crate::config::RepoConfig;
use crate::snapshot::{RepoSnapshot, TimeWindow};

pub struct CollectOptions<'a> {
    pub show_progress: bool,
    pub verbose: bool,
    pub skip_blame: bool,
    pub no_cache: bool,
    pub cache_only: bool,
    pub exclude_patterns: &'a [String],
    pub use_default_excludes: bool,
}

pub fn resolve_snapshot(
    collector: &Collector,
    current_head: &str,
    opts: &CollectOptions<'_>,
) -> Result<RepoSnapshot> {
    if opts.no_cache {
        return collect_and_cache(collector, opts, true);
    }

    if let Some(cached) = cache::load(collector.repo_path())? {
        if !cache::is_stale(&cached, current_head) {
            if opts.verbose {
                eprintln!("Using cached snapshot.");
            }
            return Ok(cached);
        }
        if opts.verbose {
            eprintln!("Cache stale, re-collecting...");
        }
    } else if opts.cache_only {
        bail!("No cache found. Run without --cache-only first.");
    }

    collect_and_cache(collector, opts, false)
}

fn collect_and_cache(
    collector: &Collector,
    opts: &CollectOptions<'_>,
    no_cache: bool,
) -> Result<RepoSnapshot> {
    let snapshot = collector.collect_snapshot_verbose(
        opts.show_progress,
        opts.verbose,
        opts.skip_blame,
        no_cache,
        opts.exclude_patterns,
        opts.use_default_excludes,
    )?;
    if let Err(e) = cache::save(&snapshot, collector.repo_path()) {
        eprintln!("Warning: Failed to save cache: {}", e);
    }
    Ok(snapshot)
}

pub fn build_time_window_from_config(cfg: &RepoConfig, args: &AnalyzeArgs) -> TimeWindow {
    if args.all {
        return TimeWindow::full_history();
    }

    let now = chrono::Utc::now();

    // config.since already has the merged value (TOML + CLI override)
    let since = cfg.since.as_ref().and_then(|s| parse_time_spec(s, now));
    let until = args.until.as_ref().and_then(|s| parse_time_spec(s, now));

    if since.is_some() || until.is_some() {
        TimeWindow {
            since,
            until: until.or(Some(now)),
            default_months: 0,
        }
    } else {
        TimeWindow::default()
    }
}

pub(crate) fn parse_relative(spec: &str, suffixes: &[&str], days_per_unit: i64) -> Option<i64> {
    suffixes
        .iter()
        .find_map(|s| spec.strip_suffix(s))
        .and_then(|n| n.trim().parse::<i64>().ok())
        .map(|n| n * days_per_unit)
}

pub fn parse_time_spec(
    spec: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<chrono::DateTime<chrono::Utc>> {
    // Try relative format: "3months", "6months", "1year", "30days"
    if let Some(days) = parse_relative(spec, &["months", "month"], 30)
        .or_else(|| parse_relative(spec, &["days", "day"], 1))
        .or_else(|| parse_relative(spec, &["years", "year"], 365))
    {
        return Some(now - chrono::Duration::days(days));
    }

    // Try ISO date format: "2024-01-01"
    if let Ok(date) = chrono::NaiveDate::parse_from_str(spec, "%Y-%m-%d") {
        return Some(date.and_hms_opt(0, 0, 0).unwrap().and_utc());
    }

    eprintln!(
        "Warning: Could not parse time spec '{}', using default.",
        spec
    );
    None
}

pub fn open_in_browser(path: &Path) -> Result<()> {
    let path_str = path.to_string_lossy();

    #[cfg(target_os = "linux")]
    let result = std::process::Command::new("xdg-open")
        .arg(path_str.as_ref())
        .spawn();

    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open")
        .arg(path_str.as_ref())
        .spawn();

    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/C", "start", "", &path_str])
        .spawn();

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let result: std::io::Result<std::process::Child> = Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Cannot detect platform for browser open",
    ));

    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Warning: Could not open browser: {}", e);
            eprintln!("Report saved to: {}", path.display());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    // --- parse_relative ---

    #[test]
    fn parse_relative_plural_suffix() {
        assert_eq!(
            parse_relative("3months", &["months", "month"], 30),
            Some(90)
        );
    }

    #[test]
    fn parse_relative_singular_suffix() {
        assert_eq!(parse_relative("1month", &["months", "month"], 30), Some(30));
    }

    #[test]
    fn parse_relative_days() {
        assert_eq!(parse_relative("30days", &["days", "day"], 1), Some(30));
        assert_eq!(parse_relative("1day", &["days", "day"], 1), Some(1));
    }

    #[test]
    fn parse_relative_years() {
        assert_eq!(parse_relative("2years", &["years", "year"], 365), Some(730));
        assert_eq!(parse_relative("1year", &["years", "year"], 365), Some(365));
    }

    #[test]
    fn parse_relative_trims_whitespace() {
        assert_eq!(
            parse_relative("3 months", &["months", "month"], 30),
            Some(90)
        );
    }

    #[test]
    fn parse_relative_non_numeric_returns_none() {
        assert_eq!(parse_relative("fewmonths", &["months", "month"], 30), None);
    }

    #[test]
    fn parse_relative_no_matching_suffix_returns_none() {
        assert_eq!(parse_relative("3years", &["months", "month"], 30), None);
    }

    // --- parse_time_spec ---

    #[test]
    fn parse_time_spec_months() {
        let now = Utc::now();
        let result = parse_time_spec("6months", now).unwrap();
        let diff = (now - result).num_days();
        assert!(
            (179..=181).contains(&diff),
            "6months should be ~180 days, got {diff}"
        );
    }

    #[test]
    fn parse_time_spec_days() {
        let now = Utc::now();
        let result = parse_time_spec("30days", now).unwrap();
        let diff = (now - result).num_days();
        assert_eq!(diff, 30);
    }

    #[test]
    fn parse_time_spec_years() {
        let now = Utc::now();
        let result = parse_time_spec("1year", now).unwrap();
        let diff = (now - result).num_days();
        assert_eq!(diff, 365);
    }

    #[test]
    fn parse_time_spec_iso_date() {
        let now = Utc::now();
        let result = parse_time_spec("2024-01-15", now).unwrap();
        assert_eq!(result.format("%Y-%m-%d").to_string(), "2024-01-15");
    }

    #[test]
    fn parse_time_spec_invalid_returns_none() {
        let now = Utc::now();
        assert!(parse_time_spec("not-a-date", now).is_none());
    }
}
