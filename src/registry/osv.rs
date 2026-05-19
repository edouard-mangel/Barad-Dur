use crate::deps::Vuln;
use anyhow::Result;

pub fn fetch_vulns(ecosystem_osv_name: &str, name: &str, version: &str) -> Result<Vec<Vuln>> {
    let url = "https://api.osv.dev/v1/query";
    let payload = serde_json::json!({
        "package": { "name": name, "ecosystem": ecosystem_osv_name },
        "version": version
    });

    let response: serde_json::Value = super::client::http()
        .post(url)
        .json(&payload)
        .send()?
        .json()?;

    let vulns = response["vulns"].as_array().cloned().unwrap_or_default();

    Ok(vulns
        .iter()
        .map(|vuln| {
            let id = vuln["id"].as_str().unwrap_or("UNKNOWN").to_string();
            let severity = vuln
                .get("database_specific")
                .and_then(|d| d.get("severity"))
                .and_then(|s| s.as_str())
                .map(|s| s.to_uppercase())
                .or_else(|| {
                    vuln.get("severity")
                        .and_then(|s| s.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|e| e.get("score"))
                        .and_then(|s| s.as_str())
                        .and_then(parse_cvss_severity)
                })
                .unwrap_or_else(|| "UNKNOWN".to_string());
            let description = vuln["summary"].as_str().unwrap_or("").to_string();
            Vuln {
                id,
                severity,
                description,
            }
        })
        .collect())
}

/// Classify CVSS vector string into HIGH/CRITICAL/MEDIUM/LOW by base score.
/// Some OSV entries put a plain numeric score in the score field.
/// For full CVSS vectors without a numeric score, returns None.
fn parse_cvss_severity(cvss: &str) -> Option<String> {
    if let Ok(score) = cvss.parse::<f64>() {
        return Some(cvss_score_to_label(score));
    }
    None
}

fn cvss_score_to_label(score: f64) -> String {
    match score {
        s if s >= 9.0 => "CRITICAL".to_string(),
        s if s >= 7.0 => "HIGH".to_string(),
        s if s >= 4.0 => "MEDIUM".to_string(),
        _ => "LOW".to_string(),
    }
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
