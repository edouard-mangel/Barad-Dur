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
