pub const JS: &str = r#"
  /* ---- Audit tab ---- */
  function buildAuditTab() {
    var container = document.createDocumentFragment();
    var audit = R.audit;

    if (!audit) {
      var empty = el('div', { className: 'no-data' });
      empty.append(txt('No audit data available.'));
      container.append(empty);
      return container;
    }

    container.append(buildTabInfo(
      'Codebase Audit',
      'Legacy-audit diagnostics: crisis-prone files, code concentration by directory, possibly dead files, and commit velocity over time.'
    ));

    container.append(buildCrisisSection(audit.crisis_files || []));
    container.append(buildConcentrationSection(audit.dir_concentration || []));
    container.append(buildDeadFilesSection(audit.dead_files || []));
    container.append(buildVelocitySection(audit.velocity_buckets || []));

    return container;
  }

  /* ---- Crisis files ---- */
  function buildCrisisSection(files) {
    var section = el('div', { className: 'section' });
    var heading = el('h2', { className: 'section-title' });
    heading.append(txt('Crisis Files'));
    var sub = el('p', { className: 'section-sub' });
    sub.append(txt('Files where a high proportion of commits contain emergency keywords (fix, hotfix, revert, urgent, broken, oops, emergency, critical, crash). Top 20 by crisis ratio.'));
    section.append(heading, sub);

    if (!files.length) {
      var none = el('div', { className: 'no-data' });
      none.append(txt('No files with crisis commits found.'));
      section.append(none);
      return section;
    }

    var table = el('table', { className: 'tbl' });
    var thead = el('thead');
    var hr = el('tr');
    ['File', 'Total commits', 'Crisis commits', 'Crisis ratio'].forEach(function(h) {
      var th = el('th');
      th.append(txt(h));
      hr.append(th);
    });
    thead.append(hr);
    table.append(thead);

    var tbody = el('tbody');
    files.forEach(function(f) {
      var tr = el('tr');

      var parts = fileParts(f.path);
      var pathCell = el('td', { className: 'path-cell' });
      var dir = el('span', { className: 'path-dir' });
      dir.append(txt(parts.dir));
      var name = el('span', { className: 'path-name' });
      name.append(txt(parts.name));
      pathCell.append(dir, name);

      var totalCell = el('td');
      totalCell.append(txt(String(f.total_commit_count)));

      var crisisCell = el('td');
      crisisCell.append(txt(String(f.crisis_commit_count)));

      var ratioCell = el('td');
      var pct = Math.round(f.crisis_ratio * 100);
      var color = pct >= 50 ? '#ef4444' : pct >= 25 ? '#f59e0b' : '#10b981';
      var barWrap = el('div', { style: { display: 'flex', alignItems: 'center', gap: '8px' } });
      barWrap.append(inlineBar(pct, color));
      var label = el('span', { style: { fontSize: '12px', color: color, minWidth: '36px' } });
      label.append(txt(pct + '%'));
      barWrap.append(label);
      ratioCell.append(barWrap);

      tr.append(pathCell, totalCell, crisisCell, ratioCell);
      tbody.append(tr);
    });
    table.append(tbody);
    section.append(table);
    return section;
  }

  /* ---- Code concentration ---- */
  function buildConcentrationSection(dirs) {
    var section = el('div', { className: 'section' });
    var heading = el('h2', { className: 'section-title' });
    heading.append(txt('Code Concentration'));
    var sub = el('p', { className: 'section-sub' });
    sub.append(txt('Lines of code by directory. Heavy concentration in one area signals tight coupling.'));
    section.append(heading, sub);

    if (!dirs.length) {
      var none = el('div', { className: 'no-data' });
      none.append(txt('No file metrics available.'));
      section.append(none);
      return section;
    }

    var maxLoc = dirs.reduce(function(m, d) { return Math.max(m, d.loc); }, 1);

    var list = el('div', { className: 'conc-list' });
    dirs.forEach(function(d) {
      var row = el('div', { className: 'conc-row' });

      var label = el('div', { className: 'conc-label' });
      label.append(txt(d.dir));

      var barArea = el('div', { className: 'conc-bar-area' });
      var pct = d.loc / maxLoc * 100;
      var fill = el('div', { className: 'conc-bar-fill', style: { width: (Number.isFinite(pct) ? pct : 0).toFixed(1) + '%' } });
      barArea.append(fill);

      var meta = el('div', { className: 'conc-meta' });
      meta.append(chip((d.loc != null ? d.loc.toLocaleString() : '—') + ' loc', '#1e293b', '#94a3b8'));
      meta.append(chip(d.file_count + ' files', '#1e293b', '#64748b'));
      meta.append(chip(fmt(d.pct_of_total, 1) + '%', '#1c3547', '#6ee7b7'));

      row.append(label, barArea, meta);
      list.append(row);
    });
    section.append(list);
    return section;
  }

  /* ---- Dead files ---- */
  function buildDeadFilesSection(files) {
    var section = el('div', { className: 'section' });
    var heading = el('h2', { className: 'section-title' });
    heading.append(txt('Possibly Dead Files'));
    var sub = el('p', { className: 'section-sub' });
    sub.append(txt('Files untouched for over 6 months with 0-1 commits in the analysis window. Candidates for removal or archival.'));
    section.append(heading, sub);

    if (!files.length) {
      var none = el('div', { className: 'no-data' });
      none.append(txt('No stale files detected.'));
      section.append(none);
      return section;
    }

    var table = el('table', { className: 'tbl' });
    var thead = el('thead');
    var hr = el('tr');
    ['File', 'Days since last touch', 'Churn (commits)'].forEach(function(h) {
      var th = el('th');
      th.append(txt(h));
      hr.append(th);
    });
    thead.append(hr);
    table.append(thead);

    var tbody = el('tbody');
    files.forEach(function(f) {
      var tr = el('tr');

      var parts = fileParts(f.path);
      var pathCell = el('td', { className: 'path-cell' });
      var dir = el('span', { className: 'path-dir' });
      dir.append(txt(parts.dir));
      var name = el('span', { className: 'path-name' });
      name.append(txt(parts.name));
      pathCell.append(dir, name);

      var daysCell = el('td');
      var daysColor = f.days_since_modified > 365 ? '#ef4444' : '#f59e0b';
      var daysSpan = el('span', { style: { color: daysColor, fontWeight: '600' } });
      daysSpan.append(txt(String(f.days_since_modified)));
      daysCell.append(daysSpan, txt(' days'));

      var churnCell = el('td');
      churnCell.append(txt(String(f.churn_count)));

      tr.append(pathCell, daysCell, churnCell);
      tbody.append(tr);
    });
    table.append(tbody);
    section.append(table);
    return section;
  }

  /* ---- Commit velocity ---- */
  function buildVelocitySection(buckets) {
    var section = el('div', { className: 'section' });
    var heading = el('h2', { className: 'section-title' });
    heading.append(txt('Commit Velocity'));
    var sub = el('p', { className: 'section-sub' });
    sub.append(txt('Commits per week across the analysis window. Sustained drops may indicate the codebase is becoming harder to change.'));
    section.append(heading, sub);

    if (!buckets.length) {
      var none = el('div', { className: 'no-data' });
      none.append(txt('No commit data available.'));
      section.append(none);
      return section;
    }

    var W = 900, H = 260;
    var pad = { top: 20, right: 20, bottom: 50, left: 45 };
    var cw = W - pad.left - pad.right;
    var ch = H - pad.top - pad.bottom;
    var n = buckets.length;
    var maxCount = buckets.reduce(function(m, b) { return Math.max(m, b.commit_count); }, 1);
    var slotW = cw / n;
    var barW = Math.max(2, Math.floor(slotW) - 2);

    var svg = svgEl('svg', {
      viewBox: '0 0 ' + W + ' ' + H,
      style: 'width:100%;height:auto;display:block'
    });

    // Grid lines + Y labels
    var gridSteps = 4;
    for (var gi = 0; gi <= gridSteps; gi++) {
      var v = Math.round(maxCount * gi / gridSteps);
      var yy = (pad.top + ch - (v / maxCount) * ch).toFixed(1);
      svg.append(svgEl('line', {
        x1: String(pad.left), y1: yy,
        x2: String(W - pad.right), y2: yy,
        stroke: '#1e293b', 'stroke-width': '1'
      }));
      var label = svgEl('text', {
        x: String(pad.left - 6), y: String(parseFloat(yy) + 4),
        'text-anchor': 'end', fill: '#8b949e',
        'font-size': '11', 'font-family': '-apple-system,sans-serif'
      });
      label.append(txt(String(v)));
      svg.append(label);
    }

    // Bars
    buckets.forEach(function(b, i) {
      var x = (pad.left + i * slotW + (slotW - barW) / 2).toFixed(1);
      var barH = Math.max(1, (b.commit_count / maxCount) * ch);
      var y = (pad.top + ch - barH).toFixed(1);
      var pct = b.commit_count / maxCount * 100;
      var color = pct >= 60 ? '#10b981' : pct >= 30 ? '#f59e0b' : '#ef4444';
      var rect = svgEl('rect', {
        x: x, y: y,
        width: String(barW), height: barH.toFixed(1),
        fill: color, rx: '2'
      });
      var titleEl = svgEl('title');
      titleEl.append(txt(b.week_start + ': ' + b.commit_count + ' commits, ' + b.author_count + ' authors'));
      rect.append(titleEl);
      svg.append(rect);
    });

    // X-axis date labels (~8 evenly spaced)
    var labelStep = Math.max(1, Math.floor(n / 8));
    for (var li = 0; li < n; li += labelStep) {
      var lx = (pad.left + li * slotW + slotW / 2).toFixed(1);
      var ly = H - 8;
      var dateLabel = svgEl('text', {
        x: lx, y: String(ly),
        'text-anchor': 'middle', fill: '#8b949e',
        'font-size': '10', 'font-family': '-apple-system,sans-serif',
        transform: 'rotate(-35 ' + lx + ' ' + ly + ')'
      });
      dateLabel.append(txt(buckets[li].week_start));
      svg.append(dateLabel);
    }

    var chartWrap = el('div', { className: 'vel-chart' });
    chartWrap.append(svg);
    section.append(chartWrap);
    return section;
  }
"#;
