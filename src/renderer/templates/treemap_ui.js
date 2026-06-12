
  /* ---- Treemap tab UI ---- */

  function buildTreemapTab() {
    var files = (R.file_hotspots || []).filter(function(f) { return f.loc >= 5; });
    if (files.length === 0) {
      var d = el('div', { className: 'no-data' });
      d.append(txt('No file data available for treemap.'));
      return d;
    }

    var capped = false;
    if (files.length > 2000) {
      files = files.slice().sort(function(a, b) { return b.loc - a.loc; }).slice(0, 2000);
      capped = true;
    }

    var fileMap = {};
    var maxCC = 0, maxChurn = 0;
    files.forEach(function(f) {
      fileMap[f.path] = f;
      if (f.cyclomatic_complexity > maxCC) maxCC = f.cyclomatic_complexity;
      if (f.churn_count > maxChurn) maxChurn = f.churn_count;
    });

    var ageMap = {};
    var maxAge = 0;
    (R.file_ages || []).forEach(function(a) {
      ageMap[a.path] = a;
      if (a.days_since_modified > maxAge) maxAge = a.days_since_modified;
    });

    var ownerMap = {};
    var authorIndex = {};
    var authorCount = 0;
    (R.author_ownership || []).forEach(function(o) {
      ownerMap[o.path] = o;
      (o.authors || []).forEach(function(a) {
        if (!(a.name in authorIndex)) {
          authorIndex[a.name] = authorCount++;
        }
      });
    });

    var tree = buildFileTree(files);
    var currentRoot = tree;
    var navStack = [];
    var selectedPath = null;

    var container = el('div');
    container.append(buildTabInfo(
      'Treemap — Spatial view of the codebase',
      'Each rectangle (or circle) represents a file — size is proportional to lines of code, color reflects the selected metric. Directories are nested containers you can click to drill into. Use the dropdown to switch between hotspot score, complexity, churn, file age, or top contributor coloring. This view helps you spot large, problematic files at a glance.',
      [
        { color: '#22c55e', label: 'Green — low metric value (good)' },
        { color: '#f59e0b', label: 'Yellow — moderate (watch)' },
        { color: '#ef4444', label: 'Red — high metric value (act)' }
      ]
    ));

    // Controls
    var controls = el('div', { className: 'tm-controls' });
    var selectLabel = el('span', { className: 'label' });
    selectLabel.append(txt('Color by'));
    var select = el('select', { className: 'tm-select', id: 'tm-metric-select' });
    var metricKeys = ['hotspot', 'complexity', 'churn', 'age', 'owner'];
    metricKeys.forEach(function(k) {
      var opt = el('option', { value: k });
      opt.append(txt(metricScales[k].label));
      select.append(opt);
    });
    var breadcrumb = el('div', { className: 'tm-breadcrumb', id: 'tm-breadcrumb' });
    // Layout toggle
    var layoutMode = 'rect';
    var layoutToggle = el('div', { className: 'tm-layout-toggle', id: 'tm-layout-toggle' });
    var btnRect = el('button', { className: 'tm-layout-btn active', 'data-mode': 'rect' });
    btnRect.append(txt('▦ Rectangles'));
    var btnCircle = el('button', { className: 'tm-layout-btn', 'data-mode': 'circle' });
    btnCircle.append(txt('○ Circles'));
    layoutToggle.append(btnRect, btnCircle);

    function setLayoutMode(mode) {
      layoutMode = mode;
      layoutToggle.querySelectorAll('.tm-layout-btn').forEach(function(b) {
        b.className = 'tm-layout-btn' + (b.getAttribute('data-mode') === mode ? ' active' : '');
      });
      animateTransition();
    }
    btnRect.addEventListener('click', function() { setLayoutMode('rect'); });
    btnCircle.addEventListener('click', function() { setLayoutMode('circle'); });

    controls.append(selectLabel, select, layoutToggle, breadcrumb);
    container.append(controls);

    if (capped) {
      var note = el('div', { style: { color: '#f59e0b', fontSize: '12px', padding: '4px 0' } });
      note.append(txt('Showing top 2000 files by LOC.'));
      container.append(note);
    }

    // SVG + detail panel wrapper
    var svgW = 960, svgH = 600;
    var tmWrap = el('div', { className: 'tm-wrap', style: { height: svgH + 'px' } });
    var svg = svgEl('svg', { id: 'tm-svg', viewBox: '0 0 ' + svgW + ' ' + svgH, width: '100%', height: '100%', preserveAspectRatio: 'xMidYMid meet', style: 'display:block;' });
    tmWrap.append(svg);

    // Detail panel (slides in from right on file click)
    var detail = el('div', { className: 'tm-detail' });
    var detailClose = el('button', { className: 'tm-detail-close' });
    detailClose.append(txt('×'));
    detail.append(detailClose);
    var detailBody = el('div');
    detail.append(detailBody);
    tmWrap.append(detail);
    container.append(tmWrap);

    detailClose.addEventListener('click', function() {
      detail.classList.remove('open');
      selectedPath = null;
      clearHighlight();
    });

    // Tooltip
    var tooltip = el('div', { className: 'tm-tooltip' });
    container.append(tooltip);

    // Hint
    var hint = el('div', { className: 'tm-hint' });
    hint.append(txt('Click a directory to zoom in · Click a file for details · Use breadcrumbs to navigate back'));
    container.append(hint);

    // Legend
    var legendDiv = el('div', { className: 'tm-legend' });
    container.append(legendDiv);

    function getMetric() { return select.value || 'hotspot'; }

    function colorForFile(f) {
      var scale = metricScales[getMetric()];
      return scale.color(f, maxCC, maxChurn, ageMap, maxAge, ownerMap, authorIndex);
    }

    function updateLegend() {
      legendDiv.replaceChildren();
      var scale = metricScales[getMetric()];
      var items = getMetric() === 'owner' ? scale.legend(authorIndex) : scale.legend();
      items.forEach(function(it) {
        var item = el('div', { className: 'tm-legend-item' });
        var swatch = el('div', { className: 'tm-legend-swatch', style: { background: it.color } });
        var lbl = el('span');
        lbl.append(txt(it.label));
        item.append(swatch, lbl);
        legendDiv.append(item);
      });
    }

    function updateBreadcrumb() {
      breadcrumb.replaceChildren();
      var rootCrumb = el('span');
      rootCrumb.append(txt('📁 /'));
      rootCrumb.addEventListener('click', function() {
        currentRoot = tree;
        navStack = [];
        animateTransition();
      });
      breadcrumb.append(rootCrumb);
      navStack.forEach(function(entry, i) {
        var sep = el('span', { className: 'tm-sep' });
        sep.append(txt('›'));
        breadcrumb.append(sep);
        var crumb = el('span');
        crumb.append(txt(entry.name));
        crumb.addEventListener('click', (function(idx) {
          return function() {
            currentRoot = navStack[idx].node;
            navStack = navStack.slice(0, idx + 1);
            animateTransition();
          };
        })(i));
        breadcrumb.append(crumb);
      });
    }

    // Animated transition: fade out, re-layout, fade in
    function animateTransition() {
      svg.style.opacity = '0.3';
      svg.style.transition = 'opacity 0.15s ease';
      setTimeout(function() {
        renderTreemap();
        svg.style.opacity = '1';
      }, 150);
    }

    function clearHighlight() {
      svg.querySelectorAll('.tm-file').forEach(function(r) {
        r.setAttribute('stroke', 'none');
        r.setAttribute('stroke-width', '0');
      });
    }

    function highlightFile(path) {
      clearHighlight();
      svg.querySelectorAll('.tm-file').forEach(function(r) {
        if (r.getAttribute('data-path') === path) {
          r.setAttribute('stroke', '#f59e0b');
          r.setAttribute('stroke-width', '2');
        }
      });
    }

    function showDetail(f) {
      selectedPath = f.path;
      highlightFile(f.path);
      detailBody.replaceChildren();
      var title = el('div', { className: 'tm-detail-title' });
      title.append(txt(f.path));
      detailBody.append(title);

      var jump = el('div', { className: 'tm-detail-links', style: { display: 'flex', gap: '6px', marginBottom: '10px' } });
      ['Hotspots', 'Graph'].forEach(function(tab) {
        var b = el('button', { className: 'chip', style: { cursor: 'pointer' } });
        b.append(txt('View in ' + tab.toLowerCase()));
        b.addEventListener('click', function() { focusFileOnTab(tab, f.path); });
        jump.append(b);
      });
      detailBody.append(jump);

      function row(label, value) {
        var r = el('div', { className: 'tm-detail-row' });
        var l = el('span');
        l.append(txt(label));
        var v = el('span');
        v.append(txt(String(value)));
        r.append(l, v);
        detailBody.append(r);
      }

      row('Lines of code', f.loc);
      row('Cyclomatic complexity', f.cyclomatic_complexity);
      row('Churn count', f.churn_count);
      row('Hotspot score', fmt(f.hotspot_score, 1));
      row('Public methods', f.public_methods);
      row('Properties', f.properties);

      var age = ageMap[f.path];
      if (age) {
        row('Days since modified', age.days_since_modified);
        if (age.last_modified) {
          row('Last modified', String(age.last_modified).slice(0, 10));
        }
      }

      var own = ownerMap[f.path];
      if (own && own.authors) {
        var ownerTitle = el('div', { style: { marginTop: '12px', marginBottom: '6px', color: '#94a3b8', fontSize: '11px', textTransform: 'uppercase', letterSpacing: '0.06em' } });
        ownerTitle.append(txt('Ownership'));
        detailBody.append(ownerTitle);
        own.authors.slice(0, 5).forEach(function(a) {
          var idx = authorIndex[a.name] != null ? authorIndex[a.name] % PALETTE.length : 0;
          var r = el('div', { className: 'tm-detail-row' });
          var nameWrap = el('span', { style: { display: 'flex', alignItems: 'center', gap: '6px' } });
          var dot = el('span', { style: { display: 'inline-block', width: '8px', height: '8px', borderRadius: '50%', background: PALETTE[idx], flexShrink: '0' } });
          var nameTxt = el('span');
          nameTxt.append(txt(a.name));
          nameWrap.append(dot, nameTxt);
          var v = el('span');
          v.append(txt(fmt(a.pct, 0) + '%'));
          r.append(nameWrap, v);
          detailBody.append(r);
        });
      }

      detail.classList.add('open');
    }

    function renderTreeNode(svgNode, node, x, y, w, h, depth) {
      if (w < 1 || h < 1) return;
      var pad = 2;
      var headerH = depth > 0 ? 18 : 0;
      var innerX = x + pad;
      var innerY = y + headerH + pad;
      var innerW = w - pad * 2;
      var innerH = h - headerH - pad * 2;
      if (innerW < 1 || innerH < 1) return;

      if (depth > 0) {
        // Directory background — clickable
        var dirBg = svgEl('rect', {
          x: String(x), y: String(y), width: String(w), height: String(h),
          fill: '#0d1117', stroke: '#1e293b', 'stroke-width': '1',
          rx: '3',
          class: 'tm-dir-bg', 'data-dir': node.name,
          style: 'cursor:pointer;'
        });
        svgNode.append(dirBg);
        // Directory label — also clickable
        if (w > 40) {
          var label = svgEl('text', {
            x: String(x + 6), y: String(y + 13),
            fill: '#94a3b8', 'font-size': '10', 'font-weight': '600', 'font-family': 'monospace',
            class: 'tm-dir-label', 'data-dir': node.name,
            style: 'cursor:pointer;pointer-events:auto;'
          });
          var maxLabelChars = Math.floor((w - 12) / 6);
          var dirLabel = node.name;
          if (dirLabel.length > maxLabelChars) dirLabel = dirLabel.slice(0, maxLabelChars - 1) + '…';
          label.append(txt(dirLabel));
          svgNode.append(label);
        }
      }

      // Collect children
      var items = [];
      var dirKeys = Object.keys(node.children);
      dirKeys.forEach(function(k) {
        var child = node.children[k];
        if (child.totalLoc > 0) {
          items.push({ size: child.totalLoc, data: { type: 'dir', node: child, name: k } });
        }
      });
      node.files.forEach(function(f) {
        if (f.loc > 0) {
          items.push({ size: f.loc, data: { type: 'file', file: f } });
        }
      });

      if (items.length === 0) return;
      var rects = squarify(items, innerX, innerY, innerW, innerH);

      rects.forEach(function(r) {
        if (r.data.type === 'file') {
          var fData = fileMap[r.data.file.path];
          var color = fData ? colorForFile(fData) : '#334155';
          var gap = 1;
          var rw = Math.max(0, r.w - gap);
          var rh = Math.max(0, r.h - gap);
          if (rw < 1 || rh < 1) return;
          var rect = svgEl('rect', {
            x: String(r.x), y: String(r.y),
            width: String(rw), height: String(rh),
            fill: color, class: 'tm-file', 'data-path': r.data.file.path,
            rx: '2',
            style: 'cursor:pointer;transition:opacity 0.15s;'
          });
          rect.addEventListener('mouseenter', function() { rect.setAttribute('opacity', '1'); });
          rect.addEventListener('mouseleave', function() { rect.setAttribute('opacity', '0.88'); });
          rect.setAttribute('opacity', '0.88');
          svgNode.append(rect);
          // File name label
          if (rw > 36 && rh > 14) {
            var textEl = svgEl('text', {
              x: String(r.x + 3), y: String(r.y + 12),
              fill: '#e2e8f0', 'font-size': '9', 'font-family': 'monospace',
              'pointer-events': 'none', opacity: '0.85'
            });
            var maxChars = Math.floor((rw - 6) / 5.5);
            var lbl = r.data.file.name;
            if (lbl.length > maxChars) lbl = lbl.slice(0, maxChars - 1) + '…';
            textEl.append(txt(lbl));
            svgNode.append(textEl);
          }
          // LOC label on larger rects
          if (rw > 50 && rh > 26) {
            var locEl = svgEl('text', {
              x: String(r.x + 3), y: String(r.y + 23),
              fill: '#94a3b8', 'font-size': '8', 'font-family': 'monospace',
              'pointer-events': 'none', opacity: '0.7'
            });
            locEl.append(txt(r.data.file.loc + ' loc'));
            svgNode.append(locEl);
          }
        } else {
          renderTreeNode(svgNode, r.data.node, r.x, r.y, r.w, r.h, depth + 1);
        }
      });
    }

    function renderCircleNode(svgNode, node, cx, cy, r, depth) {
      if (r < 2) return;

      // Draw parent circle for directories
      if (depth > 0) {
        var bg = svgEl('circle', {
          cx: String(cx), cy: String(cy), r: String(r),
          fill: '#0d1117', stroke: '#1e293b', 'stroke-width': '1',
          class: 'tm-dir-bg', 'data-dir': node.name,
          style: 'cursor:pointer;'
        });
        svgNode.append(bg);
        // Label at top of circle
        if (r > 25) {
          var label = svgEl('text', {
            x: String(cx), y: String(cy - r + 14),
            fill: '#94a3b8', 'font-size': '10', 'font-weight': '600', 'font-family': 'monospace',
            'text-anchor': 'middle',
            class: 'tm-dir-label', 'data-dir': node.name,
            style: 'cursor:pointer;pointer-events:auto;'
          });
          var maxChars = Math.floor((r * 2 - 12) / 6);
          var dirLabel = node.name;
          if (dirLabel.length > maxChars) dirLabel = dirLabel.slice(0, maxChars - 1) + '…';
          label.append(txt(dirLabel));
          svgNode.append(label);
        }
      }

      // Collect children
      var items = [];
      var dirKeys = Object.keys(node.children);
      dirKeys.forEach(function(k) {
        var child = node.children[k];
        if (child.totalLoc > 0) {
          items.push({ size: child.totalLoc, data: { type: 'dir', node: child, name: k } });
        }
      });
      node.files.forEach(function(f) {
        if (f.loc > 0) {
          items.push({ size: f.loc, data: { type: 'file', file: f } });
        }
      });

      if (items.length === 0) return;
      var innerR = depth > 0 ? r * 0.88 : r;
      var circles = circlePack(items, cx, cy, innerR);

      circles.forEach(function(c) {
        if (c.data.type === 'file') {
          var fData = fileMap[c.data.file.path];
          var color = fData ? colorForFile(fData) : '#334155';
          var circ = svgEl('circle', {
            cx: String(c.cx), cy: String(c.cy), r: String(Math.max(0, c.r - 0.5)),
            fill: color, class: 'tm-file', 'data-path': c.data.file.path,
            style: 'cursor:pointer;transition:opacity 0.15s;'
          });
          circ.addEventListener('mouseenter', function() { circ.setAttribute('opacity', '1'); });
          circ.addEventListener('mouseleave', function() { circ.setAttribute('opacity', '0.88'); });
          circ.setAttribute('opacity', '0.88');
          svgNode.append(circ);
          // Label if circle big enough
          if (c.r > 20) {
            var textEl = svgEl('text', {
              x: String(c.cx), y: String(c.cy + 1),
              fill: '#e2e8f0', 'font-size': '9', 'font-family': 'monospace',
              'pointer-events': 'none', 'text-anchor': 'middle', opacity: '0.85'
            });
            var maxC = Math.floor((c.r * 2 - 8) / 5.5);
            var lbl = c.data.file.name;
            if (lbl.length > maxC) lbl = lbl.slice(0, maxC - 1) + '…';
            textEl.append(txt(lbl));
            svgNode.append(textEl);
          }
          if (c.r > 30) {
            var locEl = svgEl('text', {
              x: String(c.cx), y: String(c.cy + 12),
              fill: '#94a3b8', 'font-size': '8', 'font-family': 'monospace',
              'pointer-events': 'none', 'text-anchor': 'middle', opacity: '0.7'
            });
            locEl.append(txt(c.data.file.loc + ' loc'));
            svgNode.append(locEl);
          }
        } else {
          renderCircleNode(svgNode, c.data.node, c.cx, c.cy, c.r, depth + 1);
        }
      });
    }

    function renderTreemap() {
      while (svg.firstChild) svg.removeChild(svg.firstChild);
      if (layoutMode === 'circle') {
        var cr = Math.min(svgW, svgH) / 2;
        renderCircleNode(svg, currentRoot, svgW / 2, svgH / 2, cr, 0);
      } else {
        renderTreeNode(svg, currentRoot, 0, 0, svgW, svgH, 0);
      }
      updateBreadcrumb();
      updateLegend();
      if (selectedPath) highlightFile(selectedPath);
    }

    // Metric change — recolor without re-layout
    select.addEventListener('change', function() {
      svg.querySelectorAll('.tm-file').forEach(function(rect) {
        var path = rect.getAttribute('data-path');
        var f = fileMap[path];
        if (f) rect.setAttribute('fill', colorForFile(f));
      });
      updateLegend();
      // Update detail panel color context
      if (selectedPath && fileMap[selectedPath]) showDetail(fileMap[selectedPath]);
    });

    // Click: directory = zoom in, file = show detail, label = zoom in
    svg.addEventListener('click', function(e) {
      var target = e.target;
      var dirName = target.getAttribute('data-dir');
      var filePath = target.getAttribute('data-path');

      if (dirName) {
        // Clicked a directory bg or label — drill down
        function findDir(node, name) {
          var keys = Object.keys(node.children);
          for (var i = 0; i < keys.length; i++) {
            if (node.children[keys[i]].name === name) return node.children[keys[i]];
            var found = findDir(node.children[keys[i]], name);
            if (found) return found;
          }
          return null;
        }
        var dirNode = findDir(currentRoot, dirName);
        if (dirNode) {
          navStack.push({ name: dirNode.name, node: dirNode });
          currentRoot = dirNode;
          animateTransition();
        }
      } else if (filePath) {
        // Clicked a file — show detail panel
        var f = fileMap[filePath];
        if (f) showDetail(f);
      }
    });

    // Hover tooltip
    svg.addEventListener('mousemove', function(e) {
      var target = e.target;
      if (target.classList && target.classList.contains('tm-file')) {
        var path = target.getAttribute('data-path');
        var f = fileMap[path];
        if (f) {
          tooltip.replaceChildren();
          tooltip.append(el('div', { style: { fontWeight: '600', marginBottom: '4px', color: '#93c5fd' } }, f.path));
          tooltip.append(el('div', null, 'LOC: ' + f.loc + '  CC: ' + f.cyclomatic_complexity + '  Churn: ' + f.churn_count));
          tooltip.append(el('div', null, 'Hotspot: ' + fmt(f.hotspot_score, 1)));
          tooltip.style.display = 'block';
          tooltip.style.left = (e.clientX + 14) + 'px';
          tooltip.style.top = (e.clientY + 14) + 'px';
        }
      } else if (target.getAttribute('data-dir')) {
        var dn = target.getAttribute('data-dir');
        tooltip.replaceChildren();
        tooltip.append(el('div', { style: { fontWeight: '600', color: '#94a3b8' } }, '📁 ' + dn));
        tooltip.append(el('div', null, 'Click to zoom in'));
        tooltip.style.display = 'block';
        tooltip.style.left = (e.clientX + 14) + 'px';
        tooltip.style.top = (e.clientY + 14) + 'px';
      } else {
        tooltip.style.display = 'none';
      }
    });

    svg.addEventListener('mouseleave', function() {
      tooltip.style.display = 'none';
    });

    // Cross-tab entry point: reset to the root view (the file may sit inside
    // a zoomed-out directory), then highlight it and open its detail panel.
    registerFileFocus('treemap', function(path) {
      var f = fileMap[path];
      if (!f) return; // beyond the top-2000 LOC cap
      navStack = [];
      currentRoot = tree;
      renderTreemap();
      showDetail(f);
    });

    renderTreemap();
    return container;
  }
