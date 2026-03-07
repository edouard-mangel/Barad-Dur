# CI/CD Self-Contained HTML Report — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `barad-dur analyze . --html -o report.html` that produces a single self-contained HTML file suitable for CI/CD artifact upload.

**Architecture:** A new `renderer::html` module renders `AnalysisReport` as a string. It serializes the report to JSON and embeds it as `<script>const R={...}</script>` in an inline HTML + CSS + JS template. The template is a multi-hundred-line Rust string literal using `format!`. No external assets, no CDN, no build step.

**Tech Stack:** Rust string formatting, `serde_json::to_string()`, vanilla JS + SVG for rendering.

---

### Task 1: Add `--html` flag to CLI and wire into main.rs

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`
- Modify: `src/renderer/mod.rs`
- Create: `src/renderer/html.rs` (stub)

**Step 1: Add the flag to `AnalyzeArgs` in `src/cli.rs`**

After the `pub json: bool` field (line ~60), add:

```rust
/// Output as self-contained HTML report
#[arg(long)]
pub html: bool,
```

**Step 2: Write a test for the new flag in `src/cli.rs` tests block**

```rust
#[test]
fn html_flag() {
    let args = parse(&["barad-dur", "analyze", ".", "--html"]);
    assert!(args.html);
    assert!(!args.json);
}
```

**Step 3: Run test to verify it passes**

```bash
cd /home/edouard/WS/barad-dur
cargo test -q cli::tests::html_flag 2>&1
```
Expected: `test cli::tests::html_flag ... ok`

**Step 4: Update `src/main.rs` — suppress progress and add render branch**

Change line:
```rust
let show_progress = !args.json;
```
to:
```rust
let show_progress = !args.json && !args.html;
```

Replace the render block:
```rust
// Render
let output = if args.json {
    renderer::json::render(&report, args.pretty)?
} else if args.html {
    renderer::html::render(&report)?
} else {
    renderer::cli::render(&report, args.verbose)
};
```

**Step 5: Add `pub mod html;` to `src/renderer/mod.rs`**

```rust
pub mod cli;
pub mod html;
pub mod json;
```

**Step 6: Create a stub `src/renderer/html.rs`**

```rust
use anyhow::Result;
use crate::scorer::AnalysisReport;

pub fn render(_report: &AnalysisReport) -> Result<String> {
    Ok(String::from("<!-- TODO -->"))
}
```

**Step 7: Verify it compiles**

```bash
cargo build -q 2>&1
```
Expected: no errors.

**Step 8: Commit**

```bash
git add src/cli.rs src/main.rs src/renderer/mod.rs src/renderer/html.rs
git commit -m "feat: add --html flag and stub HTML renderer"
```

---

### Task 2: Implement the full HTML renderer

**Files:**
- Modify: `src/renderer/html.rs`

**Goal:** Replace the stub with a complete self-contained HTML renderer. All CSS, JS, and report data are inlined. No innerHTML — use `replaceChildren()` / DOM methods only for safety.

**Step 1: Write tests first**

Add to `src/renderer/html.rs`:

```rust
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
        assert_eq!(score_color(0),  "#ef4444");
    }
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo test -q renderer::html::tests 2>&1
```
Expected: several FAILs (stub returns `<!-- TODO -->`).

**Step 3: Implement the full `src/renderer/html.rs`**

Replace the entire file with the content below. Key implementation notes:
- `render()` calls `serde_json::to_string(report)?` and injects it into the HTML
- CSS is a `const &str` with all styles inlined
- JS is built via `build_js()` which returns a `String` (needed to avoid `{{` `}}` escaping issues with `format!`)
- All DOM manipulation in JS uses `createElement` / `append` — never innerHTML
- `app.replaceChildren()` clears the container safely

Full file:

```rust
use anyhow::Result;
use crate::scorer::AnalysisReport;

/// Render the analysis report as a self-contained HTML file.
/// All CSS, JS, and data are inlined. No external dependencies.
pub fn render(report: &AnalysisReport) -> Result<String> {
    let json = serde_json::to_string(report)?;
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

pub(crate) fn score_color(score: u32) -> &'static str {
    if score >= 71 { "#10b981" } else if score >= 41 { "#f59e0b" } else { "#ef4444" }
}

// ─── Inline CSS ───────────────────────────────────────────────────────────────

const CSS: &str = "\
*,*::before,*::after{box-sizing:border-box;margin:0;padding:0}\
html,body{height:100%;background:#080a0f;color:#e2e8f0;\
  font-family:'Segoe UI',system-ui,sans-serif;font-size:14px}\
#app{max-width:1200px;margin:0 auto;padding:2rem}\
h1{font-size:clamp(1.4rem,4vw,2.2rem);font-weight:800;letter-spacing:-0.02em;\
  color:#f1f5f9;line-height:1}\
.mono{font-family:'Cascadia Code','JetBrains Mono','Fira Code',monospace}\
.chip{display:inline-flex;align-items:center;gap:4px;\
  border:1px solid rgba(255,255,255,0.07);border-radius:999px;\
  padding:2px 10px;background:rgba(255,255,255,0.02);\
  font-size:.7rem;color:rgba(148,163,184,.6)}\
.chip span{color:rgba(148,163,184,.4)}\
.label{font-size:.65rem;letter-spacing:.1em;text-transform:uppercase;\
  color:rgba(148,163,184,.4)}\
header{margin-bottom:1.5rem}\
.header-row{display:flex;align-items:flex-start;justify-content:space-between;\
  flex-wrap:wrap;gap:1rem;margin-bottom:.75rem}\
.meta-chips{display:flex;flex-wrap:wrap;gap:.4rem;margin-top:.6rem}\
.brand{display:flex;align-items:center;gap:.4rem;font-weight:700;\
  font-size:.8rem;letter-spacing:.15em;text-transform:uppercase;\
  color:rgba(245,158,11,.6)}\
.dot{width:8px;height:8px;border-radius:50%;background:#f59e0b;\
  box-shadow:0 0 8px rgba(245,158,11,.8)}\
.divider{height:1px;background:rgba(255,255,255,.05);margin-bottom:0}\
.tabs{display:flex;gap:.2rem;border-bottom:1px solid rgba(255,255,255,.06);\
  margin-bottom:1.5rem}\
.tab{background:none;border:none;border-bottom:2px solid transparent;\
  padding:.6rem 1rem;cursor:pointer;font-family:inherit;font-size:.82rem;\
  font-weight:400;color:rgba(148,163,184,.5);transition:all .15s;\
  margin-bottom:-1px;letter-spacing:.04em;white-space:nowrap}\
.tab.active{border-bottom-color:#f59e0b;color:#f59e0b;font-weight:700}\
.tab-content{display:none}.tab-content.active{display:block}\
.overview-grid{display:grid;\
  grid-template-columns:clamp(180px,22%,260px) 1fr;\
  gap:2rem;margin-bottom:2rem;align-items:start}\
.left-col{display:flex;flex-direction:column;align-items:center;gap:2rem}\
.right-col{display:flex;flex-direction:column;gap:.75rem}\
.gauge-wrap{text-align:center}\
.gauge-label{font-size:.65rem;letter-spacing:.12em;text-transform:uppercase;\
  color:rgba(148,163,184,.4);margin-top:.2rem}\
.cat-card{border:1px solid rgba(255,255,255,.06);border-radius:10px;\
  background:rgba(255,255,255,.02);overflow:hidden}\
.cat-header{display:flex;align-items:center;justify-content:space-between;\
  padding:.65rem 1rem;cursor:pointer;user-select:none}\
.cat-header:hover{background:rgba(255,255,255,.02)}\
.cat-name{font-weight:600;font-size:.85rem;color:#e2e8f0}\
.cat-right{display:flex;align-items:center;gap:.75rem}\
.cat-score{font-family:'Cascadia Code','JetBrains Mono',monospace;\
  font-size:.8rem;font-weight:600}\
.cat-toggle{color:rgba(148,163,184,.4);font-size:.75rem;transition:transform .2s}\
.cat-body{padding:0 1rem .75rem;border-top:1px solid rgba(255,255,255,.04)}\
.bar-wrap{flex:1;height:4px;background:rgba(255,255,255,.06);\
  border-radius:2px;min-width:60px}\
.bar-fill{height:100%;border-radius:2px}\
.metric-row{display:flex;align-items:center;gap:.6rem;padding:.35rem 0;\
  border-bottom:1px solid rgba(255,255,255,.03)}\
.metric-row:last-child{border-bottom:none}\
.metric-name{font-size:.78rem;color:rgba(226,232,240,.8);min-width:140px}\
.metric-raw{font-family:monospace;font-size:.68rem;\
  color:rgba(148,163,184,.45);margin-left:.4rem;white-space:nowrap}\
.metric-score{font-size:.7rem;font-weight:600;min-width:28px;text-align:right}\
.actions-section{border:1px solid rgba(255,255,255,.06);border-radius:10px;\
  padding:1rem 1.25rem;background:rgba(255,255,255,.02)}\
.action-item{display:flex;gap:.75rem;padding:.45rem 0;\
  border-bottom:1px solid rgba(255,255,255,.03);\
  font-size:.8rem;color:rgba(226,232,240,.75)}\
.action-item:last-child{border-bottom:none}\
.action-num{color:rgba(148,163,184,.35);font-family:monospace;min-width:1.5rem}\
.view-card{border:1px solid rgba(255,255,255,.06);border-radius:10px;\
  padding:1rem 1.25rem;background:rgba(255,255,255,.02);overflow-x:auto}\
table{width:100%;border-collapse:collapse;\
  font-family:'Cascadia Code','JetBrains Mono',monospace;font-size:.72rem}\
thead tr{border-bottom:1px solid rgba(255,255,255,.08);\
  color:rgba(148,163,184,.5);font-size:.65rem;\
  letter-spacing:.08em;text-transform:uppercase}\
th{padding:.4rem .5rem;font-weight:400;text-align:right}\
th:first-child{text-align:left}\
td{padding:.4rem .5rem;text-align:right}\
td:first-child{text-align:left}\
tbody tr{border-bottom:1px solid rgba(255,255,255,.03)}\
.file-name{color:rgba(226,232,240,.85)}\
.file-dir{color:rgba(148,163,184,.3);margin-left:.3rem;font-size:.62rem}\
.inline-bar{display:flex;align-items:center;gap:.5rem}\
.track{flex:1;height:4px;background:rgba(255,255,255,.06);border-radius:2px}\
.fill{height:100%;border-radius:2px}\
.th-sort{cursor:pointer;user-select:none}\
.th-sort:hover{color:rgba(148,163,184,.8)}\
.th-sort.active-sort{color:#f59e0b}\
.own-bar{display:flex;height:8px;border-radius:4px;overflow:hidden;gap:1px}\
.own-seg{flex-shrink:0}\
.own-top{margin-top:.2rem;font-size:.62rem;color:rgba(148,163,184,.4)}\
.legend{display:flex;flex-wrap:wrap;gap:.5rem;margin-bottom:1rem}\
.legend-item{display:flex;align-items:center;gap:.3rem;\
  font-size:.65rem;color:rgba(148,163,184,.6)}\
.legend-dot{width:8px;height:8px;border-radius:50%;flex-shrink:0}\
.remote-card{border:1px solid rgba(255,255,255,.06);border-radius:8px;\
  padding:.5rem 1rem;background:rgba(255,255,255,.02);\
  display:flex;flex-wrap:wrap;gap:.75rem;align-items:center;\
  margin-top:1rem;font-size:.75rem}\
.remote-item{display:flex;align-items:center;gap:.3rem;\
  color:rgba(148,163,184,.6)}\
svg.scatter{display:block;overflow:visible}\
svg.radar{display:block}\
.no-data{font-family:monospace;font-size:.8rem;color:rgba(148,163,184,.4)}\
.hotspot-wrap{display:flex;flex-direction:column;gap:1.5rem}\
";

// ─── JS (returned as String to avoid format! escaping issues) ─────────────────

fn build_js() -> String {
    // NOTE: All {{ and }} in the JS are escaped Rust format! braces.
    // The JS itself uses single braces normally after this function returns.
    r#"(function(){
'use strict';
const R = window.R;

// ── Helpers ──────────────────────────────────────────────────────────────────
function el(tag, attrs) {
  const e = document.createElement(tag);
  const children = Array.prototype.slice.call(arguments, 2);
  if (attrs) Object.keys(attrs).forEach(function(k) {
    const v = attrs[k];
    if (k === 'style' && typeof v === 'object') {
      Object.keys(v).forEach(function(sk) { e.style[sk] = v[sk]; });
    } else if (k.slice(0,2) === 'on') {
      e.addEventListener(k.slice(2).toLowerCase(), v);
    } else {
      e.setAttribute(k, v);
    }
  });
  children.flat().forEach(function(c) {
    if (c == null) return;
    e.append(typeof c === 'string' ? document.createTextNode(c) : c);
  });
  return e;
}
function svgEl(tag, attrs) {
  const e = document.createElementNS('http://www.w3.org/2000/svg', tag);
  if (attrs) Object.keys(attrs).forEach(function(k) { e.setAttribute(k, attrs[k]); });
  const children = Array.prototype.slice.call(arguments, 2);
  children.flat().forEach(function(c) { if (c) e.append(c); });
  return e;
}
function txt(s) { return document.createTextNode(String(s)); }
function scoreColor(s) {
  return s >= 71 ? '#10b981' : s >= 41 ? '#f59e0b' : '#ef4444';
}
function fileParts(path) {
  const i = path.lastIndexOf('/');
  return i < 0 ? [path, ''] : [path.slice(i+1), path.slice(0, i+1)];
}
function scoreBar(score) {
  const color = scoreColor(score);
  const wrap = el('div', {class:'bar-wrap'});
  const fill = el('div', {class:'bar-fill', style:{width:score+'%',backgroundColor:color,boxShadow:'0 0 4px '+color+'40'}});
  wrap.append(fill);
  return wrap;
}

// ── Score gauge (SVG arc) ─────────────────────────────────────────────────────
function renderGauge(score, size) {
  size = size || 180;
  const cx = size/2, cy = size/2, r = size*0.38, stroke = size*0.1;
  const startAngle = -220 * Math.PI/180;
  const endAngle   =  40  * Math.PI/180;
  const totalAngle = endAngle - startAngle;
  const color = scoreColor(score);
  function arcPath(a1, a2) {
    const x1=cx+r*Math.cos(a1),y1=cy+r*Math.sin(a1);
    const x2=cx+r*Math.cos(a2),y2=cy+r*Math.sin(a2);
    const large = (a2-a1 > Math.PI) ? 1 : 0;
    return 'M '+x1+' '+y1+' A '+r+' '+r+' 0 '+large+' 1 '+x2+' '+y2;
  }
  const pct = Math.max(0, Math.min(100, score));
  const fillAngle = startAngle + totalAngle * (pct/100);
  const s = svgEl('svg', {width:size, height:size, class:'radar'},
    svgEl('path', {d:arcPath(startAngle,endAngle), fill:'none',
      stroke:'rgba(255,255,255,0.06)', 'stroke-width':stroke, 'stroke-linecap':'round'}),
    svgEl('path', {d:arcPath(startAngle,fillAngle), fill:'none',
      stroke:color, 'stroke-width':stroke, 'stroke-linecap':'round',
      style:'filter:drop-shadow(0 0 6px '+color+'80)'}),
    svgEl('text', {x:cx, y:cy+6, 'text-anchor':'middle', fill:color,
      'font-size':Math.round(size*0.22), 'font-weight':'800',
      'font-family':'inherit'}, txt(score))
  );
  return el('div', {class:'gauge-wrap'}, s,
    el('div', {class:'gauge-label mono'}, txt('Overall Score')));
}

// ── Radar chart ───────────────────────────────────────────────────────────────
function renderRadar(categories, size) {
  size = size || 210;
  const cx=size/2, cy=size/2, r=size*0.35;
  const n = categories.length;
  if (n < 3) return el('div', null);
  const angles = categories.map(function(_,i){ return (i/n)*2*Math.PI - Math.PI/2; });
  function pt(angle, frac) { return [cx+frac*r*Math.cos(angle), cy+frac*r*Math.sin(angle)]; }
  const rings = [0.25,0.5,0.75,1.0].map(function(f) {
    const d = angles.map(function(a,i){ return (i===0?'M':'L')+pt(a,f).join(' '); }).join(' ')+'Z';
    return svgEl('path', {d:d, fill:'none', stroke:'rgba(255,255,255,0.06)', 'stroke-width':'1'});
  });
  const spokes = angles.map(function(a) {
    const p = pt(a,1);
    return svgEl('line', {x1:cx,y1:cy,x2:p[0],y2:p[1],stroke:'rgba(255,255,255,0.06)','stroke-width':'1'});
  });
  const polyPts = categories.map(function(c,i){ return pt(angles[i], c.score/100); });
  const poly = polyPts.map(function(p,i){ return (i===0?'M':'L')+p.join(' '); }).join(' ')+'Z';
  const labels = categories.map(function(c,i) {
    const lp = pt(angles[i],1.18);
    return svgEl('text', {x:lp[0],y:lp[1],'text-anchor':'middle',
      'dominant-baseline':'middle',fill:'rgba(148,163,184,0.5)',
      'font-size':'9','font-family':'inherit'}, txt(c.name.split(' ')[0]));
  });
  return svgEl('svg', {width:size,height:size,class:'radar'},
    rings, spokes,
    svgEl('path', {d:poly, fill:'rgba(245,158,11,0.15)', stroke:'#f59e0b', 'stroke-width':'1.5'}),
    labels
  );
}

// ── Category cards ────────────────────────────────────────────────────────────
function renderCategories(categories) {
  return categories.map(function(cat) {
    const color = scoreColor(cat.score);
    const toggle = el('span', {class:'cat-toggle'}, txt('▾'));
    const header = el('div', {class:'cat-header'},
      el('span', {class:'cat-name'}, txt(cat.name)),
      el('div', {class:'cat-right'},
        scoreBar(cat.score),
        el('span', {class:'cat-score mono', style:{color:color}}, txt(cat.score)),
        toggle
      )
    );
    const metricsEls = (cat.metrics||[]).map(function(m) {
      const mcolor = scoreColor(m.score);
      const raw = Array.isArray(m.raw_value) ? m.raw_value.join(', ')
                : (typeof m.raw_value === 'object' && m.raw_value !== null)
                  ? JSON.stringify(m.raw_value)
                : String(m.raw_value != null ? m.raw_value : '');
      return el('div', {class:'metric-row'},
        el('span', {class:'metric-name'}, txt(m.name)),
        scoreBar(m.score),
        el('span', {class:'metric-score', style:{color:mcolor}}, txt(m.score)),
        el('span', {class:'metric-raw mono', title:m.description}, txt(raw))
      );
    });
    const body = el('div', {class:'cat-body'}, metricsEls);
    const expanded = cat.score < 70;
    if (!expanded) body.style.display = 'none';
    header.addEventListener('click', function() {
      const open = body.style.display !== 'none';
      body.style.display = open ? 'none' : 'block';
      toggle.textContent = open ? '▾' : '▴';
    });
    return el('div', {class:'cat-card'}, header, body);
  });
}

// ── Top actions ───────────────────────────────────────────────────────────────
function renderActions(actions) {
  if (!actions || !actions.length) return el('div', null);
  const items = actions.map(function(a, i) {
    return el('div', {class:'action-item'},
      el('span', {class:'action-num mono'}, txt((i+1)+'.')),
      el('span', null, txt(a))
    );
  });
  return el('div', {class:'actions-section'},
    el('p', {class:'label', style:{marginBottom:'.75rem'}}, txt('Top actions')),
    items
  );
}

// ── Remote meta ───────────────────────────────────────────────────────────────
function renderRemoteMeta(meta) {
  if (!meta) return el('div', null);
  const items = [
    meta.stars != null && el('span', {class:'remote-item'}, txt('\u2605 '+meta.stars)),
    meta.language      && el('span', {class:'remote-item'}, txt(meta.language)),
    meta.open_issues != null && el('span', {class:'remote-item'}, txt(meta.open_issues+' open issues')),
    meta.description   && el('span', {class:'remote-item', style:{color:'rgba(148,163,184,.5)'}}, txt(meta.description)),
  ].filter(Boolean);
  return el('div', {class:'remote-card mono'}, items);
}

// ── Overview tab ──────────────────────────────────────────────────────────────
function renderOverview() {
  const grid = el('div', {class:'overview-grid'},
    el('div', {class:'left-col'}, renderGauge(R.overall_score,180), renderRadar(R.categories,210)),
    el('div', {class:'right-col'}, renderCategories(R.categories))
  );
  return el('div', null, grid, renderActions(R.top_actions));
}

// ── Hotspots tab ──────────────────────────────────────────────────────────────
function renderHotspots() {
  const files = (R.file_hotspots||[]).slice();
  if (!files.length) return el('p', {class:'no-data'}, txt('No hotspot data available.'));
  const maxCC  = Math.max.apply(null, files.map(function(f){return f.cyclomatic_complexity;})) || 1;
  const maxCh  = Math.max.apply(null, files.map(function(f){return f.churn_count;})) || 1;
  const maxLOC = Math.max.apply(null, files.map(function(f){return f.loc;})) || 1;
  const W=520,H=240,ml=45,mr=15,mt=15,mb=35,w=W-ml-mr,h=H-mt-mb;
  function xp(v){ return ml+v/maxCC*w; }
  function yp(v){ return mt+h-v/maxCh*h; }
  function rp(v){ return 2+Math.sqrt(v/maxLOC)*12; }
  function hotColor(s){ return s>70?'#ef4444':s>40?'#f59e0b':'#10b981'; }

  const circles = files.map(function(f) {
    const c = svgEl('circle', {
      cx: xp(f.cyclomatic_complexity).toFixed(1),
      cy: yp(f.churn_count).toFixed(1),
      r:  rp(f.loc).toFixed(1),
      fill: hotColor(f.hotspot_score),
      'fill-opacity':'0.45',
      stroke: hotColor(f.hotspot_score),
      'stroke-width':'1',
      'stroke-opacity':'0.8'
    });
    const title = svgEl('title', null);
    title.textContent = f.path+'\nscore:'+f.hotspot_score.toFixed(0)+' churn:'+f.churn_count+' cc:'+f.cyclomatic_complexity+' loc:'+f.loc;
    c.append(title);
    return c;
  });

  function ticks(scaleFn, max, n, horizontal) {
    const arr = [];
    for (var i=0; i<=n; i++) {
      const v = Math.round(max*i/n);
      const pos = scaleFn(v);
      if (horizontal) {
        arr.push(svgEl('line',{x1:pos,y1:mt+h,x2:pos,y2:mt+h+4,stroke:'rgba(255,255,255,0.1)','stroke-width':'1'}));
        const t = svgEl('text',{x:pos,y:mt+h+14,'text-anchor':'middle',fill:'rgba(148,163,184,.45)','font-size':'9','font-family':'inherit'});
        t.textContent = v;
        arr.push(t);
      } else {
        arr.push(svgEl('line',{x1:ml-4,y1:pos,x2:ml,y2:pos,stroke:'rgba(255,255,255,0.1)','stroke-width':'1'}));
        const t = svgEl('text',{x:ml-7,y:pos+3,'text-anchor':'end',fill:'rgba(148,163,184,.45)','font-size':'9','font-family':'inherit'});
        t.textContent = v;
        arr.push(t);
      }
    }
    return arr;
  }

  const axisX = svgEl('text',{x:ml+w/2,y:H-3,'text-anchor':'middle',fill:'rgba(148,163,184,.35)','font-size':'9','font-family':'inherit'});
  axisX.textContent = 'Cyclomatic complexity \u2192';
  const axisY = svgEl('text',{transform:'rotate(-90)',x:-(mt+h/2),y:9,'text-anchor':'middle',fill:'rgba(148,163,184,.35)','font-size':'9','font-family':'inherit'});
  axisY.textContent = 'Churn \u2192';

  const scatterSvg = svgEl('svg',{width:W,height:H,class:'scatter'},
    ticks(xp,maxCC,5,true), ticks(yp,maxCh,4,false), axisX, axisY, circles);

  // Sortable table
  var sortKey = 'hotspot_score';
  const tbody = el('tbody', null);

  function renderRows() {
    while (tbody.firstChild) tbody.removeChild(tbody.firstChild);
    const sorted = files.slice().sort(function(a,b){ return (b[sortKey]||0)-(a[sortKey]||0); }).slice(0,50);
    sorted.forEach(function(f) {
      const color = hotColor(f.hotspot_score);
      const parts = fileParts(f.path);
      const row = el('tr', null,
        el('td', null,
          el('span',{class:'file-name'},txt(parts[0])),
          parts[1] && el('span',{class:'file-dir mono'},txt(parts[1]))
        ),
        el('td',{style:{color:color,fontWeight:'600'}},txt(f.hotspot_score.toFixed(0))),
        el('td',{style:{color:'rgba(226,232,240,.7)'}},txt(f.churn_count)),
        el('td',{style:{color:'rgba(226,232,240,.7)'}},txt(f.cyclomatic_complexity)),
        el('td',{style:{color:'rgba(226,232,240,.7)'}},txt(f.loc)),
        el('td',{style:{color:'rgba(148,163,184,.45)'}},txt(f.public_methods)),
        el('td',{style:{color:'rgba(148,163,184,.45)'}},txt(f.properties))
      );
      tbody.append(row);
    });
  }
  renderRows();

  function thSort(label, key) {
    const th = el('th', {class:'th-sort'+(key===sortKey?' active-sort':'')});
    th.textContent = label+' \u2195';
    th.addEventListener('click', function() {
      sortKey = key;
      th.closest('thead').querySelectorAll('.th-sort').forEach(function(t){ t.classList.remove('active-sort'); });
      th.classList.add('active-sort');
      renderRows();
    });
    return th;
  }

  const thead = el('thead', null, el('tr', null,
    el('th',{style:{textAlign:'left'}},txt('File')),
    thSort('Score','hotspot_score'), thSort('Churn','churn_count'),
    thSort('CC','cyclomatic_complexity'), thSort('LOC','loc'),
    el('th',null,txt('Methods')), el('th',null,txt('Props'))
  ));

  return el('div',{class:'hotspot-wrap'},
    el('div',{class:'view-card'},
      el('p',{class:'label',style:{marginBottom:'.75rem'}},txt('Hotspot quadrant \u2014 bubble size = LOC \u00b7 color = risk')),
      scatterSvg
    ),
    el('div',{class:'view-card'}, el('table',null,thead,tbody))
  );
}

// ── Coupling tab ──────────────────────────────────────────────────────────────
function renderCoupling() {
  const pairs = (R.coupling_pairs||[]).slice().sort(function(a,b){ return b.coupling_pct-a.coupling_pct; });
  if (!pairs.length) return el('p',{class:'no-data'},txt('No coupling pairs detected (threshold: 3 co-changes).'));
  function coupColor(p){ return p>70?'#ef4444':p>40?'#f59e0b':'#10b981'; }
  const rows = pairs.map(function(p) {
    const color = coupColor(p.coupling_pct);
    const pa = fileParts(p.file_a), pb = fileParts(p.file_b);
    return el('tr', null,
      el('td',{title:p.file_a,style:{maxWidth:'200px',overflow:'hidden',textOverflow:'ellipsis',whiteSpace:'nowrap'}},
        el('span',{class:'file-name'},txt(pa[0])), pa[1] && el('span',{class:'file-dir mono'},txt(pa[1]))),
      el('td',{title:p.file_b,style:{maxWidth:'200px',overflow:'hidden',textOverflow:'ellipsis',whiteSpace:'nowrap'}},
        el('span',{class:'file-name'},txt(pb[0])), pb[1] && el('span',{class:'file-dir mono'},txt(pb[1]))),
      el('td',{style:{color:'rgba(148,163,184,.6)'}},txt(p.co_changes)),
      el('td',{style:{minWidth:'160px'}},
        el('div',{class:'inline-bar'},
          el('div',{class:'track'},
            el('div',{class:'fill',style:{width:p.coupling_pct+'%',backgroundColor:color,boxShadow:'0 0 6px '+color+'60'}})),
          el('span',{style:{color:color,fontWeight:'600',minWidth:'2.5rem',textAlign:'right'}},txt(p.coupling_pct.toFixed(0)+'%'))
        )
      )
    );
  });
  return el('div',{class:'view-card'},
    el('p',{class:'label',style:{marginBottom:'.75rem'}},txt('Temporal coupling \u2014 files that change together')),
    el('table',null,
      el('thead',null,el('tr',null,
        el('th',{style:{textAlign:'left'}},txt('File A')),
        el('th',{style:{textAlign:'left'}},txt('File B')),
        el('th',null,txt('Co-changes')),
        el('th',{style:{textAlign:'left',minWidth:'160px'}},txt('Coupling %'))
      )),
      el('tbody',null,rows)
    )
  );
}

// ── Ownership tab ─────────────────────────────────────────────────────────────
var PALETTE=['#f59e0b','#10b981','#3b82f6','#a78bfa','#f472b6','#34d399','#fb923c','#60a5fa'];
function renderOwnership() {
  const ownership = (R.author_ownership||[]).slice()
    .sort(function(a,b){
      if(b.authors.length!==a.authors.length) return b.authors.length-a.authors.length;
      return ((a.authors[0]&&a.authors[0].pct)||100)-((b.authors[0]&&b.authors[0].pct)||100);
    }).slice(0,60);
  if (!ownership.length) return el('p',{class:'no-data'},txt('No ownership data available.'));
  const allAuthors = [];
  ownership.forEach(function(f){ f.authors.forEach(function(a){ if(allAuthors.indexOf(a.name)<0) allAuthors.push(a.name); }); });
  function authorColor(name){ return PALETTE[allAuthors.indexOf(name)%PALETTE.length]||'#4a5568'; }
  const legendItems = allAuthors.slice(0,8).map(function(name){
    return el('div',{class:'legend-item'},
      el('div',{class:'legend-dot',style:{backgroundColor:authorColor(name)}}),
      txt('\u00a0'+name));
  });
  const rows = ownership.map(function(f) {
    const parts = fileParts(f.path);
    const segs = f.authors.map(function(a){
      const seg = el('div',{class:'own-seg',title:a.name+': '+a.pct.toFixed(0)+'%',style:{width:a.pct+'%',backgroundColor:authorColor(a.name)}});
      return seg;
    });
    const bar = el('div',{class:'own-bar'},segs);
    const top = f.authors[0] ? f.authors[0].name+' '+f.authors[0].pct.toFixed(0)+'%' : '';
    return el('tr',null,
      el('td',{title:f.path,style:{maxWidth:'220px',overflow:'hidden',textOverflow:'ellipsis',whiteSpace:'nowrap'}},
        el('span',{class:'file-name'},txt(parts[0])), parts[1] && el('span',{class:'file-dir mono'},txt(parts[1]))),
      el('td',{style:{minWidth:'180px'}}, bar, el('div',{class:'own-top'},txt(top))),
      el('td',{style:{color:f.authors.length>3?'#f59e0b':'rgba(148,163,184,.5)'}},txt(f.authors.length))
    );
  });
  return el('div',{class:'view-card'},
    el('p',{class:'label',style:{marginBottom:'.75rem'}},txt('Author ownership \u2014 blame distribution per file')),
    el('div',{class:'legend'},legendItems),
    el('table',null,
      el('thead',null,el('tr',null,
        el('th',{style:{textAlign:'left'}},txt('File')),
        el('th',{style:{textAlign:'left'}},txt('Ownership')),
        el('th',null,txt('Authors'))
      )),
      el('tbody',null,rows)
    )
  );
}

// ── Age tab ───────────────────────────────────────────────────────────────────
function renderAge() {
  const ages = (R.file_ages||[]).slice();
  if (!ages.length) return el('p',{class:'no-data'},txt('No age data available.'));
  const maxDays = (ages[0]&&ages[0].days_since_modified)||1;
  function ageBand(d){
    if(d<=30)  return {color:'#10b981',label:'fresh'};
    if(d<=90)  return {color:'#34d399',label:'< 3mo'};
    if(d<=180) return {color:'#f59e0b',label:'< 6mo'};
    if(d<=365) return {color:'#fb923c',label:'< 1yr'};
    return {color:'#ef4444',label:'> 1yr'};
  }
  const rows = ages.map(function(f) {
    const band = ageBand(f.days_since_modified);
    const pct = (f.days_since_modified/maxDays)*100;
    const date = new Date(f.last_modified).toLocaleDateString('en-US',{year:'numeric',month:'short',day:'numeric'});
    const parts = fileParts(f.path);
    return el('tr',null,
      el('td',{title:f.path,style:{maxWidth:'240px',overflow:'hidden',textOverflow:'ellipsis',whiteSpace:'nowrap'}},
        el('span',{class:'file-name'},txt(parts[0])), parts[1] && el('span',{class:'file-dir mono'},txt(parts[1]))),
      el('td',{style:{minWidth:'160px'}},
        el('div',{class:'inline-bar'},
          el('div',{class:'track'},
            el('div',{class:'fill',style:{width:pct+'%',backgroundColor:band.color,boxShadow:'0 0 4px '+band.color+'40'}})),
          el('span',{style:{color:band.color,fontSize:'.65rem',minWidth:'2.8rem'}},txt(band.label))
        )
      ),
      el('td',{style:{color:'rgba(148,163,184,.5)'}},txt(f.days_since_modified)),
      el('td',{style:{color:'rgba(148,163,184,.35)',fontSize:'.65rem'}},txt(date))
    );
  });
  return el('div',{class:'view-card'},
    el('p',{class:'label',style:{marginBottom:'.75rem'}},txt('Code age \u2014 sorted by staleness (oldest first)')),
    el('table',null,
      el('thead',null,el('tr',null,
        el('th',{style:{textAlign:'left'}},txt('File')),
        el('th',{style:{textAlign:'left',minWidth:'160px'}},txt('Age')),
        el('th',null,txt('Days')),
        el('th',null,txt('Last modified'))
      )),
      el('tbody',null,rows)
    )
  );
}

// ── App shell ─────────────────────────────────────────────────────────────────
var TABS=[['overview','Overview'],['hotspots','Hotspots'],['coupling','Coupling'],['ownership','Ownership'],['age','Age']];
var activeTab='overview';

function renderApp() {
  const app = document.getElementById('app');
  app.replaceChildren();

  const chips = [
    ['branch',R.branch],['window',R.time_window_months+'mo'],
    ['commits',R.total_commits],['authors',R.total_authors],['files',R.total_files]
  ].map(function(pair){
    return el('span',{class:'chip mono'},el('span',null,txt(pair[0]+' ')),txt(String(pair[1])));
  });

  const header = el('header',null,
    el('div',{class:'header-row'},
      el('div',null,
        el('h1',null,txt(R.repo_name)),
        el('div',{class:'meta-chips'},chips)
      ),
      el('div',{class:'brand'},el('div',{class:'dot'}),txt('Barad-d\u00fbr'))
    ),
    R.remote_meta ? renderRemoteMeta(R.remote_meta) : el('span',null)
  );

  const divider = el('div',{class:'divider'});
  const tabBar = el('div',{class:'tabs'});
  const tabContents = {};
  const tabButtons = {};

  TABS.forEach(function(pair) {
    const id=pair[0], label=pair[1];
    const btn = el('button',{class:'tab'+(id===activeTab?' active':''), 'data-tab':id},txt(label));
    btn.addEventListener('click',function(){
      activeTab=id;
      Object.keys(tabButtons).forEach(function(k){ tabButtons[k].classList.remove('active'); });
      btn.classList.add('active');
      Object.keys(tabContents).forEach(function(k){ tabContents[k].classList.remove('active'); });
      tabContents[id].classList.add('active');
    });
    tabBar.append(btn);
    tabButtons[id]=btn;
    const content=el('div',{class:'tab-content'+(id===activeTab?' active':'')});
    tabContents[id]=content;
  });

  tabContents['overview'].append(renderOverview());
  tabContents['hotspots'].append(renderHotspots());
  tabContents['coupling'].append(renderCoupling());
  tabContents['ownership'].append(renderOwnership());
  tabContents['age'].append(renderAge());

  const footer = el('footer',{style:{
    marginTop:'3rem',paddingTop:'1rem',
    borderTop:'1px solid rgba(255,255,255,.04)',
    textAlign:'center',fontFamily:'monospace',fontSize:'.65rem',
    color:'rgba(148,163,184,.25)',letterSpacing:'.08em'
  }},txt('barad-d\u00fbr repository intelligence'));

  app.append(header, divider, tabBar,
    tabContents['overview'], tabContents['hotspots'],
    tabContents['coupling'], tabContents['ownership'],
    tabContents['age'], footer);
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
        assert_eq!(score_color(0),  "#ef4444");
    }
}
```

**Step 4: Run the tests**

```bash
cargo test -q renderer::html 2>&1
```
Expected: all 5 tests pass.

**Step 5: Commit**

```bash
git add src/renderer/html.rs
git commit -m "feat: implement self-contained HTML renderer with tabs, gauge, radar, all views"
```

---

### Task 3: End-to-end smoke test

**Step 1: Build release binary**

```bash
cargo build --release -q 2>&1
```
Expected: no errors.

**Step 2: Generate a report**

```bash
./target/release/barad-dur analyze . --html -o /tmp/bd-report.html 2>&1
```
Expected: exits silently (no stdout, no errors).

**Step 3: Verify the output**

```bash
wc -c /tmp/bd-report.html
grep -c 'window.R=' /tmp/bd-report.html
grep '"repo_name"' /tmp/bd-report.html | head -c 100
```
Expected:
- File size: between 40 000 and 500 000 bytes
- Exactly 1 match for `window.R=`
- A line containing `"repo_name":"barad-dur"` (or whatever the actual repo name is)

**Step 4: Verify all existing tests still pass**

```bash
cargo test -q 2>&1 | tail -5
```
Expected: `test result: ok. N passed; 0 failed`

**Step 5: Commit design docs**

```bash
git add docs/plans/2026-03-07-cicd-html-report-design.md docs/plans/2026-03-07-cicd-html-report.md
git commit -m "docs: CI/CD HTML report design and implementation plan"
```

---

### Task 4: Add Makefile target

**Files:**
- Modify: `Makefile`

**Step 1: Read the Makefile to understand current targets, then add**

After the `analyze` target, add:

```makefile
OUTPUT_HTML ?= report.html

## Generate self-contained HTML report (TARGET=. OUTPUT_HTML=report.html)
html-report:
	cargo run --release -- analyze $(TARGET) --html -o $(OUTPUT_HTML)
```

**Step 2: Test it**

```bash
cd /home/edouard/WS/barad-dur
make html-report OUTPUT_HTML=/tmp/makefile-test.html 2>&1 | tail -3
ls -lh /tmp/makefile-test.html
```
Expected: file exists, non-zero size.

**Step 3: Commit**

```bash
git add Makefile
git commit -m "feat: add html-report Makefile target"
```

---

## CI/CD Usage Examples

**GitHub Actions:**
```yaml
- name: Analyze repository
  run: barad-dur analyze . --html -o barad-dur-report.html

- name: Upload report
  uses: actions/upload-artifact@v4
  with:
    name: barad-dur-report
    path: barad-dur-report.html
    retention-days: 30
```

**GitLab CI:**
```yaml
analyze:
  script:
    - barad-dur analyze . --html -o report.html
  artifacts:
    paths: [report.html]
    expire_in: 30 days
```
