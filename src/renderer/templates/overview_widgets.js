
  /* ---- Overview widgets: gauge, radar, category cards, actions ---- */

  /* Angle → cartesian on a circle centred at (cx, cy). 0° points up. */
  function gaugePolar(cx, cy, angleDeg, r) {
    var rad = (angleDeg - 90) * Math.PI / 180;
    return { x: cx + r * Math.cos(rad), y: cy + r * Math.sin(rad) };
  }

  function gaugeArcPath(cx, cy, startDeg, endDeg, r) {
    var large = (endDeg - startDeg) > 180 ? 1 : 0;
    var s = gaugePolar(cx, cy, startDeg, r);
    var e = gaugePolar(cx, cy, endDeg, r);
    return 'M ' + s.x + ' ' + s.y + ' A ' + r + ' ' + r + ' 0 ' + large + ' 1 ' + e.x + ' ' + e.y;
  }

  function gaugeText(x, y, content, fill, size, weight) {
    var attrs = {
      x: String(x), y: String(y),
      'text-anchor': 'middle',
      fill: fill,
      'font-size': size
    };
    if (weight) attrs['font-weight'] = weight;
    attrs['font-family'] = '-apple-system, BlinkMacSystemFont, sans-serif';
    var t = svgEl('text', attrs);
    t.append(txt(content));
    return t;
  }

  /* ---- SVG gauge ---- */
  function buildGauge(score) {
    var R_outer = 70, cx = 90, cy = 90;
    var startAngle = -220, endAngle = 40; // degrees, sweep 260
    var sweep = endAngle - startAngle; // 260
    var pct = score / 100;
    var color = scoreColor(score);

    var svg = svgEl('svg', {
      class: 'gauge',
      viewBox: '0 0 180 140',
      width: '180',
      height: '140',
      style: 'display:block;margin:0 auto;'
    });

    // Track arc
    var trackPath = gaugeArcPath(cx, cy, startAngle, endAngle, R_outer - 6);
    svg.append(svgEl('path', { d: trackPath, fill: 'none', stroke: '#1e293b', 'stroke-width': '12', 'stroke-linecap': 'round' }));
    // Filled arc
    if (pct > 0) {
      var fillPath = gaugeArcPath(cx, cy, startAngle, startAngle + sweep * pct, R_outer - 6);
      svg.append(svgEl('path', { d: fillPath, fill: 'none', stroke: color, 'stroke-width': '12', 'stroke-linecap': 'round' }));
    }
    svg.append(gaugeText(cx, cy - 2, String(score), color, '32', '700'));
    svg.append(gaugeText(cx, cy + 18, '/ 100', '#64748b', '10', null));
    return svg;
  }

  /* ---- SVG radar ---- */

  /* Vertex i of the n-gon at value val (0–100), 12 o'clock first. */
  function radarPoint(cx, cy, maxR, n, i, val) {
    var angle = (i / n) * 2 * Math.PI - Math.PI / 2;
    var r = (val / 100) * maxR;
    return { x: cx + r * Math.cos(angle), y: cy + r * Math.sin(angle) };
  }

  /* 'x,y x,y …' polygon points at per-vertex values from valueAt(i). */
  function radarPolygonPoints(cx, cy, maxR, n, valueAt) {
    var pts = [];
    for (var i = 0; i < n; i++) {
      var p = radarPoint(cx, cy, maxR, n, i, valueAt(i));
      pts.push(p.x + ',' + p.y);
    }
    return pts.join(' ');
  }

  /* Category name + score at each vertex, anchored away from the centre. */
  function radarLabels(svg, cats, cx, cy, maxR) {
    for (var k = 0; k < cats.length; k++) {
      var lp = radarPoint(cx, cy, maxR, cats.length, k, 115);
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
  }

  function buildRadar(cats) {
    if (!cats || cats.length === 0) return el('div', { className: 'no-data' }, 'No categories');
    var cx = 110, cy = 110, maxR = 85;
    var svg = svgEl('svg', { class: 'radar', viewBox: '0 0 220 220', width: '220', height: '220' });
    var n = cats.length;

    // Grid rings
    [25, 50, 75, 100].forEach(function(v) {
      svg.append(svgEl('polygon', {
        points: radarPolygonPoints(cx, cy, maxR, n, function() { return v; }),
        fill: 'none',
        stroke: '#1e293b',
        'stroke-width': '1'
      }));
    });

    // Axes
    for (var i = 0; i < n; i++) {
      var p = radarPoint(cx, cy, maxR, n, i, 100);
      svg.append(svgEl('line', {
        x1: String(cx), y1: String(cy),
        x2: String(p.x), y2: String(p.y),
        stroke: '#1e293b', 'stroke-width': '1'
      }));
    }

    // Data polygon
    svg.append(svgEl('polygon', {
      points: radarPolygonPoints(cx, cy, maxR, n, function(j) { return cats[j].score; }),
      fill: '#f59e0b22',
      stroke: '#f59e0b',
      'stroke-width': '2'
    }));

    radarLabels(svg, cats, cx, cy, maxR);
    return svg;
  }

  /* ---- Category cards ---- */

  /* One metric line: name, raw description, score, score bar. */
  function catMetricRow(m) {
    var row = el('div', { className: 'metric-row' });
    var nameDiv = el('div', { className: 'metric-name' });
    nameDiv.append(txt(m.name));
    var mTip = METRIC_TIPS[m.name];
    if (mTip) nameDiv.append(tipIcon(mTip));
    var rawDiv = el('div', { className: 'metric-raw' });
    rawDiv.append(txt(m.description || formatRaw(m.raw_value)));
    // score == null: not enough data to judge this metric — show a dash
    var unscored = m.score == null;
    var scoreDiv = el('div', {
      className: 'metric-score',
      style: { color: unscored ? '#64748b' : scoreColor(m.score) }
    });
    scoreDiv.append(txt(unscored ? '—' : String(m.score)));
    if (unscored) scoreDiv.title = 'Not enough data to score this metric';
    var barDiv = el('div', { style: { width: '80px' } });
    if (!unscored) barDiv.append(scoreBar(m.score));
    row.append(nameDiv, rawDiv, scoreDiv, barDiv);
    return row;
  }

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
    cat.metrics.forEach(function(m) { body.append(catMetricRow(m)); });

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

  function buildMethodologyDetails(metrics) {
    var details = document.createElement('details');
    details.style.cssText = 'margin-top:12px;border-top:1px solid #1e293b;padding-top:10px;';
    var summary = document.createElement('summary');
    summary.style.cssText = 'cursor:pointer;color:#94a3b8;font-size:12px;font-weight:600;letter-spacing:0.03em;user-select:none;padding:4px 0;';
    summary.append(txt('Methodology'));
    details.append(summary);

    var wrap = el('div', { style: { fontSize: '12px', lineHeight: '1.6', color: '#94a3b8', marginTop: '8px' } });

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

  function buildHealthMethodology() {
    return buildMethodologyDetails([
      { name: 'Bus Factor',
        what: 'Number of active contributors required to cover 80% of attributable lines.',
        scoring: '1 → 25 | 2 → 50 | 3 → 75 | 4+ → 100',
        why: 'Low bus factor means critical knowledge is concentrated in too few people.' },
      { name: 'God Objects',
        what: 'Files with LOC > 500, or LOC > 300 with >15 public methods, or that structurally dominate the import graph as a connectivity hub.',
        scoring: 'Prevalence: 0% → 100 | ≤1% → 90 | ≤5% → 75 | ≤20% → 50 | >20% → 25 (300+ source files; blended with the count bands between 100 and 300, count bands alone below 100)',
        why: 'Large or overly central files are hard to understand and change (Fowler: Large Class).' },
      { name: 'Complex Hotspots',
        what: 'Files above the 75th percentile in both cyclomatic complexity and churn.',
        scoring: 'Prevalence: 0% → 100 | ≤1% → 90 | ≤5% → 75 | ≤20% → 50 | >20% → 25 (300+ source files; blended with the count bands between 100 and 300, count bands alone below 100)',
        why: 'Code that is both complex and frequently changed is the highest-risk area for bugs (Tornhill).' },
      { name: 'Long Methods',
        what: 'Functions with LOC > 40 or cyclomatic complexity > 10.',
        scoring: '0% → 100 | ≤5% → 75 | ≤15% → 50 | >15% → 25',
        why: 'Long or complex functions are harder to test, understand, and maintain (Fowler: Long Method).' },
      { name: 'Code Biomarkers',
        what: 'Files with nesting depth > 4 or nesting variance > 2.0.',
        scoring: '0% → 100 | ≤3% → 75 | ≤10% → 50 | >10% → 25',
        why: 'Deeply nested code signals accumulated complexity. High variance indicates erratic structure (Tornhill: Code Biomarkers).' }
    ]);
  }

  function buildCouplingMethodology() {
    return buildMethodologyDetails([
      { name: 'Afferent coupling (Ca)',
        what: 'Number of files that import this file (incoming). Built by parsing use / import / require statements via tree-sitter into an import graph.',
        scoring: 'Scored on median Ca across all files (including leaves with Ca = 0): ≤2 → 100 | ≤5 → 75 | ≤10 → 50 | >10 → 25',
        why: 'Median rather than max — a single hub (e.g. main.rs) with many importers is expected; what matters is whether the typical file is over-imported. A 0.00 median is normal and healthy.' },
      { name: 'Efferent coupling (Ce)',
        what: 'Number of files this file imports (outgoing). Extracted from the same import graph.',
        scoring: 'Scored on median Ce across all files: ≤3 → 100 | ≤6 → 75 | ≤12 → 50 | >12 → 25',
        why: 'Most files in a well-structured codebase are leaf nodes that import few others. A 0.00 median is expected and correct.' },
      { name: 'Circular dependencies',
        what: 'Production-source files that form import cycles: A→B and B→A (depth 1), or A→B→C→A (depth 2). Self-imports are ignored.',
        scoring: 'Affected-file prevalence: 0% → 100 | ≤1% → 90 | ≤5% → 75 | ≤20% → 50 | >20% → 25 (300+ source files; blended with the count bands between 100 and 300, count bands alone below 100)',
        why: 'Cycles prevent independent compilation, testing, and deployment. They also make mental models of the codebase harder to build.' },
      { name: 'Change coupling smells',
        what: 'Cross-boundary file pairs that co-change above the configured ratio; import-graph communities provide structural corroboration.',
        scoring: 'Prevalence of source files in corroborated pairs: 0% → 100 | ≤1% → 90 | ≤5% → 75 | ≤20% → 50 | >20% → 25 (300+ source files; blended with the count bands between 100 and 300, count bands alone below 100)',
        why: 'Cross-boundary co-change is a structural red flag: two files that always change together but belong to different modules suggest a hidden dependency that should be made explicit.' }
    ]);
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

  /* One numbered recommendation; deep-links to its tab when the action
     carries a target. */
  function actionRow(a, i) {
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
        document.dispatchEvent(new CustomEvent('barad:switch-tab', {
          detail: { tab: actionObj.target_tab, sortBy: actionObj.sort_by || null }
        }));
      });
      textEl.append(link);
    } else {
      textEl.append(txt(actionObj.text));
    }
    item.append(num, textEl);
    return item;
  }

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
    actions.forEach(function(a, i) { section.append(actionRow(a, i)); });
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
