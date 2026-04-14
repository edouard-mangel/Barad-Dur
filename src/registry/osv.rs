use anyhow::Result;
use crate::deps::Vuln;

pub fn fetch_vulns(ecosystem_osv_name: &str, name: &str, version: &str) -> Result<Vec<Vuln>> {
    let url = "https://api.osv.dev/v1/query";
    let payload = serde_json::json!({
        "package": { "name": name, "ecosystem": ecosystem_osv_name },
        "version": version
    });

    let response: serde_json::Value = reqwest::blocking::Client::new()
        .post(url)
        .json(&payload)
        .send()?
        .json()?;

    let vulns = response["vulns"].as_array().cloned().unwrap_or_default();

    Ok(vulns
        .iter()
        .map(|v| {
            let id = v["id"].as_str().unwrap_or("UNKNOWN").to_string();
            let severity = v["severity"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|s| s["score"].as_str())
                .unwrap_or("UNKNOWN")
                .to_string();
            let description = v["summary"].as_str().unwrap_or("").to_string();
            Vuln {
                id,
                severity,
                description,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "network"]
    fn fetch_known_vulnerable_package() {
        let vulns = fetch_vulns("npm", "lodash", "4.17.15").unwrap();
        assert!(!vulns.is_empty(), "lodash 4.17.15 should have known vulns");
    }

    #[test]
    #[ignore = "network"]
    fn fetch_clean_package_returns_empty() {
        let vulns = fetch_vulns("crates.io", "serde", "1.0.197").unwrap();
        assert!(vulns.is_empty());
    }
}
