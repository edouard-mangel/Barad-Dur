mod css;
mod js_age;
mod js_authors;
mod js_coupling;
mod js_hotspots;
mod js_overview;
mod js_ownership;
mod js_shared;
mod js_treemap;
mod js_trends;

#[cfg(test)]
mod tests;

use crate::scorer::AnalysisReport;
use anyhow::Result;

/// Render the analysis report as a self-contained HTML file.
/// All CSS, JS, and data are inlined. No external dependencies.
pub fn render(report: &AnalysisReport) -> Result<String> {
    let json = serde_json::to_string(report)?;
    let json = json.replace("</", "<\\/");
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
        css = css::CSS,
        js = build_js(),
    );
    Ok(html)
}

#[cfg(test)]
fn score_color(score: u32) -> &'static str {
    if score >= 71 {
        "#10b981"
    } else if score >= 41 {
        "#f59e0b"
    } else {
        "#ef4444"
    }
}

fn build_js() -> String {
    [
        js_shared::JS,
        js_overview::JS,
        js_hotspots::JS,
        js_coupling::JS,
        js_ownership::JS,
        js_age::JS,
        js_treemap::JS,
        js_trends::JS,
        js_authors::JS,
    ]
    .concat()
}
