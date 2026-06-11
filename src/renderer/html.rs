const CSS: &str = include_str!("templates/style.css");
const JS_SHARED: &str = include_str!("templates/shared.js");
const JS_OVERVIEW: &str = include_str!("templates/overview.js");
const JS_HOTSPOTS: &str = include_str!("templates/hotspots.js");
const JS_COUPLING: &str = include_str!("templates/coupling.js");
const JS_GRAPH: &str = include_str!("templates/graph.js");
const JS_OWNERSHIP: &str = include_str!("templates/ownership.js");
const JS_AGE: &str = include_str!("templates/age.js");
const JS_TREEMAP_LAYOUT: &str = include_str!("templates/treemap_layout.js");
const JS_TREEMAP_UI: &str = include_str!("templates/treemap_ui.js");
const JS_TRENDS: &str = include_str!("templates/trends.js");
const JS_DEPS: &str = include_str!("templates/deps.js");
const JS_AUDIT: &str = include_str!("templates/audit.js");
const JS_AUTHORS: &str = include_str!("templates/authors.js");

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_extra;

use crate::scorer::AnalysisReport;
use anyhow::Result;

/// Render the analysis report as a self-contained HTML file.
/// All CSS, JS, and data are inlined. No external dependencies.
pub fn render(report: &AnalysisReport) -> Result<String> {
    let json = serde_json::to_string(report)?;
    let json = super::escape::escape_json_for_script(&json);
    let title = format!("{} — Barad-dûr Report", report.repo_name);

    let html = format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"UTF-8\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
<title>{title}</title>\n<style>\n{css}\n</style>\n</head>\n<body>\n\
<script>window.R={json};</script>\n\
<div id=\"app\"></div>\n\
<script>\n{js}\n</script>\n</body>\n</html>",
        title = title,
        json = json,
        css = CSS,
        js = build_js(report),
    );
    Ok(html)
}

// _report is kept for future per-report JS customisation (feature flags, conditional modules).
fn build_js(_report: &AnalysisReport) -> String {
    [
        JS_SHARED,
        JS_OVERVIEW,
        JS_HOTSPOTS,
        JS_COUPLING,
        JS_GRAPH,
        JS_OWNERSHIP,
        JS_AGE,
        JS_TREEMAP_LAYOUT,
        JS_TREEMAP_UI,
        JS_TRENDS,
        JS_DEPS,
        JS_AUDIT,
        JS_AUTHORS,
    ]
    .concat()
}
