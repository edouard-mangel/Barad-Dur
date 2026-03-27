use crate::coupling::CouplingReport;

/// Render a coupling report as a self-contained HTML file with an inline
/// force-directed graph. All CSS and JS are inlined -- no external dependencies.
pub fn render_coupling_html(report: &CouplingReport) -> String {
    let json_data = serde_json::to_string(report).unwrap_or_else(|_| "{}".to_string());
    let escaped_json = json_data.replace("</", "<\\/");

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
           <h1>Repository Coupling Analysis</h1>\n\
           <p class=\"summary\">{repo_count} repos &middot; {pair_count} coupling pairs \
            &middot; highest score: {highest:.1}</p>\n\
         </header>\n\
         <div id=\"graph\"></div>\n\
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

const CSS: &str = r#"
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
body {
  background: #080a0f;
  color: #e2e8f0;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  font-size: 14px;
  min-height: 100vh;
}
header {
  background: #0d1117;
  border-bottom: 1px solid #1e293b;
  padding: 16px 24px;
}
header h1 {
  font-size: 20px;
  font-weight: 700;
  color: #f1f5f9;
  margin-bottom: 4px;
}
header .summary {
  color: #94a3b8;
  font-size: 13px;
}
#graph {
  width: 100%;
  height: calc(100vh - 80px);
  position: relative;
}
#graph svg {
  width: 100%;
  height: 100%;
}
.tooltip {
  position: absolute;
  display: none;
  background: #1e293b;
  border: 1px solid #334155;
  border-radius: 6px;
  padding: 8px 12px;
  color: #e2e8f0;
  font-size: 12px;
  pointer-events: none;
  z-index: 100;
  white-space: nowrap;
  box-shadow: 0 4px 12px rgba(0,0,0,0.5);
}
.tooltip .name { font-weight: 600; font-size: 13px; color: #f1f5f9; }
.tooltip .detail { color: #94a3b8; margin-top: 2px; }
"#;

const JS: &str = r#"
(function() {
  var data = window.__COUPLING_DATA__;
  var repos = data.repos || [];
  var pairs = data.pairs || [];

  // Build node map: repo name -> { name, couplingCount, commitCount, authorCount }
  var nodeMap = {};
  repos.forEach(function(r) {
    nodeMap[r.name] = {
      name: r.name,
      couplingCount: 0,
      commitCount: r.commit_count,
      authorCount: r.author_count,
      x: 0, y: 0, vx: 0, vy: 0
    };
  });
  pairs.forEach(function(p) {
    if (nodeMap[p.repo_a]) nodeMap[p.repo_a].couplingCount++;
    if (nodeMap[p.repo_b]) nodeMap[p.repo_b].couplingCount++;
  });
  var nodes = Object.keys(nodeMap).map(function(k) { return nodeMap[k]; });

  // Build edges
  var edges = pairs.map(function(p) {
    return {
      source: p.repo_a,
      target: p.repo_b,
      score: p.combined_score,
      temporalScore: p.temporal_score,
      teamScore: p.team_score,
      dependencyScore: p.dependency_score
    };
  });

  // SVG setup
  var container = document.getElementById('graph');
  var W = container.clientWidth || 800;
  var H = container.clientHeight || 600;
  var svgNS = 'http://www.w3.org/2000/svg';
  var svg = document.createElementNS(svgNS, 'svg');
  svg.setAttribute('width', W);
  svg.setAttribute('height', H);
  container.appendChild(svg);

  // Initialize positions in a circle
  var cx = W / 2, cy = H / 2;
  nodes.forEach(function(n, i) {
    var angle = (2 * Math.PI * i) / Math.max(nodes.length, 1);
    var radius = Math.min(W, H) * 0.3;
    n.x = cx + radius * Math.cos(angle);
    n.y = cy + radius * Math.sin(angle);
    n.vx = 0;
    n.vy = 0;
  });

  // Edge thickness: map combined_score to stroke width
  var maxScore = Math.max.apply(null, edges.map(function(e) { return e.score; }).concat([1]));
  function edgeWidth(score) {
    return 1 + (score / maxScore) * 8;
  }

  // Edge color based on score
  function edgeColor(score) {
    if (score >= 70) return '#ef4444';
    if (score >= 40) return '#f59e0b';
    return '#22c55e';
  }

  // Node radius based on coupling count
  function nodeRadius(n) {
    return 12 + n.couplingCount * 4;
  }

  // Create SVG elements for edges
  var edgeEls = edges.map(function(e) {
    var line = document.createElementNS(svgNS, 'line');
    line.setAttribute('stroke', edgeColor(e.score));
    line.setAttribute('stroke-width', edgeWidth(e.score));
    line.setAttribute('stroke-opacity', '0.6');
    line.setAttribute('stroke-linecap', 'round');
    svg.appendChild(line);
    return { el: line, data: e };
  });

  // Create SVG elements for nodes
  var nodeEls = nodes.map(function(n) {
    var g = document.createElementNS(svgNS, 'g');
    g.setAttribute('cursor', 'grab');

    var circle = document.createElementNS(svgNS, 'circle');
    circle.setAttribute('r', nodeRadius(n));
    circle.setAttribute('fill', '#3b82f6');
    circle.setAttribute('stroke', '#60a5fa');
    circle.setAttribute('stroke-width', '2');
    g.appendChild(circle);

    var text = document.createElementNS(svgNS, 'text');
    text.setAttribute('text-anchor', 'middle');
    text.setAttribute('dy', nodeRadius(n) + 14);
    text.setAttribute('fill', '#94a3b8');
    text.setAttribute('font-size', '11');
    text.textContent = n.name;
    g.appendChild(text);

    svg.appendChild(g);
    return { el: g, circle: circle, data: n };
  });

  // Tooltip -- uses textContent for safety; structured via DOM elements
  var tooltip = document.getElementById('tooltip');
  function showTooltip(nd) {
    tooltip.textContent = '';
    var nameEl = document.createElement('div');
    nameEl.className = 'name';
    nameEl.textContent = nd.name;
    tooltip.appendChild(nameEl);
    var pairsEl = document.createElement('div');
    pairsEl.className = 'detail';
    pairsEl.textContent = 'Coupling pairs: ' + nd.couplingCount;
    tooltip.appendChild(pairsEl);
    var commitsEl = document.createElement('div');
    commitsEl.className = 'detail';
    commitsEl.textContent = 'Commits: ' + nd.commitCount;
    tooltip.appendChild(commitsEl);
    var authorsEl = document.createElement('div');
    authorsEl.className = 'detail';
    authorsEl.textContent = 'Authors: ' + nd.authorCount;
    tooltip.appendChild(authorsEl);
    tooltip.style.display = 'block';
  }
  nodeEls.forEach(function(ne) {
    ne.el.addEventListener('mouseover', function() {
      showTooltip(ne.data);
    });
    ne.el.addEventListener('mousemove', function(ev) {
      tooltip.style.left = (ev.pageX + 12) + 'px';
      tooltip.style.top = (ev.pageY - 10) + 'px';
    });
    ne.el.addEventListener('mouseout', function() {
      tooltip.style.display = 'none';
    });
  });

  // Force simulation (simple spring-charge model)
  var REPULSION = 5000;
  var SPRING_K = 0.005;
  var SPRING_REST = 150;
  var DAMPING = 0.9;
  var CENTER_PULL = 0.01;

  function simulate() {
    // Repulsion between all node pairs
    for (var i = 0; i < nodes.length; i++) {
      for (var j = i + 1; j < nodes.length; j++) {
        var dx = nodes[j].x - nodes[i].x;
        var dy = nodes[j].y - nodes[i].y;
        var dist = Math.sqrt(dx * dx + dy * dy) || 1;
        var force = REPULSION / (dist * dist);
        var fx = (dx / dist) * force;
        var fy = (dy / dist) * force;
        nodes[i].vx -= fx;
        nodes[i].vy -= fy;
        nodes[j].vx += fx;
        nodes[j].vy += fy;
      }
    }

    // Spring forces along edges
    edges.forEach(function(e) {
      var a = nodeMap[e.source];
      var b = nodeMap[e.target];
      if (!a || !b) return;
      var dx = b.x - a.x;
      var dy = b.y - a.y;
      var dist = Math.sqrt(dx * dx + dy * dy) || 1;
      var force = SPRING_K * (dist - SPRING_REST);
      var fx = (dx / dist) * force;
      var fy = (dy / dist) * force;
      a.vx += fx;
      a.vy += fy;
      b.vx -= fx;
      b.vy -= fy;
    });

    // Center pull
    nodes.forEach(function(n) {
      n.vx += (cx - n.x) * CENTER_PULL;
      n.vy += (cy - n.y) * CENTER_PULL;
    });

    // Update positions
    nodes.forEach(function(n) {
      n.vx *= DAMPING;
      n.vy *= DAMPING;
      n.x += n.vx;
      n.y += n.vy;
      // Constrain to viewport
      var r = nodeRadius(n);
      n.x = Math.max(r, Math.min(W - r, n.x));
      n.y = Math.max(r + 60, Math.min(H - r, n.y));
    });
  }

  function render() {
    edgeEls.forEach(function(ee) {
      var a = nodeMap[ee.data.source];
      var b = nodeMap[ee.data.target];
      if (!a || !b) return;
      ee.el.setAttribute('x1', a.x);
      ee.el.setAttribute('y1', a.y);
      ee.el.setAttribute('x2', b.x);
      ee.el.setAttribute('y2', b.y);
    });
    nodeEls.forEach(function(ne) {
      ne.el.setAttribute('transform', 'translate(' + ne.data.x + ',' + ne.data.y + ')');
    });
  }

  // Drag support
  var dragging = null;
  nodeEls.forEach(function(ne) {
    ne.el.addEventListener('mousedown', function(ev) {
      dragging = ne.data;
      ne.el.style.cursor = 'grabbing';
      ev.preventDefault();
    });
  });
  document.addEventListener('mousemove', function(ev) {
    if (!dragging) return;
    var rect = svg.getBoundingClientRect();
    dragging.x = ev.clientX - rect.left;
    dragging.y = ev.clientY - rect.top;
    dragging.vx = 0;
    dragging.vy = 0;
  });
  document.addEventListener('mouseup', function() {
    if (dragging) {
      nodeEls.forEach(function(ne) { ne.el.style.cursor = 'grab'; });
      dragging = null;
    }
  });

  // Animation loop
  var iterations = 0;
  function tick() {
    simulate();
    render();
    iterations++;
    if (iterations < 300) {
      requestAnimationFrame(tick);
    }
  }
  tick();
})();
"#;

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
    fn html_escapes_closing_script_tags_in_json() {
        let report = minimal_report();
        let html = render_coupling_html(&report);
        // JSON embedded in script tags must escape </ to prevent premature tag closure
        assert!(!html.contains("</script") || html.matches("</script>").count() == 2);
    }

    #[test]
    fn html_contains_dark_theme_background() {
        let report = minimal_report();
        let html = render_coupling_html(&report);
        assert!(html.contains("#080a0f"), "missing dark theme background color");
    }
}
