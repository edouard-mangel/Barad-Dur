
  /* ---- Trends tab ---- */

  /* Score of one history entry for the selected metric — null when the
     entry never recorded it (unscored/advisory metrics are absent from
     history entries; a null must render as a gap, not a drop to 0). */
  function trGetScore(entry, metric) {
    if (metric === 'Overall Score') return entry.overall_score;
    if (entry.category_scores && entry.category_scores[metric] !== undefined) return entry.category_scores[metric];
    if (entry.metrics && entry.metrics[metric] !== undefined) return entry.metrics[metric];
    return null;
  }

  function trFmtDate(d) {
    return d.getFullYear() + '-' + String(d.getMonth() + 1).padStart(2, '0') + '-' + String(d.getDate()).padStart(2, '0');
  }

  /* Multi-line hover text for one data point. */
  function trTooltipText(entry, metric, isBackfill) {
    var score = trGetScore(entry, metric);
    return trFmtDate(new Date(entry.timestamp)) + ' (' + entry.head.substring(0, 7) + ')\n'
      + metric + ': ' + (score === null ? 'n/a' : score) + '\n'
      + entry.counts.commits + ' commits, '
      + entry.counts.files + ' files, '
      + entry.counts.authors + ' authors\n'
      + (entry.counts.content_coupling != null
          ? 'coupling findings: ' + entry.counts.content_coupling + ' content, '
            + entry.counts.common_coupling + ' common, '
            + (entry.counts.inheritance_coupling != null
                ? entry.counts.inheritance_coupling + ' inheritance, '
                : '')
            + entry.counts.control_coupling + ' control\n'
          : '')
      + (isBackfill ? 'Source: Backfill' : 'Source: Live analysis');
  }

  /* The score-over-time chart as an SVG element. Dots carry data-idx so
     the tab can wire hover tooltips. */
  function trBuildChart(history, metric) {
    var W = 900, H = 350;
    var pad = { top: 20, right: 30, bottom: 40, left: 45 };
    var cw = W - pad.left - pad.right;
    var ch = H - pad.top - pad.bottom;

    var scores = history.map(function(e) { return trGetScore(e, metric); });
    // Entries that never recorded this metric (null) render as gaps.
    var known = scores.filter(function(s) { return s !== null; });
    if (known.length === 0) {
      var emptyNote = el('div', { className: 'tr-empty' });
      emptyNote.append(txt('No recorded scores for this metric.'));
      return emptyNote;
    }
    var dates = history.map(function(e) { return new Date(e.timestamp); });

    var minT = dates[0].getTime();
    var maxT = dates[dates.length - 1].getTime();
    var rangeT = maxT - minT || 1;

    // Dynamic Y-axis: pad 10 points above max and below min, clamped to 0-100
    var rawMin = Math.min.apply(null, known);
    var rawMax = Math.max.apply(null, known);
    var yMin = Math.max(0, Math.floor((rawMin - 10) / 5) * 5);
    var yMax = Math.min(100, Math.ceil((rawMax + 10) / 5) * 5);
    if (yMin === yMax) { yMin = Math.max(0, yMin - 10); yMax = Math.min(100, yMax + 10); }
    var yRange = yMax - yMin || 1;

    function x(i) { return pad.left + (dates[i].getTime() - minT) / rangeT * cw; }
    function y(s) { return pad.top + (1 - (s - yMin) / yRange) * ch; }

    var svg = svgEl('svg', {
      xmlns: 'http://www.w3.org/2000/svg',
      viewBox: '0 0 ' + W + ' ' + H,
      style: 'width:100%;height:auto'
    });

    // Grid lines and Y labels
    var gridSteps = 5;
    for (var gi = 0; gi <= gridSteps; gi++) {
      var v = Math.round(yMin + (yRange * gi / gridSteps));
      var yy = y(v);
      svg.append(svgEl('line', {
        x1: String(pad.left), y1: String(yy), x2: String(W - pad.right), y2: String(yy),
        stroke: '#1e293b', 'stroke-width': '1'
      }));
      var yLabel = svgEl('text', {
        x: String(pad.left - 8), y: String(yy + 4), 'text-anchor': 'end',
        fill: '#8b949e', 'font-size': '11'
      });
      yLabel.append(txt(String(v)));
      svg.append(yLabel);
    }

    // X-axis date labels
    var labelCount = Math.min(history.length, 8);
    var step = Math.max(1, Math.floor(history.length / labelCount));
    for (var li = 0; li < history.length; li += step) {
      var xLabel = svgEl('text', {
        x: String(x(li)), y: String(H - 5), 'text-anchor': 'middle',
        fill: '#8b949e', 'font-size': '10'
      });
      xLabel.append(txt(trFmtDate(dates[li])));
      svg.append(xLabel);
    }

    // Line — only through entries that recorded a score
    var lineColor = scoreColor(known[known.length - 1]);
    var points = history
      .map(function(_, i) { return scores[i] === null ? null : x(i) + ',' + y(scores[i]); })
      .filter(function(p) { return p !== null; })
      .join(' ');
    svg.append(svgEl('polyline', {
      points: points, fill: 'none', stroke: lineColor,
      'stroke-width': '2', 'stroke-linejoin': 'round'
    }));

    // Dots — backfill entries render hollow, live analysis filled
    history.forEach(function(entry, i) {
      if (scores[i] === null) return;
      var isBackfill = entry.source === 'backfill';
      var attrs = {
        class: 'tr-dot',
        cx: String(x(i)), cy: String(y(scores[i])), r: '4',
        fill: isBackfill ? 'none' : scoreColor(scores[i]),
        stroke: isBackfill ? scoreColor(scores[i]) : '#0d1117',
        'stroke-width': '1.5',
        'data-idx': String(i)
      };
      if (isBackfill) {
        attrs.style = 'pointer-events:all';
        attrs['data-backfill'] = '1';
      }
      svg.append(svgEl('circle', attrs));
    });

    return svg;
  }

  /* Backfill-vs-live legend, shown only when backfill entries exist. */
  function trLegend() {
    var leg = el('div');
    leg.className = 'tr-legend';

    var dotBackfill = el('span');
    dotBackfill.className = 'tr-legend-dot';
    dotBackfill.style.cssText = 'border:2px solid #8b949e;background:transparent;';
    leg.append(dotBackfill);
    leg.append(txt('Backfill'));

    var dotLive = el('span');
    dotLive.className = 'tr-legend-dot';
    dotLive.style.cssText = 'background:var(--c-good);';
    leg.append(dotLive);
    leg.append(txt('Live analysis'));

    return leg;
  }

  function buildTrendsTab() {
    var container = document.createDocumentFragment();
    var history = R.history || [];

    var info = buildTabInfo(
      'Score Trends',
      'Track how your repository scores change over time. Each data point represents an analysis run at a unique commit.'
    );
    container.append(info);

    if (history.length < 2) {
      var empty = el('div', { className: 'tr-empty' });
      empty.append(txt('Trends appear after multiple analysis runs on different commits. Run barad-dur analyze again after making commits to start tracking.'));
      container.append(empty);
      return container;
    }

    // Build metric options from first entry
    // Note: HistoryEntry serializes its categories field as "category_scores"
    var metricNames = ['Overall Score'];
    var catKeys = Object.keys(history[0].category_scores || {}).sort();
    catKeys.forEach(function(k) { metricNames.push(k); });
    var mKeys = Object.keys(history[0].metrics || {}).sort();
    mKeys.forEach(function(k) { metricNames.push(k); });

    // Controls row
    var controls = el('div', { className: 'tr-controls' });
    var label = el('label');
    label.append(txt('Metric: '));
    var select = el('select', { className: 'tr-select' });
    select.id = 'tr-metric-select';
    metricNames.forEach(function(name) {
      var opt = el('option');
      opt.value = name;
      opt.append(txt(name));
      select.append(opt);
    });
    label.append(select);
    controls.append(label);

    if (history.some(function(e) { return e.source === 'backfill'; })) {
      controls.append(trLegend());
    }

    container.append(controls);

    // Chart container
    var chartDiv = el('div', { className: 'tr-chart' });
    chartDiv.id = 'tr-chart';
    container.append(chartDiv);

    // Tooltip
    var tooltip = el('div', { className: 'tr-tooltip' });
    container.append(tooltip);

    function renderChart() {
      var metric = select.value;
      chartDiv.replaceChildren(trBuildChart(history, metric));

      // Wire dot hover events
      chartDiv.querySelectorAll('.tr-dot').forEach(function(dot) {
        dot.addEventListener('mouseenter', function(e) {
          var idx = parseInt(dot.getAttribute('data-idx'), 10);
          tooltip.textContent = trTooltipText(history[idx], metric, dot.dataset.backfill === '1');
          tooltip.style.display = 'block';
          tooltip.style.left = (e.clientX + 14) + 'px';
          tooltip.style.top = (e.clientY + 14) + 'px';
        });
        dot.addEventListener('mouseleave', function() {
          tooltip.style.display = 'none';
        });
      });
    }

    select.addEventListener('change', renderChart);
    setTimeout(renderChart, 0);

    return container;
  }
