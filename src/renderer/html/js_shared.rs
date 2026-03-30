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
      var rawDiv = el('div', { className: 'metric-raw' });
      rawDiv.append(txt(formatRaw(m.raw_value)));
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

    card.append(header, body);
    return card;
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
"#;
