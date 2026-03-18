use anyhow::Result;
use serde_json::{json, Value};

use crate::scorer::AnalysisReport;
use crate::trend::TrendSummary;

/// Render the analysis report as JSON.
///
/// When `trend` is `Some(summary)`, injects a top-level "trend" object into the
/// JSON output. When `None`, the output is structurally identical to pre-trend
/// runs (no "trend" key).
pub fn render(report: &AnalysisReport, pretty: bool, trend: Option<&TrendSummary>) -> Result<String> {
    let mut value: Value = serde_json::to_value(report)?;

    if let Some(summary) = trend {
        let trend_object = build_trend_object(summary);
        if let Value::Object(ref mut map) = value {
            map.insert("trend".to_string(), trend_object);
        }
    }

    if pretty {
        Ok(serde_json::to_string_pretty(&value)?)
    } else {
        Ok(serde_json::to_string(&value)?)
    }
}

fn build_trend_object(summary: &TrendSummary) -> Value {
    let velocity_per_week = match &summary.velocity {
        None => Value::Null,
        Some(v) => json!(v.points_per_run),
    };

    let direction = match &summary.velocity {
        None => "stable",
        Some(v) => match v.direction {
            crate::trend::VelocityDirection::Improving => "improving",
            crate::trend::VelocityDirection::Declining => "declining",
            crate::trend::VelocityDirection::Stable => "stable",
        },
    };

    let delta_vs_last = if summary.delta.is_first {
        Value::Null
    } else {
        json!(summary.delta.overall)
    };

    let delta_vs_oldest = if summary.delta.is_first {
        Value::Null
    } else {
        json!(summary.delta.overall)
    };

    let snapshots: Vec<Value> = summary
        .history
        .iter()
        .map(|entry| {
            json!({
                "timestamp": entry.timestamp.to_rfc3339(),
                "commit": entry.head,
                "branch": entry.branch,
                "overall_score": entry.overall_score,
                "category_scores": entry.categories,
            })
        })
        .collect();

    json!({
        "schema_version": 1,
        "direction": direction,
        "delta_vs_last": delta_vs_last,
        "delta_vs_oldest": delta_vs_oldest,
        "velocity_per_week": velocity_per_week,
        "snapshots": snapshots,
    })
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
            file_hotspots: vec![],
            coupling_pairs: vec![],
            author_ownership: vec![],
            file_ages: vec![],
            history: vec![],
        }
    }

    #[test]
    fn json_output_is_valid() {
        let report = make_report();
        let output = render(&report, false, None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.is_object());
    }

    #[test]
    fn json_contains_expected_fields() {
        let report = make_report();
        let output = render(&report, false, None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed["overall_score"].is_number());
        assert!(parsed["categories"].is_array());
        assert!(parsed["top_actions"].is_array());
        assert!(parsed["repo_name"].is_string());
    }

    #[test]
    fn pretty_mode_is_indented() {
        let report = make_report();
        let output = render(&report, true, None).unwrap();
        assert!(output.contains('\n'));
        assert!(output.contains("  ")); // indentation
    }

    #[test]
    fn compact_mode_is_single_line() {
        let report = make_report();
        let output = render(&report, false, None).unwrap();
        // Compact JSON should not have newlines (except possibly within string values)
        assert!(!output.starts_with("{\n"));
    }

    #[test]
    fn json_render_without_trend_data_has_no_trend_key() {
        let report = make_report();
        let output = render(&report, false, None).unwrap();
        assert!(
            !output.contains("\"trend\""),
            "JSON output should not contain 'trend' key when trend_data is None"
        );
    }

    #[test]
    fn json_render_with_trend_first_run_has_null_velocity() {
        use crate::trend::{TrendDelta, TrendSummary};
        use std::collections::HashMap;

        let report = make_report();
        let summary = TrendSummary {
            delta: TrendDelta {
                overall: 0,
                categories: HashMap::new(),
                is_first: true,
            },
            sparkline: vec![],
            velocity: None,
            branch_mismatch_warning: false,
            history: vec![],
        };

        let output = render(&report, false, Some(&summary)).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let trend = parsed.get("trend").expect("trend key must be present when Some(summary) is passed");
        assert!(
            trend.as_object().unwrap().contains_key("velocity_per_week"),
            "trend.velocity_per_week key must be present (as null)"
        );
        assert!(
            trend["velocity_per_week"].is_null(),
            "trend.velocity_per_week should be JSON null when velocity is None, got: {}",
            trend["velocity_per_week"]
        );
        assert_eq!(
            trend["schema_version"].as_i64().unwrap(),
            1,
            "trend.schema_version should be 1"
        );
    }
}
