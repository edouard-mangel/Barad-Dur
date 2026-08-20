use anyhow::Result;
use serde_json::{json, Value};

use crate::scorer::AnalysisReport;
use crate::trend::TrendSummary;

/// Render the analysis report as compact (single-line) JSON; see
/// [`render_pretty`] for the indented form.
///
/// When `trend` is `Some(summary)`, injects a top-level "trend" object into the
/// JSON output. When `None`, the output is structurally identical to pre-trend
/// runs (no "trend" key).
pub fn render(report: &AnalysisReport, trend: Option<&TrendSummary>) -> Result<String> {
    Ok(serde_json::to_string(&report_value(report, trend)?)?)
}

/// [`render`], indented for human readability.
pub fn render_pretty(report: &AnalysisReport, trend: Option<&TrendSummary>) -> Result<String> {
    Ok(serde_json::to_string_pretty(&report_value(report, trend)?)?)
}

fn report_value(report: &AnalysisReport, trend: Option<&TrendSummary>) -> Result<Value> {
    let mut value: Value = serde_json::to_value(report)?;
    if let Some(summary) = trend {
        let trend_object = build_trend_object(summary);
        if let Value::Object(ref mut map) = value {
            map.insert("trend".to_string(), trend_object);
        }
    }
    Ok(value)
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
        json!(summary.delta.delta_vs_oldest)
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
    use crate::scorer::ActionItem;

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
                    score: Some(50),
                }],
            }],
            top_actions: vec![ActionItem {
                text: "Fix bus factor".into(),
                target_tab: Some("ownership".into()),
                sort_by: None,
            }],
            coupling_actions: vec![],
            remote_meta: None,
            file_hotspots: vec![],
            coupling_pairs: vec![],
            author_ownership: vec![],
            file_ages: vec![],
            author_cards: vec![],
            history: vec![],
            dep_ecosystem_reports: vec![],
            audit: None,
            per_file_coupling: vec![],
            import_edges: vec![],
            import_cycles: vec![],
            coupling_finding_counts: None,
            call_graph: None,
            churn_timeline: None,
            score_thresholds: Default::default(),
        }
    }

    #[test]
    fn json_unscored_metric_serializes_as_null() {
        let mut report = make_report();
        report.categories[0].metrics.push(MetricValue {
            name: "Knowledge distribution".into(),
            description: "Solo project — not applicable".into(),
            raw_value: RawValue::Text("N/A".into()),
            score: None,
        });
        let output = render(&report, None).unwrap();
        assert!(
            output.contains(r#""score":null"#),
            "insufficient-data metrics must serialize score as null, not a number"
        );
    }

    #[test]
    fn json_output_is_valid() {
        let report = make_report();
        let output = render(&report, None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.is_object());
    }

    #[test]
    fn json_contains_expected_fields() {
        let report = make_report();
        let output = render(&report, None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed["overall_score"].is_number());
        assert!(parsed["categories"].is_array());
        assert!(parsed["top_actions"].is_array());
        assert!(parsed["repo_name"].is_string());
    }

    #[test]
    fn pretty_mode_is_indented() {
        let report = make_report();
        let output = render_pretty(&report, None).unwrap();
        assert!(output.contains('\n'));
        assert!(output.contains("  ")); // indentation
    }

    #[test]
    fn compact_mode_is_single_line() {
        let report = make_report();
        let output = render(&report, None).unwrap();
        // Compact JSON should not have newlines (except possibly within string values)
        assert!(!output.starts_with("{\n"));
    }

    #[test]
    fn json_render_without_trend_data_has_no_trend_key() {
        let report = make_report();
        let output = render(&report, None).unwrap();
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
                delta_vs_oldest: 0,
                categories: HashMap::new(),
                is_first: true,
            },
            sparkline: vec![],
            velocity: None,
            branch_mismatch_warning: false,
            history: vec![],
        };

        let output = render(&report, Some(&summary)).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let trend = parsed
            .get("trend")
            .expect("trend key must be present when Some(summary) is passed");
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

    #[test]
    fn json_output_contains_per_file_coupling_array() {
        use crate::scorer::FileCouplingMetrics;

        let mut report = make_report();
        report.per_file_coupling = vec![FileCouplingMetrics {
            path: "src/lib.rs".into(),
            ca: 3,
            ce: 5,
            instability: 0.625,
        }];

        let output = render(&report, None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let arr = parsed["per_file_coupling"]
            .as_array()
            .expect("per_file_coupling must be a JSON array");
        assert_eq!(arr.len(), 1, "array must contain exactly one entry");

        let entry = &arr[0];
        assert_eq!(
            entry["path"].as_str().unwrap(),
            "src/lib.rs",
            "entry.path must match"
        );
        assert_eq!(entry["ca"].as_u64().unwrap(), 3, "entry.ca must match");
        assert_eq!(entry["ce"].as_u64().unwrap(), 5, "entry.ce must match");
        assert!(
            (entry["instability"].as_f64().unwrap() - 0.625).abs() < 1e-9,
            "entry.instability must match"
        );
    }

    #[test]
    fn json_output_contains_import_edges_array() {
        use crate::scorer::ImportEdge;

        let mut report = make_report();
        report.import_edges = vec![ImportEdge {
            from: "src/main.rs".into(),
            to: "src/lib.rs".into(),
        }];

        let output = render(&report, None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let arr = parsed["import_edges"]
            .as_array()
            .expect("import_edges must be a JSON array");
        assert_eq!(arr.len(), 1, "array must contain exactly one edge");
        assert_eq!(arr[0]["from"].as_str().unwrap(), "src/main.rs");
        assert_eq!(arr[0]["to"].as_str().unwrap(), "src/lib.rs");
    }

    #[test]
    fn json_output_contains_import_cycles_array() {
        let mut report = make_report();
        report.import_cycles = vec![vec!["src/a.rs".into(), "src/b.rs".into()]];

        let output = render(&report, None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let arr = parsed["import_cycles"]
            .as_array()
            .expect("import_cycles must be a JSON array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0][0].as_str().unwrap(), "src/a.rs");
        assert_eq!(arr[0][1].as_str().unwrap(), "src/b.rs");
    }

    #[test]
    fn json_output_per_file_coupling_empty_when_no_data() {
        let report = make_report(); // per_file_coupling is already vec![]

        let output = render(&report, None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let arr = parsed["per_file_coupling"]
            .as_array()
            .expect("per_file_coupling must be present as an empty JSON array, not absent");
        assert!(arr.is_empty(), "per_file_coupling array must be empty");
    }

    #[test]
    fn json_render_trend_snapshots_have_required_fields() {
        use crate::scorer::HistoryEntry;
        use crate::trend::{TrendDelta, TrendSummary};
        use std::collections::HashMap;

        let report = make_report();

        let mut categories = HashMap::new();
        categories.insert("Health".to_string(), 30u32);
        categories.insert("Team".to_string(), 18u32);
        categories.insert("Evolution".to_string(), 14u32);
        categories.insert("Git Hygiene".to_string(), 10u32);

        let entry = HistoryEntry {
            timestamp: chrono::DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            head: "abc1234def5678901234567890abcdef12345678".to_string(),
            overall_score: 72,
            categories,
            metrics: HashMap::new(),
            counts: crate::scorer::HistoryCounts::default(),
            branch: "main".to_string(),
            schema_version: 1,
            source: None,
        };

        let summary = TrendSummary {
            delta: TrendDelta {
                overall: 0,
                delta_vs_oldest: 0,
                categories: HashMap::new(),
                is_first: true,
            },
            sparkline: vec![],
            velocity: None,
            branch_mismatch_warning: false,
            history: vec![entry],
        };

        let output = render(&report, Some(&summary)).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let trend = parsed.get("trend").expect("trend key must be present");
        let snapshots = trend["snapshots"]
            .as_array()
            .expect("trend.snapshots should be an array");
        assert_eq!(snapshots.len(), 1, "should have exactly 1 snapshot");

        let snap = &snapshots[0];
        assert!(
            snap["timestamp"].is_string(),
            "snapshot.timestamp should be a string"
        );
        let ts = snap["timestamp"].as_str().unwrap();
        assert!(
            ts.ends_with('Z') || ts.contains('+'),
            "snapshot.timestamp should be ISO8601 UTC, got: {ts}"
        );
        assert!(
            snap["commit"].is_string(),
            "snapshot.commit should be a string (mapped from head)"
        );
        assert_eq!(
            snap["commit"].as_str().unwrap(),
            "abc1234def5678901234567890abcdef12345678",
            "snapshot.commit should contain the full SHA"
        );
        assert!(
            snap["branch"].is_string(),
            "snapshot.branch should be a string"
        );
        assert_eq!(
            snap["branch"].as_str().unwrap(),
            "main",
            "snapshot.branch should match entry branch"
        );
        assert!(
            snap["overall_score"].is_number(),
            "snapshot.overall_score should be a number"
        );
        assert_eq!(
            snap["overall_score"].as_u64().unwrap(),
            72,
            "snapshot.overall_score should match entry value"
        );

        let cat = snap["category_scores"]
            .as_object()
            .expect("snapshot.category_scores should be an object");
        for key in &["Health", "Team", "Evolution", "Git Hygiene"] {
            assert!(
                cat.contains_key(*key),
                "snapshot.category_scores should contain key '{key}'"
            );
        }
    }
}
