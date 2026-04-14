use anyhow::Result;
use chrono::{DateTime, Utc};

pub fn fetch_dates(
    name: &str,
    version: &str,
) -> Result<(Option<DateTime<Utc>>, String, DateTime<Utc>)> {
    let url = format!("https://registry.npmjs.org/{}", name);
    let body: serde_json::Value = reqwest::blocking::get(&url)?.json()?;
    let time = body["time"]
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("no time object"))?;

    let current_published = time
        .get(version)
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok());

    let latest_version = body["dist-tags"]["latest"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let latest_published = time
        .get(&latest_version)
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
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
