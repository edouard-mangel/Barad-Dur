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
        per_file_coupling: vec![],
        import_edges: vec![],
        import_cycles: vec![],
        score_thresholds: Default::default(),
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

#[test]
fn html_fmt_helper_present() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("function fmt("),
        "js_shared.rs must export a fmt() helper so all tabs can safely format numbers"
    );
}

#[test]
fn html_coupling_pct_uses_fmt() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("fmt(p.coupling_pct,"),
        "coupling_pct must be formatted via fmt() to handle missing values safely"
    );
}

#[test]
fn html_ownership_pct_uses_fmt() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("fmt(topAuthor.pct,"),
        "ownership pct must be formatted via fmt() to handle missing values safely"
    );
}

#[test]
fn html_audit_pct_of_total_uses_fmt() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("fmt(d.pct_of_total,"),
        "audit pct_of_total must be formatted via fmt() to handle missing values safely"
    );
}

#[test]
fn html_full_report_with_all_data_renders_ok() {
    use crate::scorer::{
        AuditReport, AuthorShare, CouplingPair, DirConcentration, FileOwnership, HotspotFile,
    };

    let mut report = make_report();
    report.coupling_pairs = vec![CouplingPair {
        file_a: "src/a.rs".into(),
        file_b: "src/b.rs".into(),
        co_changes: 10,
        coupling_pct: 75.5,
        cross_boundary: true,
        is_test_pair: false,
    }];
    report.file_hotspots = vec![HotspotFile {
        path: "src/big.rs".into(),
        churn_count: 20,
        bug_commit_count: 3,
        loc: 500,
        total_lines: 600,
        cyclomatic_complexity: 15,
        public_methods: 10,
        properties: 5,
        hotspot_score: 8.5,
    }];
    report.author_ownership = vec![FileOwnership {
        path: "src/a.rs".into(),
        authors: vec![AuthorShare {
            name: "Alice".into(),
            pct: 60.0,
        }],
    }];
    report.audit = Some(AuditReport {
        crisis_files: vec![],
        dir_concentration: vec![DirConcentration {
            dir: "src/".into(),
            file_count: 10,
            loc: 1000,
            pct_of_total: 74.3,
        }],
        dead_files: vec![],
        velocity_buckets: vec![],
    });

    let result = render(&report);
    assert!(
        result.is_ok(),
        "full-data render must succeed: {:?}",
        result.err()
    );
    let html = result.unwrap();
    assert!(
        html.len() > 10_000,
        "full report HTML should be substantial, got {} bytes",
        html.len()
    );
    assert!(
        html.contains("75.5"),
        "coupling_pct value must appear in window.R"
    );
    assert!(
        html.contains("8.5"),
        "hotspot_score value must appear in window.R"
    );
    assert!(
        html.contains("74.3"),
        "pct_of_total value must appear in window.R"
    );
}

#[test]
fn html_tab_render_has_error_boundary() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("safeRender"),
        "Tab lazy-render must use safeRender() wrapper so any tab builder exception shows a graceful error instead of a blank tab"
    );
}

#[test]
fn html_deps_mean_drift_uses_fmt() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("fmt(eco.mean_drift_years,"),
        "mean_drift_years must go through fmt() so missing values show — instead of crashing"
    );
}

#[test]
fn html_deps_drift_years_uses_fmt() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("fmt(dep.drift_years,"),
        "dep.drift_years must go through fmt() so missing values show — instead of crashing"
    );
}

#[test]
fn html_safe_render_error_has_tab_error_class() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("tab-error"),
        "safeRender error div must use CSS class 'tab-error' for consistent styling"
    );
}

#[test]
fn html_authors_empty_state_uses_no_data_class() {
    // make_report() has author_cards: vec![] so we hit the empty path
    let html = render(&make_report()).unwrap();
    assert!(
        !html.contains("padding: '48px'"),
        "Authors tab empty state must not use inline padding style — use className: 'no-data' instead"
    );
}

// ---- Empty-state tests: one per tab ----

#[test]
fn html_treemap_empty_state() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("No file data available for treemap."),
        "Treemap tab must show its empty-state message when file_hotspots is empty"
    );
}

#[test]
fn html_authors_empty_state() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("No author data available. Run with blame enabled."),
        "Authors tab must show its empty-state message when author_cards is empty"
    );
}

#[test]
fn html_coupling_empty_state() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("No temporal coupling data available."),
        "Coupling tab must show its empty-state message when coupling_pairs is empty"
    );
}

#[test]
fn html_ownership_empty_state() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("No ownership data available."),
        "Ownership tab must show its empty-state message when author_ownership is empty"
    );
}

#[test]
fn html_age_empty_state() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("No file age data available."),
        "Age tab must show its empty-state message when file_ages is empty"
    );
}

#[test]
fn html_audit_empty_state() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("No audit data available."),
        "Audit tab must show its empty-state message when audit is None"
    );
}

#[test]
fn html_no_bare_green_hex_in_js_output() {
    let html = render(&make_report()).unwrap();
    // The CSS :root block legitimately contains --c-good: #10b981 and --c-good-lo: #22c55e.
    // Verify the CSS token infrastructure is present (positive assertion).
    assert!(
        html.contains("--c-good:"),
        "rendered HTML must define the --c-good CSS token in :root"
    );
    assert!(
        html.contains("body.cbf"),
        "rendered HTML must define the body.cbf override block for the CBF palette"
    );
    // Verify that the JS section uses var(--c-*) tokens rather than only bare hex.
    let js_start = html
        .find("<script>\n")
        .unwrap_or_else(|| html.find("<script>").unwrap_or(html.len()));
    let js_section = &html[js_start..];
    assert!(
        js_section.contains("var(--c-good)"),
        "JS section must use var(--c-good) token — bare hex has been replaced"
    );
    assert!(
        js_section.contains("var(--c-danger)"),
        "JS section must use var(--c-danger) token — bare hex has been replaced"
    );
}

#[test]
fn html_theme_toggle_button_present() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("theme-btn"),
        "rendered HTML must contain the theme toggle button with id=theme-btn"
    );
    assert!(
        html.contains("toggleTheme"),
        "rendered HTML must call toggleTheme() in the theme button click handler"
    );
}

#[test]
fn js_shared_has_theme_btn_id() {
    assert!(
        super::JS_SHARED.contains("theme-btn"),
        "shared.js must contain the string 'theme-btn' (the theme toggle button id)"
    );
}

#[test]
fn html_cbf_toggle_button_present() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("cbf-btn"),
        "rendered HTML must contain the CBF toggle button with id=cbf-btn"
    );
    assert!(
        html.contains("cbf-palette"),
        "rendered HTML must contain localStorage key cbf-palette"
    );
}

// ---- Test pair badge tests (step 05) ----

#[test]
fn coupling_tab_shows_test_pair_badge_when_is_test_pair() {
    let mut report = make_report();
    report.coupling_pairs = vec![crate::scorer::CouplingPair {
        file_a: "src/user.rs".into(),
        file_b: "src/user_test.rs".into(),
        co_changes: 10,
        coupling_pct: 80.0,
        cross_boundary: false,
        is_test_pair: true,
    }];
    let html = render(&report).unwrap();
    // The emoji appears in the JS template; window.R carrying is_test_pair:true is what
    // drives the JS badge condition — assert the data was serialised correctly.
    assert!(
        html.contains("\"is_test_pair\":true"),
        "window.R must carry is_test_pair:true so the JS badge condition fires"
    );
    assert!(
        html.contains("\u{1F9EA}"),
        "JS template must contain the 🧪 badge code"
    );
    assert!(
        html.contains("Expected coupling"),
        "JS template must include the tooltip text"
    );
}

#[test]
fn coupling_tab_no_test_pair_badge_for_regular_pairs() {
    let mut report = make_report();
    report.coupling_pairs = vec![crate::scorer::CouplingPair {
        file_a: "src/user.rs".into(),
        file_b: "src/order.rs".into(),
        co_changes: 5,
        coupling_pct: 60.0,
        cross_boundary: false,
        is_test_pair: false,
    }];
    let html = render(&report).unwrap();
    assert!(
        !html.contains("\"is_test_pair\":true"),
        "window.R must not carry is_test_pair:true for regular coupling pairs"
    );
}

// ---- Instability panel tests (step 03-01) ----

#[test]
fn coupling_tab_renders_instability_panel_when_data_present() {
    use crate::scorer::FileCouplingMetrics;

    let mut report = make_report();
    report.per_file_coupling = vec![FileCouplingMetrics {
        path: "src/main.rs".into(),
        ca: 2,
        ce: 8,
        instability: 0.8,
    }];
    let html = render(&report).unwrap();
    // The "Instability by File" heading must appear in the JS function
    assert!(
        html.contains("Instability by File"),
        "coupling tab must render an 'Instability by File' heading"
    );
    // The per_file_coupling data must be serialised into window.R
    assert!(
        html.contains("per_file_coupling"),
        "window.R must contain the per_file_coupling field when data is present"
    );
    // The instability value 0.8 must appear in window.R JSON
    assert!(
        html.contains("0.8"),
        "window.R must contain the instability value 0.8 for the test entry"
    );
}

#[test]
fn coupling_tab_shows_no_data_message_when_per_file_coupling_empty() {
    // make_report() already has per_file_coupling: vec![]
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("No static import data available."),
        "coupling tab JS must contain the no-data message literal for the empty-state branch"
    );
}

// ---- Instability table column tooltips (step 03-02) ----

#[test]
fn coupling_tab_instability_table_has_column_tooltips() {
    use crate::scorer::FileCouplingMetrics;

    let mut report = make_report();
    report.per_file_coupling = vec![
        FileCouplingMetrics {
            path: "src/stable.rs".into(),
            ca: 10,
            ce: 2,
            instability: 0.17,
        },
        FileCouplingMetrics {
            path: "src/unstable.rs".into(),
            ca: 1,
            ce: 9,
            instability: 0.9,
        },
    ];
    let html = render(&report).unwrap();

    // Ca column tooltip
    assert!(
        html.contains("Afferent coupling: number of files that import this file. High Ca = many dependents, risky to change."),
        "Ca column header must use thWithTip() with the exact afferent coupling tooltip"
    );

    // Ce column tooltip
    assert!(
        html.contains(
            "Efferent coupling: number of files this file imports. High Ce = many dependencies."
        ),
        "Ce column header must use thWithTip() with the exact efferent coupling tooltip"
    );

    // Instability column tooltip
    assert!(
        html.contains(
            "Ce / (Ca + Ce). 0 = stable (depended upon). 1 = unstable (depends on others)."
        ),
        "Instability column header must use thWithTip() with the exact instability formula tooltip"
    );
}

// ---- Import dependency graph tab ----

#[test]
fn graph_tab_is_registered_in_tab_bar() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("'Graph'"),
        "the tab bar must include a 'Graph' entry in tabNames"
    );
    assert!(
        html.contains("function buildGraphTab"),
        "the embedded JS must define buildGraphTab()"
    );
}

#[test]
fn graph_tab_embeds_import_edges_in_window_r() {
    use crate::scorer::ImportEdge;

    let mut report = make_report();
    report.import_edges = vec![ImportEdge {
        from: "src/cmd/analyze.rs".into(),
        to: "src/scorer.rs".into(),
    }];
    let html = render(&report).unwrap();
    assert!(
        html.contains("import_edges"),
        "window.R must contain the import_edges field"
    );
    assert!(
        html.contains("src/cmd/analyze.rs"),
        "window.R must contain the edge source path"
    );
}

#[test]
fn graph_tab_has_empty_state_message() {
    // make_report() has import_edges: vec![] — the JS must carry the
    // empty-state branch for repos where no imports were resolved.
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("No import graph data available."),
        "graph tab JS must contain the no-data message literal"
    );
}

#[test]
fn graph_tab_marks_cycle_edges() {
    use crate::scorer::ImportEdge;

    let mut report = make_report();
    report.import_edges = vec![
        ImportEdge {
            from: "src/a.rs".into(),
            to: "src/b.rs".into(),
        },
        ImportEdge {
            from: "src/b.rs".into(),
            to: "src/a.rs".into(),
        },
    ];
    report.import_cycles = vec![vec!["src/a.rs".into(), "src/b.rs".into()]];
    let html = render(&report).unwrap();
    assert!(
        html.contains("import_cycles"),
        "window.R must carry import_cycles and the JS must read it"
    );
    assert!(
        html.contains("stroke-dasharray"),
        "cycle edges must be drawn dashed (stroke-dasharray present in JS)"
    );
    assert!(
        html.contains("Circular dependency"),
        "the legend must explain the dashed cycle edges"
    );
}

#[test]
fn graph_tab_has_focus_mode() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("Click a node to focus its neighbourhood"),
        "the graph tab must advertise click-to-focus"
    );
}

#[test]
fn graph_tab_has_directory_grouping_toggle() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("Group by directory"),
        "the graph tab must offer a directory aggregation toggle"
    );
}

#[test]
fn graph_tab_has_min_degree_slider() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("Min degree"),
        "the graph tab must offer a min-degree filter"
    );
}

#[test]
fn graph_tab_has_svg_export() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("Export SVG"),
        "the graph tab must offer an SVG export button"
    );
}

#[test]
fn coupling_instability_table_links_to_graph() {
    use crate::scorer::FileCouplingMetrics;

    let mut report = make_report();
    report.per_file_coupling = vec![FileCouplingMetrics {
        path: "src/main.rs".into(),
        ca: 2,
        ce: 8,
        instability: 0.8,
    }];
    let html = render(&report).unwrap();
    assert!(
        html.contains("__focusGraphNode"),
        "instability table rows must drill through to the graph tab via __focusGraphNode"
    );
}

#[test]
fn report_has_cross_tab_file_navigation() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("function focusFileOnTab("),
        "report must expose a shared cross-tab file navigation entry point"
    );
    assert!(
        html.contains("registerFileFocus("),
        "tabs must register their file-focus handlers in the shared registry"
    );
}

#[test]
fn report_restores_state_from_url_hash() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("function parseHashState("),
        "report must parse tab/file deep-link state from the URL hash"
    );
    assert!(
        html.contains("history.replaceState"),
        "selection changes must be reflected in the URL hash for shareable deep links"
    );
}

#[test]
fn age_and_ownership_tables_link_to_hotspots() {
    let html = render(&make_report()).unwrap();
    let links = html
        .matches("linkFileCell(fileCell, f.path, 'Hotspots')")
        .count();
    assert!(
        links >= 2,
        "age and ownership file cells must drill through to the Hotspots tab (found {links} call sites)"
    );
}

#[test]
fn hotspot_table_shows_bug_commit_column() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("'Bugs', 'bug_commit_count'"),
        "hotspot table must expose the bug_commit_count field as a sortable Bugs column"
    );
}

#[test]
fn hotspot_scatter_has_axis_ticks() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("hs-axis-tick"),
        "hotspot scatter must render numeric axis tick labels"
    );
}

#[test]
fn hotspot_table_has_text_filter() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("hs-filter"),
        "hotspot table must offer a text filter input"
    );
}

#[test]
fn hotspot_selection_plots_missing_dot() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("function makeDot("),
        "selecting an unplotted file must add its scatter dot on demand via makeDot()"
    );
}

#[test]
fn hotspot_table_rows_highlight_scatter_dot() {
    let html = render(&make_report()).unwrap();
    assert!(
        html.contains("function selectHotspot("),
        "Hotspots tab must share one selectHotspot() between scatter dots and table rows"
    );
    assert!(
        html.contains("Highlight in scatter plot"),
        "hotspot table rows must advertise click-to-highlight via their tooltip"
    );
}
