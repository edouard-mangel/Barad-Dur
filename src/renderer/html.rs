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
.tab-info {
  background: #111827;
  border: 1px solid #1e293b;
  border-radius: 8px;
  padding: 14px 18px;
  margin-bottom: 20px;
  font-size: 13px;
  line-height: 1.6;
  color: #94a3b8;
}
.tab-info-title {
  color: #e2e8f0;
  font-weight: 600;
  font-size: 14px;
  margin-bottom: 6px;
}
.tab-info .score-hint {
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px solid #1e293b;
  font-size: 12px;
  display: flex;
  gap: 16px;
  flex-wrap: wrap;
}
.tab-info .score-hint span { white-space: nowrap; }
.tab-info .dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-right: 4px;
  vertical-align: middle;
}
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
.hotspot-wrap .tab-info { grid-column: 1 / -1; }
@media (max-width: 900px) {
  .hotspot-wrap { grid-template-columns: 1fr; }
}
.cp-dismiss {
  background: none;
  border: none;
  color: #475569;
  cursor: pointer;
  font-size: 14px;
  padding: 2px 6px;
  border-radius: 4px;
  transition: color 0.15s, background 0.15s;
}
.cp-dismiss:hover { color: #ef4444; background: rgba(239,68,68,0.1); }
.cp-controls {
  display: flex;
  gap: 12px;
  align-items: center;
  margin-bottom: 10px;
  font-size: 12px;
  color: #64748b;
}
.cp-controls button {
  background: #1e293b;
  border: 1px solid #334155;
  color: #94a3b8;
  padding: 4px 10px;
  border-radius: 4px;
  font-size: 11px;
  cursor: pointer;
  transition: color 0.15s, border-color 0.15s;
}
.cp-controls button:hover { color: #e2e8f0; border-color: #475569; }
.cp-controls button.active { color: #f59e0b; border-color: #f59e0b; }
.cp-auto-excluded { opacity: 0.45; }
.cp-auto-tag {
  display: inline-block;
  background: #1e293b;
  color: #64748b;
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 3px;
  margin-left: 6px;
}
.hs-scatter-dot { cursor: pointer; transition: opacity 0.15s, stroke-width 0.15s; }
.hs-scatter-dot:hover { opacity: 1 !important; }
.hs-scatter-dot.active { stroke: #f59e0b; stroke-width: 3; opacity: 1 !important; }
.hs-row-highlight { background: #1e293b !important; outline: 2px solid #f59e0b; outline-offset: -2px; }
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
.tm-wrap {
  position: relative;
  overflow: hidden;
  border: 1px solid #1e293b;
  border-radius: 8px;
  background: #080a0f;
}
.tm-detail {
  position: absolute;
  right: 0;
  top: 0;
  width: 280px;
  height: 100%;
  background: #0d1117;
  border-left: 1px solid #1e293b;
  padding: 16px;
  font-size: 12px;
  overflow-y: auto;
  transform: translateX(100%);
  transition: transform 0.25s ease;
  z-index: 10;
}
.tm-detail.open { transform: translateX(0); }
.tm-detail-title {
  font-family: monospace;
  font-size: 13px;
  font-weight: 700;
  color: #93c5fd;
  word-break: break-all;
  margin-bottom: 12px;
}
.tm-detail-row {
  display: flex;
  justify-content: space-between;
  padding: 5px 0;
  border-bottom: 1px solid #0f172a;
  color: #cbd5e1;
}
.tm-detail-row span:last-child { font-weight: 600; font-family: monospace; }
.tm-detail-close {
  position: absolute;
  top: 8px;
  right: 10px;
  background: none;
  border: none;
  color: #64748b;
  font-size: 16px;
  cursor: pointer;
}
.tm-detail-close:hover { color: #e2e8f0; }
.tm-hint {
  color: #475569;
  font-size: 11px;
  padding: 4px 0;
  text-align: center;
}
.tm-layout-toggle {
  display: flex;
  gap: 0;
  border: 1px solid #1e293b;
  border-radius: 6px;
  overflow: hidden;
}
.tm-layout-btn {
  background: #0d1117;
  color: #64748b;
  border: none;
  padding: 5px 12px;
  font-size: 12px;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}
.tm-layout-btn:not(:last-child) { border-right: 1px solid #1e293b; }
.tm-layout-btn:hover { color: #e2e8f0; }
.tm-layout-btn.active { background: #1e293b; color: #f59e0b; }
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

  var defaultScoreHints = [
    { color: '#ef4444', label: '0\u201339 Critical' },
    { color: '#f59e0b', label: '40\u201369 Needs work' },
    { color: '#22c55e', label: '70\u2013100 Healthy' }
  ];

  /* ---- Hotspots tab ---- */
  function buildHotspotsTab() {
    var files = R.file_hotspots || [];
    if (files.length === 0) {
      var d = el('div', { className: 'no-data' });
      d.append(txt('No hotspot data available.'));
      return d;
    }

    var wrap = el('div', { className: 'hotspot-wrap' });
    wrap.append(buildTabInfo(
      'Hotspot Score \u2014 Where risk concentrates',
      'Files are ranked by a composite Hotspot Score combining cyclomatic complexity (code branching), churn count (how often the file changes), and lines of code. High-churn, high-complexity files are the most likely sources of bugs and the hardest to review. Focus refactoring efforts on the top-right corner of the scatter plot.',
      [
        { color: '#22c55e', label: 'Low risk \u2014 simple + rarely changed' },
        { color: '#f59e0b', label: 'Medium \u2014 monitor these files' },
        { color: '#ef4444', label: 'High risk \u2014 complex + frequently changed' }
      ]
    ));

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
        fill: color, opacity: '0.7',
        class: 'hs-scatter-dot', 'data-path': f.path
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
        var row = el('tr', { 'data-path': f.path });
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

    // Click scatter dot → highlight matching table row
    var selectedDot = null;
    scatter.addEventListener('click', function(e) {
      var dot = e.target;
      // Walk up for SVG elements (closest() unreliable on SVG)
      while (dot && dot !== scatter) {
        if (dot.classList && dot.classList.contains('hs-scatter-dot')) break;
        dot = dot.parentNode;
      }
      if (!dot || dot === scatter) return;
      var path = dot.getAttribute('data-path');

      // Clear previous
      scatter.querySelectorAll('.hs-scatter-dot').forEach(function(d) {
        d.setAttribute('class', 'hs-scatter-dot');
      });
      tableWrap.querySelectorAll('.hs-row-highlight').forEach(function(r) {
        r.classList.remove('hs-row-highlight');
      });

      // Toggle off if same dot clicked
      if (selectedDot === path) {
        selectedDot = null;
        return;
      }
      selectedDot = path;

      // Highlight dot
      dot.setAttribute('class', 'hs-scatter-dot active');

      // Highlight and scroll to table row
      var row = tableWrap.querySelector('tr[data-path="' + CSS.escape(path) + '"]');
      if (row) {
        row.classList.add('hs-row-highlight');
        row.scrollIntoView({ behavior: 'smooth', block: 'center' });
      }
    });

    return wrap;
  }

  /* ---- Coupling tab ---- */
  // Auto-exclude patterns for coupling: interface/implementation, lock files, test files, module indexes
  function isAutoExcluded(a, b) {
    var na = a.split('/').pop(), nb = b.split('/').pop();
    var da = a.substring(0, a.lastIndexOf('/') + 1);
    var db = b.substring(0, b.lastIndexOf('/') + 1);
    // Lock files: Cargo.lock/Cargo.toml, package-lock.json/package.json, yarn.lock, pnpm-lock.yaml, *.lock
    var lockFiles = ['Cargo.lock', 'package-lock.json', 'yarn.lock', 'pnpm-lock.yaml', 'composer.lock', 'Gemfile.lock', 'poetry.lock'];
    var manifestFiles = ['Cargo.toml', 'package.json', 'yarn.lock', 'pnpm-lock.yaml', 'composer.json', 'Gemfile', 'pyproject.toml'];
    if (lockFiles.indexOf(na) >= 0 || lockFiles.indexOf(nb) >= 0) return 'lock file';
    // Project files: *.csproj, *.sln, pom.xml, build.gradle
    var projFiles = ['.csproj', '.sln', '.fsproj', '.vbproj'];
    if (projFiles.some(function(ext) { return na.endsWith(ext) || nb.endsWith(ext); })) return 'project file';
    if (na === 'pom.xml' || nb === 'pom.xml' || na === 'build.gradle' || nb === 'build.gradle') return 'build file';
    // Module index files: mod.rs, index.ts/js, __init__.py, lib.rs
    var indexFiles = ['mod.rs', 'lib.rs', 'index.ts', 'index.js', 'index.tsx', 'index.jsx', '__init__.py'];
    if (da === db && (indexFiles.indexOf(na) >= 0 || indexFiles.indexOf(nb) >= 0)) return 'module index';
    // Test file pairs: foo.ts <-> foo.spec.ts, foo.test.ts, foo_test.go, FooTest.java, FooTests.cs
    function stripTestSuffix(name) {
      return name
        .replace(/\.spec\.(ts|js|tsx|jsx|mjs)$/, '.$1')
        .replace(/\.test\.(ts|js|tsx|jsx|mjs|py)$/, '.$1')
        .replace(/_test\.go$/, '.go')
        .replace(/Tests?\.(java|cs|fs)$/, '.$1')
        .replace(/Tests?\.(cs|fs)$/, '.$1');
    }
    if (stripTestSuffix(na) !== na && stripTestSuffix(na) === nb) return 'test file';
    if (stripTestSuffix(nb) !== nb && stripTestSuffix(nb) === na) return 'test file';
    // C# interface: IFoo.cs <-> Foo.cs (same dir)
    if (da === db && na.endsWith('.cs') && nb.endsWith('.cs')) {
      var aBase = na.slice(0, -3), bBase = nb.slice(0, -3);
      if (aBase === 'I' + bBase || bBase === 'I' + aBase) return 'interface/impl';
    }
    // Java interface/impl: FooInterface.java <-> FooImpl.java
    if (na.endsWith('.java') && nb.endsWith('.java')) {
      var aj = na.slice(0, -5), bj = nb.slice(0, -5);
      if (aj + 'Impl' === bj || bj + 'Impl' === aj) return 'interface/impl';
      if (aj + 'Interface' === bj || bj + 'Interface' === aj) return 'interface/impl';
    }
    return null;
  }

  function buildCouplingTab() {
    var pairs = (R.coupling_pairs || []).slice().sort(function(a, b) {
      return b.coupling_pct - a.coupling_pct;
    });

    if (pairs.length === 0) {
      var d = el('div', { className: 'no-data' });
      d.append(txt('No temporal coupling data available.'));
      return d;
    }

    var container = el('div');
    container.append(buildTabInfo(
      'Temporal Coupling \u2014 Files that change together',
      'Temporal coupling measures how often two files are modified in the same commit. A high percentage means the files are implicitly linked \u2014 changing one almost always requires changing the other. This can indicate hidden dependencies, duplicated logic, or missing abstractions. Consider extracting shared interfaces or merging tightly coupled files.',
      [
        { color: '#22c55e', label: '<30% \u2014 Normal co-change' },
        { color: '#f59e0b', label: '30\u201360% \u2014 Worth investigating' },
        { color: '#ef4444', label: '>60% \u2014 Strongly coupled, refactor candidate' }
      ]
    ));

    // Track hidden state
    var dismissed = {};
    var showAutoExcluded = false;

    // Controls
    var controls = el('div', { className: 'cp-controls' });
    var toggleAutoBtn = el('button');
    toggleAutoBtn.append(txt('Show auto-excluded'));
    var statusSpan = el('span');
    var resetBtn = el('button');
    resetBtn.append(txt('Reset dismissed'));
    controls.append(toggleAutoBtn, statusSpan, resetBtn);
    container.append(controls);

    var card = el('div', { className: 'view-card' });
    var tableWrap = el('div', { style: { overflowX: 'auto' } });

    function renderTable() {
      tableWrap.replaceChildren();
      var table = el('table');
      var thead = el('thead');
      var hRow = el('tr');
      ['File A', 'File B', 'Co-changes', 'Coupling %', '', ''].forEach(function(h) {
        var t = el('th');
        t.append(txt(h));
        hRow.append(t);
      });
      thead.append(hRow);
      table.append(thead);

      var tbody = el('tbody');
      var hiddenCount = 0;
      var autoCount = 0;

      pairs.slice(0, 100).forEach(function(p, idx) {
        var excludeReason = isAutoExcluded(p.file_a, p.file_b);
        if (excludeReason) autoCount++;
        if (dismissed[idx]) { hiddenCount++; return; }
        if (excludeReason && !showAutoExcluded) { hiddenCount++; return; }

        var row = el('tr');
        if (excludeReason) row.className = 'cp-auto-excluded';

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
        if (excludeReason) {
          var tag = el('span', { className: 'cp-auto-tag' });
          tag.append(txt(excludeReason));
          bCell.append(tag);
        }

        var coCell = el('td');
        coCell.append(txt(String(p.co_changes)));

        var pctCell = el('td');
        var pctSpan = el('span', { style: { fontWeight: '700', color: p.coupling_pct > 70 ? '#ef4444' : p.coupling_pct > 40 ? '#f59e0b' : '#10b981' } });
        pctSpan.append(txt(p.coupling_pct.toFixed(1) + '%'));
        pctCell.append(pctSpan);

        var barCell = el('td', { className: 'inline-bar' });
        barCell.append(inlineBar(p.coupling_pct, p.coupling_pct > 70 ? '#ef4444' : p.coupling_pct > 40 ? '#f59e0b' : '#10b981'));

        var dismissCell = el('td');
        var dismissBtn = el('button', { className: 'cp-dismiss' });
        dismissBtn.append(txt('\u00d7'));
        dismissBtn.addEventListener('click', (function(i) {
          return function() { dismissed[i] = true; renderTable(); };
        })(idx));
        dismissCell.append(dismissBtn);

        row.append(aCell, bCell, coCell, pctCell, barCell, dismissCell);
        tbody.append(row);
      });
      table.append(tbody);
      tableWrap.append(table);

      // Update status
      statusSpan.replaceChildren();
      var parts = [];
      if (autoCount > 0) parts.push(autoCount + ' auto-excluded');
      var dismissedCount = Object.keys(dismissed).length;
      if (dismissedCount > 0) parts.push(dismissedCount + ' dismissed');
      if (parts.length > 0) {
        statusSpan.append(txt(parts.join(', ') + ' \u2014 ' + hiddenCount + ' hidden'));
      }
      resetBtn.style.display = dismissedCount > 0 ? '' : 'none';
      toggleAutoBtn.className = showAutoExcluded ? 'active' : '';
      toggleAutoBtn.replaceChildren();
      toggleAutoBtn.append(txt(showAutoExcluded ? 'Hide auto-excluded' : 'Show auto-excluded (' + autoCount + ')'));
    }

    toggleAutoBtn.addEventListener('click', function() {
      showAutoExcluded = !showAutoExcluded;
      renderTable();
    });
    resetBtn.addEventListener('click', function() {
      dismissed = {};
      renderTable();
    });

    renderTable();
    card.append(tableWrap);
    container.append(card);
    return container;
  }

  /* ---- Ownership tab ---- */
  function buildOwnershipTab() {
    var files = R.author_ownership || [];
    if (files.length === 0) {
      var d = el('div', { className: 'no-data' });
      d.append(txt('No ownership data available.'));
      return d;
    }

    var container = el('div');
    container.append(buildTabInfo(
      'Code Ownership \u2014 Who knows what',
      'Ownership is derived from git blame: each file shows the percentage of lines last modified by each contributor. This reveals knowledge distribution \u2014 files dominated by a single author are "knowledge silos" (bus factor risk), while evenly distributed files have shared understanding. The Gini coefficient (0=perfectly equal, 1=one person owns everything) summarizes overall balance.',
      [
        { color: '#22c55e', label: 'Shared \u2014 multiple contributors, low bus-factor risk' },
        { color: '#f59e0b', label: 'Concentrated \u2014 one author >70%, knowledge silo risk' },
        { color: '#ef4444', label: 'Sole owner \u2014 single author >90%, critical bus-factor' }
      ]
    ));

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

    container.append(card);
    return container;
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

    var container = el('div');
    container.append(buildTabInfo(
      'Code Age \u2014 Freshness of the codebase',
      'Each file\u2019s age is the number of days since its last modification (based on the most recent commit that touched it). Old, untouched files may indicate stable, battle-tested code \u2014 or forgotten, potentially brittle code that nobody dares change. Cross-reference with ownership and complexity to distinguish the two.',
      [
        { color: '#10b981', label: 'Fresh (<90 days) \u2014 actively maintained' },
        { color: '#eab308', label: '3\u20136 months \u2014 aging, review periodically' },
        { color: '#f59e0b', label: '6\u201312 months \u2014 stale, check if still relevant' },
        { color: '#ef4444', label: '>1 year \u2014 potentially abandoned' }
      ]
    ));

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
    container.append(card);
    return container;
  }

  /* ---- Overview tab ---- */
  function buildOverviewTab() {
    var wrapper = el('div');
    wrapper.append(buildTabInfo(
      'Overview \u2014 Repository health at a glance',
      'The overall score (0\u2013100) is a weighted average of four categories: Health (30%), Team (30%), Evolution (20%), and Git Hygiene (20%). Each category aggregates several metrics scored individually. The radar chart shows balance across categories \u2014 a lopsided shape reveals areas needing attention. Recommendations below target the lowest-scoring metrics.',
      defaultScoreHints
    ));
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
    wrapper.append(div);
    return wrapper;
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

  function circlePack(items, cx, cy, r) {
    if (items.length === 0) return [];
    var sorted = items.slice().sort(function(a, b) { return b.size - a.size; });
    var totalSize = 0;
    sorted.forEach(function(it) { totalSize += it.size; });
    if (totalSize <= 0) return [];

    // Assign radii proportional to sqrt(size) so area ~ size
    var radii = [];
    var sumR2 = 0;
    sorted.forEach(function(it) {
      var ri = Math.sqrt(it.size / totalSize);
      radii.push(ri);
      sumR2 += ri * ri;
    });
    // Scale so circles fit inside parent radius with padding
    var scale = r * 0.85 / Math.sqrt(sumR2);
    radii = radii.map(function(ri) { return ri * scale; });

    // Place circles using simple greedy front-chain approach
    var placed = [];
    for (var i = 0; i < sorted.length; i++) {
      var ri = Math.max(radii[i], 2);
      if (i === 0) {
        placed.push({ cx: cx, cy: cy, r: ri, data: sorted[i].data });
      } else if (i === 1) {
        placed.push({ cx: cx + placed[0].r + ri + 1, cy: cy, r: ri, data: sorted[i].data });
      } else {
        // Find position that doesn't overlap existing circles, closest to center
        var bestX = cx, bestY = cy, bestDist = Infinity;
        for (var j = 0; j < placed.length; j++) {
          for (var k = j + 1; k < placed.length; k++) {
            // Try placing tangent to circles j and k
            var candidates = tangentPositions(placed[j], placed[k], ri);
            for (var c = 0; c < candidates.length; c++) {
              var px = candidates[c].x, py = candidates[c].y;
              var dist = Math.sqrt((px - cx) * (px - cx) + (py - cy) * (py - cy));
              if (dist + ri > r * 0.95) continue; // outside parent
              var overlaps = false;
              for (var m = 0; m < placed.length; m++) {
                var dx = px - placed[m].cx, dy = py - placed[m].cy;
                if (Math.sqrt(dx * dx + dy * dy) < ri + placed[m].r - 0.5) {
                  overlaps = true;
                  break;
                }
              }
              if (!overlaps && dist < bestDist) {
                bestDist = dist;
                bestX = px;
                bestY = py;
              }
            }
          }
        }
        placed.push({ cx: bestX, cy: bestY, r: ri, data: sorted[i].data });
      }
    }

    // Center the packed circles within the parent
    if (placed.length > 0) {
      var avgX = 0, avgY = 0;
      placed.forEach(function(p) { avgX += p.cx; avgY += p.cy; });
      avgX /= placed.length;
      avgY /= placed.length;
      var shiftX = cx - avgX, shiftY = cy - avgY;
      placed.forEach(function(p) { p.cx += shiftX; p.cy += shiftY; });
    }

    return placed;
  }

  function tangentPositions(c1, c2, r) {
    var dx = c2.cx - c1.cx, dy = c2.cy - c1.cy;
    var d = Math.sqrt(dx * dx + dy * dy);
    if (d < 0.001) return [{ x: c1.cx + c1.r + r, y: c1.cy }];
    var d1 = c1.r + r, d2 = c2.r + r;
    if (d > d1 + d2) return [];
    var a = (d1 * d1 - d2 * d2 + d * d) / (2 * d);
    var h2 = d1 * d1 - a * a;
    if (h2 < 0) h2 = 0;
    var h = Math.sqrt(h2);
    var mx = c1.cx + a * dx / d, my = c1.cy + a * dy / d;
    return [
      { x: mx + h * dy / d, y: my - h * dx / d },
      { x: mx - h * dy / d, y: my + h * dx / d }
    ];
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
    var selectedPath = null;

    var container = el('div');
    container.append(buildTabInfo(
      'Treemap \u2014 Spatial view of the codebase',
      'Each rectangle (or circle) represents a file \u2014 size is proportional to lines of code, color reflects the selected metric. Directories are nested containers you can click to drill into. Use the dropdown to switch between hotspot score, complexity, churn, file age, or top contributor coloring. This view helps you spot large, problematic files at a glance.',
      [
        { color: '#22c55e', label: 'Green \u2014 low metric value (good)' },
        { color: '#f59e0b', label: 'Yellow \u2014 moderate (watch)' },
        { color: '#ef4444', label: 'Red \u2014 high metric value (act)' }
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
    btnRect.append(txt('\u25a6 Rectangles'));
    var btnCircle = el('button', { className: 'tm-layout-btn', 'data-mode': 'circle' });
    btnCircle.append(txt('\u25cb Circles'));
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
    detailClose.append(txt('\u00d7'));
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
    hint.append(txt('Click a directory to zoom in \u00b7 Click a file for details \u00b7 Use breadcrumbs to navigate back'));
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
      rootCrumb.append(txt('\ud83d\udcc1 /'));
      rootCrumb.addEventListener('click', function() {
        currentRoot = tree;
        navStack = [];
        animateTransition();
      });
      breadcrumb.append(rootCrumb);
      navStack.forEach(function(entry, i) {
        var sep = el('span', { className: 'tm-sep' });
        sep.append(txt('\u203a'));
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
      row('Hotspot score', f.hotspot_score.toFixed(1));
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
          v.append(txt(a.pct.toFixed(0) + '%'));
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
          if (dirLabel.length > maxLabelChars) dirLabel = dirLabel.slice(0, maxLabelChars - 1) + '\u2026';
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
            if (lbl.length > maxChars) lbl = lbl.slice(0, maxChars - 1) + '\u2026';
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
          if (dirLabel.length > maxChars) dirLabel = dirLabel.slice(0, maxChars - 1) + '\u2026';
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
            if (lbl.length > maxC) lbl = lbl.slice(0, maxC - 1) + '\u2026';
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
          tooltip.append(el('div', null, 'Hotspot: ' + f.hotspot_score.toFixed(1)));
          tooltip.style.display = 'block';
          tooltip.style.left = (e.clientX + 14) + 'px';
          tooltip.style.top = (e.clientY + 14) + 'px';
        }
      } else if (target.getAttribute('data-dir')) {
        var dn = target.getAttribute('data-dir');
        tooltip.replaceChildren();
        tooltip.append(el('div', { style: { fontWeight: '600', color: '#94a3b8' } }, '\ud83d\udcc1 ' + dn));
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
            history: vec![],
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

    #[test]
    fn html_treemap_has_detail_panel() {
        let html = render(&make_treemap_report()).unwrap();
        assert!(html.contains("tm-detail"), "Should contain tm-detail panel");
    }

    #[test]
    fn html_treemap_has_animated_transition() {
        let html = render(&make_treemap_report()).unwrap();
        assert!(
            html.contains("animateTransition"),
            "Should contain animated transition function"
        );
    }

    #[test]
    fn html_treemap_has_circle_pack() {
        let html = render(&make_treemap_report()).unwrap();
        assert!(
            html.contains("circlePack"),
            "Should contain circlePack layout function"
        );
    }

    #[test]
    fn html_treemap_has_layout_toggle() {
        let html = render(&make_treemap_report()).unwrap();
        assert!(
            html.contains("tm-layout-toggle"),
            "Should contain layout toggle control"
        );
    }

    #[test]
    fn html_tabs_have_info_banners() {
        let html = render(&make_treemap_report()).unwrap();
        assert!(
            html.contains("tab-info"),
            "Should contain tab info banner CSS class"
        );
        // Each tab should have a description explaining the metric
        assert!(
            html.contains("Hotspot Score"),
            "Hotspots tab should explain hotspot scoring"
        );
        assert!(
            html.contains("temporal coupling"),
            "Coupling tab should explain temporal coupling"
        );
        assert!(
            html.contains("knowledge distribution"),
            "Ownership tab should explain knowledge distribution"
        );
        assert!(
            html.contains("days since"),
            "Age tab should explain file age measurement"
        );
    }

    #[test]
    fn html_hotspot_scatter_is_clickable() {
        let html = render(&make_treemap_report()).unwrap();
        assert!(
            html.contains("hs-scatter-dot"),
            "Scatter plot circles should have hs-scatter-dot class for click targeting"
        );
        assert!(
            html.contains("hs-row-highlight"),
            "Should have CSS class for highlighting table rows"
        );
    }

    #[test]
    fn html_coupling_has_auto_exclude() {
        let html = render(&make_treemap_report()).unwrap();
        assert!(
            html.contains("isAutoExcluded"),
            "Coupling tab should have auto-exclude logic for interface/implementation pairs"
        );
    }

    #[test]
    fn html_coupling_rows_are_dismissable() {
        let html = render(&make_treemap_report()).unwrap();
        assert!(
            html.contains("cp-dismiss"),
            "Coupling tab rows should have dismiss buttons"
        );
    }
}
