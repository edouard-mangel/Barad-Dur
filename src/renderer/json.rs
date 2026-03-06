use anyhow::Result;

use crate::scorer::AnalysisReport;

/// Render the analysis report as JSON.
pub fn render(report: &AnalysisReport, pretty: bool) -> Result<String> {
    if pretty {
        Ok(serde_json::to_string_pretty(report)?)
    } else {
        Ok(serde_json::to_string(report)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{CategoryResult, MetricValue, RawValue};

    fn make_report() -> AnalysisReport {
        AnalysisReport {
            repo_name: "test-repo".into(),
            branch: "main".into(),
            time_window_months: 6,
            total_commits: 100,
            total_authors: 5,
            total_files: 50,
            overall_score: 72,
            categories: vec![CategoryResult {
                name: "Health".into(),
                score: 72,
                metrics: vec![MetricValue {
                    name: "Bus factor".into(),
                    description: "2 (risky)".into(),
                    raw_value: RawValue::Integer(2),
                    score: 50,
                }],
            }],
            top_actions: vec!["Fix bus factor".into()],
            remote_meta: None,
        }
    }

    #[test]
    fn json_output_is_valid() {
        let report = make_report();
        let output = render(&report, false).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.is_object());
    }

    #[test]
    fn json_contains_expected_fields() {
        let report = make_report();
        let output = render(&report, false).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed["overall_score"].is_number());
        assert!(parsed["categories"].is_array());
        assert!(parsed["top_actions"].is_array());
        assert!(parsed["repo_name"].is_string());
    }

    #[test]
    fn pretty_mode_is_indented() {
        let report = make_report();
        let output = render(&report, true).unwrap();
        assert!(output.contains('\n'));
        assert!(output.contains("  ")); // indentation
    }

    #[test]
    fn compact_mode_is_single_line() {
        let report = make_report();
        let output = render(&report, false).unwrap();
        // Compact JSON should not have newlines (except possibly within string values)
        assert!(!output.starts_with("{\n"));
    }
}
