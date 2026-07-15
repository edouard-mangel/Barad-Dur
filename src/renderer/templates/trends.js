
  /* ---- Trends tab ---- */
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

    // Legend — conditionally rendered when backfill entries exist
    if (window.R.history.some(function(e){ return e.source === 'backfill'; })) {
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

      controls.append(leg);
    }

    container.append(controls);

    // Chart container
    var chartDiv = el('div', { className: 'tr-chart' });
    chartDiv.id = 'tr-chart';
    container.append(chartDiv);

    // Tooltip
    var tooltip = el('div', { className: 'tr-tooltip' });
    container.append(tooltip);

    function getScore(entry, metric) {
      if (metric === 'Overall Score') return entry.overall_score;
      if (entry.category_scores && entry.category_scores[metric] !== undefined) return entry.category_scores[metric];
      if (entry.metrics && entry.metrics[metric] !== undefined) return entry.metrics[metric];
      return 0;
    }

    function renderChart() {
      var metric = select.value;
      var W = 900, H = 350;
      var pad = { top: 20, right: 30, bottom: 40, left: 45 };
      var cw = W - pad.left - pad.right;
      var ch = H - pad.top - pad.bottom;

      var scores = history.map(function(e) { return getScore(e, metric); });
      var dates = history.map(function(e) { return new Date(e.timestamp); });

      var minT = dates[0].getTime();
      var maxT = dates[dates.length - 1].getTime();
      var rangeT = maxT - minT || 1;

      // Dynamic Y-axis: pad 10 points above max and below min, clamped to 0-100
      var rawMin = Math.min.apply(null, scores);
      var rawMax = Math.max.apply(null, scores);
      var yMin = Math.max(0, Math.floor((rawMin - 10) / 5) * 5);
      var yMax = Math.min(100, Math.ceil((rawMax + 10) / 5) * 5);
      if (yMin === yMax) { yMin = Math.max(0, yMin - 10); yMax = Math.min(100, yMax + 10); }
      var yRange = yMax - yMin || 1;

      function x(i) { return pad.left + (dates[i].getTime() - minT) / rangeT * cw; }
      function y(s) { return pad.top + (1 - (s - yMin) / yRange) * ch; }

      var svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ' + W + ' ' + H + '" style="width:100%;height:auto">';

      // Grid lines and Y labels (avoid literal quote-hash in SVG attr values)
      var h = String.fromCharCode(35);
      var gridCol = h + '1e293b';
      var labelCol = h + '8b949e';
      var bgCol = h + '0d1117';
      var gridSteps = 5;
      for (var gi = 0; gi <= gridSteps; gi++) {
        var v = yMin + (yRange * gi / gridSteps);
        v = Math.round(v);
        var yy = y(v);
        svg += '<line x1="' + pad.left + '" y1="' + yy + '" x2="' + (W - pad.right) + '" y2="' + yy + '" stroke="' + gridCol + '" stroke-width="1"/>';
        svg += '<text x="' + (pad.left - 8) + '" y="' + (yy + 4) + '" text-anchor="end" fill="' + labelCol + '" font-size="11">' + v + '</text>';
      }

      // X-axis date labels
      var labelCount = Math.min(history.length, 8);
      var step = Math.max(1, Math.floor(history.length / labelCount));
      for (var li = 0; li < history.length; li += step) {
        var dx = x(li);
        var d = dates[li];
        var dateStr = d.getFullYear() + '-' + String(d.getMonth() + 1).padStart(2, '0') + '-' + String(d.getDate()).padStart(2, '0');
        svg += '<text x="' + dx + '" y="' + (H - 5) + '" text-anchor="middle" fill="' + labelCol + '" font-size="10">' + dateStr + '</text>';
      }

      // Line
      var lastScore = scores[scores.length - 1];
      var lineColor = scoreColor(lastScore);
      var points = history.map(function(_, i) { return x(i) + ',' + y(scores[i]); }).join(' ');
      svg += '<polyline points="' + points + '" fill="none" stroke="' + lineColor + '" stroke-width="2" stroke-linejoin="round"/>';

      // Dots
      history.forEach(function(entry, i) {
        var cx = x(i);
        var cy = y(scores[i]);
        var isBackfill = entry.source === 'backfill';
        var dotFill = isBackfill ? 'none' : scoreColor(scores[i]);
        var dotStroke = isBackfill ? scoreColor(scores[i]) : bgCol;
        var dotStyle = isBackfill ? ' style="pointer-events:all"' : '';
        svg += '<circle class="tr-dot" cx="' + cx + '" cy="' + cy + '" r="4" fill="' + dotFill + '" '
          + 'data-idx="' + i + '" stroke="' + dotStroke + '" stroke-width="1.5"'
          + dotStyle
          + (isBackfill ? ' data-backfill="1"' : '') + '/>';
      });

      svg += '</svg>';
      chartDiv.innerHTML = svg;

      // Wire dot hover events
      chartDiv.querySelectorAll('.tr-dot').forEach(function(dot) {
        dot.addEventListener('mouseenter', function(e) {
          var idx = parseInt(dot.getAttribute('data-idx'), 10);
          var entry = history[idx];
          var d = new Date(entry.timestamp);
          var dateStr = d.getFullYear() + '-' + String(d.getMonth() + 1).padStart(2, '0') + '-' + String(d.getDate()).padStart(2, '0');
          var head7 = entry.head.substring(0, 7);
          var srcLabel = dot.dataset.backfill === '1' ? 'Source: Backfill' : 'Source: Live analysis';
          var lines = dateStr + ' (' + head7 + ')\n'
            + metric + ': ' + getScore(entry, metric) + '\n'
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
            + srcLabel;
          tooltip.textContent = lines;
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
