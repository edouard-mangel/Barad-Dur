
  /* ---- Coupling tab ---- */

  /* Auto-exclude heuristics: pairs whose co-change is expected rather than
     a design smell. Each rule is a small predicate over the pair's file
     names (na/nb) and directories (da/db); the first match wins. */

  var CP_LOCK_FILES = ['Cargo.lock', 'package-lock.json', 'yarn.lock', 'pnpm-lock.yaml', 'composer.lock', 'Gemfile.lock', 'poetry.lock'];
  var CP_PROJECT_EXTS = ['.csproj', '.sln', '.fsproj', '.vbproj'];
  var CP_BUILD_FILES = ['pom.xml', 'build.gradle'];
  var CP_INDEX_FILES = ['mod.rs', 'lib.rs', 'index.ts', 'index.js', 'index.tsx', 'index.jsx', '__init__.py'];

  function cpStripTestSuffix(name) {
    return name
      .replace(/\.spec\.(ts|js|tsx|jsx|mjs)$/, '.$1')
      .replace(/\.test\.(ts|js|tsx|jsx|mjs|py)$/, '.$1')
      .replace(/_test\.go$/, '.go')
      .replace(/Tests?\.(java|cs|fs)$/, '.$1');
  }

  function cpIsTestPair(p) {
    if (cpStripTestSuffix(p.na) !== p.na && cpStripTestSuffix(p.na) === p.nb) return true;
    return cpStripTestSuffix(p.nb) !== p.nb && cpStripTestSuffix(p.nb) === p.na;
  }

  /* IFoo.cs <-> Foo.cs (same dir), FooImpl/FooInterface <-> Foo (Java). */
  function cpIsInterfaceImplPair(p) {
    if (p.da === p.db && p.na.endsWith('.cs') && p.nb.endsWith('.cs')) {
      var aBase = p.na.slice(0, -3), bBase = p.nb.slice(0, -3);
      if (aBase === 'I' + bBase || bBase === 'I' + aBase) return true;
    }
    if (p.na.endsWith('.java') && p.nb.endsWith('.java')) {
      var aj = p.na.slice(0, -5), bj = p.nb.slice(0, -5);
      if (aj + 'Impl' === bj || bj + 'Impl' === aj) return true;
      if (aj + 'Interface' === bj || bj + 'Interface' === aj) return true;
    }
    return false;
  }

  var CP_EXCLUDE_RULES = [
    { reason: 'lock file', match: function(p) {
      return CP_LOCK_FILES.indexOf(p.na) >= 0 || CP_LOCK_FILES.indexOf(p.nb) >= 0;
    } },
    { reason: 'project file', match: function(p) {
      return CP_PROJECT_EXTS.some(function(ext) { return p.na.endsWith(ext) || p.nb.endsWith(ext); });
    } },
    { reason: 'build file', match: function(p) {
      return CP_BUILD_FILES.indexOf(p.na) >= 0 || CP_BUILD_FILES.indexOf(p.nb) >= 0;
    } },
    { reason: 'module index', match: function(p) {
      return p.da === p.db && (CP_INDEX_FILES.indexOf(p.na) >= 0 || CP_INDEX_FILES.indexOf(p.nb) >= 0);
    } },
    { reason: 'test file', match: cpIsTestPair },
    { reason: 'interface/impl', match: cpIsInterfaceImplPair }
  ];

  /* Reason string when the pair matches an auto-exclude rule, else null. */
  function isAutoExcluded(a, b) {
    var pair = {
      na: a.split('/').pop(),
      nb: b.split('/').pop(),
      da: a.substring(0, a.lastIndexOf('/') + 1),
      db: b.substring(0, b.lastIndexOf('/') + 1)
    };
    for (var i = 0; i < CP_EXCLUDE_RULES.length; i++) {
      if (CP_EXCLUDE_RULES[i].match(pair)) return CP_EXCLUDE_RULES[i].reason;
    }
    return null;
  }

  function cpPctColor(pct) {
    return pct > 70 ? 'var(--c-danger)' : pct > 40 ? 'var(--c-warn)' : 'var(--c-good)';
  }

  /* A file cell with dimmed directory prefix. */
  function cpFileCell(path) {
    var cell = el('td');
    var parts = fileParts(path);
    var dir = el('span', { className: 'file-dir' });
    dir.append(txt(parts.dir));
    var name = el('span', { className: 'file-name' });
    name.append(txt(parts.name));
    cell.append(dir, name);
    return cell;
  }

  /* One coupled-pair row. `ui.rerender` rebuilds the table after dismiss. */
  function cpPairRow(p, idx, excludeReason, state, ui) {
    var row = el('tr');
    if (excludeReason) row.className = 'cp-auto-excluded';

    var aCell = cpFileCell(p.file_a);
    var bCell = cpFileCell(p.file_b);
    if (excludeReason) {
      var tag = el('span', { className: 'cp-auto-tag' });
      tag.append(txt(excludeReason));
      bCell.append(tag);
    }

    var coCell = el('td');
    coCell.append(txt(String(p.co_changes)));

    var pctCell = el('td');
    var pctSpan = el('span', { style: { fontWeight: '700', color: cpPctColor(p.coupling_pct) } });
    pctSpan.append(txt(fmt(p.coupling_pct, 1) + '%'));
    pctCell.append(pctSpan);

    var cbCell = el('td');
    if (p.cross_boundary) {
      var cbBadge = el('span', { style: { color: 'var(--c-warn)', fontWeight: '600', fontSize: '0.75rem' } });
      cbBadge.append(txt('⚠ cross-boundary'));
      cbCell.append(cbBadge);
    }
    if (p.is_test_pair) {
      var tpBadge = el('span', { title: 'Expected coupling — production file and its test file naturally change together.', style: { marginLeft: '4px', cursor: 'default' } });
      tpBadge.append(txt('🧪'));
      cbCell.append(tpBadge);
    }

    var barCell = el('td', { className: 'inline-bar' });
    barCell.append(inlineBar(p.coupling_pct, cpPctColor(p.coupling_pct)));

    var dismissCell = el('td');
    var dismissBtn = el('button', { className: 'cp-dismiss' });
    dismissBtn.append(txt('×'));
    dismissBtn.addEventListener('click', function() {
      state.dismissed[idx] = true;
      ui.rerender();
    });
    dismissCell.append(dismissBtn);

    row.append(aCell, bCell, coCell, pctCell, cbCell, barCell, dismissCell);
    return row;
  }

  var CP_COL_TIPS = {
    'Co-changes': 'Number of commits where both files were modified together.',
    'Coupling %': 'Co-changes ÷ min(commits A, commits B). Answers: “Of the less-frequently-changed file’s commits, what share also touched the other file?” 100 % means the two files always move together.',
    'Cross-boundary': 'The files live in different top-level modules or directories. Cross-boundary coupling is riskier because it signals hidden dependencies between components that should be independent.'
  };

  /* The coupled-pairs table. Returns the table plus the counts the status
     bar needs (auto-excluded matches and rows currently hidden). */
  function cpPairsTable(pairs, state, ui) {
    var table = el('table');
    var thead = el('thead');
    var hRow = el('tr');
    ['File A', 'File B', 'Co-changes', 'Coupling %', 'Cross-boundary', '', ''].forEach(function(h) {
      hRow.append(thWithTip(h, CP_COL_TIPS[h] || null));
    });
    thead.append(hRow);
    table.append(thead);

    var tbody = el('tbody');
    var hiddenCount = 0;
    var autoCount = 0;

    pairs.slice(0, 100).forEach(function(p, idx) {
      var excludeReason = isAutoExcluded(p.file_a, p.file_b);
      if (excludeReason) autoCount++;
      if (state.dismissed[idx]) { hiddenCount++; return; }
      if (excludeReason && !state.showAutoExcluded) { hiddenCount++; return; }
      tbody.append(cpPairRow(p, idx, excludeReason, state, ui));
    });
    table.append(tbody);
    return { table: table, autoCount: autoCount, hiddenCount: hiddenCount };
  }

  /* Instability panel: Ce/(Ca+Ce) per file from the static import graph. */
  function cpInstabilityCard() {
    var card = el('div', { className: 'view-card' });

    var header = el('div', { className: 'tab-info-title' });
    header.append(txt('Instability by File'));
    card.append(header);

    var desc = el('div', { style: { fontSize: '13px', color: 'var(--text-muted)', margin: '6px 0 12px' } });
    desc.append(txt('Instability = Ce ÷ (Ca + Ce). 0 = maximally stable (depended upon, changes carefully). 1 = maximally unstable (depends on others, safe to change freely).'));
    card.append(desc);

    var perFileCoupling = R.per_file_coupling;
    if (!perFileCoupling || perFileCoupling.length === 0) {
      var noData = el('div', { className: 'no-data' });
      noData.append(txt('No static import data available.'));
      card.append(noData);
      return card;
    }

    var wrap = el('div', { style: { overflowX: 'auto' } });
    var table = el('table');
    var thead = el('thead');
    var hRow = el('tr');
    [
      { label: 'File', tip: null },
      { label: 'Ca', tip: 'Afferent coupling: number of files that import this file. High Ca = many dependents, risky to change.' },
      { label: 'Ce', tip: 'Efferent coupling: number of files this file imports. High Ce = many dependencies.' },
      { label: 'Instability', tip: 'Ce / (Ca + Ce). 0 = stable (depended upon). 1 = unstable (depends on others).' }
    ].forEach(function(col) {
      hRow.append(thWithTip(col.label, col.tip));
    });
    thead.append(hRow);
    table.append(thead);

    var tbody = el('tbody');
    perFileCoupling.slice().sort(function(a, b) { return b.instability - a.instability; }).slice(0, 50).forEach(function(f) {
      var row = el('tr');

      var fileCell = cpFileCell(f.path);
      linkFileCell(fileCell, f.path, 'Graph');

      var caCell = el('td');
      caCell.append(txt(String(f.ca)));

      var ceCell = el('td');
      ceCell.append(txt(String(f.ce)));

      var instColor = f.instability <= 0.3
        ? 'var(--c-good)'
        : f.instability <= 0.7
          ? 'var(--c-warn)'
          : 'var(--c-danger)';

      var instCell = el('td', { style: { display: 'flex', alignItems: 'center', gap: '8px' } });
      var instVal = el('span', { style: { fontWeight: '700', color: instColor, minWidth: '3.5ch' } });
      instVal.append(txt(fmt(f.instability, 2)));
      instCell.append(instVal, inlineBar(f.instability * 100, instColor));

      row.append(fileCell, caCell, ceCell, instCell);
      tbody.append(row);
    });
    table.append(tbody);
    wrap.append(table);
    card.append(wrap);
    return card;
  }

  /* Prioritised per-file refactoring suggestions, or null when none. */
  function cpActionsCard() {
    var couplingActions = R.coupling_actions;
    if (!couplingActions || couplingActions.length === 0) return null;

    var card = el('div', { className: 'view-card' });
    var header = el('div', { className: 'tab-info-title' });
    header.append(txt('Coupling Actions'));
    card.append(header);

    var desc = el('div', { style: { fontSize: '13px', color: 'var(--text-muted)', margin: '6px 0 12px' } });
    desc.append(txt('Prioritised per-file refactoring suggestions, worst coupling rung first.'));
    card.append(desc);

    var list = el('ol', { className: 'coupling-actions-list' });
    couplingActions.forEach(function(a) {
      var li = el('li');
      li.append(txt(a.text));
      list.append(li);
    });
    card.append(list);
    return card;
  }

  function buildCouplingTab() {
    var pairs = (R.coupling_pairs || []).slice().sort(function(a, b) {
      return b.coupling_pct - a.coupling_pct;
    });

    var container = el('div');

    if (pairs.length === 0) {
      var noTempData = el('div', { className: 'no-data' });
      noTempData.append(txt('No temporal coupling data available.'));
      container.append(noTempData);
    } else {
      container.append(buildTabInfo(
        'Temporal Coupling — Files that change together',
        'Temporal coupling measures how often two files are modified in the same commit. A high percentage means the files are implicitly linked — changing one almost always requires changing the other. This can indicate hidden dependencies, duplicated logic, or missing abstractions. Consider extracting shared interfaces or merging tightly coupled files.',
        [
          { color: 'var(--c-good-lo)', label: '<30% — Normal co-change' },
          { color: 'var(--c-warn)',    label: '30–60% — Worth investigating' },
          { color: 'var(--c-danger)', label: '>60% — Strongly coupled, refactor candidate' }
        ]
      ));

      // Hidden-row state, mutated by the dismiss buttons and the toggle.
      var state = { dismissed: {}, showAutoExcluded: false };

      // Controls
      var controls = el('div', { className: 'cp-controls' });
      var toggleAutoBtn = el('button');
      toggleAutoBtn.append(txt('Show auto-excluded'));
      var statusSpan = el('span');
      var resetBtn = el('button');
      resetBtn.append(txt('Reset dismissed'));
      controls.append(toggleAutoBtn, statusSpan, resetBtn);
      container.append(controls);

      var card = el('div', { className: 'view-card' });
      var tableWrap = el('div', { style: { overflowX: 'auto' } });

      var ui = { rerender: renderTable };

      function renderTable() {
        var built = cpPairsTable(pairs, state, ui);
        tableWrap.replaceChildren(built.table);

        // Status line + control labels reflect the new counts
        statusSpan.replaceChildren();
        var parts = [];
        if (built.autoCount > 0) parts.push(built.autoCount + ' auto-excluded');
        var dismissedCount = Object.keys(state.dismissed).length;
        if (dismissedCount > 0) parts.push(dismissedCount + ' dismissed');
        if (parts.length > 0) {
          statusSpan.append(txt(parts.join(', ') + ' — ' + built.hiddenCount + ' hidden'));
        }
        resetBtn.style.display = dismissedCount > 0 ? '' : 'none';
        toggleAutoBtn.className = state.showAutoExcluded ? 'active' : '';
        toggleAutoBtn.replaceChildren();
        toggleAutoBtn.append(txt(state.showAutoExcluded ? 'Hide auto-excluded' : 'Show auto-excluded (' + built.autoCount + ')'));
      }

      toggleAutoBtn.addEventListener('click', function() {
        state.showAutoExcluded = !state.showAutoExcluded;
        renderTable();
      });
      resetBtn.addEventListener('click', function() {
        state.dismissed = {};
        renderTable();
      });

      renderTable();
      card.append(tableWrap);
      container.append(card);
    }

    container.append(cpInstabilityCard());

    var actions = cpActionsCard();
    if (actions) container.append(actions);

    return container;
  }
