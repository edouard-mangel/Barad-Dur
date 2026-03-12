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
        css = CSS,
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

const CSS: &str = r#"
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
body {
  background: #080a0f;
  color: #e2e8f0;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  font-size: 14px;
  min-height: 100vh;
}
.chip {
  display: inline-block;
  padding: 2px 8px;
  border-radius: 12px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}
.label {
  color: #94a3b8;
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
}
header {
  background: #0d1117;
  border-bottom: 1px solid #1e293b;
  padding: 16px 24px;
}
.header-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 12px;
}
.meta-chips {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  align-items: center;
}
.brand {
  font-size: 20px;
  font-weight: 700;
  color: #f59e0b;
  letter-spacing: -0.02em;
}
.dot {
  color: #334155;
}
.divider {
  height: 1px;
  background: #1e293b;
  margin: 0;
}
.tabs {
  display: flex;
  gap: 0;
  border-bottom: 1px solid #1e293b;
  background: #0d1117;
  padding: 0 24px;
  overflow-x: auto;
}
.tab {
  padding: 10px 18px;
  cursor: pointer;
  color: #64748b;
  font-size: 13px;
  font-weight: 500;
  border-bottom: 2px solid transparent;
  white-space: nowrap;
  background: none;
  border-top: none;
  border-left: none;
  border-right: none;
  transition: color 0.15s, border-color 0.15s;
}
.tab:hover { color: #e2e8f0; }
.tab.active { color: #f59e0b; border-bottom-color: #f59e0b; }
.tab-content { display: none; padding: 24px; }
.tab-content.active { display: block; }
.overview-grid {
  display: grid;
  grid-template-columns: 1fr 280px;
  gap: 24px;
  align-items: start;
}
@media (max-width: 768px) {
  .overview-grid { grid-template-columns: 1fr; }
}
.left-col { display: flex; flex-direction: column; gap: 16px; }
.right-col { display: flex; flex-direction: column; gap: 16px; }
.gauge-wrap {
  background: #0d1117;
  border: 1px solid #1e293b;
  border-radius: 12px;
  padding: 24px;
  text-align: center;
}
.gauge-label {
  color: #94a3b8;
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  margin-top: 8px;
}
.cat-card {
  background: #0d1117;
  border: 1px solid #1e293b;
  border-radius: 10px;
  overflow: hidden;
}
.cat-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  cursor: pointer;
  user-select: none;
}
.cat-header:hover { background: #131b2b; }
.cat-name { font-weight: 600; font-size: 14px; }
.cat-right { display: flex; align-items: center; gap: 12px; }
.cat-score { font-weight: 700; font-size: 16px; }
.cat-toggle { color: #475569; font-size: 12px; transition: transform 0.2s; }
.cat-body { border-top: 1px solid #1e293b; }
.bar-wrap {
  height: 6px;
  background: #1e293b;
  border-radius: 3px;
  overflow: hidden;
  margin-top: 4px;
}
.bar-fill {
  height: 100%;
  border-radius: 3px;
  transition: width 0.5s ease;
}
.metric-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 16px;
  border-bottom: 1px solid #0f172a;
  gap: 8px;
}
.metric-row:last-child { border-bottom: none; }
.metric-name { flex: 1; font-size: 13px; color: #cbd5e1; }
.metric-raw { color: #64748b; font-size: 12px; font-family: monospace; min-width: 60px; text-align: right; }
.metric-score { font-weight: 700; font-size: 13px; min-width: 32px; text-align: right; }
.actions-section {
  background: #0d1117;
  border: 1px solid #1e293b;
  border-radius: 10px;
  padding: 16px;
}
.action-item {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 10px 0;
  border-bottom: 1px solid #0f172a;
  font-size: 13px;
  line-height: 1.5;
}
.action-item:last-child { border-bottom: none; }
.action-num {
  color: #f59e0b;
  font-weight: 700;
  font-size: 12px;
  min-width: 20px;
  padding-top: 1px;
}
.view-card {
  background: #0d1117;
  border: 1px solid #1e293b;
  border-radius: 10px;
  overflow: hidden;
}
table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}
thead tr { background: #0f172a; }
th {
  padding: 10px 12px;
  text-align: left;
  color: #64748b;
  font-weight: 600;
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  white-space: nowrap;
  border-bottom: 1px solid #1e293b;
}
td {
  padding: 9px 12px;
  border-bottom: 1px solid #0f172a;
  vertical-align: middle;
}
tbody tr:last-child td { border-bottom: none; }
tbody tr:hover { background: #0f172a; }
.file-name { font-family: monospace; font-size: 12px; color: #93c5fd; }
.file-dir { color: #475569; font-size: 11px; }
.inline-bar { width: 100px; }
.track {
  height: 6px;
  background: #1e293b;
  border-radius: 3px;
  overflow: hidden;
}
.fill {
  height: 100%;
  border-radius: 3px;
}
.th-sort { cursor: pointer; }
.th-sort:hover { color: #e2e8f0; }
.th-sort.active-sort { color: #f59e0b; }
.own-bar {
  display: flex;
  height: 8px;
  border-radius: 4px;
  overflow: hidden;
  width: 140px;
  gap: 1px;
}
.own-seg { height: 100%; }
.own-top { font-size: 11px; color: #94a3b8; font-family: monospace; }
.legend {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 16px;
  padding: 12px 16px;
  border-top: 1px solid #1e293b;
  background: #080a0f;
}
.legend-item { display: flex; align-items: center; gap: 6px; font-size: 11px; color: #94a3b8; }
.legend-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
.remote-card {
  background: #0d1117;
  border: 1px solid #1e293b;
  border-radius: 10px;
  padding: 16px;
}
.remote-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 0;
  border-bottom: 1px solid #0f172a;
  font-size: 13px;
}
.remote-item:last-child { border-bottom: none; }
svg.scatter { display: block; width: 100%; }
svg.radar { display: block; margin: 0 auto; }
.no-data {
  text-align: center;
  color: #475569;
  padding: 48px;
  font-size: 13px;
}
.hotspot-wrap {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
}
@media (max-width: 900px) {
  .hotspot-wrap { grid-template-columns: 1fr; }
}
.tm-controls {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 0;
  flex-wrap: wrap;
}
.tm-select {
  background: #0d1117;
  color: #e2e8f0;
  border: 1px solid #1e293b;
  border-radius: 6px;
  padding: 6px 10px;
  font-size: 13px;
  cursor: pointer;
}
.tm-select:focus { outline: 1px solid #f59e0b; }
.tm-breadcrumb {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: #94a3b8;
}
.tm-breadcrumb span {
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 4px;
}
.tm-breadcrumb span:hover { background: #1e293b; color: #e2e8f0; }
.tm-breadcrumb .tm-sep { cursor: default; color: #334155; }
.tm-breadcrumb .tm-sep:hover { background: none; color: #334155; }
.tm-tooltip {
  position: fixed;
  background: #0d1117;
  border: 1px solid #1e293b;
  border-radius: 8px;
  padding: 10px 14px;
  font-size: 12px;
  color: #e2e8f0;
  pointer-events: none;
  z-index: 1000;
  display: none;
  max-width: 320px;
  line-height: 1.6;
  box-shadow: 0 4px 12px rgba(0,0,0,0.5);
}
.tm-legend {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 16px;
  padding: 10px 0;
  font-size: 11px;
  color: #94a3b8;
}
.tm-legend-item {
  display: flex;
  align-items: center;
  gap: 5px;
}
.tm-legend-swatch {
  width: 12px;
  height: 12px;
  border-radius: 3px;
  flex-shrink: 0;
}
"#;

fn build_js() -> String {
    r#"
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
      var text = el('div');
      text.append(txt(a));
      item.append(num, text);
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

  /* ---- Hotspots tab ---- */
  function buildHotspotsTab() {
    var files = R.file_hotspots || [];
    if (files.length === 0) {
      var d = el('div', { className: 'no-data' });
      d.append(txt('No hotspot data available.'));
      return d;
    }

    var wrap = el('div', { className: 'hotspot-wrap' });

    // Scatter plot
    var plotCard = el('div', { className: 'view-card', style: { padding: '16px' } });
    var plotHeading = el('div', { style: { marginBottom: '12px' } });
    var plotH = el('span', { className: 'label' });
    plotH.append(txt('Complexity vs Churn (radius = LOC)'));
    plotHeading.append(plotH);
    plotCard.append(plotHeading);

    var maxCC = 1, maxChurn = 1, maxLOC = 1;
    files.forEach(function(f) {
      if (f.cyclomatic_complexity > maxCC) maxCC = f.cyclomatic_complexity;
      if (f.churn_count > maxChurn) maxChurn = f.churn_count;
      if (f.loc > maxLOC) maxLOC = f.loc;
    });

    var svgW = 340, svgH = 220, pad = 36;
    var scatter = svgEl('svg', {
      class: 'scatter',
      viewBox: '0 0 ' + svgW + ' ' + svgH,
      preserveAspectRatio: 'xMidYMid meet'
    });

    // Axes
    scatter.append(svgEl('line', { x1: String(pad), y1: String(pad), x2: String(pad), y2: String(svgH - pad), stroke: '#1e293b', 'stroke-width': '1' }));
    scatter.append(svgEl('line', { x1: String(pad), y1: String(svgH - pad), x2: String(svgW - pad), y2: String(svgH - pad), stroke: '#1e293b', 'stroke-width': '1' }));

    // Axis labels
    var xLabel = svgEl('text', { x: String((svgW + pad) / 2), y: String(svgH - 6), 'text-anchor': 'middle', fill: '#475569', 'font-size': '9', 'font-family': 'sans-serif' });
    xLabel.append(txt('Cyclomatic Complexity'));
    scatter.append(xLabel);

    var yLabel = svgEl('text', { x: '10', y: String(svgH / 2), 'text-anchor': 'middle', fill: '#475569', 'font-size': '9', 'font-family': 'sans-serif', transform: 'rotate(-90, 10, ' + (svgH / 2) + ')' });
    yLabel.append(txt('Churn'));
    scatter.append(yLabel);

    var plotW = svgW - pad * 2;
    var plotH2 = svgH - pad * 2;

    files.slice(0, 80).forEach(function(f) {
      var cx = pad + (f.cyclomatic_complexity / maxCC) * plotW;
      var cy = (svgH - pad) - (f.churn_count / maxChurn) * plotH2;
      var r = 4 + (f.loc / maxLOC) * 10;
      var color = scoreColor(Math.round(100 - f.hotspot_score));
      var circle = svgEl('circle', {
        cx: String(cx), cy: String(cy), r: String(r),
        fill: color, opacity: '0.7'
      });
      var titleEl = svgEl('title');
      titleEl.append(txt(fileParts(f.path).name + ' (CC:' + f.cyclomatic_complexity + ', churn:' + f.churn_count + ', LOC:' + f.loc + ')'));
      circle.append(titleEl);
      scatter.append(circle);
    });

    plotCard.append(scatter);
    wrap.append(plotCard);

    // Table
    var tableCard = el('div', { className: 'view-card' });
    var tableWrap = el('div', { style: { overflowX: 'auto' } });
    var sortCol = 'hotspot_score';
    var sortAsc = false;

    function buildTable() {
      var sorted = files.slice().sort(function(a, b) {
        var av = a[sortCol], bv = b[sortCol];
        if (typeof av === 'string') av = av.toLowerCase();
        if (typeof bv === 'string') bv = bv.toLowerCase();
        if (av < bv) return sortAsc ? -1 : 1;
        if (av > bv) return sortAsc ? 1 : -1;
        return 0;
      });

      var table = el('table');
      var thead = el('thead');
      var tr = el('tr');

      function th(label, col) {
        var t = el('th', { className: 'th-sort' + (col === sortCol ? ' active-sort' : '') });
        t.append(txt(label + (col === sortCol ? (sortAsc ? ' ▲' : ' ▼') : '')));
        t.addEventListener('click', function() {
          if (sortCol === col) { sortAsc = !sortAsc; } else { sortCol = col; sortAsc = false; }
          tableWrap.replaceChildren(buildTable());
        });
        return t;
      }

      tr.append(
        th('File', 'path'),
        th('Score', 'hotspot_score'),
        th('CC', 'cyclomatic_complexity'),
        th('Churn', 'churn_count'),
        th('LOC', 'loc')
      );
      thead.append(tr);
      table.append(thead);

      var tbody = el('tbody');
      sorted.slice(0, 50).forEach(function(f) {
        var parts = fileParts(f.path);
        var row = el('tr');
        var fileCell = el('td');
        var dirSpan = el('span', { className: 'file-dir' });
        dirSpan.append(txt(parts.dir));
        var nameSpan = el('span', { className: 'file-name' });
        nameSpan.append(txt(parts.name));
        fileCell.append(dirSpan, nameSpan);

        var scoreCell = el('td');
        var scoreVal = Math.round(f.hotspot_score);
        var scoreSpan = el('span', { style: { color: scoreColor(100 - scoreVal), fontWeight: '700' } });
        scoreSpan.append(txt(String(scoreVal)));
        scoreCell.append(scoreSpan);

        var ccCell = el('td');
        ccCell.append(txt(String(f.cyclomatic_complexity)));

        var churnCell = el('td');
        churnCell.append(txt(String(f.churn_count)));

        var locCell = el('td');
        locCell.append(txt(String(f.loc)));

        row.append(fileCell, scoreCell, ccCell, churnCell, locCell);
        tbody.append(row);
      });
      table.append(tbody);
      return table;
    }

    tableWrap.append(buildTable());
    tableCard.append(tableWrap);
    wrap.append(tableCard);
    return wrap;
  }

  /* ---- Coupling tab ---- */
  function buildCouplingTab() {
    var pairs = (R.coupling_pairs || []).slice().sort(function(a, b) {
      return b.coupling_pct - a.coupling_pct;
    });

    if (pairs.length === 0) {
      var d = el('div', { className: 'no-data' });
      d.append(txt('No temporal coupling data available.'));
      return d;
    }

    var card = el('div', { className: 'view-card' });
    var tableWrap = el('div', { style: { overflowX: 'auto' } });
    var table = el('table');
    var thead = el('thead');
    var hRow = el('tr');
    ['File A', 'File B', 'Co-changes', 'Coupling %', ''].forEach(function(h) {
      var t = el('th');
      t.append(txt(h));
      hRow.append(t);
    });
    thead.append(hRow);
    table.append(thead);

    var tbody = el('tbody');
    pairs.slice(0, 100).forEach(function(p) {
      var row = el('tr');

      var aCell = el('td');
      var aParts = fileParts(p.file_a);
      var aDir = el('span', { className: 'file-dir' });
      aDir.append(txt(aParts.dir));
      var aName = el('span', { className: 'file-name' });
      aName.append(txt(aParts.name));
      aCell.append(aDir, aName);

      var bCell = el('td');
      var bParts = fileParts(p.file_b);
      var bDir = el('span', { className: 'file-dir' });
      bDir.append(txt(bParts.dir));
      var bName = el('span', { className: 'file-name' });
      bName.append(txt(bParts.name));
      bCell.append(bDir, bName);

      var coCell = el('td');
      coCell.append(txt(String(p.co_changes)));

      var pctCell = el('td');
      var pctSpan = el('span', { style: { fontWeight: '700', color: p.coupling_pct > 70 ? '#ef4444' : p.coupling_pct > 40 ? '#f59e0b' : '#10b981' } });
      pctSpan.append(txt(p.coupling_pct.toFixed(1) + '%'));
      pctCell.append(pctSpan);

      var barCell = el('td', { className: 'inline-bar' });
      barCell.append(inlineBar(p.coupling_pct, p.coupling_pct > 70 ? '#ef4444' : p.coupling_pct > 40 ? '#f59e0b' : '#10b981'));

      row.append(aCell, bCell, coCell, pctCell, barCell);
      tbody.append(row);
    });
    table.append(tbody);
    tableWrap.append(table);
    card.append(tableWrap);
    return card;
  }

  /* ---- Ownership tab ---- */
  function buildOwnershipTab() {
    var files = R.author_ownership || [];
    if (files.length === 0) {
      var d = el('div', { className: 'no-data' });
      d.append(txt('No ownership data available.'));
      return d;
    }

    // Collect all unique authors for legend
    var authorSet = [];
    var authorIndex = {};
    files.forEach(function(f) {
      (f.authors || []).forEach(function(a) {
        if (!(a.name in authorIndex)) {
          authorIndex[a.name] = authorSet.length;
          authorSet.push(a.name);
        }
      });
    });

    var sorted = files.slice().sort(function(a, b) {
      var aMax = a.authors && a.authors[0] ? a.authors[0].pct : 0;
      var bMax = b.authors && b.authors[0] ? b.authors[0].pct : 0;
      return bMax - aMax;
    });

    var card = el('div', { className: 'view-card' });
    var tableWrap = el('div', { style: { overflowX: 'auto' } });
    var table = el('table');
    var thead = el('thead');
    var hRow = el('tr');
    ['File', 'Top Owner', 'Ownership'].forEach(function(h) {
      var t = el('th');
      t.append(txt(h));
      hRow.append(t);
    });
    thead.append(hRow);
    table.append(thead);

    var tbody = el('tbody');
    sorted.slice(0, 100).forEach(function(f) {
      var row = el('tr');

      var fileCell = el('td');
      var parts = fileParts(f.path);
      var dirSpan = el('span', { className: 'file-dir' });
      dirSpan.append(txt(parts.dir));
      var nameSpan = el('span', { className: 'file-name' });
      nameSpan.append(txt(parts.name));
      fileCell.append(dirSpan, nameSpan);

      var topCell = el('td', { className: 'own-top' });
      var topAuthor = f.authors && f.authors[0];
      if (topAuthor) {
        topCell.append(txt(topAuthor.name + ' (' + topAuthor.pct.toFixed(0) + '%)'));
      } else {
        topCell.append(txt('—'));
      }

      var barCell = el('td');
      var bar = el('div', { className: 'own-bar' });
      (f.authors || []).slice(0, 8).forEach(function(a) {
        var idx = authorIndex[a.name] % PALETTE.length;
        var seg = el('div', { className: 'own-seg', style: {
          width: a.pct.toFixed(1) + '%',
          background: PALETTE[idx]
        }});
        bar.append(seg);
      });
      barCell.append(bar);

      row.append(fileCell, topCell, barCell);
      tbody.append(row);
    });
    table.append(tbody);
    tableWrap.append(table);
    card.append(tableWrap);

    // Legend
    if (authorSet.length > 0) {
      var legend = el('div', { className: 'legend' });
      authorSet.slice(0, 20).forEach(function(name, i) {
        var item = el('div', { className: 'legend-item' });
        var dot = el('span', { className: 'legend-dot', style: { background: PALETTE[i % PALETTE.length] } });
        var label = el('span');
        label.append(txt(name));
        item.append(dot, label);
        legend.append(item);
      });
      card.append(legend);
    }

    return card;
  }

  /* ---- Age tab ---- */
  function buildAgeTab() {
    var files = (R.file_ages || []).slice().sort(function(a, b) {
      return b.days_since_modified - a.days_since_modified;
    });

    if (files.length === 0) {
      var d = el('div', { className: 'no-data' });
      d.append(txt('No file age data available.'));
      return d;
    }

    function ageBand(days) {
      if (days > 365) return { color: '#ef4444', label: '> 1y' };
      if (days > 180) return { color: '#f59e0b', label: '> 6mo' };
      if (days > 90)  return { color: '#eab308', label: '> 3mo' };
      return { color: '#10b981', label: 'Fresh' };
    }

    var card = el('div', { className: 'view-card' });
    var tableWrap = el('div', { style: { overflowX: 'auto' } });
    var table = el('table');
    var thead = el('thead');
    var hRow = el('tr');
    ['File', 'Last Modified', 'Days Old', 'Age Band'].forEach(function(h) {
      var t = el('th');
      t.append(txt(h));
      hRow.append(t);
    });
    thead.append(hRow);
    table.append(thead);

    var tbody = el('tbody');
    files.slice(0, 100).forEach(function(f) {
      var row = el('tr');

      var fileCell = el('td');
      var parts = fileParts(f.path);
      var dirSpan = el('span', { className: 'file-dir' });
      dirSpan.append(txt(parts.dir));
      var nameSpan = el('span', { className: 'file-name' });
      nameSpan.append(txt(parts.name));
      fileCell.append(dirSpan, nameSpan);

      var dateCell = el('td', { style: { fontFamily: 'monospace', fontSize: '11px', color: '#64748b' } });
      var dateStr = f.last_modified ? f.last_modified.slice(0, 10) : '—';
      dateCell.append(txt(dateStr));

      var daysCell = el('td', { style: { fontFamily: 'monospace' } });
      daysCell.append(txt(String(f.days_since_modified)));

      var band = ageBand(f.days_since_modified);
      var bandCell = el('td');
      var bandChip = el('span', { className: 'chip', style: { background: band.color + '22', color: band.color } });
      bandChip.append(txt(band.label));
      bandCell.append(bandChip);

      row.append(fileCell, dateCell, daysCell, bandCell);
      tbody.append(row);
    });
    table.append(tbody);
    tableWrap.append(table);
    card.append(tableWrap);
    return card;
  }

  /* ---- Overview tab ---- */
  function buildOverviewTab() {
    var div = el('div', { className: 'overview-grid' });

    // Left column
    var left = el('div', { className: 'left-col' });

    // Category cards
    (R.categories || []).forEach(function(cat) {
      left.append(buildCatCard(cat));
    });

    // Top actions
    left.append(buildActions(R.top_actions));

    // Right column
    var right = el('div', { className: 'right-col' });

    // Gauge
    var gaugeWrap = el('div', { className: 'gauge-wrap' });
    gaugeWrap.append(buildGauge(R.overall_score || 0));
    var gaugeLabel = el('div', { className: 'gauge-label' });
    gaugeLabel.append(txt('Overall Score'));
    gaugeWrap.append(gaugeLabel);
    right.append(gaugeWrap);

    // Radar chart
    if (R.categories && R.categories.length > 0) {
      var radarWrap = el('div', { className: 'gauge-wrap' });
      radarWrap.append(buildRadar(R.categories));
      right.append(radarWrap);
    }

    // Remote meta
    var remoteMeta = buildRemoteMeta(R.remote_meta);
    if (remoteMeta) right.append(remoteMeta);

    div.append(left, right);
    return div;
  }

  /* ---- Treemap tab ---- */

  function buildFileTree(files) {
    var root = { name: '/', children: {}, files: [], totalLoc: 0 };
    files.forEach(function(f) {
      var parts = f.path.split('/');
      var fname = parts.pop();
      var node = root;
      parts.forEach(function(p) {
        if (!node.children[p]) {
          node.children[p] = { name: p, children: {}, files: [], totalLoc: 0 };
        }
        node = node.children[p];
      });
      node.files.push({ name: fname, path: f.path, loc: f.loc });
    });
    function computeLoc(node) {
      var sum = 0;
      node.files.forEach(function(f) { sum += f.loc; });
      var keys = Object.keys(node.children);
      keys.forEach(function(k) { sum += computeLoc(node.children[k]); });
      node.totalLoc = sum;
      return sum;
    }
    computeLoc(root);
    function squashSingle(node) {
      var keys = Object.keys(node.children);
      keys.forEach(function(k) { squashSingle(node.children[k]); });
      keys = Object.keys(node.children);
      if (keys.length === 1 && node.files.length === 0 && node.name !== '/') {
        var child = node.children[keys[0]];
        node.name = node.name + '/' + child.name;
        node.children = child.children;
        node.files = child.files;
      }
    }
    squashSingle(root);
    return root;
  }

  function squarify(items, x, y, w, h) {
    if (items.length === 0) return [];
    var results = [];
    var remaining = items.slice().sort(function(a, b) { return b.size - a.size; });
    var totalArea = w * h;
    var totalSize = 0;
    remaining.forEach(function(it) { totalSize += it.size; });
    if (totalSize <= 0) return [];

    function layoutRow(row, rowSize, rx, ry, rw, rh) {
      var short = Math.min(rw, rh);
      var rowArea = (rowSize / totalSize) * totalArea;
      var rowLen = short > 0 ? rowArea / short : 0;
      var offset = 0;
      var horizontal = rw >= rh;
      row.forEach(function(it) {
        var frac = rowSize > 0 ? it.size / rowSize : 0;
        var itemLen = frac * short;
        if (horizontal) {
          results.push({ x: rx, y: ry + offset, w: rowLen, h: itemLen, data: it.data });
        } else {
          results.push({ x: rx + offset, y: ry, w: itemLen, h: rowLen, data: it.data });
        }
        offset += itemLen;
      });
      if (horizontal) {
        return { x: rx + rowLen, y: ry, w: rw - rowLen, h: rh };
      } else {
        return { x: rx, y: ry + rowLen, w: rw, h: rh - rowLen };
      }
    }

    function worstRatio(row, rowSize, short) {
      if (row.length === 0 || short <= 0) return Infinity;
      var rowArea = (rowSize / totalSize) * totalArea;
      var worst = 0;
      row.forEach(function(it) {
        var frac = it.size / rowSize;
        var itemArea = frac * rowArea;
        var itemLen = short > 0 ? frac * short : 0;
        var itemWidth = itemLen > 0 ? itemArea / itemLen : 0;
        var r = itemWidth > itemLen ? itemWidth / itemLen : itemLen / itemWidth;
        if (r > worst) worst = r;
      });
      return worst;
    }

    var rx = x, ry = y, rw = w, rh = h;
    while (remaining.length > 0) {
      var short = Math.min(rw, rh);
      if (short <= 0) break;
      var row = [remaining[0]];
      var rowSize = remaining[0].size;
      remaining.splice(0, 1);
      var currentWorst = worstRatio(row, rowSize, short);

      while (remaining.length > 0) {
        var next = remaining[0];
        var newSize = rowSize + next.size;
        var newRow = row.concat([next]);
        var newWorst = worstRatio(newRow, newSize, short);
        if (newWorst <= currentWorst) {
          row = newRow;
          rowSize = newSize;
          currentWorst = newWorst;
          remaining.splice(0, 1);
        } else {
          break;
        }
      }
      var rest = layoutRow(row, rowSize, rx, ry, rw, rh);
      rx = rest.x; ry = rest.y; rw = rest.w; rh = rest.h;
    }
    return results;
  }

  var metricScales = {
    hotspot: {
      label: 'Hotspot Score',
      color: function(f) {
        var s = f.hotspot_score || 0;
        var t = Math.min(s, 100) / 100;
        var h = (1 - t) * 120;
        return 'hsl(' + h + ',80%,' + (35 + t * 15) + '%)';
      },
      legend: function() {
        return [
          { label: 'Low', color: 'hsl(120,80%,35%)' },
          { label: 'Medium', color: 'hsl(60,80%,42%)' },
          { label: 'High', color: 'hsl(0,80%,50%)' }
        ];
      }
    },
    complexity: {
      label: 'Cyclomatic Complexity',
      color: function(f, maxCC) {
        var t = maxCC > 0 ? Math.min(f.cyclomatic_complexity || 0, maxCC) / maxCC : 0;
        return 'hsl(0,70%,' + (75 - t * 40) + '%)';
      },
      legend: function() {
        return [
          { label: 'Low', color: 'hsl(0,70%,75%)' },
          { label: 'High', color: 'hsl(0,70%,35%)' }
        ];
      }
    },
    churn: {
      label: 'Churn Count',
      color: function(f, _mc, maxChurn) {
        var t = maxChurn > 0 ? Math.min(f.churn_count || 0, maxChurn) / maxChurn : 0;
        return 'hsl(30,80%,' + (75 - t * 40) + '%)';
      },
      legend: function() {
        return [
          { label: 'Low', color: 'hsl(30,80%,75%)' },
          { label: 'High', color: 'hsl(30,80%,35%)' }
        ];
      }
    },
    age: {
      label: 'File Age (days)',
      color: function(f, _mc, _mch, ageMap, maxAge) {
        var a = ageMap[f.path];
        var days = a ? a.days_since_modified : 0;
        var t = maxAge > 0 ? Math.min(days, maxAge) / maxAge : 0;
        return 'hsl(220,70%,' + (75 - t * 40) + '%)';
      },
      legend: function() {
        return [
          { label: 'Recent', color: 'hsl(220,70%,75%)' },
          { label: 'Old', color: 'hsl(220,70%,35%)' }
        ];
      }
    },
    owner: {
      label: 'Top Contributor',
      color: function(f, _mc, _mch, _am, _ma, ownerMap, authorIndex) {
        var own = ownerMap[f.path];
        if (own && own.authors && own.authors[0]) {
          var idx = authorIndex[own.authors[0].name];
          return PALETTE[idx != null ? idx % PALETTE.length : 0];
        }
        return '#334155';
      },
      legend: function(authorIndex) {
        var items = [];
        for (var name in authorIndex) {
          items.push({ label: name, color: PALETTE[authorIndex[name] % PALETTE.length] });
        }
        return items;
      }
    }
  };

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

    var container = el('div');

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
    controls.append(selectLabel, select, breadcrumb);
    container.append(controls);

    if (capped) {
      var note = el('div', { style: { color: '#f59e0b', fontSize: '12px', padding: '4px 0' } });
      note.append(txt('Showing top 2000 files by LOC.'));
      container.append(note);
    }

    var svgW = 960, svgH = 600;
    var svg = svgEl('svg', { id: 'tm-svg', viewBox: '0 0 ' + svgW + ' ' + svgH, width: '100%', style: 'display:block;background:#080a0f;border:1px solid #1e293b;border-radius:8px;' });
    container.append(svg);

    var tooltip = el('div', { className: 'tm-tooltip' });
    container.append(tooltip);

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
        var label = el('span');
        label.append(txt(it.label));
        item.append(swatch, label);
        legendDiv.append(item);
      });
    }

    function updateBreadcrumb() {
      breadcrumb.replaceChildren();
      var rootCrumb = el('span');
      rootCrumb.append(txt('/'));
      rootCrumb.addEventListener('click', function() {
        currentRoot = tree;
        navStack = [];
        renderTreemap();
      });
      breadcrumb.append(rootCrumb);
      navStack.forEach(function(entry, i) {
        var sep = el('span', { className: 'tm-sep' });
        sep.append(txt('/'));
        breadcrumb.append(sep);
        var crumb = el('span');
        crumb.append(txt(entry.name));
        crumb.addEventListener('click', (function(idx) {
          return function() {
            currentRoot = navStack[idx].node;
            navStack = navStack.slice(0, idx + 1);
            renderTreemap();
          };
        })(i));
        breadcrumb.append(crumb);
      });
    }

    function renderTreeNode(svgNode, node, x, y, w, h, depth) {
      if (w < 1 || h < 1) return;
      var headerH = depth > 0 ? 16 : 0;
      var innerY = y + headerH;
      var innerH = h - headerH;
      if (innerH < 1) return;

      if (depth > 0) {
        svgNode.append(svgEl('rect', {
          x: String(x), y: String(y), width: String(w), height: String(h),
          fill: '#0d1117', stroke: '#1e293b', 'stroke-width': '1',
          class: 'tm-dir-bg', 'data-dir': node.name
        }));
        if (w > 40) {
          var label = svgEl('text', {
            x: String(x + 4), y: String(y + 12),
            fill: '#64748b', 'font-size': '10', 'font-family': 'monospace',
            class: 'tm-dir-label'
          });
          label.append(txt(node.name));
          svgNode.append(label);
        }
      }

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
      var rects = squarify(items, x, innerY, w, innerH);

      rects.forEach(function(r) {
        if (r.data.type === 'file') {
          var fData = fileMap[r.data.file.path];
          var color = fData ? colorForFile(fData) : '#334155';
          var rect = svgEl('rect', {
            x: String(r.x), y: String(r.y),
            width: String(Math.max(0, r.w - 1)), height: String(Math.max(0, r.h - 1)),
            fill: color, class: 'tm-file', 'data-path': r.data.file.path,
            rx: '2', opacity: '0.9'
          });
          var titleEl = svgEl('title');
          titleEl.append(txt(r.data.file.path + ' (' + r.data.file.loc + ' LOC)'));
          rect.append(titleEl);
          svgNode.append(rect);
          if (r.w > 40 && r.h > 14) {
            var textEl = svgEl('text', {
              x: String(r.x + 3), y: String(r.y + 12),
              fill: '#e2e8f0', 'font-size': '9', 'font-family': 'monospace',
              'pointer-events': 'none', opacity: '0.8'
            });
            var maxChars = Math.floor((r.w - 6) / 5.5);
            var lbl = r.data.file.name;
            if (lbl.length > maxChars) lbl = lbl.slice(0, maxChars - 1) + '\u2026';
            textEl.append(txt(lbl));
            svgNode.append(textEl);
          }
        } else {
          renderTreeNode(svgNode, r.data.node, r.x, r.y, r.w, r.h, depth + 1);
        }
      });
    }

    function renderTreemap() {
      while (svg.firstChild) svg.removeChild(svg.firstChild);
      renderTreeNode(svg, currentRoot, 0, 0, svgW, svgH, 0);
      updateBreadcrumb();
      updateLegend();
    }

    select.addEventListener('change', function() {
      var allFiles = svg.querySelectorAll('.tm-file');
      allFiles.forEach(function(rect) {
        var path = rect.getAttribute('data-path');
        var f = fileMap[path];
        if (f) rect.setAttribute('fill', colorForFile(f));
      });
      updateLegend();
    });

    svg.addEventListener('click', function(e) {
      var target = e.target;
      if (target.classList && target.classList.contains('tm-dir-bg')) {
        var dirName = target.getAttribute('data-dir');
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
          renderTreemap();
        }
      }
    });

    svg.addEventListener('mousemove', function(e) {
      var target = e.target;
      if (target.classList && target.classList.contains('tm-file')) {
        var path = target.getAttribute('data-path');
        var f = fileMap[path];
        if (f) {
          tooltip.replaceChildren();
          tooltip.append(el('div', { style: { fontWeight: '600', marginBottom: '4px' } }, f.path));
          tooltip.append(el('div', null, 'LOC: ' + f.loc + '  CC: ' + f.cyclomatic_complexity));
          tooltip.append(el('div', null, 'Churn: ' + f.churn_count + '  Score: ' + f.hotspot_score.toFixed(1)));
          var age = ageMap[f.path];
          if (age) tooltip.append(el('div', null, 'Age: ' + age.days_since_modified + ' days'));
          var own = ownerMap[f.path];
          if (own && own.authors && own.authors[0]) {
            tooltip.append(el('div', null, 'Owner: ' + own.authors[0].name + ' (' + own.authors[0].pct.toFixed(0) + '%)'));
          }
          tooltip.style.display = 'block';
          tooltip.style.left = (e.clientX + 12) + 'px';
          tooltip.style.top = (e.clientY + 12) + 'px';
        }
      } else {
        tooltip.style.display = 'none';
      }
    });

    svg.addEventListener('mouseleave', function() {
      tooltip.style.display = 'none';
    });

    renderTreemap();
    return container;
  }

  /* ---- Main render ---- */
  function renderApp() {
    var app = document.getElementById('app');

    // Header
    var header = el('header');
    var headerRow = el('div', { className: 'header-row' });
    var brandWrap = el('div', { style: { display: 'flex', alignItems: 'center', gap: '10px' } });
    var brand = el('span', { className: 'brand' });
    brand.append(txt('Barad-dûr'));
    var dotSpan = el('span', { className: 'dot' });
    dotSpan.append(txt('|'));
    var repoName = el('span', { style: { fontWeight: '600', fontSize: '16px' } });
    repoName.append(txt(R.repo_name || ''));
    brandWrap.append(brand, dotSpan, repoName);

    var chips = el('div', { className: 'meta-chips' });
    chips.append(chip(R.branch || 'main', '#1e293b', '#94a3b8'));
    chips.append(chip(R.total_commits + ' commits', '#1e3a5f', '#93c5fd'));
    chips.append(chip(R.total_authors + ' authors', '#1c3547', '#6ee7b7'));
    chips.append(chip(R.total_files + ' files', '#2d1b3d', '#c4b5fd'));
    if (R.time_window_months && R.time_window_months > 0) {
      chips.append(chip(R.time_window_months + 'mo window', '#2a1f0a', '#fcd34d'));
    }
    headerRow.append(brandWrap, chips);
    header.append(headerRow);

    // Tabs
    var tabNames = ['Overview', 'Hotspots', 'Coupling', 'Ownership', 'Age', 'Treemap'];
    var tabContents = [
      buildOverviewTab,
      buildHotspotsTab,
      buildCouplingTab,
      buildOwnershipTab,
      buildAgeTab,
      buildTreemapTab
    ];

    var tabs = el('div', { className: 'tabs' });
    var contentDivs = [];
    var activeTab = 0;

    tabNames.forEach(function(name, i) {
      var tab = el('button', { className: 'tab' + (i === 0 ? ' active' : '') });
      tab.append(txt(name));

      var contentDiv = el('div', { className: 'tab-content' + (i === 0 ? ' active' : '') });
      contentDivs.push(contentDiv);

      tab.addEventListener('click', (function(idx, t) {
        return function() {
          // Remove active from all
          var allTabs = tabs.querySelectorAll('.tab');
          allTabs.forEach(function(tb) { tb.className = 'tab'; });
          contentDivs.forEach(function(cd) { cd.className = 'tab-content'; });
          t.className = 'tab active';
          contentDivs[idx].className = 'tab-content active';

          // Lazy-render on first visit
          if (contentDivs[idx].dataset.rendered !== '1') {
            contentDivs[idx].replaceChildren(tabContents[idx]());
            contentDivs[idx].dataset.rendered = '1';
          }
        };
      })(i, tab));

      tabs.append(tab);
    });

    // Pre-render overview immediately
    contentDivs[0].replaceChildren(tabContents[0]());
    contentDivs[0].dataset.rendered = '1';

    app.replaceChildren(header, tabs, ...contentDivs);
  }

  renderApp();
})();
"#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{CategoryResult, MetricValue, RawValue};
    use crate::scorer::AnalysisReport;

    fn make_report() -> AnalysisReport {
        AnalysisReport {
            repo_name: "my-repo".into(),
            branch: "main".into(),
            time_window_months: 6,
            total_commits: 42,
            total_authors: 3,
            total_files: 20,
            overall_score: 75,
            categories: vec![CategoryResult {
                name: "Health".into(),
                score: 75,
                metrics: vec![MetricValue {
                    name: "Bus factor".into(),
                    description: "OK".into(),
                    raw_value: RawValue::Integer(3),
                    score: 75,
                }],
            }],
            top_actions: vec!["Improve test coverage".into()],
            remote_meta: None,
            file_hotspots: vec![],
            coupling_pairs: vec![],
            author_ownership: vec![],
            file_ages: vec![],
        }
    }

    #[test]
    fn html_is_valid_document() {
        let report = make_report();
        let html = render(&report).unwrap();
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn html_embeds_report_data() {
        let report = make_report();
        let html = render(&report).unwrap();
        assert!(html.contains("my-repo"));
    }

    #[test]
    fn html_contains_tab_markers() {
        let report = make_report();
        let html = render(&report).unwrap();
        assert!(html.contains("Hotspots"));
        assert!(html.contains("Coupling"));
        assert!(html.contains("Ownership"));
        assert!(html.contains("Age"));
    }

    #[test]
    fn html_title_contains_repo_name() {
        let report = make_report();
        let html = render(&report).unwrap();
        assert!(html.contains("<title>my-repo"));
    }

    #[test]
    fn score_color_thresholds() {
        assert_eq!(score_color(71), "#10b981");
        assert_eq!(score_color(70), "#f59e0b");
        assert_eq!(score_color(41), "#f59e0b");
        assert_eq!(score_color(40), "#ef4444");
        assert_eq!(score_color(0), "#ef4444");
    }

    // ---- Treemap tests ----

    fn make_treemap_report() -> AnalysisReport {
        use crate::scorer::{AuthorShare, FileAge, FileOwnership, HotspotFile};
        use chrono::Utc;

        let mut report = make_report();
        report.file_hotspots = vec![
            HotspotFile {
                path: "src/main.rs".into(),
                churn_count: 12,
                loc: 200,
                total_lines: 210,
                cyclomatic_complexity: 15,
                public_methods: 3,
                properties: 1,
                hotspot_score: 72.0,
            },
            HotspotFile {
                path: "src/lib.rs".into(),
                churn_count: 8,
                loc: 150,
                total_lines: 160,
                cyclomatic_complexity: 10,
                public_methods: 5,
                properties: 2,
                hotspot_score: 45.0,
            },
            HotspotFile {
                path: "tests/test_a.rs".into(),
                churn_count: 3,
                loc: 80,
                total_lines: 85,
                cyclomatic_complexity: 4,
                public_methods: 2,
                properties: 0,
                hotspot_score: 20.0,
            },
            HotspotFile {
                path: "tests/test_b.rs".into(),
                churn_count: 1,
                loc: 60,
                total_lines: 65,
                cyclomatic_complexity: 2,
                public_methods: 1,
                properties: 0,
                hotspot_score: 10.0,
            },
        ];
        report.file_ages = vec![
            FileAge {
                path: "src/main.rs".into(),
                last_modified: Utc::now(),
                days_since_modified: 5,
            },
            FileAge {
                path: "src/lib.rs".into(),
                last_modified: Utc::now(),
                days_since_modified: 30,
            },
            FileAge {
                path: "tests/test_a.rs".into(),
                last_modified: Utc::now(),
                days_since_modified: 90,
            },
            FileAge {
                path: "tests/test_b.rs".into(),
                last_modified: Utc::now(),
                days_since_modified: 200,
            },
        ];
        report.author_ownership = vec![
            FileOwnership {
                path: "src/main.rs".into(),
                authors: vec![
                    AuthorShare {
                        name: "Alice".into(),
                        pct: 70.0,
                    },
                    AuthorShare {
                        name: "Bob".into(),
                        pct: 30.0,
                    },
                ],
            },
            FileOwnership {
                path: "src/lib.rs".into(),
                authors: vec![
                    AuthorShare {
                        name: "Bob".into(),
                        pct: 60.0,
                    },
                    AuthorShare {
                        name: "Alice".into(),
                        pct: 40.0,
                    },
                ],
            },
            FileOwnership {
                path: "tests/test_a.rs".into(),
                authors: vec![AuthorShare {
                    name: "Alice".into(),
                    pct: 100.0,
                }],
            },
            FileOwnership {
                path: "tests/test_b.rs".into(),
                authors: vec![AuthorShare {
                    name: "Bob".into(),
                    pct: 100.0,
                }],
            },
        ];
        report
    }

    #[test]
    fn html_contains_treemap_tab() {
        let html = render(&make_treemap_report()).unwrap();
        assert!(html.contains("Treemap"), "Should contain Treemap tab name");
    }

    #[test]
    fn html_treemap_has_svg() {
        let html = render(&make_treemap_report()).unwrap();
        assert!(
            html.contains("tm-svg"),
            "Should contain tm-svg container id"
        );
    }

    #[test]
    fn html_treemap_has_metric_select() {
        let html = render(&make_treemap_report()).unwrap();
        assert!(
            html.contains("tm-metric-select"),
            "Should contain tm-metric-select dropdown id"
        );
    }

    #[test]
    fn html_treemap_has_squarify() {
        let html = render(&make_treemap_report()).unwrap();
        assert!(
            html.contains("squarify"),
            "Should contain squarify layout function"
        );
    }

    #[test]
    fn html_treemap_has_color_scales() {
        let html = render(&make_treemap_report()).unwrap();
        assert!(
            html.contains("metricScales"),
            "Should contain metricScales color scale object"
        );
    }

    #[test]
    fn html_treemap_has_breadcrumb() {
        let html = render(&make_treemap_report()).unwrap();
        assert!(
            html.contains("tm-breadcrumb"),
            "Should contain tm-breadcrumb navigation"
        );
    }
}
