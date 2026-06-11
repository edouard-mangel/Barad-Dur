
  /* ---- Authors tab ---- */
  function buildAuthorsTab() {
    var container = el('div');
    container.append(buildTabInfo(
      'Author Report Cards',
      'Per-contributor metrics derived from git blame and commit history. Files owned = files where author has >50% blame lines. Commit quality scores message length, conventional prefixes, and penalizes low-effort messages.',
      [
        { color: 'var(--c-good)',   label: 'Active \u2014 committed in last 30 days' },
        { color: 'var(--c-warn)',   label: 'Aging \u2014 30\u201390 days since last commit' },
        { color: 'var(--c-danger)', label: 'Stale \u2014 90+ days since last commit' }
      ]
    ));

    container.append(buildExplainer('Understanding commit quality and author metrics', [
      {
        heading: 'How commit quality is scored',
        text: 'Each commit message is scored 0\u2013100 using a heuristic. The per-author quality is the average across all their commits in the analysis window.'
      },
      {
        heading: 'Scoring breakdown',
        items: [
          'Base: +10 points for any non-empty message.',
          'Length: +10 (4\u201310 chars), +30 (11\u201350 chars), or +40 (51+ chars).',
          'Conventional prefix: +30 if the message starts with feat:, fix:, docs:, style:, refactor:, perf:, test:, chore:, ci:, or build:.',
          'Descriptive: +20 if the message has a body (newline) or exceeds 20 characters.',
          'Penalty: messages that are exactly "wip", "fix", "update", or "." are capped at 10.'
        ]
      },
      {
        heading: 'Files owned',
        text: 'A file is "owned" by an author when they have more than 50% of the blame lines in that file. This is the same threshold used in the Ownership tab.'
      },
      {
        heading: 'Activity status',
        items: [
          'Green (Active): committed within the last 30 days.',
          'Yellow (Aging): last commit was 30\u201390 days ago.',
          'Red (Stale): no commits in over 90 days.'
        ]
      },
      {
        heading: 'Directories touched',
        text: 'The number of unique parent directories across all files changed by this author\u2019s commits. A high number indicates broad codebase knowledge; a low number suggests a specialist focused on a narrow area.'
      }
    ]));

    var cards = (R.author_cards || []).slice();
    if (cards.length === 0) {
      var empty = el('div', { className: 'no-data' });
      empty.append(txt('No author data available. Run with blame enabled.'));
      container.append(empty);
      return container;
    }

    var toolbar = el('div', { className: 'ac-toolbar' });
    var search = el('input', { className: 'ac-search', placeholder: 'Filter by name or email...' });
    search.setAttribute('type', 'text');

    var sortOptions = [
      { key: 'commits', label: 'Commits' },
      { key: 'files', label: 'Files Owned' },
      { key: 'active', label: 'Last Active' },
      { key: 'quality', label: 'Commit Quality' }
    ];
    var currentSort = 'commits';

    var sortBtns = sortOptions.map(function(opt) {
      var btn = el('button', { className: 'ac-sort-btn' + (opt.key === 'commits' ? ' active' : '') });
      btn.append(txt(opt.label));
      btn.addEventListener('click', function() {
        currentSort = opt.key;
        toolbar.querySelectorAll('.ac-sort-btn').forEach(function(b) { b.className = 'ac-sort-btn'; });
        btn.className = 'ac-sort-btn active';
        renderCards();
      });
      return btn;
    });

    toolbar.append(search);
    sortBtns.forEach(function(b) { toolbar.append(b); });

    var grid = el('div', { className: 'ac-grid' });

    function activityColor(days) {
      if (days <= 30) return 'var(--c-good)';
      if (days <= 90) return 'var(--c-warn)';
      return 'var(--c-danger)';
    }

    function qualityColor(q) {
      if (q >= 70) return 'var(--c-good)';
      if (q >= 40) return 'var(--c-warn)';
      return 'var(--c-danger)';
    }

    function renderCards() {
      var q = search.value.toLowerCase();
      var filtered = cards.filter(function(c) {
        if (!q) return true;
        return c.name.toLowerCase().indexOf(q) >= 0 ||
               c.email.toLowerCase().indexOf(q) >= 0;
      });

      filtered.sort(function(a, b) {
        if (currentSort === 'commits') return b.commit_count - a.commit_count;
        if (currentSort === 'files') return b.files_owned - a.files_owned;
        if (currentSort === 'active') return a.days_since_active - b.days_since_active;
        if (currentSort === 'quality') return b.avg_commit_quality - a.avg_commit_quality;
        return 0;
      });

      grid.replaceChildren();
      filtered.forEach(function(c) {
        var card = el('div', { className: 'ac-card' });

        var nameEl = el('div', { className: 'ac-name' });
        var badge = el('span', { className: 'ac-badge' });
        badge.style.backgroundColor = activityColor(c.days_since_active);
        nameEl.append(badge, txt(c.name));
        var emailEl = el('div', { className: 'ac-email' });
        emailEl.append(txt(c.email));

        var stats = el('div', { className: 'ac-stats' });
        function addStat(label, value) {
          var lbl = el('div', { className: 'ac-stat-label' });
          lbl.append(txt(label));
          var val = el('div', { className: 'ac-stat-value' });
          val.append(txt(String(value)));
          stats.append(lbl, val);
        }
        addStat('Commits', c.commit_count);
        addStat('Files Owned', c.files_owned);
        addStat('Lines Owned', c.lines_owned);
        addStat('Dirs Touched', c.directories_touched);

        var qLabel = el('div', { className: 'ac-stat-label', style: { marginTop: '8px' } });
        qLabel.append(txt('Commit Quality'));
        var qBar = el('div', { style: { height: '6px', borderRadius: '3px', background: '#1e293b', marginTop: '4px' } });
        var qRounded = Math.round(c.avg_commit_quality);
        var qFill = el('div', { style: { height: '100%', borderRadius: '3px', width: qRounded + '%', background: qualityColor(qRounded) } });
        qBar.append(qFill);
        var qVal = el('div', { style: { fontSize: '11px', color: '#94a3b8', marginTop: '2px' } });
        qVal.append(txt(qRounded + '/100'));

        var activeEl = el('div', { style: { fontSize: '12px', color: activityColor(c.days_since_active), marginTop: '8px' } });
        var daysText = c.days_since_active === 0 ? 'Active today' :
                       c.days_since_active === 1 ? '1 day ago' :
                       c.days_since_active + ' days ago';
        activeEl.append(txt(daysText));

        var filesDiv = el('div', { className: 'ac-files' });
        if (c.top_files && c.top_files.length > 0) {
          var fLabel = el('div', { className: 'ac-stat-label' });
          fLabel.append(txt('Top Files'));
          filesDiv.append(fLabel);
          c.top_files.forEach(function(f) {
            var fi = el('div', { className: 'ac-file-item' });
            fi.append(txt(f));
            filesDiv.append(fi);
          });
        }

        card.append(nameEl, emailEl, stats, qLabel, qBar, qVal, activeEl, filesDiv);
        grid.append(card);
      });
    }

    search.addEventListener('input', renderCards);
    renderCards();

    container.append(toolbar, grid);
    return container;
  }

  function renderApp() {
    if (localStorage.getItem('cbf-palette')) document.body.classList.add('cbf');
    var app = document.getElementById('app');

    // Header
    var header = el('header');
    var headerRow = el('div', { className: 'header-row' });
    var brandWrap = el('div', { style: { display: 'flex', alignItems: 'center', gap: '10px' } });
    var brand = el('span', { className: 'brand' });
    brand.append(txt('Barad-dûr'));
    var dotSpan = el('span', { className: 'dot' });
    dotSpan.append(txt('|'));
    var repoName = el('span', { style: { fontWeight: '600', fontSize: '16px' } });
    repoName.append(txt(R.repo_name || ''));
    brandWrap.append(brand, dotSpan, repoName);

    var chips = el('div', { className: 'meta-chips' });
    chips.append(chip(R.branch || 'main', '#1e293b', '#94a3b8'));
    chips.append(chip(R.total_commits + ' commits', '#1e3a5f', '#93c5fd'));
    chips.append(chip(R.total_authors + ' authors', '#1c3547', '#6ee7b7'));
    chips.append(chip(R.total_files + ' files', '#2d1b3d', '#c4b5fd'));
    if (R.time_window_months && R.time_window_months > 0) {
      chips.append(chip(R.time_window_months + 'mo window', '#2a1f0a', '#fcd34d'));
    }
    var cbfBtn = el('button', { id: 'cbf-btn', className: 'chip' });
    function updateCbfBtn() {
      var on = document.body.classList.contains('cbf');
      cbfBtn.textContent = on ? '◐ Default' : '◑ CBF';
      cbfBtn.style.cssText = on
        ? 'border:1px solid #38bdf8;color:#38bdf8;background:#0c1929;cursor:pointer'
        : 'border:1px solid #334155;color:#64748b;background:transparent;cursor:pointer';
    }
    cbfBtn.addEventListener('click', function() {
      document.body.classList.toggle('cbf');
      if (document.body.classList.contains('cbf')) {
        localStorage.setItem('cbf-palette', '1');
      } else {
        localStorage.removeItem('cbf-palette');
      }
      updateCbfBtn();
    });
    updateCbfBtn();
    chips.append(cbfBtn);
    chips.append(buildThemeBtn());
    headerRow.append(brandWrap, chips);
    header.append(headerRow);

    function safeRender(name, builder) {
      try {
        return builder();
      } catch (e) {
        console.error('Tab render error (' + name + '):', e);
        var d = el('div', { className: 'tab-error' });
        d.append(txt('Failed to render "' + name + '" tab. Open browser console for details.'));
        return d;
      }
    }

    // Tabs
    var tabNames = ['Overview', 'Hotspots', 'Coupling', 'Ownership', 'Age', 'Treemap', 'Trends', 'Authors', 'Dependencies', 'Audit'];
    var tabContents = [
      buildOverviewTab,
      buildHotspotsTab,
      buildCouplingTab,
      buildOwnershipTab,
      buildAgeTab,
      buildTreemapTab,
      buildTrendsTab,
      buildAuthorsTab,
      buildDepsTab,
      buildAuditTab
    ];

    var tabs = el('div', { className: 'tabs' });
    var contentDivs = [];
    var activeTab = 0;

    tabNames.forEach(function(name, i) {
      var tab = el('button', { className: 'tab' + (i === 0 ? ' active' : '') });
      tab.append(txt(name));

      var contentDiv = el('div', { className: 'tab-content' + (i === 0 ? ' active' : '') });
      contentDivs.push(contentDiv);

      tab.addEventListener('click', (function(idx, t) {
        return function() {
          // Remove active from all
          var allTabs = tabs.querySelectorAll('.tab');
          allTabs.forEach(function(tb) { tb.className = 'tab'; });
          contentDivs.forEach(function(cd) { cd.className = 'tab-content'; });
          t.className = 'tab active';
          contentDivs[idx].className = 'tab-content active';

          // Lazy-render on first visit
          if (contentDivs[idx].dataset.rendered !== '1') {
            contentDivs[idx].replaceChildren(safeRender(tabNames[idx], tabContents[idx]));
            contentDivs[idx].dataset.rendered = '1';
          }
        };
      })(i, tab));

      tabs.append(tab);
    });

    // Pre-render overview immediately
    contentDivs[0].replaceChildren(safeRender(tabNames[0], tabContents[0]));
    contentDivs[0].dataset.rendered = '1';

    // Expose tab-switching for drill-through links
    window.__switchToTab = function(tabName, sortBy) {
      var idx = tabNames.indexOf(tabName.charAt(0).toUpperCase() + tabName.slice(1));
      if (idx < 0) return;
      var allTabs = tabs.querySelectorAll('.tab');
      allTabs.forEach(function(tb) { tb.className = 'tab'; });
      contentDivs.forEach(function(cd) { cd.className = 'tab-content'; });
      allTabs[idx].className = 'tab active';
      contentDivs[idx].className = 'tab-content active';
      if (contentDivs[idx].dataset.rendered !== '1') {
        contentDivs[idx].replaceChildren(safeRender(tabNames[idx], tabContents[idx]));
        contentDivs[idx].dataset.rendered = '1';
      }
      contentDivs[idx].scrollIntoView({ behavior: 'smooth', block: 'start' });
    };

    app.replaceChildren(header, tabs, ...contentDivs);
  }

  initTheme();
  renderApp();
})();
