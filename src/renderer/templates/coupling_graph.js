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
