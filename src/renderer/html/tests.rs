use super::*;
use crate::metrics::{CategoryResult, MetricValue, RawValue};

use crate::scorer::{ActionItem, AnalysisReport};

#[test]
fn css_defines_bg_primary_variable() {
    assert!(
        super::CSS.contains("--bg-primary:"),
        "CSS const must declare --bg-primary custom property in body rule"
    );
}

#[test]
fn css_has_light_mode_block() {
    assert!(
        super::CSS.contains("body.light"),
        "CSS const must contain a body.light selector block for light mode overrides"
    );
}

#[test]
fn css_has_light_cbf_compose_block() {
    assert!(
        super::CSS.contains("body.light.cbf"),
        "CSS const must contain a body.light.cbf selector block for CBF + light mode composition"
    );
}

fn make_report() -> AnalysisReport {
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
                score: Some(75),
            }],
        }],
        top_actions: vec![ActionItem {
            text: "Improve test coverage".into(),
            target_tab: Some("hotspots".into()),
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
        score_thresholds: Default::default(),
    }
}

#[test]
fn html_is_valid_document() {
    let report = make_report();
    let html = render(&report).unwrap();
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("</html>"));
}

#[test]
fn html_has_cbf_css_tokens() {
    let html = render(&make_report()).unwrap();
    assert!(html.contains("--c-good:"), "--c-good token must be in CSS");
    assert!(html.contains("--c-warn:"), "--c-warn token must be in CSS");
    assert!(
        html.contains("--c-danger:"),
        "--c-danger token must be in CSS"
    );
    assert!(
        html.contains("body.cbf"),
        "body.cbf override block must exist"
    );
    assert!(
        html.contains("#38bdf8"),
        "CBF block must contain sky-blue for --c-good"
    );
}

#[test]
fn html_css_uses_custom_properties() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("--bg-primary"),
        "body rule must declare --bg-primary CSS custom property"
    );
    assert!(
        html.contains("--text-primary"),
        "body rule must declare --text-primary CSS custom property"
    );
    assert!(
        html.contains("--bg-secondary"),
        "body rule must declare --bg-secondary CSS custom property"
    );
    assert!(
        html.contains("--border-color"),
        "body rule must declare --border-color CSS custom property"
    );
    assert!(
        html.contains("--text-muted"),
        "body rule must declare --text-muted CSS custom property"
    );
    assert!(
        html.contains("--bg-card"),
        "body rule must declare --bg-card CSS custom property"
    );
}

#[test]
fn html_hotspots_has_dismiss_controls() {
    // AC-D6: the hotspots view ships a per-row dismiss control and a reset control,
    // mirroring the coupling-pair dismissal. Behavior (click → hide row) is client-side
    // JS verified manually; this guards that the controls are present in the template
    // and not accidentally removed. Markers are hotspots-specific (hs-*) because
    // coupling.js already contributes "Reset dismissed"/"cp-dismiss" to the document.
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("hs-dismiss"),
        "hotspots table must include a per-row dismiss control (hs-dismiss class)"
    );
    assert!(
        html.contains("hs-dismiss-reset"),
        "hotspots view must include a reset-dismissed control (hs-dismiss-reset class)"
    );
}

#[test]
fn html_hotspots_dismiss_is_keyed_by_path() {
    // AC-D3: dismissal must be keyed by file path, so re-sorting/filtering the table
    // keeps the correct rows hidden (not whatever row lands in the dismissed slot).
    // The JS isn't executable from the Rust harness, so this statically pins the
    // path-keyed wiring — `dismissed[f.path]` — against a regression to index keying.
    // The behavioral counterpart lives in dashboard HotspotsView.test.tsx.
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("dismissed[f.path]"),
        "hotspots dismissal must be keyed by file path (dismissed[f.path]) so it \
         survives re-sorting/filtering, not by row index"
    );
}

#[test]
fn html_embeds_report_data() {
    let report = make_report();
    let html = render(&report).unwrap();
    assert!(html.contains("my-repo"));
}

#[test]
fn html_contains_tab_markers() {
    let report = make_report();
    let html = render(&report).unwrap();
    assert!(html.contains("Hotspots"));
    assert!(html.contains("Coupling"));
    assert!(html.contains("Ownership"));
    assert!(html.contains("Age"));
}

#[test]
fn html_title_contains_repo_name() {
    let report = make_report();
    let html = render(&report).unwrap();
    assert!(html.contains("<title>my-repo"));
}

#[test]
fn html_embeds_score_thresholds_in_window_r() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains(r#""score_thresholds":{"good_min":71,"warn_min":41}"#),
        "window.R must carry the band thresholds so JS consumers read them instead of hardcoding"
    );
}

#[test]
fn js_score_color_reads_embedded_thresholds() {
    assert!(
        super::JS_SHARED.contains("score_thresholds"),
        "shared.js scoreColor must read R.score_thresholds, not hardcode 71/41"
    );
}

#[test]
fn js_score_color_is_defined_exactly_once() {
    let js = super::build_js(&make_report());
    assert_eq!(
        js.matches("function scoreColor(").count(),
        1,
        "scoreColor must exist only in shared.js — tab modules must not shadow it"
    );
}

#[test]
fn html_score_color_uses_css_vars() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("var(--c-good)"),
        "scoreColor must return var(--c-good)"
    );
    assert!(
        html.contains("var(--c-warn)"),
        "scoreColor must return var(--c-warn)"
    );
    assert!(
        html.contains("var(--c-danger)"),
        "scoreColor must return var(--c-danger)"
    );
}

// ---- Treemap tests ----

fn make_treemap_report() -> AnalysisReport {
    use crate::scorer::{AuthorShare, FileAge, FileOwnership, HotspotFile};
    use chrono::Utc;

    let mut report = make_report();
    report.file_hotspots = vec![
        HotspotFile {
            path: "src/main.rs".into(),
            churn_count: 12,
            bug_commit_count: 0,
            loc: 200,
            total_lines: 210,
            cyclomatic_complexity: 15,
            public_methods: 3,
            properties: 1,
            hotspot_score: 72.0,
            content_findings: 0,
            common_findings: 0,
            control_findings: 0,
            inheritance_findings: 0,
            churn_timeline: vec![],
        },
        HotspotFile {
            path: "src/lib.rs".into(),
            churn_count: 8,
            bug_commit_count: 0,
            loc: 150,
            total_lines: 160,
            cyclomatic_complexity: 10,
            public_methods: 5,
            properties: 2,
            hotspot_score: 45.0,
            content_findings: 0,
            common_findings: 0,
            control_findings: 0,
            inheritance_findings: 0,
            churn_timeline: vec![],
        },
        HotspotFile {
            path: "tests/test_a.rs".into(),
            churn_count: 3,
            bug_commit_count: 0,
            loc: 80,
            total_lines: 85,
            cyclomatic_complexity: 4,
            public_methods: 2,
            properties: 0,
            hotspot_score: 20.0,
            content_findings: 0,
            common_findings: 0,
            control_findings: 0,
            inheritance_findings: 0,
            churn_timeline: vec![],
        },
        HotspotFile {
            path: "tests/test_b.rs".into(),
            churn_count: 1,
            bug_commit_count: 0,
            loc: 60,
            total_lines: 65,
            cyclomatic_complexity: 2,
            public_methods: 1,
            properties: 0,
            hotspot_score: 10.0,
            content_findings: 0,
            common_findings: 0,
            control_findings: 0,
            inheritance_findings: 0,
            churn_timeline: vec![],
        },
    ];
    report.file_ages = vec![
        FileAge {
            path: "src/main.rs".into(),
            last_modified: Utc::now(),
            days_since_modified: 5,
        },
        FileAge {
            path: "src/lib.rs".into(),
            last_modified: Utc::now(),
            days_since_modified: 30,
        },
        FileAge {
            path: "tests/test_a.rs".into(),
            last_modified: Utc::now(),
            days_since_modified: 90,
        },
        FileAge {
            path: "tests/test_b.rs".into(),
            last_modified: Utc::now(),
            days_since_modified: 200,
        },
    ];
    report.author_ownership = vec![
        FileOwnership {
            path: "src/main.rs".into(),
            authors: vec![
                AuthorShare {
                    name: "Alice".into(),
                    pct: 70.0,
                },
                AuthorShare {
                    name: "Bob".into(),
                    pct: 30.0,
                },
            ],
        },
        FileOwnership {
            path: "src/lib.rs".into(),
            authors: vec![
                AuthorShare {
                    name: "Bob".into(),
                    pct: 60.0,
                },
                AuthorShare {
                    name: "Alice".into(),
                    pct: 40.0,
                },
            ],
        },
        FileOwnership {
            path: "tests/test_a.rs".into(),
            authors: vec![AuthorShare {
                name: "Alice".into(),
                pct: 100.0,
            }],
        },
        FileOwnership {
            path: "tests/test_b.rs".into(),
            authors: vec![AuthorShare {
                name: "Bob".into(),
                pct: 100.0,
            }],
        },
    ];
    report
}

#[test]
fn html_contains_treemap_tab() {
    let html = render(&make_treemap_report()).unwrap();
    assert!(html.contains("Treemap"), "Should contain Treemap tab name");
}

#[test]
fn html_treemap_has_svg() {
    let html = render(&make_treemap_report()).unwrap();
    assert!(
        html.contains("tm-svg"),
        "Should contain tm-svg container id"
    );
}

#[test]
fn html_audit_none_serializes_as_null_in_window_r() {
    // audit: None must appear as "audit":null in window.R so the JS `if (!audit)` guard works.
    let report = make_report(); // audit is None
    let html = render(&report).unwrap();
    assert!(
        html.contains("\"audit\":null"),
        "audit:None must serialise as null so JS falsy check handles the missing-data path"
    );
}

#[test]
fn html_contains_audit_tab_label() {
    let html = render(&make_report()).unwrap();
    assert!(html.contains("'Audit'"), "tabNames must include 'Audit'");
}

#[test]
fn html_audit_build_function_present() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("buildAuditTab"),
        "buildAuditTab JS function must be present"
    );
}

#[test]
fn html_audit_populated_data_appears_in_window_r() {
    use crate::scorer::{AuditReport, CrisisFile, DeadFile, DirConcentration, VelocityBucket};

    let mut report = make_report();
    report.audit = Some(AuditReport {
        crisis_files: vec![CrisisFile {
            path: "src/danger.rs".into(),
            crisis_commit_count: 3,
            total_commit_count: 4,
            crisis_ratio: 0.75,
        }],
        dir_concentration: vec![DirConcentration {
            dir: "src/metrics".into(),
            file_count: 5,
            loc: 1200,
            pct_of_total: 42.0,
        }],
        dead_files: vec![DeadFile {
            path: "old/legacy.rs".into(),
            days_since_modified: 730,
            churn_count: 0,
        }],
        velocity_buckets: vec![VelocityBucket {
            week_start: "2024-03-04".into(),
            commit_count: 7,
            author_count: 2,
        }],
    });

    let html = render(&report).unwrap();
    assert!(
        html.contains("src/danger.rs"),
        "crisis file path must be in window.R"
    );
    assert!(html.contains("0.75"), "crisis_ratio must be in window.R");
    assert!(html.contains("src/metrics"), "dir name must be in window.R");
    assert!(
        html.contains("old/legacy.rs"),
        "dead file path must be in window.R"
    );
    assert!(
        html.contains("2024-03-04"),
        "velocity week_start must be in window.R"
    );
}

#[test]
fn html_audit_field_serialized_in_window_r() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("\"audit\""),
        "window.R JSON blob must include the audit field"
    );
}

#[test]
fn html_treemap_has_metric_select() {
    let html = render(&make_treemap_report()).unwrap();
    assert!(
        html.contains("tm-metric-select"),
        "Should contain tm-metric-select dropdown id"
    );
}

#[test]
fn html_treemap_has_squarify() {
    let html = render(&make_treemap_report()).unwrap();
    assert!(
        html.contains("squarify"),
        "Should contain squarify layout function"
    );
}

#[test]
fn html_treemap_has_color_scales() {
    let html = render(&make_treemap_report()).unwrap();
    assert!(
        html.contains("metricScales"),
        "Should contain metricScales color scale object"
    );
}

#[test]
fn html_treemap_has_breadcrumb() {
    let html = render(&make_treemap_report()).unwrap();
    assert!(
        html.contains("tm-breadcrumb"),
        "Should contain tm-breadcrumb navigation"
    );
}

#[test]
fn html_treemap_has_detail_panel() {
    let html = render(&make_treemap_report()).unwrap();
    assert!(html.contains("tm-detail"), "Should contain tm-detail panel");
}

#[test]
fn html_treemap_has_animated_transition() {
    let html = render(&make_treemap_report()).unwrap();
    assert!(
        html.contains("animateTransition"),
        "Should contain animated transition function"
    );
}

#[test]
fn html_treemap_has_circle_pack() {
    let html = render(&make_treemap_report()).unwrap();
    assert!(
        html.contains("circlePack"),
        "Should contain circlePack layout function"
    );
}

#[test]
fn html_treemap_has_layout_toggle() {
    let html = render(&make_treemap_report()).unwrap();
    assert!(
        html.contains("tm-layout-toggle"),
        "Should contain layout toggle control"
    );
}

#[test]
fn html_tabs_have_info_banners() {
    let html = render(&make_treemap_report()).unwrap();
    assert!(
        html.contains("tab-info"),
        "Should contain tab info banner CSS class"
    );
    assert!(
        html.contains("Hotspot Score"),
        "Hotspots tab should explain hotspot scoring"
    );
    assert!(
        html.contains("temporal coupling"),
        "Coupling tab should explain temporal coupling"
    );
    assert!(
        html.contains("knowledge distribution"),
        "Ownership tab should explain knowledge distribution"
    );
    assert!(
        html.contains("days since"),
        "Age tab should explain file age measurement"
    );
}

#[test]
fn html_hotspot_scatter_is_clickable() {
    let html = render(&make_treemap_report()).unwrap();
    assert!(
        html.contains("hs-scatter-dot"),
        "Scatter plot circles should have hs-scatter-dot class for click targeting"
    );
    assert!(
        html.contains("hs-row-highlight"),
        "Should have CSS class for highlighting table rows"
    );
}

#[test]
fn html_coupling_has_auto_exclude() {
    let html = render(&make_treemap_report()).unwrap();
    assert!(
        html.contains("isAutoExcluded"),
        "Coupling tab should have auto-exclude logic for interface/implementation pairs"
    );
}

#[test]
fn html_coupling_rows_are_dismissable() {
    let html = render(&make_treemap_report()).unwrap();
    assert!(
        html.contains("cp-dismiss"),
        "Coupling tab rows should have dismiss buttons"
    );
}

// ---- Trends tab tests ----

#[test]
fn html_contains_trends_tab() {
    let html = render(&make_report()).unwrap();
    assert!(html.contains("Trends"), "Should have Trends tab");
}

#[test]
fn html_trends_has_metric_select() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("tr-metric-select"),
        "Should have metric selector dropdown"
    );
}

#[test]
fn html_trends_has_chart() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("tr-chart"),
        "Should have trends chart container"
    );
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
            ..Default::default()
        },
        branch: "main".into(),
        schema_version: 1,
        source: source.map(|s| s.to_string()),
    }
}

#[test]
fn html_trends_backfill_source_wired_through() {
    let mut report = make_report();
    report.history = vec![make_history_entry(58, Some("backfill"))];
    let html = render(&report).unwrap();
    assert!(
        html.contains("source === 'backfill'"),
        "JS must contain guard: entry.source === 'backfill'"
    );
    assert!(
        html.contains(r#""source":"backfill""#),
        "window.R history entry must carry source:backfill in JSON"
    );
}

#[test]
fn html_trends_hollow_circle_js_fill_none() {
    let mut report = make_report();
    report.history = vec![make_history_entry(58, Some("backfill"))];
    let html = render(&report).unwrap();
    assert!(
        html.contains("isBackfill ? 'none'"),
        "Hollow circle conditional must assign fill='none' for backfill entries"
    );
    assert!(
        html.contains("pointer-events:all"),
        "Hollow circles need pointer-events:all for correct hover area (SVG fill=none issue)"
    );
}

#[test]
fn html_trends_live_circle_js_uses_score_color() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("scoreColor(scores[i])") || html.contains("scoreColor("),
        "Live circles must assign scoreColor to fill"
    );
}

#[test]
fn html_trends_hollow_circle_stroke_is_score_color() {
    let mut report = make_report();
    report.history = vec![make_history_entry(58, Some("backfill"))];
    let html = render(&report).unwrap();
    assert!(
        html.contains("dotStroke") || html.contains("scoreColor"),
        "Hollow circle stroke must be set to scoreColor"
    );
}

#[test]
fn html_trends_legacy_entry_has_no_source_in_window_r() {
    let mut report = make_report();
    report.history = vec![make_history_entry(65, None)];
    let html = render(&report).unwrap();
    assert!(
        !html.contains(r#""source":"backfill""#),
        "Legacy entry (source=None) must not appear as source:backfill in window.R"
    );
}

#[test]
fn html_trends_legend_labels_in_js() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("Backfill"),
        "Rendered JS should contain the 'Backfill' label string"
    );
    assert!(
        html.contains("Live analysis"),
        "Rendered JS should contain the 'Live analysis' label string"
    );
}

#[test]
fn html_has_light_mode_css_block() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("body.light"),
        "rendered HTML must contain a body.light CSS selector block"
    );
    assert!(
        html.contains("--bg-primary: #f8fafc"),
        "body.light block must override --bg-primary with light-mode value #f8fafc"
    );
}

#[test]
fn html_cbf_light_compose() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("body.light.cbf"),
        "rendered HTML must contain a body.light.cbf CSS selector block for CBF + light mode composition"
    );
    assert!(
        html.contains("#38bdf8"),
        "CBF sky-blue value #38bdf8 must appear in the rendered CSS"
    );
}

#[test]
fn html_trends_legend_no_inner_html() {
    let html = render(&make_report()).unwrap();
    let legend_region = html
        .find("tr-legend")
        .map(|pos| &html[pos.saturating_sub(500)..std::cmp::min(pos + 500, html.len())]);
    if let Some(region) = legend_region {
        assert!(
            !region.contains("innerHTML"),
            "Legend construction must not use innerHTML (security constraint)"
        );
    }
}

#[test]
fn html_theme_init_function_present() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("initTheme"),
        "rendered HTML must contain initTheme JS function"
    );
    assert!(
        html.contains("prefers-color-scheme"),
        "initTheme must check prefers-color-scheme media query for system theme detection"
    );
}

#[test]
fn js_shared_has_toggle_theme_function() {
    assert!(
        super::JS_SHARED.contains("toggleTheme"),
        "shared.js must define a toggleTheme function"
    );
}
