use crate::coupling::CouplingReport;

/// Render a coupling report as a self-contained HTML file with an inline
/// force-directed graph. All CSS and JS are inlined -- no external dependencies.
pub fn render_coupling_html(report: &CouplingReport) -> String {
    let json_data = serde_json::to_string(report).unwrap_or_else(|_| "{}".to_string());
    let escaped_json = super::escape::escape_json_for_script(&json_data);

    format!(
        "<!DOCTYPE html>\n\
         <html lang=\"en\">\n\
         <head>\n\
         <meta charset=\"UTF-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>Coupling Report — Barad-dur</title>\n\
         <style>\n{css}\n</style>\n\
         </head>\n\
         <body>\n\
         <header>\n\
           <h1>Multi-Repository Coupling Analysis</h1>\n\
           <p class=\"summary\">{repo_count} repos &middot; {pair_count} coupling pairs \
            &middot; highest score: {highest:.1}</p>\n\
         </header>\n\
         <nav class=\"tabs\">\n\
           <button class=\"tab active\" data-tab=\"tab-graph\">Graph</button>\n\
           <button class=\"tab\" data-tab=\"tab-matrix\">Matrix</button>\n\
           <button class=\"tab\" data-tab=\"tab-methodology\">Methodology</button>\n\
         </nav>\n\
         <div class=\"filters\">\n\
           <label><input type=\"checkbox\" id=\"filter-temporal\" checked> Temporal</label>\n\
           <label><input type=\"checkbox\" id=\"filter-team\" checked> Team</label>\n\
           <label><input type=\"checkbox\" id=\"filter-dependency\" checked> Dependency</label>\n\
         </div>\n\
         <div id=\"tab-graph\" class=\"tab-content active\">\n\
           <div id=\"graph\"></div>\n\
           <div class=\"legend\">\n\
             <h3>How to read this graph</h3>\n\
             <div class=\"legend-section\">\n\
               <div class=\"label\">Line color = coupling strength</div>\n\
               <div class=\"legend-row\"><span class=\"legend-swatch\" style=\"background:#22c55e\"></span> Low (&lt; 40)</div>\n\
               <div class=\"legend-row\"><span class=\"legend-swatch\" style=\"background:#f59e0b\"></span> Medium (40 &ndash; 70)</div>\n\
               <div class=\"legend-row\"><span class=\"legend-swatch\" style=\"background:#ef4444\"></span> High (&gt; 70)</div>\n\
             </div>\n\
             <div class=\"legend-section\">\n\
               <div class=\"label\">Line thickness = coupling score</div>\n\
               <div style=\"color:#64748b\">Thicker lines mean a higher combined score</div>\n\
             </div>\n\
             <div class=\"legend-section\">\n\
               <div class=\"label\">Circle size = number of connections</div>\n\
               <div class=\"legend-row\"><span class=\"legend-circle\" style=\"width:10px;height:10px\"></span> Few connections</div>\n\
               <div class=\"legend-row\"><span class=\"legend-circle\" style=\"width:18px;height:18px\"></span> Many connections</div>\n\
             </div>\n\
             <div class=\"legend-section\" style=\"color:#64748b\">\n\
               Hover a node for details. Drag to rearrange.\n\
             </div>\n\
           </div>\n\
         </div>\n\
         <div id=\"tab-matrix\" class=\"tab-content\">\n\
           <div id=\"matrix\"></div>\n\
         </div>\n\
         <div id=\"tab-methodology\" class=\"tab-content\">\n\
           <div class=\"methodology\">\n\
             <h2>How Coupling Scores Are Calculated</h2>\n\
             <p class=\"intro\">Each pair of repositories is scored on three independent dimensions, \
              then combined into a single 0&ndash;100 score. Higher means tighter coupling &mdash; \
              changes in one repo are more likely to require changes in the other.</p>\n\
             <div class=\"method-section\">\n\
               <h3>1. Temporal Coupling (35% of combined score)</h3>\n\
               <p><strong>What it measures:</strong> How often commits happen in both repos within \
                the same time window (default: 24 hours).</p>\n\
               <p><strong>How it works:</strong></p>\n\
               <ol>\n\
                 <li>All commits across all repos are merged into a single timeline.</li>\n\
                 <li>For each commit, we look for commits in other repos within &plusmn;24h.</li>\n\
                 <li><strong>Same-author boost:</strong> If the <em>same person</em> committed to \
                  both repos within the window, that co-change counts <strong>3&times;</strong> more \
                  than different-author co-changes. A developer intentionally working across repos \
                  is strong evidence of real coupling.</li>\n\
                 <li><strong>Statistical baseline:</strong> We subtract the number of co-changes you \
                  would expect <em>by pure coincidence</em> given how often each repo is committed to. \
                  This filters out false positives from teams that simply commit during the same \
                  business hours.</li>\n\
                 <li>The adjusted count is divided by the smaller repo&rsquo;s commit count and \
                  expressed as a percentage (0&ndash;100).</li>\n\
               </ol>\n\
               <p class=\"formula\">score = min(100, max(0, weighted_co_changes &minus; expected_random) \
                / min(commits_A, commits_B) &times; 100)</p>\n\
             </div>\n\
             <div class=\"method-section\">\n\
               <h3>2. Team Coupling (30% of combined score)</h3>\n\
               <p><strong>What it measures:</strong> How much the contributor pools overlap between \
                two repos.</p>\n\
               <p><strong>How it works:</strong></p>\n\
               <ol>\n\
                 <li>Authors are matched by display name (case-insensitive).</li>\n\
                 <li>The score is the ratio of shared authors to total unique authors across \
                  both repos.</li>\n\
               </ol>\n\
               <p class=\"formula\">score = shared_authors / (unique_authors_A &cup; unique_authors_B) \
                &times; 100</p>\n\
               <p>A high team score means the same people maintain both repos &mdash; changes in one \
                are likely understood (and possibly required) by someone who also works on the other.</p>\n\
             </div>\n\
             <div class=\"method-section\">\n\
               <h3>3. Dependency Coupling (35% of combined score)</h3>\n\
               <p><strong>What it measures:</strong> Structural dependencies between repos based on \
                their declared packages and imports.</p>\n\
               <p><strong>How it works:</strong></p>\n\
               <ol>\n\
                 <li>Manifest files are scanned (Cargo.toml, package.json, go.mod, requirements.txt).</li>\n\
                 <li>Shared third-party dependencies are counted.</li>\n\
                 <li>Direct repo-to-repo dependencies are detected (repo A imports repo B).</li>\n\
               </ol>\n\
               <p>A high dependency score means both repos rely on the same libraries or directly \
                depend on each other &mdash; a breaking change in a shared dependency affects both.</p>\n\
             </div>\n\
             <div class=\"method-section\">\n\
               <h3>Combined Score</h3>\n\
               <p>The three dimension scores are combined with fixed weights:</p>\n\
               <p class=\"formula\">combined = temporal &times; 0.35 + team &times; 0.30 + dependency \
                &times; 0.35</p>\n\
               <p>Temporal is weighted lower because commit-timing correlation is inherently noisy. \
                Team and dependency signals are structural facts rather than statistical inferences.</p>\n\
             </div>\n\
             <div class=\"method-section\">\n\
               <h3>Confidence Levels</h3>\n\
               <p>Temporal coupling pairs also carry a confidence rating based on raw co-change count:</p>\n\
               <ul>\n\
                 <li><strong>Low:</strong> 3&ndash;9 co-changes &mdash; could be coincidence, treat \
                  with caution</li>\n\
                 <li><strong>Medium:</strong> 10&ndash;29 co-changes &mdash; likely real coupling</li>\n\
                 <li><strong>High:</strong> 30+ co-changes &mdash; strong evidence of coupled \
                  development</li>\n\
               </ul>\n\
             </div>\n\
           </div>\n\
         </div>\n\
         <div id=\"tooltip\" class=\"tooltip\"></div>\n\
         <script>window.__COUPLING_DATA__={json};</script>\n\
         <script>\n{js}\n</script>\n\
         </body>\n\
         </html>",
        css = CSS,
        repo_count = report.summary.total_repos,
        pair_count = report.summary.pairs_above_threshold,
        highest = report.summary.highest_coupling_score,
        json = escaped_json,
        js = JS,
    )
}

const CSS: &str = include_str!("templates/coupling_style.css");

const JS: &str = include_str!("templates/coupling_graph.js");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coupling::{
        CouplingDetails, CouplingPair, CouplingReportSummary, DependencyDetails, RepoInfo,
        TeamDetails, TemporalDetails,
    };
    use std::path::PathBuf;

    fn minimal_report() -> CouplingReport {
        CouplingReport {
            repos: vec![
                RepoInfo {
                    name: "alpha".to_string(),
                    path: PathBuf::from("/r/alpha"),
                    commit_count: 10,
                    author_count: 2,
                },
                RepoInfo {
                    name: "beta".to_string(),
                    path: PathBuf::from("/r/beta"),
                    commit_count: 20,
                    author_count: 3,
                },
            ],
            pairs: vec![CouplingPair {
                repo_a: "alpha".to_string(),
                repo_b: "beta".to_string(),
                temporal_score: 50.0,
                team_score: 25.0,
                dependency_score: 10.0,
                combined_score: 55.0,
                details: CouplingDetails {
                    temporal: TemporalDetails {
                        co_commit_count: 5,
                        total_windows: 10,
                    },
                    team: TeamDetails {
                        shared_authors: 1,
                        total_authors: 4,
                    },
                    dependency: DependencyDetails {
                        shared_dependencies: 2,
                        relationship: "shared-lib".to_string(),
                    },
                },
            }],
            summary: CouplingReportSummary {
                total_repos: 2,
                total_pairs_analyzed: 1,
                pairs_above_threshold: 1,
                highest_coupling_score: 55.0,
            },
            blast_radius: vec![],
        }
    }

    #[test]
    fn json_data_with_closing_script_tag_in_repo_name_is_escaped() {
        // If a repo name contains "</script>" the embedded JSON must not break out of
        // the <script> block. Escaping "</" as "<\/" is the standard defence.
        let mut report = minimal_report();
        report.repos[0].name = "evil</script><script>alert(1)</script>".to_string();
        report.pairs[0].repo_a = report.repos[0].name.clone();

        let html = render_coupling_html(&report);

        // The unescaped attack string must NOT appear verbatim in the output
        assert!(
            !html.contains("</script><script>alert(1)"),
            "unescaped closing script tag in JSON data allows XSS"
        );
        // The escaped form <\/ must appear instead
        assert!(
            html.contains("<\\/script>"),
            "expected <\\/ escaping in embedded JSON"
        );
    }

    #[test]
    fn html_contains_dark_theme_background() {
        let report = minimal_report();
        let html = render_coupling_html(&report);
        assert!(
            html.contains("#080a0f"),
            "missing dark theme background color"
        );
    }

    #[test]
    fn html_contains_matrix_tab_navigation() {
        let report = minimal_report();
        let html = render_coupling_html(&report);
        assert!(
            html.contains("tab-graph") && html.contains("tab-matrix"),
            "missing tab navigation containers"
        );
    }

    #[test]
    fn html_contains_dimension_filter_checkboxes() {
        let report = minimal_report();
        let html = render_coupling_html(&report);
        assert!(
            html.contains("type=\"checkbox\""),
            "missing dimension filter checkboxes"
        );
    }

    #[test]
    fn html_contains_heatmap_color_function() {
        let report = minimal_report();
        let html = render_coupling_html(&report);
        // JS must have a function that maps scores to heatmap colors
        assert!(
            html.contains("heatmapColor") || html.contains("cellColor"),
            "missing heatmap color mapping function in JS"
        );
    }
}
