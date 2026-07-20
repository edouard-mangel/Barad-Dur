
  /* ---- Hotspots tab ---- */

  /* The builders below are pure DOM producers over `files` + a `state`
     object (sort/filter/dismiss/selection). The tab's event wiring and
     state mutation stay inside buildHotspotsTab. */

  /* Static intro: what the tab shows and how to read churn. */
  function hsIntro() {
    var info = buildTabInfo(
      'Hotspot Score — Where risk concentrates',
      'Files are ranked by a composite Hotspot Score combining cyclomatic complexity (code branching), churn count (how often the file changes), and lines of code. High-churn, high-complexity files are the most likely sources of bugs and the hardest to review. Focus refactoring efforts on the top-right corner of the scatter plot.',
      [
        { color: 'var(--c-good-lo)', label: 'Low risk — simple + rarely changed' },
        { color: 'var(--c-warn)',    label: 'Medium — monitor these files' },
        { color: 'var(--c-danger)',  label: 'High risk — complex + frequently changed' }
      ]
    );
    var explainer = buildExplainer('Understanding churn and why it matters', [
      {
        heading: 'What is churn?',
        text: 'Churn is the number of times a file has been modified (committed to) within the analysis window. A file touched in 30 separate commits has a churn count of 30. Renames and moves are tracked as the same logical file when git detects them.'
      },
      {
        heading: 'Why churn alone is not enough',
        text: 'High churn is not inherently bad — a configuration file or changelog will naturally have high churn. Churn becomes a risk signal only when combined with high complexity: a complex file that changes often is statistically more likely to introduce defects because each change interacts with more code paths.'
      },
      {
        heading: 'How the Hotspot Score combines churn with complexity',
        items: [
          'Churn count and cyclomatic complexity are each normalized to a 0–1 scale (divided by the repository maximum).',
          'The hotspot score is the product of normalized churn, normalized complexity, and a size factor (LOC), scaled to 0–100.',
          'This means a file must rank high on multiple dimensions to surface as a true hotspot — one dimension alone will not flag it.'
        ]
      },
      {
        heading: 'What to do with high-churn hotspots',
        items: [
          'Break large files apart — extract stable logic into smaller modules so that changes are isolated.',
          'Increase test coverage — high-churn files benefit most from regression tests.',
          'Review ownership — if many authors touch the same hotspot, coordinate on conventions.',
          'Reduce complexity — simplify branching (fewer if/match arms) to make future changes safer.',
          'Watch the trend — a file whose churn is rising over time is an escalating risk.'
        ]
      },
      {
        heading: 'Reading the scatter plot',
        items: [
          'X-axis = cyclomatic complexity (number of independent code paths).',
          'Y-axis = churn count (commits touching the file in the analysis window).',
          'Bubble size = lines of code.',
          'Top-right corner = high complexity + high churn — these are the files that need attention first.'
        ]
      }
    ]);
    return [info, explainer];
  }

  /* Complexity-vs-churn scatter. Returns the card plus the svg and a
     makeDot factory so the tab can plot off-cap files on demand. */
  function hsScatterPlot(files) {
    var card = el('div', { className: 'view-card', style: { padding: '16px' } });
    var heading = el('div', { style: { marginBottom: '12px' } });
    var h = el('span', { className: 'label' });
    h.append(txt('Complexity vs Churn (radius = LOC)'));
    heading.append(h);
    card.append(heading);

    var maxCC = 1, maxChurn = 1, maxLOC = 1;
    files.forEach(function(f) {
      if (f.cyclomatic_complexity > maxCC) maxCC = f.cyclomatic_complexity;
      if (f.churn_count > maxChurn) maxChurn = f.churn_count;
      if (f.loc > maxLOC) maxLOC = f.loc;
    });

    var svgW = 340, svgH = 220, pad = 36;
    var svg = svgEl('svg', {
      class: 'scatter',
      viewBox: '0 0 ' + svgW + ' ' + svgH,
      preserveAspectRatio: 'xMidYMid meet'
    });

    // Axes
    svg.append(svgEl('line', { x1: String(pad), y1: String(pad), x2: String(pad), y2: String(svgH - pad), stroke: '#1e293b', 'stroke-width': '1' }));
    svg.append(svgEl('line', { x1: String(pad), y1: String(svgH - pad), x2: String(svgW - pad), y2: String(svgH - pad), stroke: '#1e293b', 'stroke-width': '1' }));

    // Axis labels
    var xLabel = svgEl('text', { x: String((svgW + pad) / 2), y: String(svgH - 6), 'text-anchor': 'middle', fill: '#475569', 'font-size': '9', 'font-family': 'sans-serif' });
    xLabel.append(txt('Cyclomatic Complexity'));
    svg.append(xLabel);

    var yLabel = svgEl('text', { x: '10', y: String(svgH / 2), 'text-anchor': 'middle', fill: '#475569', 'font-size': '9', 'font-family': 'sans-serif', transform: 'rotate(-90, 10, ' + (svgH / 2) + ')' });
    yLabel.append(txt('Churn'));
    svg.append(yLabel);

    var plotW = svgW - pad * 2;
    var plotH = svgH - pad * 2;

    // Numeric tick labels so the axes are readable without hovering each bubble
    [0, Math.round(maxCC / 2), maxCC].forEach(function(v) {
      var tickX = pad + (v / maxCC) * plotW;
      var t = svgEl('text', {
        x: String(tickX), y: String(svgH - pad + 10), 'text-anchor': 'middle',
        fill: '#475569', 'font-size': '8', 'font-family': 'sans-serif', class: 'hs-axis-tick'
      });
      t.append(txt(String(v)));
      svg.append(t);
    });
    [0, Math.round(maxChurn / 2), maxChurn].forEach(function(v) {
      var tickY = (svgH - pad) - (v / maxChurn) * plotH;
      var t = svgEl('text', {
        x: String(pad - 4), y: String(tickY + 3), 'text-anchor': 'end',
        fill: '#475569', 'font-size': '8', 'font-family': 'sans-serif', class: 'hs-axis-tick'
      });
      t.append(txt(String(v)));
      svg.append(t);
    });

    function makeDot(f) {
      var cx = pad + (f.cyclomatic_complexity / maxCC) * plotW;
      var cy = (svgH - pad) - (f.churn_count / maxChurn) * plotH;
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
      return circle;
    }

    files.slice(0, 300).forEach(function(f) {
      svg.append(makeDot(f));
    });

    card.append(svg);
    return { card: card, svg: svg, makeDot: makeDot };
  }

  /* Mini bar chart of f.churn_timeline: commits per 1/12 of the window,
     oldest on the left. All rows share the window axis, so shapes compare. */
  function hsSparkline(buckets) {
    var bw = 4, gap = 1, h = 14;
    var w = buckets.length * (bw + gap);
    var svg = svgEl('svg', {
      class: 'hs-sparkline', width: String(w), height: String(h),
      viewBox: '0 0 ' + w + ' ' + h
    });
    var max = buckets.reduce(function(m, v) { return Math.max(m, v); }, 1);
    buckets.forEach(function(v, i) {
      var bh = v === 0 ? 1 : Math.max(2, Math.round(v / max * h));
      svg.append(svgEl('rect', {
        x: String(i * (bw + gap)), y: String(h - bh),
        width: String(bw), height: String(bh), rx: '1',
        fill: v === 0 ? '#1e293b' : '#60a5fa'
      }));
    });
    var titleEl = svgEl('title');
    titleEl.append(txt('Commits per 1/12 of the analysis window (oldest → newest)'));
    svg.append(titleEl);
    return svg;
  }

  var HS_COL_TIPS = {
    File: 'Path relative to the repository root. Click a row to highlight its bubble in the scatter plot.',
    Score: 'Composite hotspot score 0–100: normalized churn (50%), cyclomatic complexity (30%) and '
      + 'LOC (20%), each relative to the repository maximum. High = complex, large and frequently changed.',
    CC: 'Cyclomatic complexity — the number of independent paths through the file’s code '
      + '(decision points such as if/match/loops), measured by tree-sitter AST analysis.',
    Churn: 'Number of commits that touched this file within the analysis window.',
    Trend: 'Commits touching this file per 1/12 of the analysis window, oldest on the left. '
      + 'All rows share the same time axis, so shapes are comparable.',
    Bugs: 'Commits touching this file whose message contains fix, bug, broken, crash or regression '
      + '(case-insensitive substring match). A heuristic for how often the file needs fixing — '
      + 'displayed for context, not part of the Score.',
    LOC: 'Lines of code, excluding blanks and comments.',
    Coupling: 'Pressman coupling findings in this file: Cn = content, Cm = common, Ct = control. '
      + 'Content and Common findings multiply the Score (configurable hotspot_multiplier, default 1.25).'
  };

  /* Per-kind coupling finding badge cell (Cn/Cm/Ih/Ct), or a dash. */
  function hsCouplingCell(f) {
    var cell = el('td');
    var cn = f.content_findings || 0;
    var cm = f.common_findings || 0;
    var ih = f.inheritance_findings || 0;
    var ct = f.control_findings || 0;
    if (cn + cm + ih + ct === 0) {
      cell.append(txt('—'));
      return cell;
    }
    var labels = [];
    if (cn) labels.push('Cn ' + cn);
    if (cm) labels.push('Cm ' + cm);
    if (ih) labels.push('Ih ' + ih);
    if (ct) labels.push('Ct ' + ct);
    var badge = el('span', {
      style: {
        fontWeight: '600',
        color: (cn + cm > 0) ? '#f87171' : 'rgba(148,163,184,0.7)'
      }
    });
    badge.append(txt(labels.join(' · ')));
    cell.append(badge);
    return cell;
  }

  /* One data row of the hotspot table. `ui.rerender` rebuilds the table
     after a dismiss; `ui.updateDismissBar` refreshes the reset controls. */
  function hsTableRow(f, state, ui) {
    var parts = fileParts(f.path);
    var row = el('tr', {
      'data-path': f.path,
      className: f.path === state.selected ? 'hs-row-highlight' : '',
      title: 'Highlight in scatter plot',
      style: { cursor: 'pointer' }
    });
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

    var trendCell = el('td');
    trendCell.append(hsSparkline(f.churn_timeline || []));

    var bugsCell = el('td');
    bugsCell.append(txt(String(f.bug_commit_count)));

    var locCell = el('td');
    locCell.append(txt(String(f.loc)));

    var dismissCell = el('td');
    var dismissBtn = el('button', {
      className: 'hs-dismiss',
      title: 'Dismiss this file from the list',
      style: {
        background: 'none', border: 'none', color: 'var(--text-muted, #94a3b8)',
        cursor: 'pointer', fontSize: '15px', lineHeight: '1', padding: '0 4px'
      }
    });
    dismissBtn.append(txt('×'));
    dismissBtn.addEventListener('click', function(ev) {
      ev.stopPropagation();
      state.dismissed[f.path] = true;
      ui.rerender();
      ui.updateDismissBar();
    });
    dismissCell.append(dismissBtn);

    row.append(fileCell, scoreCell, ccCell, churnCell, trendCell, bugsCell, locCell,
      hsCouplingCell(f), dismissCell);
    return row;
  }

  /* The sortable, filterable hotspot table. Sort clicks mutate `state`
     and trigger `ui.rerender`. */
  function hsBuildTable(files, state, ui) {
    var visible = files.filter(function(f) {
      if (state.dismissed[f.path]) return false;
      if (state.filterQuery && f.path.toLowerCase().indexOf(state.filterQuery) === -1) return false;
      return true;
    });
    var sorted = visible.slice().sort(function(a, b) {
      var av = a[state.sortCol], bv = b[state.sortCol];
      if (typeof av === 'string') av = av.toLowerCase();
      if (typeof bv === 'string') bv = bv.toLowerCase();
      if (av < bv) return state.sortAsc ? -1 : 1;
      if (av > bv) return state.sortAsc ? 1 : -1;
      return 0;
    });

    var table = el('table');
    var thead = el('thead');
    var tr = el('tr');

    function th(label, col, tip) {
      var t = el('th', { className: 'th-sort' + (col === state.sortCol ? ' active-sort' : '') });
      t.append(txt(label + (col === state.sortCol ? (state.sortAsc ? ' ▲' : ' ▼') : '')));
      if (tip) {
        var icon = tipIcon(tip);
        // hovering explains, clicking should not also re-sort
        icon.addEventListener('click', function(ev) { ev.stopPropagation(); });
        t.append(icon);
      }
      t.addEventListener('click', function() {
        if (state.sortCol === col) { state.sortAsc = !state.sortAsc; } else { state.sortCol = col; state.sortAsc = false; }
        ui.rerender();
      });
      return t;
    }

    var trendTh = el('th');
    trendTh.append(txt('Trend'), tipIcon(HS_COL_TIPS.Trend));
    var cplTh = el('th');
    cplTh.append(txt('Coupling'), tipIcon(HS_COL_TIPS.Coupling));
    tr.append(
      th('File', 'path', HS_COL_TIPS.File),
      th('Score', 'hotspot_score', HS_COL_TIPS.Score),
      th('CC', 'cyclomatic_complexity', HS_COL_TIPS.CC),
      th('Churn', 'churn_count', HS_COL_TIPS.Churn),
      trendTh,
      th('Bugs', 'bug_commit_count', HS_COL_TIPS.Bugs),
      th('LOC', 'loc', HS_COL_TIPS.LOC),
      cplTh,
      el('th', { title: 'Dismiss a file from this list' })
    );
    thead.append(tr);
    table.append(thead);

    var tbody = el('tbody');
    sorted.slice(0, 50).forEach(function(f) {
      tbody.append(hsTableRow(f, state, ui));
    });
    table.append(tbody);
    return table;
  }

  function buildHotspotsTab() {
    var files = R.file_hotspots || [];
    if (files.length === 0) {
      var d = el('div', { className: 'no-data' });
      d.append(txt('No hotspot data available.'));
      return d;
    }

    var wrap = el('div', { className: 'hotspot-wrap' });
    hsIntro().forEach(function(block) { wrap.append(block); });

    var plot = hsScatterPlot(files);
    var scatter = plot.svg;
    wrap.append(plot.card);

    // Table state: mutated by sort clicks, the filter box, and dismissals.
    var state = {
      sortCol: 'hotspot_score',
      sortAsc: false,
      selected: null,
      filterQuery: '',
      dismissed: {}
    };
    var tableCard = el('div', { className: 'view-card' });
    var tableWrap = el('div', { style: { overflowX: 'auto' } });
    var ui = {
      rerender: function() { tableWrap.replaceChildren(hsBuildTable(files, state, ui)); },
      updateDismissBar: updateDismissBar
    };

    var filterInput = el('input', {
      type: 'search',
      placeholder: 'Filter files…',
      className: 'hs-filter',
      style: {
        background: 'var(--bg-panel, #0f172a)', color: 'inherit',
        border: '1px solid #334155', borderRadius: '6px',
        padding: '5px 10px', fontSize: '13px', width: '220px',
        margin: '12px 12px 0 12px'
      }
    });
    filterInput.addEventListener('input', function() {
      state.filterQuery = filterInput.value.toLowerCase();
      ui.rerender();
    });

    // Dismissed-rows controls: a reset button + a count, mirroring the coupling tab.
    // Dismissal is client-side and ephemeral (lost on reload).
    var dismissStatus = el('span', {
      className: 'hs-dismiss-status',
      style: { fontSize: '12px', color: 'var(--text-muted, #94a3b8)', margin: '0 8px' }
    });
    var resetBtn = el('button', {
      className: 'hs-dismiss-reset',
      style: {
        background: 'var(--bg-panel, #0f172a)', color: 'inherit',
        border: '1px solid #334155', borderRadius: '6px',
        padding: '5px 10px', fontSize: '12px', cursor: 'pointer',
        margin: '12px 0 0 0'
      }
    });
    resetBtn.append(txt('Reset dismissed'));
    resetBtn.addEventListener('click', function() {
      state.dismissed = {};
      ui.rerender();
      updateDismissBar();
    });

    function updateDismissBar() {
      var n = Object.keys(state.dismissed).length;
      dismissStatus.replaceChildren(txt(n > 0 ? (n + ' dismissed') : ''));
      resetBtn.style.display = n > 0 ? '' : 'none';
    }

    ui.rerender();
    tableCard.append(filterInput, resetBtn, dismissStatus, tableWrap);
    updateDismissBar();
    wrap.append(tableCard);

    // Shared selection: one file highlighted in both the scatter plot and the
    // table. Returns false when the click toggled the selection off.
    function selectHotspot(path) {
      scatter.querySelectorAll('.hs-scatter-dot').forEach(function(d) {
        d.setAttribute('class', 'hs-scatter-dot');
      });
      tableWrap.querySelectorAll('.hs-row-highlight').forEach(function(r) {
        r.classList.remove('hs-row-highlight');
      });

      if (state.selected === path) {
        state.selected = null;
        setHashState('hotspots', null);
        return false;
      }
      state.selected = path;
      setHashState('hotspots', path);

      var dot = scatter.querySelector('.hs-scatter-dot[data-path="' + CSS.escape(path) + '"]');
      if (!dot) {
        // File outside the initial render cap — plot its dot on demand
        var match = files.find(function(x) { return x.path === path; });
        if (match) {
          dot = plot.makeDot(match);
          scatter.append(dot);
        }
      }
      if (dot) dot.setAttribute('class', 'hs-scatter-dot active');
      var row = tableWrap.querySelector('tr[data-path="' + CSS.escape(path) + '"]');
      if (row) row.classList.add('hs-row-highlight');
      return true;
    }

    // Click scatter dot → highlight + scroll to matching table row
    scatter.addEventListener('click', function(e) {
      var dot = e.target;
      // Walk up for SVG elements (closest() unreliable on SVG)
      while (dot && dot !== scatter) {
        if (dot.classList && dot.classList.contains('hs-scatter-dot')) break;
        dot = dot.parentNode;
      }
      if (!dot || dot === scatter) return;
      if (!selectHotspot(dot.getAttribute('data-path'))) return;
      var row = tableWrap.querySelector('.hs-row-highlight');
      if (row) row.scrollIntoView({ behavior: 'smooth', block: 'center' });
    });

    // Click table row → highlight + scroll to matching scatter dot.
    // Delegated on tableWrap so it survives sort rebuilds; header rows carry
    // no data-path and fall through to their own sort handlers.
    tableWrap.addEventListener('click', function(e) {
      var row = e.target;
      while (row && row !== tableWrap) {
        if (row.tagName === 'TR' && row.getAttribute('data-path')) break;
        row = row.parentNode;
      }
      if (!row || row === tableWrap) return;
      if (!selectHotspot(row.getAttribute('data-path'))) return;
      plot.card.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    });

    // Cross-tab entry point: select the file and scroll its row into view
    registerFileFocus('hotspots', function(path) {
      if (state.selected !== path) selectHotspot(path);
      var row = tableWrap.querySelector('tr[data-path="' + CSS.escape(path) + '"]');
      if (!row) {
        // File outside the visible top-50 — narrow the filter so its row appears
        filterInput.value = path;
        state.filterQuery = path.toLowerCase();
        ui.rerender();
        row = tableWrap.querySelector('tr[data-path="' + CSS.escape(path) + '"]');
      }
      if (row) row.scrollIntoView({ behavior: 'smooth', block: 'center' });
    });

    return wrap;
  }
