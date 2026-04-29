pub const JS: &str = r#"
(function() {
  'use strict';

  var R = window.R;

  // Palette for ownership colours
  var PALETTE = [
    '#3b82f6','#10b981','#f59e0b','#ef4444','#8b5cf6',
    '#06b6d4','#ec4899','#84cc16','#f97316','#6366f1'
  ];

  /* ---- helpers ---- */
  function txt(s) { return document.createTextNode(String(s)); }

  function el(tag, attrs) {
    var node = document.createElement(tag);
    if (attrs) {
      for (var k in attrs) {
        if (k === 'style') {
          for (var s in attrs[k]) { node.style[s] = attrs[k][s]; }
        } else if (k === 'className') {
          node.className = attrs[k];
        } else if (k === 'onClick') {
          node.addEventListener('click', attrs[k]);
        } else {
          node.setAttribute(k, attrs[k]);
        }
      }
    }
    for (var i = 2; i < arguments.length; i++) {
      var child = arguments[i];
      if (child == null) continue;
      if (typeof child === 'string' || typeof child === 'number') {
        node.append(txt(child));
      } else {
        node.append(child);
      }
    }
    return node;
  }

  function svgEl(tag, attrs) {
    var node = document.createElementNS('http://www.w3.org/2000/svg', tag);
    if (attrs) {
      for (var k in attrs) {
        node.setAttribute(k, attrs[k]);
      }
    }
    for (var i = 2; i < arguments.length; i++) {
      var child = arguments[i];
      if (child == null) continue;
      node.append(child);
    }
    return node;
  }

  function scoreColor(s) {
    return s >= 71 ? '#10b981' : s >= 41 ? '#f59e0b' : '#ef4444';
  }

  function fileParts(path) {
    var idx = path.lastIndexOf('/');
    if (idx === -1) return { dir: '', name: path };
    return { dir: path.slice(0, idx + 1), name: path.slice(idx + 1) };
  }

  function scoreBar(score) {
    var wrap = el('div', { className: 'bar-wrap' });
    var fill = el('div', { className: 'bar-fill', style: {
      width: score + '%',
      background: scoreColor(score)
    }});
    wrap.append(fill);
    return wrap;
  }

  function inlineBar(pct, color) {
    var track = el('div', { className: 'track' });
    var fill = el('div', { className: 'fill', style: {
      width: Math.min(100, pct) + '%',
      background: color || '#3b82f6'
    }});
    track.append(fill);
    return track;
  }

  function chip(text, bg, fg) {
    var c = el('span', { className: 'chip', style: {
      background: bg || '#1e293b',
      color: fg || '#e2e8f0'
    }});
    c.append(txt(text));
    return c;
  }

  /* ---- SVG gauge ---- */
  function buildGauge(score) {
    var R_outer = 70, cx = 90, cy = 90;
    var startAngle = -220, endAngle = 40; // degrees, sweep 260
    var sweep = endAngle - startAngle; // 260
    var pct = score / 100;

    function polar(angleDeg, r) {
      var rad = (angleDeg - 90) * Math.PI / 180;
      return { x: cx + r * Math.cos(rad), y: cy + r * Math.sin(rad) };
    }

    function arcPath(startDeg, endDeg, r, strokeWidth) {
      var large = (endDeg - startDeg) > 180 ? 1 : 0;
      var s = polar(startDeg, r);
      var e = polar(endDeg, r);
      return 'M ' + s.x + ' ' + s.y + ' A ' + r + ' ' + r + ' 0 ' + large + ' 1 ' + e.x + ' ' + e.y;
    }

    var trackPath = arcPath(startAngle, endAngle, R_outer - 6, 12);
    var fillEnd = startAngle + sweep * pct;
    var fillPath = arcPath(startAngle, fillEnd, R_outer - 6, 12);

    var color = scoreColor(score);

    var svg = svgEl('svg', {
      class: 'gauge',
      viewBox: '0 0 180 140',
      width: '180',
      height: '140',
      style: 'display:block;margin:0 auto;'
    });

    // Track arc
    svg.append(svgEl('path', { d: trackPath, fill: 'none', stroke: '#1e293b', 'stroke-width': '12', 'stroke-linecap': 'round' }));
    // Filled arc
    if (pct > 0) {
      svg.append(svgEl('path', { d: fillPath, fill: 'none', stroke: color, 'stroke-width': '12', 'stroke-linecap': 'round' }));
    }
    // Score text
    var scoreText = svgEl('text', {
      x: String(cx), y: String(cy - 2),
      'text-anchor': 'middle',
      fill: color,
      'font-size': '32',
      'font-weight': '700',
      'font-family': '-apple-system, BlinkMacSystemFont, sans-serif'
    });
    scoreText.append(txt(String(score)));
    svg.append(scoreText);

    var labelText = svgEl('text', {
      x: String(cx), y: String(cy + 18),
      'text-anchor': 'middle',
      fill: '#64748b',
      'font-size': '10',
      'font-family': '-apple-system, BlinkMacSystemFont, sans-serif'
    });
    labelText.append(txt('/ 100'));
    svg.append(labelText);

    return svg;
  }

  /* ---- SVG radar ---- */
  function buildRadar(cats) {
    if (!cats || cats.length === 0) return el('div', { className: 'no-data' }, 'No categories');
    var size = 220, cx = 110, cy = 110, maxR = 85;
    var svg = svgEl('svg', { class: 'radar', viewBox: '0 0 220 220', width: '220', height: '220' });
    var n = cats.length;

    function point(i, val) {
      var angle = (i / n) * 2 * Math.PI - Math.PI / 2;
      var r = (val / 100) * maxR;
      return { x: cx + r * Math.cos(angle), y: cy + r * Math.sin(angle) };
    }

    // Grid rings
    [25, 50, 75, 100].forEach(function(v) {
      var pts = [];
      for (var i = 0; i < n; i++) {
        var p = point(i, v);
        pts.push(p.x + ',' + p.y);
      }
      svg.append(svgEl('polygon', {
        points: pts.join(' '),
        fill: 'none',
        stroke: '#1e293b',
        'stroke-width': '1'
      }));
    });

    // Axes
    for (var i = 0; i < n; i++) {
      var p = point(i, 100);
      svg.append(svgEl('line', {
        x1: String(cx), y1: String(cy),
        x2: String(p.x), y2: String(p.y),
        stroke: '#1e293b', 'stroke-width': '1'
      }));
    }

    // Data polygon
    var dataPts = [];
    for (var j = 0; j < n; j++) {
      var dp = point(j, cats[j].score);
      dataPts.push(dp.x + ',' + dp.y);
    }
    svg.append(svgEl('polygon', {
      points: dataPts.join(' '),
      fill: '#f59e0b22',
      stroke: '#f59e0b',
      'stroke-width': '2'
    }));

    // Labels
    for (var k = 0; k < n; k++) {
      var lp = point(k, 115);
      var anchor = lp.x < cx - 2 ? 'end' : lp.x > cx + 2 ? 'start' : 'middle';
      var labelEl = svgEl('text', {
        x: String(lp.x), y: String(lp.y),
        'text-anchor': anchor,
        fill: '#94a3b8',
        'font-size': '9',
        'font-family': '-apple-system, BlinkMacSystemFont, sans-serif'
      });
      labelEl.append(txt(cats[k].name));
      svg.append(labelEl);

      var scoreEl = svgEl('text', {
        x: String(lp.x), y: String(lp.y + 11),
        'text-anchor': anchor,
        fill: scoreColor(cats[k].score),
        'font-size': '9',
        'font-weight': '700',
        'font-family': '-apple-system, BlinkMacSystemFont, sans-serif'
      });
      scoreEl.append(txt(String(cats[k].score)));
      svg.append(scoreEl);
    }

    return svg;
  }

  /* ---- Category cards ---- */
  function buildCatCard(cat) {
    var card = el('div', { className: 'cat-card' });
    var header = el('div', { className: 'cat-header' });
    var nameEl = el('span', { className: 'cat-name' });
    nameEl.append(txt(cat.name));
    var catTip = CAT_TIPS[cat.name];
    if (catTip) nameEl.append(tipIcon(catTip));
    var right = el('div', { className: 'cat-right' });
    var scoreEl = el('span', { className: 'cat-score', style: { color: scoreColor(cat.score) } });
    scoreEl.append(txt(String(cat.score)));
    var toggle = el('span', { className: 'cat-toggle' });
    toggle.append(txt('▼'));
    right.append(scoreEl, toggle);
    header.append(nameEl, right);

    var body = el('div', { className: 'cat-body' });
    cat.metrics.forEach(function(m) {
      var row = el('div', { className: 'metric-row' });
      var nameDiv = el('div', { className: 'metric-name' });
      nameDiv.append(txt(m.name));
      var mTip = METRIC_TIPS[m.name];
      if (mTip) nameDiv.append(tipIcon(mTip));
      var rawDiv = el('div', { className: 'metric-raw' });
      rawDiv.append(txt(m.description || formatRaw(m.raw_value)));
      var scoreDiv = el('div', { className: 'metric-score', style: { color: scoreColor(m.score) } });
      scoreDiv.append(txt(String(m.score)));
      var barDiv = el('div', { style: { width: '80px' } });
      barDiv.append(scoreBar(m.score));
      row.append(nameDiv, rawDiv, scoreDiv, barDiv);
      body.append(row);
    });

    var expanded = true;
    body.style.display = '';

    header.addEventListener('click', function() {
      expanded = !expanded;
      body.style.display = expanded ? '' : 'none';
      toggle.textContent = expanded ? '▼' : '▶';
    });

    if (cat.name === 'Health') {
      body.append(buildHealthMethodology());
    }
    if (cat.name === 'Coupling') {
      body.append(buildCouplingMethodology());
    }

    card.append(header, body);
    return card;
  }

  function buildHealthMethodology() {
    var details = document.createElement('details');
    details.style.cssText = 'margin-top:12px;border-top:1px solid #1e293b;padding-top:10px;';
    var summary = document.createElement('summary');
    summary.style.cssText = 'cursor:pointer;color:#94a3b8;font-size:12px;font-weight:600;letter-spacing:0.03em;user-select:none;padding:4px 0;';
    summary.append(txt('Methodology'));
    details.append(summary);

    var wrap = el('div', { style: { fontSize: '12px', lineHeight: '1.6', color: '#94a3b8', marginTop: '8px' } });

    var metrics = [
      { name: 'Bus Factor',
        what: 'Percentage of files where a single author owns >50% of lines.',
        scoring: '<10% \u2192 100 | <25% \u2192 75 | <50% \u2192 50 | >50% \u2192 25',
        why: 'Low bus factor means critical knowledge is concentrated in too few people.' },
      { name: 'God Objects',
        what: 'Files with LOC > 500, or LOC > 300 with >15 public methods.',
        scoring: '0% \u2192 100 | \u22642% \u2192 75 | \u22648% \u2192 50 | >8% \u2192 25',
        why: 'Large classes with many responsibilities are hard to understand and change (Fowler: Large Class).' },
      { name: 'Complex Hotspots',
        what: 'Files above the 75th percentile in both cyclomatic complexity and churn.',
        scoring: '0 \u2192 100 | 1\u20132 \u2192 75 | 3\u20135 \u2192 50 | >5 \u2192 25',
        why: 'Code that is both complex and frequently changed is the highest-risk area for bugs (Tornhill).' },
      { name: 'Long Methods',
        what: 'Functions with LOC > 40 or cyclomatic complexity > 10.',
        scoring: '0% \u2192 100 | \u22645% \u2192 75 | \u226415% \u2192 50 | >15% \u2192 25',
        why: 'Long or complex functions are harder to test, understand, and maintain (Fowler: Long Method).' },
      { name: 'Code Biomarkers',
        what: 'Files with nesting depth > 4 or nesting variance > 2.0.',
        scoring: '0% \u2192 100 | \u22643% \u2192 75 | \u226410% \u2192 50 | >10% \u2192 25',
        why: 'Deeply nested code signals accumulated complexity. High variance indicates erratic structure (Tornhill: Code Biomarkers).' }
    ];

    metrics.forEach(function(m) {
      var block = el('div', { style: { marginBottom: '10px' } });
      var title = el('div', { style: { color: '#e2e8f0', fontWeight: '600', marginBottom: '2px' } });
      title.append(txt(m.name));
      block.append(title);

      var what = el('div');
      var whatLabel = el('span', { style: { color: '#64748b' } });
      whatLabel.append(txt('What: '));
      what.append(whatLabel, txt(m.what));
      block.append(what);

      var scoring = el('div');
      var scoringLabel = el('span', { style: { color: '#64748b' } });
      scoringLabel.append(txt('Scoring: '));
      var scoringCode = el('span', { style: { fontFamily: 'monospace', fontSize: '11px' } });
      scoringCode.append(txt(m.scoring));
      scoring.append(scoringLabel, scoringCode);
      block.append(scoring);

      var why = el('div');
      var whyLabel = el('span', { style: { color: '#64748b' } });
      whyLabel.append(txt('Why: '));
      why.append(whyLabel, txt(m.why));
      block.append(why);

      wrap.append(block);
    });

    details.append(wrap);
    return details;
  }

  function buildCouplingMethodology() {
    var details = document.createElement('details');
    details.style.cssText = 'margin-top:12px;border-top:1px solid #1e293b;padding-top:10px;';
    var summary = document.createElement('summary');
    summary.style.cssText = 'cursor:pointer;color:#94a3b8;font-size:12px;font-weight:600;letter-spacing:0.03em;user-select:none;padding:4px 0;';
    summary.append(txt('Methodology'));
    details.append(summary);

    var wrap = el('div', { style: { fontSize: '12px', lineHeight: '1.6', color: '#94a3b8', marginTop: '8px' } });

    var metrics = [
      { name: 'Afferent coupling (Ca)',
        what: 'Number of files that import this file (incoming). Built by parsing use / import / require statements via tree-sitter into an import graph.',
        scoring: 'Scored on median Ca across all files (including leaves with Ca\u202f=\u202f0): \u22642 \u2192 100 | \u22645 \u2192 75 | \u226410 \u2192 50 | >10 \u2192 25',
        why: 'Median rather than max — a single hub (e.g. main.rs) with many importers is expected; what matters is whether the typical file is over-imported. A 0.00 median is normal and healthy.' },
      { name: 'Efferent coupling (Ce)',
        what: 'Number of files this file imports (outgoing). Extracted from the same import graph.',
        scoring: 'Scored on median Ce across all files: \u22643 \u2192 100 | \u22646 \u2192 75 | \u226412 \u2192 50 | >12 \u2192 25',
        why: 'Most files in a well-structured codebase are leaf nodes that import few others. A 0.00 median is expected and correct.' },
      { name: 'Circular dependencies',
        what: 'File pairs that form import cycles: A\u2192B and B\u2192A (depth\u202f1), or A\u2192B\u2192C\u2192A (depth\u202f2).',
        scoring: '0 \u2192 100 | 1\u20132 \u2192 75 | 3\u20135 \u2192 50 | >5 \u2192 25',
        why: 'Cycles prevent independent compilation, testing, and deployment. They also make mental models of the codebase harder to build.' },
      { name: 'Change coupling smells',
        what: 'File pairs that co-change in \u2265\u202f50% of their commits AND live in different top-level components (detected by directory depth).',
        scoring: '0 \u2192 100 | 1\u20132 \u2192 75 | 3\u20135 \u2192 50 | >5 \u2192 25',
        why: 'Cross-boundary co-change is a structural red flag: two files that always change together but belong to different modules suggest a hidden dependency that should be made explicit.' }
    ];

    metrics.forEach(function(m) {
      var block = el('div', { style: { marginBottom: '10px' } });
      var title = el('div', { style: { color: '#e2e8f0', fontWeight: '600', marginBottom: '2px' } });
      title.append(txt(m.name));
      block.append(title);

      var what = el('div');
      var whatLabel = el('span', { style: { color: '#64748b' } });
      whatLabel.append(txt('What: '));
      what.append(whatLabel, txt(m.what));
      block.append(what);

      var scoring = el('div');
      var scoringLabel = el('span', { style: { color: '#64748b' } });
      scoringLabel.append(txt('Scoring: '));
      var scoringCode = el('span', { style: { fontFamily: 'monospace', fontSize: '11px' } });
      scoringCode.append(txt(m.scoring));
      scoring.append(scoringLabel, scoringCode);
      block.append(scoring);

      var why = el('div');
      var whyLabel = el('span', { style: { color: '#64748b' } });
      whyLabel.append(txt('Why: '));
      why.append(whyLabel, txt(m.why));
      block.append(why);

      wrap.append(block);
    });

    details.append(wrap);
    return details;
  }

  function formatRaw(rv) {
    if (rv == null) return '';
    if (typeof rv === 'object') {
      var keys = Object.keys(rv);
      if (keys.length === 1) {
        var k = keys[0];
        var v = rv[k];
        if (k === 'List') return Array.isArray(v) ? v.join(', ') : String(v);
        if (k === 'Float' || k === 'Percentage') return Number(v).toFixed(2);
        return String(v);
      }
    }
    return String(rv);
  }

  /* ---- Top actions ---- */
  function buildActions(actions) {
    var section = el('div', { className: 'actions-section' });
    var heading = el('div', { style: { marginBottom: '8px' } });
    var h = el('span', { className: 'label' });
    h.append(txt('Top Recommendations'));
    heading.append(h);
    section.append(heading);
    if (!actions || actions.length === 0) {
      var none = el('div', { style: { color: '#64748b', padding: '8px 0', fontSize: '13px' } });
      none.append(txt('No recommendations — all metrics look good!'));
      section.append(none);
      return section;
    }
    actions.forEach(function(a, i) {
      var item = el('div', { className: 'action-item' });
      var num = el('div', { className: 'action-num' });
      num.append(txt(String(i + 1)));
      var actionObj = typeof a === 'string' ? { text: a } : a;
      var textEl = el('div');
      if (actionObj.target_tab) {
        var link = el('a', {
          className: 'action-link',
          style: { cursor: 'pointer' }
        });
        link.setAttribute('href', '#');
        link.append(txt(actionObj.text));
        link.addEventListener('click', function(e) {
          e.preventDefault();
          if (window.__switchToTab) {
            window.__switchToTab(actionObj.target_tab, actionObj.sort_by || null);
          }
        });
        textEl.append(link);
      } else {
        textEl.append(txt(actionObj.text));
      }
      item.append(num, textEl);
      section.append(item);
    });
    return section;
  }

  /* ---- Remote meta ---- */
  function buildRemoteMeta(meta) {
    if (!meta) return null;
    var card = el('div', { className: 'remote-card' });
    var heading = el('div', { style: { marginBottom: '10px' } });
    var h = el('span', { className: 'label' });
    h.append(txt('Remote'));
    heading.append(h);
    card.append(heading);

    function row(label, value) {
      if (value == null) return;
      var r = el('div', { className: 'remote-item' });
      var lEl = el('span', { style: { color: '#64748b' } });
      lEl.append(txt(label));
      var vEl = el('span', { style: { fontWeight: '600' } });
      vEl.append(txt(String(value)));
      r.append(lEl, vEl);
      card.append(r);
    }

    row('URL', meta.url);
    row('Stars', meta.stars != null ? '★ ' + meta.stars : null);
    row('Language', meta.language);
    row('Open Issues', meta.open_issues != null ? meta.open_issues : null);
    if (meta.description) {
      var desc = el('div', { style: { marginTop: '8px', color: '#94a3b8', fontSize: '12px', lineHeight: '1.5' } });
      desc.append(txt(meta.description));
      card.append(desc);
    }
    return card;
  }

  /* ---- Tab info banners ---- */
  function buildTabInfo(title, description, scoreHints) {
    var info = el('div', { className: 'tab-info' });
    var t = el('div', { className: 'tab-info-title' });
    t.append(txt(title));
    info.append(t);
    var d = el('div');
    d.append(txt(description));
    info.append(d);
    if (scoreHints && scoreHints.length > 0) {
      var hints = el('div', { className: 'score-hint' });
      scoreHints.forEach(function(h) {
        var s = el('span');
        var dot = el('span', { className: 'dot', style: { background: h.color } });
        s.append(dot, txt(h.label));
        hints.append(s);
      });
      info.append(hints);
    }
    return info;
  }

  function buildExplainer(summaryText, sections) {
    var details = el('details', { className: 'explainer' });
    var summary = document.createElement('summary');
    summary.append(txt(summaryText));
    details.append(summary);
    var body = el('div', { className: 'explainer-body' });
    sections.forEach(function(s) {
      var h = el('h4');
      h.append(txt(s.heading));
      body.append(h);
      if (s.items) {
        var ul = el('ul');
        s.items.forEach(function(item) {
          var li = el('li');
          li.append(txt(item));
          ul.append(li);
        });
        body.append(ul);
      }
      if (s.text) {
        var p = el('div', { style: { marginTop: '4px' } });
        p.append(txt(s.text));
        body.append(p);
      }
    });
    details.append(body);
    return details;
  }

  var defaultScoreHints = [
    { color: '#ef4444', label: '0\u201339 Critical' },
    { color: '#f59e0b', label: '40\u201369 Needs work' },
    { color: '#22c55e', label: '70\u2013100 Healthy' }
  ];

  /* ---- Shared floating tooltip singleton ---- */
  var _floatingTip = el('div', { className: 'cp-tooltip' });
  document.body.append(_floatingTip);

  function showFloatingTip(icon, text) {
    _floatingTip.replaceChildren();
    _floatingTip.append(txt(text));
    _floatingTip.style.display = 'block';

    function onMove(e) {
      var vw = window.innerWidth, vh = window.innerHeight;
      var tw = _floatingTip.offsetWidth, th = _floatingTip.offsetHeight;
      var x = e.clientX + 14;
      var y = e.clientY - th / 2;
      if (x + tw > vw - 8) x = e.clientX - tw - 14;
      if (y < 8) y = 8;
      if (y + th > vh - 8) y = vh - th - 8;
      _floatingTip.style.left = x + 'px';
      _floatingTip.style.top  = y + 'px';
    }
    function onLeave() {
      _floatingTip.style.display = 'none';
      icon.removeEventListener('mousemove', onMove);
      icon.removeEventListener('mouseleave', onLeave);
    }
    icon.addEventListener('mousemove', onMove);
    icon.addEventListener('mouseleave', onLeave);
  }

  function tipIcon(text) {
    var icon = el('span', { className: 'th-tip' });
    icon.append(txt('?'));
    icon.addEventListener('mouseenter', function() { showFloatingTip(icon, text); });
    return icon;
  }

  function thWithTip(label, tipText) {
    var th = el('th');
    th.append(txt(label));
    if (tipText) th.append(tipIcon(tipText));
    return th;
  }

  /* ---- Metric & category tooltip definitions ---- */
  var CAT_TIPS = {
    'Health':     'Code quality and maintainability indicators — 35% of the overall score.',
    'Team':       'Team knowledge spread, activity, and collaboration health — 10% of the overall score.',
    'Evolution':  'How the codebase is growing and changing over time — 20% of the overall score.',
    'Git Hygiene':'Commit discipline, message quality, and history cleanliness — 15% of the overall score.',
    'Coupling':   'Structural and change-based coupling between modules — 20% of the overall score.',
    'Dependencies': 'Dependency freshness, vulnerability exposure, and licence risk (scored separately when --deps is used).'
  };

  var METRIC_TIPS = {
    // Health
    'Bus factor':           'Percentage of files where a single author owns >50% of blame lines. A low score means critical knowledge is concentrated in too few people.',
    'God objects':          'Files with LOC > 500, or LOC > 300 with >15 public methods. Large classes with many responsibilities are hard to understand and change (Fowler: Large Class).',
    'Complex hotspots':     'Files above the 75th percentile in both cyclomatic complexity and churn. Code that is both complex and frequently changed is the highest-risk area for bugs (Tornhill).',
    'Long methods':         'Functions with LOC > 40 or cyclomatic complexity > 10. Long or complex functions are harder to test, understand, and maintain (Fowler: Long Method).',
    'Code biomarkers':      'Files with nesting depth > 4 or nesting variance > 2.0. Deeply nested code signals accumulated complexity; high variance indicates erratic structure (Tornhill: Code Biomarkers).',
    'Churn-ownership risk': 'Frequently changed files that are also solo-owned. A single person responsible for high-churn code is both a knowledge risk and a review bottleneck.',
    // Team
    'Knowledge distribution':  'How evenly commit knowledge is spread across contributors. Concentration in one or two people is a bus-factor risk for the whole team.',
    'Contributor activity':    'Percentage of contributors who committed in the last 3 months. High churn means accumulated context is regularly lost.',
    'Ownership clarity':       'Percentage of files with a clear owner (one author > 50% of blame lines). Clear ownership improves accountability and review quality.',
    'Collaboration patterns':  'How many pairs of authors co-modify the same files. Isolated contributors indicate knowledge silos and single points of failure.',
    'Merge patterns':          'Ratio of merge commits and history irregularities. Excessive merges obscure history; very few may mean force-pushes that hide context.',
    // Evolution
    'Growth trend':      'Net file and line additions in the analysis window. Rapid unchecked growth can outpace review capacity and increase maintenance burden.',
    'Refactoring ratio': 'Percentage of commits that invest in structure (refactor / clean / improve keywords). A low ratio means technical debt is accumulating without dedicated paydown.',
    'Code age':          'Median age of code weighted by lines. Very old untouched code may be stale or dangerously stable \u2014 worth verifying it is still intentional.',
    'Commit cadence':    'Average commits per day. Irregular cadence (bursts then silence) can signal integration problems or batch-and-dump workflows.',
    // Git Hygiene
    'Commit message quality': 'Percentage of commits with meaningful messages (\u2265 20 chars). Conventional commit format (feat:, fix:, chore:) scores higher.',
    'History cleanliness':    'Presence of merge commits, octopus merges, and empty messages. A clean history makes git bisect, blame, and revert reliable.',
    'Gitignore coverage':     'Whether build artefacts, OS metadata, or credentials are tracked in git. Tracked artefacts bloat the repo and can expose sensitive data.',
    'Firefighting ratio':     'Percentage of commits containing fix / bug / hotfix / urgent keywords. A high ratio means the team is mostly reacting to failures rather than building.',
    // Coupling
    'Afferent coupling':      'Incoming dependencies (Ca) — how many files import this one. Detected via import graph built from use/import/require statements. Scored on the median Ca across ALL files, including the majority that have zero incoming deps. A value of 0.00 is normal and healthy for most repos. Scoring: median \u22642 \u2192 100, \u22645 \u2192 75, \u226410 \u2192 50, >10 \u2192 25.',
    'Efferent coupling':      'Outgoing dependencies (Ce) — how many files this one imports. Scored on the median Ce across ALL files. Most healthy codebases have median Ce near 0 because most files are leaf nodes that import few others. Scoring: median \u22643 \u2192 100, \u22646 \u2192 75, \u226412 \u2192 50, >12 \u2192 25.',
    'Circular dependencies':  'Files that mutually depend on each other: A\u2192B and B\u2192A (depth 1), or A\u2192B\u2192C\u2192A (depth 2). Cycles break independent deployment, testing, and understanding of component boundaries. Scoring: 0 \u2192 100, 1\u20132 \u2192 75, 3\u20135 \u2192 50, >5 \u2192 25.',
    'Change coupling smells': 'File pairs that co-change in \u2265 threshold% of commits AND live in different top-level components. Signals hidden cross-module dependencies that violate component boundaries. Scoring: 0 \u2192 100, 1\u20132 \u2192 75, 3\u20135 \u2192 50, >5 \u2192 25.'
  };
"#;
