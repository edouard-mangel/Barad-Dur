pub const JS: &str = r#"
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
        topCell.append(txt(topAuthor.name + ' (' + fmt(topAuthor.pct, 0) + '%)'));
      } else {
        topCell.append(txt('—'));
      }

      var barCell = el('td');
      var bar = el('div', { className: 'own-bar' });
      (f.authors || []).slice(0, 8).forEach(function(a) {
        var idx = authorIndex[a.name] % PALETTE.length;
        var seg = el('div', { className: 'own-seg', style: {
          width: (Number.isFinite(+a.pct) ? +a.pct : 0).toFixed(1) + '%',
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
"#;
