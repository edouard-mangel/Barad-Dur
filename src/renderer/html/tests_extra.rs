use super::*;
use crate::scorer::AnalysisReport;

fn make_report() -> AnalysisReport {
    use crate::metrics::{CategoryResult, MetricValue, RawValue};
    use crate::scorer::ActionItem;
    AnalysisReport {
        repo_name: "my-repo".into(),
        branch: "main".into(),
        time_window_months: 6,
        total_commits: 42,
        total_authors: 3,
        total_files: 20,
        overall_score: 75,
        categories: vec![CategoryResult {
            name: "Health".into(),
            score: 75,
            metrics: vec![MetricValue {
                name: "Bus factor".into(),
                description: "OK".into(),
                raw_value: RawValue::Integer(3),
                score: 75,
            }],
        }],
        top_actions: vec![ActionItem {
            text: "Improve test coverage".into(),
            target_tab: Some("hotspots".into()),
            sort_by: None,
        }],
        remote_meta: None,
        file_hotspots: vec![],
        coupling_pairs: vec![],
        author_ownership: vec![],
        file_ages: vec![],
        author_cards: vec![],
        history: vec![],
        dep_ecosystem_reports: vec![],
        audit: None,
    }
}

fn make_history_entry(score: u32, source: Option<&str>) -> crate::scorer::HistoryEntry {
    crate::scorer::HistoryEntry {
        timestamp: chrono::Utc::now(),
        head: "a".repeat(40),
        overall_score: score,
        categories: std::collections::HashMap::new(),
        metrics: std::collections::HashMap::new(),
        counts: crate::scorer::HistoryCounts {
            commits: 10,
            files: 5,
            authors: 2,
        },
        branch: "main".into(),
        schema_version: 1,
        source: source.map(|s| s.to_string()),
    }
}

#[test]
fn html_trends_legend_css_class_present() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("tr-legend"),
        "CSS class .tr-legend must be present for legend styling"
    );
}

#[test]
fn html_trends_tooltip_source_backfill_label() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("Source: Backfill"),
        "Tooltip JS must contain the literal string 'Source: Backfill'"
    );
}

#[test]
fn html_trends_tooltip_source_live_label() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("Source: Live analysis"),
        "Tooltip JS must contain the literal string 'Source: Live analysis'"
    );
}

#[test]
fn html_trends_tooltip_no_inner_html() {
    let html = render(&make_report()).unwrap();
    let tooltip_region = html
        .find("mouseenter")
        .map(|pos| &html[pos..std::cmp::min(pos + 800, html.len())]);
    if let Some(region) = tooltip_region {
        assert!(
            !region.contains(".innerHTML"),
            "Tooltip handler must use textContent, not innerHTML"
        );
    }
}

#[test]
fn html_trends_zero_backfill_window_r_has_no_source_field() {
    let mut report = make_report();
    report.history = vec![
        make_history_entry(70, None),
        make_history_entry(72, None),
        make_history_entry(75, None),
    ];
    let html = render(&report).unwrap();
    assert!(
        !html.contains(r#""source":"backfill""#),
        "Zero-backfill history must not inject source:backfill into window.R"
    );
}

#[test]
fn html_trends_all_backfill_window_r_all_have_source() {
    let mut report = make_report();
    report.history = vec![
        make_history_entry(55, Some("backfill")),
        make_history_entry(58, Some("backfill")),
        make_history_entry(60, Some("backfill")),
    ];
    let html = render(&report).unwrap();
    let count = html.matches(r#""source":"backfill""#).count();
    assert_eq!(
        count, 3,
        "All-backfill history: window.R must contain exactly 3 source:backfill entries"
    );
}

#[test]
fn html_trends_hollow_dot_pointer_events_all() {
    let mut report = make_report();
    report.history = vec![make_history_entry(58, Some("backfill"))];
    let html = render(&report).unwrap();
    assert!(
        html.contains("pointer-events:all"),
        "Hollow circle JS must set pointer-events:all to fix hover area on fill=none circles"
    );
}

// ---- Dependencies tab tests ----

#[test]
fn html_contains_dependencies_tab_label() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("Dependencies"),
        "Should contain Dependencies tab name in JS tabNames array"
    );
}

#[test]
fn html_deps_build_function_present() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("buildDepsTab"),
        "Should contain buildDepsTab JS function confirming the module is wired in"
    );
}

#[test]
fn html_deps_field_serialized_in_window_r() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("dep_ecosystem_reports"),
        "window.R JSON blob must include the dep_ecosystem_reports field"
    );
}

#[test]
fn html_deps_ecosystem_card_data_in_window_r() {
    use crate::deps::{Ecosystem, EcosystemReport};

    let mut report = make_report();
    report.dep_ecosystem_reports = vec![EcosystemReport {
        ecosystem: Ecosystem::Cargo,
        total_deps: 3,
        mean_drift_years: 1.5,
        total_drift_years: 4.5,
        critical_deps: vec![],
    }];
    let html = render(&report).unwrap();
    assert!(
        html.contains("dep_ecosystem_reports"),
        "window.R must contain dep_ecosystem_reports key"
    );
    assert!(
        html.contains("1.5"),
        "window.R must contain the mean_drift_years value 1.5 serialized in the JSON data blob"
    );
}
