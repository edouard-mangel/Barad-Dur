use anyhow::Result;
use chrono::{DateTime, Utc};

pub fn fetch_dates(
    name: &str,
    version: &str,
) -> Result<(Option<DateTime<Utc>>, String, DateTime<Utc>)> {
    let url = format!("https://pypi.org/pypi/{}/json", name);
    let body: serde_json::Value = super::client::http().get(&url).send()?.json()?;
    let releases = body["releases"]
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("no releases"))?;

    let current_published = releases
        .get(version)
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|f| f["upload_time_iso_8601"].as_str())
        .and_then(|s| s.parse().ok());

    let latest_version = body["info"]["version"].as_str().unwrap_or("").to_string();
    let latest_published = releases
        .get(&latest_version)
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|f| f["upload_time_iso_8601"].as_str())
        .and_then(|s| s.parse().ok())
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
