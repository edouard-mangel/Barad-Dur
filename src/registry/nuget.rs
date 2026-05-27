use anyhow::Result;
use chrono::{DateTime, Utc};

pub fn fetch_dates(
    name: &str,
    version: &str,
) -> Result<(Option<DateTime<Utc>>, String, DateTime<Utc>)> {
    let url = format!(
        "https://api.nuget.org/v3/registration5/{}/index.json",
        name.to_lowercase()
    );
    let client = super::client::http().ok_or_else(|| anyhow::anyhow!("HTTP client unavailable"))?;
    let body: serde_json::Value = client.get(&url).send()?.json()?;
    let items = body["items"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("no items"))?;

    let mut latest_version = String::new();
    let mut latest_published: Option<DateTime<Utc>> = None;
    let mut current_published: Option<DateTime<Utc>> = None;

    for page in items {
        if let Some(page_items) = page["items"].as_array() {
            for entry in page_items {
                let catalog = &entry["catalogEntry"];
                let v = catalog["version"].as_str().unwrap_or("");
                let published: Option<DateTime<Utc>> =
                    catalog["published"].as_str().and_then(|s| s.parse().ok());
                if let Some(pub_date) = published {
                    if latest_published.is_none_or(|lp| pub_date > lp) {
                        latest_version = v.to_string();
                        latest_published = Some(pub_date);
                    }
                    if v == version {
                        current_published = published;
                    }
                }
            }
        }
    }

    Ok((
        current_published,
        latest_version,
        latest_published.ok_or_else(|| anyhow::anyhow!("no published date found"))?,
    ))
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
