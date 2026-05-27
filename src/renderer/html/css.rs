pub const CSS: &str = r#"
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
.explainer {
  grid-column: 1 / -1;
  background: #111827;
  border: 1px solid #1e293b;
  border-radius: 8px;
  font-size: 13px;
  line-height: 1.6;
  color: #94a3b8;
}
.explainer summary {
  cursor: pointer;
  padding: 12px 16px;
  font-weight: 600;
  font-size: 14px;
  color: #e2e8f0;
  list-style: none;
  display: flex;
  align-items: center;
  gap: 8px;
  user-select: none;
}
.explainer summary::-webkit-details-marker { display: none; }
.explainer summary::before {
  content: '\25b6';
  font-size: 10px;
  transition: transform 0.2s;
  color: #f59e0b;
}
.explainer[open] summary::before { transform: rotate(90deg); }
.explainer-body {
  padding: 0 16px 16px;
}
.explainer-body h4 {
  color: #e2e8f0;
  font-size: 13px;
  margin: 12px 0 4px;
}
.explainer-body h4:first-child { margin-top: 0; }
.explainer-body ul {
  margin: 4px 0 0 16px;
  padding: 0;
}
.explainer-body li { margin-bottom: 2px; }
.explainer-body code {
  background: #1e293b;
  padding: 1px 5px;
  border-radius: 3px;
  font-size: 12px;
  color: #f59e0b;
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
.metric-raw { color: #64748b; font-size: 12px; min-width: 60px; max-width: 320px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-align: right; }
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
.action-link { color: #38bdf8; text-decoration: underline; cursor: pointer; transition: color 0.2s; }
.action-link:hover { color: #7dd3fc; }
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
.tr-controls { display: flex; gap: 12px; align-items: center; margin-bottom: 16px; }
.tr-select { background: #0d1117; color: #c9d1d9; border: 1px solid #1e293b;
  border-radius: 6px; padding: 6px 12px; font-size: 14px; }
.tr-chart { width: 100%; background: #161b22; border-radius: 8px; padding: 16px; }
.tr-dot { cursor: pointer; }
.tr-dot:hover { r: 5; }
.tr-tooltip { position: fixed; background: #1e293b; color: #c9d1d9;
  padding: 8px 12px; border-radius: 6px; font-size: 12px;
  pointer-events: none; z-index: 1000; display: none; white-space: pre-line; }
.tr-empty { text-align: center; color: #8b949e; padding: 60px 20px; font-size: 16px; }
.tr-legend { display:flex; align-items:center; gap:8px; margin-left:auto; font-size:12px; color:#aaa; }
.tr-legend-dot { width:8px; height:8px; border-radius:50%; display:inline-block; }
.ac-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
  gap: 16px;
  padding: 24px;
}
.ac-card {
  background: #0d1117;
  border: 1px solid #1e293b;
  border-radius: 12px;
  padding: 20px;
  transition: border-color 0.15s;
}
.ac-card:hover { border-color: #334155; }
.ac-name {
  font-size: 16px;
  font-weight: 700;
  color: #e2e8f0;
  margin-bottom: 2px;
}
.ac-email { font-size: 11px; color: #64748b; margin-bottom: 12px; }
.ac-stats {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px 16px;
  margin-bottom: 12px;
}
.ac-stat-label {
  font-size: 11px;
  color: #64748b;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.ac-stat-value { font-size: 18px; font-weight: 700; color: #e2e8f0; }
.ac-badge {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-right: 6px;
}
.ac-files {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid #1e293b;
}
.ac-file-item {
  font-size: 12px;
  color: #94a3b8;
  padding: 2px 0;
  font-family: monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ac-toolbar {
  display: flex;
  gap: 12px;
  align-items: center;
  padding: 16px 24px;
  border-bottom: 1px solid #1e293b;
  background: #0d1117;
  flex-wrap: wrap;
}
.ac-search {
  background: #161b22;
  border: 1px solid #1e293b;
  border-radius: 6px;
  color: #e2e8f0;
  padding: 6px 12px;
  font-size: 13px;
  min-width: 200px;
}
.ac-search:focus { outline: none; border-color: #3b82f6; }
.ac-sort-btn {
  background: #161b22;
  border: 1px solid #1e293b;
  border-radius: 6px;
  color: #94a3b8;
  padding: 6px 12px;
  font-size: 12px;
  cursor: pointer;
}
.ac-sort-btn.active { border-color: #3b82f6; color: #e2e8f0; }
.th-tip {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 13px;
  height: 13px;
  border-radius: 50%;
  background: #1e293b;
  color: #64748b;
  font-size: 9px;
  font-weight: 700;
  cursor: help;
  flex-shrink: 0;
  margin-left: 4px;
  vertical-align: middle;
  line-height: 1;
  user-select: none;
}
.th-tip:hover { background: #334155; color: #e2e8f0; }
.cp-tooltip {
  position: fixed;
  background: #0d1117;
  border: 1px solid #334155;
  border-radius: 8px;
  padding: 10px 14px;
  font-size: 12px;
  font-weight: 400;
  color: #e2e8f0;
  pointer-events: none;
  z-index: 2000;
  display: none;
  max-width: 280px;
  line-height: 1.6;
  box-shadow: 0 4px 16px rgba(0,0,0,0.6);
  text-transform: none;
  letter-spacing: 0;
  white-space: normal;
}
/* ---- Audit tab ---- */
.section {
  background: #0d1117;
  border: 1px solid #1e293b;
  border-radius: 12px;
  padding: 20px 24px;
  margin-bottom: 16px;
}
.section-title {
  font-size: 15px;
  font-weight: 700;
  color: #e2e8f0;
  margin-bottom: 6px;
}
.section-sub {
  font-size: 12px;
  color: #64748b;
  margin-bottom: 16px;
  line-height: 1.5;
}
.path-cell { max-width: 400px; }
.path-dir { color: #475569; font-family: monospace; font-size: 12px; }
.path-name { color: #93c5fd; font-family: monospace; font-size: 12px; }
.conc-list { display: flex; flex-direction: column; gap: 10px; }
.conc-row { display: grid; grid-template-columns: 220px 1fr auto; gap: 12px; align-items: center; }
.conc-label { font-family: monospace; font-size: 12px; color: #94a3b8; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.conc-bar-area { background: #1e293b; border-radius: 4px; height: 10px; overflow: hidden; }
.conc-bar-fill { background: #3b82f6; height: 100%; border-radius: 4px; transition: width 0.3s; }
.conc-meta { display: flex; gap: 6px; flex-wrap: nowrap; }
.vel-chart { overflow-x: auto; }
.tab-error { padding: 24px; color: #ef4444; font-size: 13px; }
"#;
