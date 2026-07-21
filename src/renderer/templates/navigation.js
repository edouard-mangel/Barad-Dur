
  /* ---- Theme initialisation and toggle ---- */
  function initTheme() {
    var stored = localStorage.getItem('theme');
    if (stored === 'light') {
      document.body.classList.add('light');
    } else if (stored === 'dark') {
      // explicit dark preference \u2014 do nothing (dark is default)
    } else {
      // no stored preference \u2014 check system
      if (window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches) {
        document.body.classList.add('light');
      }
      // else: dark default, do nothing
    }
  }

  function toggleTheme() {
    var isLight = document.body.classList.toggle('light');
    if (isLight) {
      localStorage.setItem('theme', 'light');
    } else {
      localStorage.removeItem('theme');
    }
  }

  function buildThemeBtn() {
    var themeBtn = el('button', {
      id: 'theme-btn',
      className: 'chip',
      'aria-label': 'Toggle theme',
      onClick: function() {
        toggleTheme();
        themeBtn.firstChild.nodeValue = document.body.classList.contains('light') ? '☾' : '☀';
      }
    });
    themeBtn.append(txt(document.body.classList.contains('light') ? '☾' : '☀'));
    return themeBtn;
  }

  /* ---- Cross-tab file navigation + URL hash state ----
     Tabs register a file-focus handler at build time; focusFileOnTab()
     switches tabs (building lazily if needed), invokes the handler, and
     mirrors the selection into the URL hash so views are deep-linkable. */
  var fileFocusHandlers = {};

  function registerFileFocus(tabName, handler) {
    fileFocusHandlers[tabName.toLowerCase()] = handler;
  }

  function focusFileOnTab(tabName, path) {
    document.dispatchEvent(new CustomEvent('barad:switch-tab', {
      detail: { tab: tabName, sortBy: null }
    }));
    var handler = fileFocusHandlers[tabName.toLowerCase()];
    if (handler) handler(path);
    setHashState(tabName, path);
  }

  function setHashState(tabName, path) {
    var h = '#tab=' + encodeURIComponent(tabName.toLowerCase());
    if (path) h += '&file=' + encodeURIComponent(path);
    history.replaceState(null, '', h);
  }

  function parseHashState() {
    var out = {};
    location.hash.replace(/^#/, '').split('&').forEach(function(part) {
      var i = part.indexOf('=');
      if (i > 0) out[decodeURIComponent(part.slice(0, i))] = decodeURIComponent(part.slice(i + 1));
    });
    return out;
  }

  /* Make a table file-cell drill through to a file-centric tab */
  function linkFileCell(cell, path, tabName) {
    cell.style.cursor = 'pointer';
    cell.title = 'View in ' + tabName.toLowerCase();
    cell.addEventListener('click', function() { focusFileOnTab(tabName, path); });
  }

  /* ---- Quick-open palette (Ctrl/Cmd+K) ----
     Fuzzy-ish jump to any analyzed file: row click / Enter goes to
     Hotspots, per-row buttons go to Graph or Treemap. */

  /* One result row: dimmed dir + name, plus Graph/Treemap jump buttons.
     `go(tab, path)` closes the palette and focuses the file; `onHover`
     lets the palette move its keyboard selection to the hovered row. */
  function qoRow(f, go, onHover) {
    var row = el('div', { className: 'qo-row', style: {
      display: 'flex', alignItems: 'center', justifyContent: 'space-between',
      gap: '10px', padding: '8px 14px', cursor: 'pointer', fontSize: '13px'
    }});
    var parts = fileParts(f.path);
    var label = el('span', { style: { overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' } });
    var dirSpan = el('span', { className: 'file-dir' });
    dirSpan.append(txt(parts.dir));
    var nameSpan = el('span', { className: 'file-name' });
    nameSpan.append(txt(parts.name));
    label.append(dirSpan, nameSpan);
    var dests = el('span', { style: { display: 'flex', gap: '4px', flexShrink: '0' } });
    ['Graph', 'Treemap'].forEach(function(tab) {
      var b = el('button', { className: 'chip', style: { cursor: 'pointer', fontSize: '10px' } });
      b.append(txt(tab.toLowerCase()));
      b.addEventListener('click', function(ev) { ev.stopPropagation(); go(tab, f.path); });
      dests.append(b);
    });
    row.append(label, dests);
    row.addEventListener('click', function() { go('Hotspots', f.path); });
    row.addEventListener('mousemove', function() { onHover(row); });
    return row;
  }

  /* The hotspot rows matching the query, capped at 12. */
  function qoMatches(q) {
    return (R.file_hotspots || [])
      .filter(function(f) { return f.path.toLowerCase().indexOf(q) !== -1; })
      .slice(0, 12);
  }

  function buildQuickOpen() {
    var overlay = el('div', { className: 'qo-overlay', style: {
      position: 'fixed', inset: '0', background: 'rgba(2,6,23,0.6)',
      display: 'none', zIndex: '1000', alignItems: 'flex-start', justifyContent: 'center'
    }});
    var box = el('div', { style: {
      marginTop: '12vh', width: 'min(560px, 90vw)',
      background: 'var(--bg-panel, #0f172a)', border: '1px solid #334155',
      borderRadius: '10px', boxShadow: '0 18px 50px rgba(0,0,0,0.5)', overflow: 'hidden'
    }});
    var input = el('input', { type: 'search', placeholder: 'Jump to file… (Esc to close)', style: {
      width: '100%', boxSizing: 'border-box', background: 'transparent', color: 'inherit',
      border: 'none', borderBottom: '1px solid #1e293b', outline: 'none',
      padding: '12px 14px', fontSize: '14px'
    }});
    var list = el('div', { style: { maxHeight: '50vh', overflowY: 'auto' } });
    box.append(input, list);
    overlay.append(box);

    var sel = 0, rows = [];

    function go(tab, path) {
      close();
      focusFileOnTab(tab, path);
    }

    function paint() {
      rows.forEach(function(r, i) {
        r.style.background = i === sel ? '#1e293b' : 'transparent';
      });
    }

    function hover(row) {
      sel = rows.indexOf(row);
      paint();
    }

    function renderList() {
      var q = input.value.toLowerCase();
      list.replaceChildren();
      rows = [];
      sel = 0;
      if (!q) return;
      qoMatches(q).forEach(function(f) {
        var row = qoRow(f, go, hover);
        list.append(row);
        rows.push(row);
      });
      paint();
    }

    function open() {
      overlay.style.display = 'flex';
      input.value = '';
      list.replaceChildren();
      rows = [];
      input.focus();
    }
    function close() { overlay.style.display = 'none'; }

    input.addEventListener('input', renderList);
    input.addEventListener('keydown', function(e) {
      if (e.key === 'ArrowDown') { sel = Math.min(sel + 1, rows.length - 1); paint(); e.preventDefault(); }
      else if (e.key === 'ArrowUp') { sel = Math.max(sel - 1, 0); paint(); e.preventDefault(); }
      else if (e.key === 'Enter' && rows[sel]) { rows[sel].click(); }
      else if (e.key === 'Escape') { close(); }
    });
    overlay.addEventListener('click', function(e) { if (e.target === overlay) close(); });
    document.addEventListener('keydown', function(e) {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        if (overlay.style.display === 'none') open(); else close();
      }
    });

    document.body.append(overlay);
    return { open: open };
  }
