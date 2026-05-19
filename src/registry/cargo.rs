use anyhow::Result;
use chrono::{DateTime, Utc};

/// Returns (current_version_published_at, latest_version, latest_published_at).
pub fn fetch_dates(
    name: &str,
    version: &str,
) -> Result<(Option<DateTime<Utc>>, String, DateTime<Utc>)> {
    let url = format!("https://crates.io/api/v1/crates/{}/versions", name);
    let body: serde_json::Value = super::client::http()
        .get(&url)
        .header(
            "User-Agent",
            "barad-dur (https://lab.frogg.it/Edouard_Mangel/barad-dur)",
        )
        .send()?
        .json()?;

    let versions = body["versions"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("no versions array"))?;

    let current_published = versions
        .iter()
        .find(|v| v["num"].as_str() == Some(version))
        .and_then(|v| v["created_at"].as_str())
        .and_then(|s| s.parse::<DateTime<Utc>>().ok());

    let latest = versions
        .iter()
        .find(|v| !v["yanked"].as_bool().unwrap_or(false))
        .ok_or_else(|| anyhow::anyhow!("no non-yanked version"))?;

    let latest_version = latest["num"].as_str().unwrap_or("").to_string();
    let latest_published = latest["created_at"]
        .as_str()
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
