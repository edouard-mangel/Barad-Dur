
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
        { color: 'var(--c-good-lo)', label: 'Low risk \u2014 simple + rarely changed' },
        { color: 'var(--c-warn)',    label: 'Medium \u2014 monitor these files' },
        { color: 'var(--c-danger)',  label: 'High risk \u2014 complex + frequently changed' }
      ]
    ));

    wrap.append(buildExplainer('Understanding churn and why it matters', [
      {
        heading: 'What is churn?',
        text: 'Churn is the number of times a file has been modified (committed to) within the analysis window. A file touched in 30 separate commits has a churn count of 30. Renames and moves are tracked as the same logical file when git detects them.'
      },
      {
        heading: 'Why churn alone is not enough',
        text: 'High churn is not inherently bad \u2014 a configuration file or changelog will naturally have high churn. Churn becomes a risk signal only when combined with high complexity: a complex file that changes often is statistically more likely to introduce defects because each change interacts with more code paths.'
      },
      {
        heading: 'How the Hotspot Score combines churn with complexity',
        items: [
          'Churn count and cyclomatic complexity are each normalized to a 0\u20131 scale (divided by the repository maximum).',
          'The hotspot score is the product of normalized churn, normalized complexity, and a size factor (LOC), scaled to 0\u2013100.',
          'This means a file must rank high on multiple dimensions to surface as a true hotspot \u2014 one dimension alone will not flag it.'
        ]
      },
      {
        heading: 'What to do with high-churn hotspots',
        items: [
          'Break large files apart \u2014 extract stable logic into smaller modules so that changes are isolated.',
          'Increase test coverage \u2014 high-churn files benefit most from regression tests.',
          'Review ownership \u2014 if many authors touch the same hotspot, coordinate on conventions.',
          'Reduce complexity \u2014 simplify branching (fewer if/match arms) to make future changes safer.',
          'Watch the trend \u2014 a file whose churn is rising over time is an escalating risk.'
        ]
      },
      {
        heading: 'Reading the scatter plot',
        items: [
          'X-axis = cyclomatic complexity (number of independent code paths).',
          'Y-axis = churn count (commits touching the file in the analysis window).',
          'Bubble size = lines of code.',
          'Top-right corner = high complexity + high churn \u2014 these are the files that need attention first.'
        ]
      }
    ]));

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

    // Numeric tick labels so the axes are readable without hovering each bubble
    [0, Math.round(maxCC / 2), maxCC].forEach(function(v) {
      var tickX = pad + (v / maxCC) * plotW;
      var t = svgEl('text', {
        x: String(tickX), y: String(svgH - pad + 10), 'text-anchor': 'middle',
        fill: '#475569', 'font-size': '8', 'font-family': 'sans-serif', class: 'hs-axis-tick'
      });
      t.append(txt(String(v)));
      scatter.append(t);
    });
    [0, Math.round(maxChurn / 2), maxChurn].forEach(function(v) {
      var tickY = (svgH - pad) - (v / maxChurn) * plotH2;
      var t = svgEl('text', {
        x: String(pad - 4), y: String(tickY + 3), 'text-anchor': 'end',
        fill: '#475569', 'font-size': '8', 'font-family': 'sans-serif', class: 'hs-axis-tick'
      });
      t.append(txt(String(v)));
      scatter.append(t);
    });

    function makeDot(f) {
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
      return circle;
    }

    files.slice(0, 300).forEach(function(f) {
      scatter.append(makeDot(f));
    });

    plotCard.append(scatter);
    wrap.append(plotCard);

    // Table
    var tableCard = el('div', { className: 'view-card' });
    var tableWrap = el('div', { style: { overflowX: 'auto' } });
    var sortCol = 'hotspot_score';
    var sortAsc = false;
    var selected = null;
    var filterQuery = '';

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
      filterQuery = filterInput.value.toLowerCase();
      tableWrap.replaceChildren(buildTable());
    });

    // Mini bar chart of f.churn_timeline: commits per 1/12 of the window,
    // oldest on the left. All rows share the window axis, so shapes compare.
    function buildSparkline(buckets) {
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

    function buildTable() {
      var visible = filterQuery
        ? files.filter(function(f) { return f.path.toLowerCase().indexOf(filterQuery) !== -1; })
        : files;
      var sorted = visible.slice().sort(function(a, b) {
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

      var trendTh = el('th', { title: 'Commits over the analysis window (oldest → newest)' });
      trendTh.append(txt('Trend'));
      tr.append(
        th('File', 'path'),
        th('Score', 'hotspot_score'),
        th('CC', 'cyclomatic_complexity'),
        th('Churn', 'churn_count'),
        trendTh,
        th('Bugs', 'bug_commit_count'),
        th('LOC', 'loc')
      );
      thead.append(tr);
      table.append(thead);

      var tbody = el('tbody');
      sorted.slice(0, 50).forEach(function(f) {
        var parts = fileParts(f.path);
        var row = el('tr', {
          'data-path': f.path,
          className: f.path === selected ? 'hs-row-highlight' : '',
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
        trendCell.append(buildSparkline(f.churn_timeline || []));

        var bugsCell = el('td');
        bugsCell.append(txt(String(f.bug_commit_count)));

        var locCell = el('td');
        locCell.append(txt(String(f.loc)));

        row.append(fileCell, scoreCell, ccCell, churnCell, trendCell, bugsCell, locCell);
        tbody.append(row);
      });
      table.append(tbody);
      return table;
    }

    tableWrap.append(buildTable());
    tableCard.append(filterInput, tableWrap);
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

      if (selected === path) {
        selected = null;
        setHashState('hotspots', null);
        return false;
      }
      selected = path;
      setHashState('hotspots', path);

      var dot = scatter.querySelector('.hs-scatter-dot[data-path="' + CSS.escape(path) + '"]');
      if (!dot) {
        // File outside the initial render cap — plot its dot on demand
        var match = files.find(function(x) { return x.path === path; });
        if (match) {
          dot = makeDot(match);
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
      plotCard.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    });

    // Cross-tab entry point: select the file and scroll its row into view
    registerFileFocus('hotspots', function(path) {
      if (selected !== path) selectHotspot(path);
      var row = tableWrap.querySelector('tr[data-path="' + CSS.escape(path) + '"]');
      if (!row) {
        // File outside the visible top-50 — narrow the filter so its row appears
        filterInput.value = path;
        filterQuery = path.toLowerCase();
        tableWrap.replaceChildren(buildTable());
        row = tableWrap.querySelector('tr[data-path="' + CSS.escape(path) + '"]');
      }
      if (row) row.scrollIntoView({ behavior: 'smooth', block: 'center' });
    });

    return wrap;
  }
