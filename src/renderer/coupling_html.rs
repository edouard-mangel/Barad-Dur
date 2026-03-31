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

const CSS: &str = r#"
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
body {
  background: #080a0f;
  color: #e2e8f0;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  font-size: 14px;
  height: 100vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
header {
  background: #0d1117;
  border-bottom: 1px solid #1e293b;
  padding: 12px 24px;
  flex-shrink: 0;
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
  flex: 1;
  position: relative;
  overflow: hidden;
}
#graph svg {
  width: 100%;
  height: 100%;
  cursor: grab;
}
#graph svg.panning {
  cursor: grabbing;
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
.tabs {
  display: flex;
  gap: 0;
  background: #0d1117;
  border-bottom: 1px solid #1e293b;
  padding: 0 24px;
  flex-shrink: 0;
}
.tab {
  background: transparent;
  border: none;
  color: #94a3b8;
  padding: 10px 20px;
  cursor: pointer;
  font-size: 14px;
  border-bottom: 2px solid transparent;
  transition: color 0.2s, border-color 0.2s;
}
.tab:hover { color: #e2e8f0; }
.tab.active {
  color: #3b82f6;
  border-bottom-color: #3b82f6;
}
.tab-content { display: none; flex: 1; overflow: hidden; }
.tab-content.active { display: flex; flex-direction: column; }
#tab-matrix, #tab-methodology { overflow-y: auto; }
.filters {
  background: #0d1117;
  padding: 8px 24px;
  display: flex;
  gap: 16px;
  border-bottom: 1px solid #1e293b;
  flex-shrink: 0;
}
.filters label {
  color: #94a3b8;
  font-size: 13px;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 4px;
}
.filters input[type="checkbox"] {
  accent-color: #3b82f6;
}
#matrix {
  padding: 24px;
  overflow-x: auto;
  overflow-y: visible;
}
#matrix table {
  border-collapse: collapse;
  margin: 0 auto;
}
#matrix th {
  padding: 6px 10px;
  color: #94a3b8;
  font-size: 12px;
  font-weight: 600;
  white-space: nowrap;
}
#matrix td {
  width: 48px;
  height: 48px;
  text-align: center;
  font-size: 11px;
  color: #e2e8f0;
  border: 1px solid #1e293b;
  position: relative;
}
#matrix td.matrix-cell {
  cursor: default;
}
#matrix td.diagonal {
  background: #0d1117;
}
.methodology {
  max-width: 700px;
  margin: 0 auto;
  padding: 32px 24px;
  line-height: 1.6;
  overflow-x: hidden;
  word-wrap: break-word;
}
.methodology h2 {
  font-size: 18px;
  font-weight: 700;
  color: #f1f5f9;
  margin-bottom: 8px;
}
.methodology .intro {
  color: #94a3b8;
  margin-bottom: 24px;
  font-size: 14px;
}
.method-section {
  margin-bottom: 28px;
  padding-bottom: 20px;
  border-bottom: 1px solid #1e293b;
}
.method-section:last-child {
  border-bottom: none;
}
.method-section h3 {
  font-size: 15px;
  font-weight: 600;
  color: #e2e8f0;
  margin-bottom: 8px;
}
.method-section p {
  color: #cbd5e1;
  margin-bottom: 8px;
  font-size: 13px;
}
.method-section ol, .method-section ul {
  color: #cbd5e1;
  margin: 8px 0 12px 20px;
  font-size: 13px;
}
.method-section li {
  margin-bottom: 4px;
}
.method-section .formula {
  background: #0d1117;
  border: 1px solid #1e293b;
  border-radius: 4px;
  padding: 8px 12px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 12px;
  color: #7dd3fc;
  margin: 8px 0;
  overflow-x: auto;
  white-space: nowrap;
}
.legend {
  position: absolute;
  bottom: 16px;
  left: 16px;
  background: rgba(13,17,23,0.92);
  border: 1px solid #1e293b;
  border-radius: 8px;
  padding: 12px 16px;
  font-size: 12px;
  color: #94a3b8;
  z-index: 50;
  max-width: 260px;
}
.legend h3 {
  font-size: 12px;
  font-weight: 600;
  color: #e2e8f0;
  margin-bottom: 8px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}
.legend-section {
  margin-bottom: 10px;
}
.legend-section:last-child {
  margin-bottom: 0;
}
.legend-section .label {
  font-weight: 600;
  color: #cbd5e1;
  margin-bottom: 4px;
}
.legend-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 3px;
}
.legend-swatch {
  width: 24px;
  height: 4px;
  border-radius: 2px;
  flex-shrink: 0;
}
.legend-circle {
  border-radius: 50%;
  flex-shrink: 0;
  background: #3b82f6;
  border: 1.5px solid #60a5fa;
}
.matrix-legend {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0 auto 16px auto;
  justify-content: center;
  font-size: 12px;
  color: #94a3b8;
}
.matrix-legend .gradient {
  width: 180px;
  height: 12px;
  border-radius: 3px;
  border: 1px solid #1e293b;
}
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

  // SVG setup with pan/zoom support
  var container = document.getElementById('graph');
  var W = container.clientWidth || 800;
  var H = container.clientHeight || 600;
  var svgNS = 'http://www.w3.org/2000/svg';
  var svg = document.createElementNS(svgNS, 'svg');
  svg.setAttribute('width', '100%');
  svg.setAttribute('height', '100%');
  svg.setAttribute('viewBox', '0 0 ' + W + ' ' + H);
  container.appendChild(svg);

  // All graph content goes inside a transform group for pan/zoom
  var graphGroup = document.createElementNS(svgNS, 'g');
  svg.appendChild(graphGroup);

  // Pan/zoom state
  var viewX = 0, viewY = 0, viewScale = 1;
  var isPanning = false, panStartX = 0, panStartY = 0, panStartViewX = 0, panStartViewY = 0;

  function updateViewBox() {
    var vw = W / viewScale;
    var vh = H / viewScale;
    svg.setAttribute('viewBox', viewX + ' ' + viewY + ' ' + vw + ' ' + vh);
  }

  // Mouse wheel zoom — zoom toward cursor position
  container.addEventListener('wheel', function(ev) {
    ev.preventDefault();
    var rect = svg.getBoundingClientRect();
    var mouseX = ev.clientX - rect.left;
    var mouseY = ev.clientY - rect.top;

    // Convert mouse position to SVG coordinates
    var svgX = viewX + (mouseX / rect.width) * (W / viewScale);
    var svgY = viewY + (mouseY / rect.height) * (H / viewScale);

    var factor = ev.deltaY < 0 ? 1.15 : 1 / 1.15;
    var newScale = Math.max(0.2, Math.min(5, viewScale * factor));

    // Adjust viewX/viewY so the point under the cursor stays fixed
    viewX = svgX - (mouseX / rect.width) * (W / newScale);
    viewY = svgY - (mouseY / rect.height) * (H / newScale);
    viewScale = newScale;
    updateViewBox();
  }, { passive: false });

  // Pan: mousedown on SVG background (not on a node)
  svg.addEventListener('mousedown', function(ev) {
    if (dragging) return; // node drag takes priority
    isPanning = true;
    panStartX = ev.clientX;
    panStartY = ev.clientY;
    panStartViewX = viewX;
    panStartViewY = viewY;
    svg.classList.add('panning');
    ev.preventDefault();
  });

  document.addEventListener('mousemove', function(ev) {
    if (isPanning && !dragging) {
      var rect = svg.getBoundingClientRect();
      var dx = (ev.clientX - panStartX) / rect.width * (W / viewScale);
      var dy = (ev.clientY - panStartY) / rect.height * (H / viewScale);
      viewX = panStartViewX - dx;
      viewY = panStartViewY - dy;
      updateViewBox();
    }
  });

  document.addEventListener('mouseup', function() {
    if (isPanning) {
      isPanning = false;
      svg.classList.remove('panning');
    }
  });

  // Initialize positions in a circle
  var cx = W / 2, cy = H / 2;
  nodes.forEach(function(n, i) {
    var angle = (2 * Math.PI * i) / Math.max(nodes.length, 1);
    var radius = Math.min(W, H) * 0.4;
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
    return 6 + Math.sqrt(n.couplingCount) * 4;
  }

  // Create SVG elements for edges
  var edgeEls = edges.map(function(e) {
    var line = document.createElementNS(svgNS, 'line');
    line.setAttribute('stroke', edgeColor(e.score));
    line.setAttribute('stroke-width', edgeWidth(e.score));
    line.setAttribute('stroke-opacity', '0.6');
    line.setAttribute('stroke-linecap', 'round');
    graphGroup.appendChild(line);
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

    graphGroup.appendChild(g);
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
  var REPULSION = 8000;
  var SPRING_K = 0.003;
  var SPRING_REST = 250;
  var DAMPING = 0.9;
  var CENTER_PULL = 0.008;

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

  // Drag support — converts screen coordinates to SVG space via viewBox
  var dragging = null;
  nodeEls.forEach(function(ne) {
    ne.el.addEventListener('mousedown', function(ev) {
      dragging = ne.data;
      ne.el.style.cursor = 'grabbing';
      ev.stopPropagation(); // prevent pan
      ev.preventDefault();
    });
  });
  document.addEventListener('mousemove', function(ev) {
    if (!dragging) return;
    var rect = svg.getBoundingClientRect();
    // Convert screen position to SVG coordinates
    dragging.x = viewX + ((ev.clientX - rect.left) / rect.width) * (W / viewScale);
    dragging.y = viewY + ((ev.clientY - rect.top) / rect.height) * (H / viewScale);
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
    if (iterations < 500) {
      requestAnimationFrame(tick);
    }
  }
  tick();

  // === Tab navigation ===
  var tabButtons = document.querySelectorAll('.tab');
  tabButtons.forEach(function(btn) {
    btn.addEventListener('click', function() {
      tabButtons.forEach(function(b) { b.classList.remove('active'); });
      btn.classList.add('active');
      document.querySelectorAll('.tab-content').forEach(function(tc) {
        tc.classList.remove('active');
      });
      var target = document.getElementById(btn.getAttribute('data-tab'));
      if (target) target.classList.add('active');
    });
  });

  // === Dimension filtering ===
  var filterTemporal = document.getElementById('filter-temporal');
  var filterTeam = document.getElementById('filter-team');
  var filterDependency = document.getElementById('filter-dependency');

  // Compute a display score using only the checked dimensions.
  // This is an unweighted sum of the enabled dimension scores — used for
  // visual comparison only, not the original weighted combined_score.
  // Returns 0 when all checkboxes are unchecked (hides all edges).
  function computeFilteredScore(edge) {
    var score = 0;
    var count = 0;
    if (filterTemporal.checked) { score += edge.temporalScore; count++; }
    if (filterTeam.checked) { score += edge.teamScore; count++; }
    if (filterDependency.checked) { score += edge.dependencyScore; count++; }
    return count > 0 ? score : 0;
  }

  function heatmapColor(score) {
    // Map score 0-100 to green->yellow->red gradient
    if (score <= 0) return 'rgba(34,197,94,0.1)';
    var ratio = Math.min(score / 100, 1);
    var r, g;
    if (ratio < 0.5) {
      // green to yellow
      r = Math.round(255 * (ratio * 2));
      g = 200;
    } else {
      // yellow to red
      r = 255;
      g = Math.round(200 * (1 - (ratio - 0.5) * 2));
    }
    var alpha = 0.15 + ratio * 0.75;
    return 'rgba(' + r + ',' + g + ',50,' + alpha.toFixed(2) + ')';
  }

  function updateFilters() {
    // Update graph edges
    edgeEls.forEach(function(ee) {
      var filtered = computeFilteredScore(ee.data);
      ee.el.setAttribute('stroke', edgeColor(filtered));
      ee.el.setAttribute('stroke-width', edgeWidth(filtered));
      ee.el.setAttribute('stroke-opacity', filtered > 0 ? '0.6' : '0.05');
    });
    // Re-render matrix
    renderMatrix();
  }

  filterTemporal.addEventListener('change', updateFilters);
  filterTeam.addEventListener('change', updateFilters);
  filterDependency.addEventListener('change', updateFilters);

  // === Matrix / heatmap rendering ===
  function renderMatrix() {
    var container = document.getElementById('matrix');
    container.textContent = '';

    // Matrix legend: gradient bar with labels
    var legendDiv = document.createElement('div');
    legendDiv.className = 'matrix-legend';
    var labelLow = document.createElement('span');
    labelLow.textContent = '0 (independent)';
    var gradientBar = document.createElement('div');
    gradientBar.className = 'gradient';
    gradientBar.style.background = 'linear-gradient(to right, rgba(34,197,94,0.2), #f59e0b, #ef4444)';
    var labelHigh = document.createElement('span');
    labelHigh.textContent = '100 (tightly coupled)';
    legendDiv.appendChild(labelLow);
    legendDiv.appendChild(gradientBar);
    legendDiv.appendChild(labelHigh);
    container.appendChild(legendDiv);

    var desc = document.createElement('div');
    desc.style.cssText = 'text-align:center;color:#64748b;font-size:12px;margin-bottom:16px';
    desc.textContent = 'Each cell shows the coupling score between two repos. Higher scores mean changes in one repo often require changes in the other.';
    container.appendChild(desc);

    var repoNames = repos.map(function(r) { return r.name; });
    var n = repoNames.length;

    // Build score lookup: "repoA|repoB" -> edge
    var scoreLookup = {};
    edges.forEach(function(e) {
      scoreLookup[e.source + '|' + e.target] = e;
      scoreLookup[e.target + '|' + e.source] = e;
    });

    var table = document.createElement('table');

    // Header row
    var thead = document.createElement('thead');
    var headerRow = document.createElement('tr');
    var emptyTh = document.createElement('th');
    headerRow.appendChild(emptyTh);
    repoNames.forEach(function(name) {
      var th = document.createElement('th');
      th.textContent = name;
      headerRow.appendChild(th);
    });
    thead.appendChild(headerRow);
    table.appendChild(thead);

    // Body rows
    var tbody = document.createElement('tbody');
    repoNames.forEach(function(rowName, i) {
      var tr = document.createElement('tr');
      var rowHeader = document.createElement('th');
      rowHeader.textContent = rowName;
      tr.appendChild(rowHeader);

      repoNames.forEach(function(colName, j) {
        var td = document.createElement('td');
        td.className = 'matrix-cell';
        if (i === j) {
          td.className += ' diagonal';
          td.textContent = '-';
        } else {
          var key = rowName + '|' + colName;
          var edge = scoreLookup[key];
          if (edge) {
            var score = computeFilteredScore(edge);
            td.textContent = score.toFixed(1);
            td.style.background = heatmapColor(score);
          } else {
            td.textContent = '0';
            td.style.background = heatmapColor(0);
          }
        }
        tr.appendChild(td);
      });
      tbody.appendChild(tr);
    });
    table.appendChild(tbody);
    container.appendChild(table);
  }

  // Initial matrix render
  renderMatrix();
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
