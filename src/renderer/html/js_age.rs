pub const JS: &str = r#"
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
        { color: 'var(--c-good)',    label: 'Fresh (<90 days) \u2014 actively maintained' },
        { color: 'var(--c-age-mid)', label: '3\u20136 months \u2014 aging, review periodically' },
        { color: 'var(--c-warn)',    label: '6\u201312 months \u2014 stale, check if still relevant' },
        { color: 'var(--c-danger)',  label: '>1 year \u2014 potentially abandoned' }
      ]
    ));

    function ageBand(days) {
      if (days > 365) return { color: 'var(--c-danger)',  bg: 'var(--c-danger-bg)',  label: '> 1y' };
      if (days > 180) return { color: 'var(--c-warn)',    bg: 'var(--c-warn-bg)',    label: '> 6mo' };
      if (days > 90)  return { color: 'var(--c-age-mid)', bg: 'var(--c-age-mid-bg)', label: '> 3mo' };
      return            { color: 'var(--c-good)',    bg: 'var(--c-good-bg)',    label: 'Fresh' };
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
      var bandChip = el('span', { className: 'chip', style: { background: band.bg, color: band.color } });
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
"#;
