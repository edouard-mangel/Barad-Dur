pub const JS: &str = r#"
  /* ---- Coupling tab ---- */
  // Auto-exclude patterns for coupling: interface/implementation, lock files, test files, module indexes
  function isAutoExcluded(a, b) {
    var na = a.split('/').pop(), nb = b.split('/').pop();
    var da = a.substring(0, a.lastIndexOf('/') + 1);
    var db = b.substring(0, b.lastIndexOf('/') + 1);
    // Lock files: Cargo.lock/Cargo.toml, package-lock.json/package.json, yarn.lock, pnpm-lock.yaml, *.lock
    var lockFiles = ['Cargo.lock', 'package-lock.json', 'yarn.lock', 'pnpm-lock.yaml', 'composer.lock', 'Gemfile.lock', 'poetry.lock'];
    var manifestFiles = ['Cargo.toml', 'package.json', 'yarn.lock', 'pnpm-lock.yaml', 'composer.json', 'Gemfile', 'pyproject.toml'];
    if (lockFiles.indexOf(na) >= 0 || lockFiles.indexOf(nb) >= 0) return 'lock file';
    // Project files: *.csproj, *.sln, pom.xml, build.gradle
    var projFiles = ['.csproj', '.sln', '.fsproj', '.vbproj'];
    if (projFiles.some(function(ext) { return na.endsWith(ext) || nb.endsWith(ext); })) return 'project file';
    if (na === 'pom.xml' || nb === 'pom.xml' || na === 'build.gradle' || nb === 'build.gradle') return 'build file';
    // Module index files: mod.rs, index.ts/js, __init__.py, lib.rs
    var indexFiles = ['mod.rs', 'lib.rs', 'index.ts', 'index.js', 'index.tsx', 'index.jsx', '__init__.py'];
    if (da === db && (indexFiles.indexOf(na) >= 0 || indexFiles.indexOf(nb) >= 0)) return 'module index';
    // Test file pairs: foo.ts <-> foo.spec.ts, foo.test.ts, foo_test.go, FooTest.java, FooTests.cs
    function stripTestSuffix(name) {
      return name
        .replace(/\.spec\.(ts|js|tsx|jsx|mjs)$/, '.$1')
        .replace(/\.test\.(ts|js|tsx|jsx|mjs|py)$/, '.$1')
        .replace(/_test\.go$/, '.go')
        .replace(/Tests?\.(java|cs|fs)$/, '.$1')
        .replace(/Tests?\.(cs|fs)$/, '.$1');
    }
    if (stripTestSuffix(na) !== na && stripTestSuffix(na) === nb) return 'test file';
    if (stripTestSuffix(nb) !== nb && stripTestSuffix(nb) === na) return 'test file';
    // C# interface: IFoo.cs <-> Foo.cs (same dir)
    if (da === db && na.endsWith('.cs') && nb.endsWith('.cs')) {
      var aBase = na.slice(0, -3), bBase = nb.slice(0, -3);
      if (aBase === 'I' + bBase || bBase === 'I' + aBase) return 'interface/impl';
    }
    // Java interface/impl: FooInterface.java <-> FooImpl.java
    if (na.endsWith('.java') && nb.endsWith('.java')) {
      var aj = na.slice(0, -5), bj = nb.slice(0, -5);
      if (aj + 'Impl' === bj || bj + 'Impl' === aj) return 'interface/impl';
      if (aj + 'Interface' === bj || bj + 'Interface' === aj) return 'interface/impl';
    }
    return null;
  }

  function buildCouplingTab() {
    var pairs = (R.coupling_pairs || []).slice().sort(function(a, b) {
      return b.coupling_pct - a.coupling_pct;
    });

    if (pairs.length === 0) {
      var d = el('div', { className: 'no-data' });
      d.append(txt('No temporal coupling data available.'));
      return d;
    }

    var container = el('div');
    container.append(buildTabInfo(
      'Temporal Coupling \u2014 Files that change together',
      'Temporal coupling measures how often two files are modified in the same commit. A high percentage means the files are implicitly linked \u2014 changing one almost always requires changing the other. This can indicate hidden dependencies, duplicated logic, or missing abstractions. Consider extracting shared interfaces or merging tightly coupled files.',
      [
        { color: '#22c55e', label: '<30% \u2014 Normal co-change' },
        { color: '#f59e0b', label: '30\u201360% \u2014 Worth investigating' },
        { color: '#ef4444', label: '>60% \u2014 Strongly coupled, refactor candidate' }
      ]
    ));

    // Track hidden state
    var dismissed = {};
    var showAutoExcluded = false;

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

    var COL_TIPS = {
      'Co-changes': 'Number of commits where both files were modified together.',
      'Coupling %': 'Co-changes \u00f7 min(commits A, commits B). Answers: \u201cOf the less-frequently-changed file\u2019s commits, what share also touched the other file?\u201d 100\u202f% means the two files always move together.',
      'Cross-boundary': 'The files live in different top-level modules or directories. Cross-boundary coupling is riskier because it signals hidden dependencies between components that should be independent.'
    };

    function renderTable() {
      tableWrap.replaceChildren();
      var table = el('table');
      var thead = el('thead');
      var hRow = el('tr');
      ['File A', 'File B', 'Co-changes', 'Coupling %', 'Cross-boundary', '', ''].forEach(function(h) {
        hRow.append(thWithTip(h, COL_TIPS[h] || null));
      });
      thead.append(hRow);
      table.append(thead);

      var tbody = el('tbody');
      var hiddenCount = 0;
      var autoCount = 0;

      pairs.slice(0, 100).forEach(function(p, idx) {
        var excludeReason = isAutoExcluded(p.file_a, p.file_b);
        if (excludeReason) autoCount++;
        if (dismissed[idx]) { hiddenCount++; return; }
        if (excludeReason && !showAutoExcluded) { hiddenCount++; return; }

        var row = el('tr');
        if (excludeReason) row.className = 'cp-auto-excluded';

        var aCell = el('td');
        var aParts = fileParts(p.file_a);
        var aDir = el('span', { className: 'file-dir' });
        aDir.append(txt(aParts.dir));
        var aName = el('span', { className: 'file-name' });
        aName.append(txt(aParts.name));
        aCell.append(aDir, aName);

        var bCell = el('td');
        var bParts = fileParts(p.file_b);
        var bDir = el('span', { className: 'file-dir' });
        bDir.append(txt(bParts.dir));
        var bName = el('span', { className: 'file-name' });
        bName.append(txt(bParts.name));
        bCell.append(bDir, bName);
        if (excludeReason) {
          var tag = el('span', { className: 'cp-auto-tag' });
          tag.append(txt(excludeReason));
          bCell.append(tag);
        }

        var coCell = el('td');
        coCell.append(txt(String(p.co_changes)));

        var pctCell = el('td');
        var pctSpan = el('span', { style: { fontWeight: '700', color: p.coupling_pct > 70 ? '#ef4444' : p.coupling_pct > 40 ? '#f59e0b' : '#10b981' } });
        pctSpan.append(txt(p.coupling_pct.toFixed(1) + '%'));
        pctCell.append(pctSpan);

        var cbCell = el('td');
        if (p.cross_boundary) {
          var cbBadge = el('span', { style: { color: '#f59e0b', fontWeight: '600', fontSize: '0.75rem' } });
          cbBadge.append(txt('\u26a0 cross-boundary'));
          cbCell.append(cbBadge);
        }

        var barCell = el('td', { className: 'inline-bar' });
        barCell.append(inlineBar(p.coupling_pct, p.coupling_pct > 70 ? '#ef4444' : p.coupling_pct > 40 ? '#f59e0b' : '#10b981'));

        var dismissCell = el('td');
        var dismissBtn = el('button', { className: 'cp-dismiss' });
        dismissBtn.append(txt('\u00d7'));
        dismissBtn.addEventListener('click', (function(i) {
          return function() { dismissed[i] = true; renderTable(); };
        })(idx));
        dismissCell.append(dismissBtn);

        row.append(aCell, bCell, coCell, pctCell, cbCell, barCell, dismissCell);
        tbody.append(row);
      });
      table.append(tbody);
      tableWrap.append(table);

      // Update status
      statusSpan.replaceChildren();
      var parts = [];
      if (autoCount > 0) parts.push(autoCount + ' auto-excluded');
      var dismissedCount = Object.keys(dismissed).length;
      if (dismissedCount > 0) parts.push(dismissedCount + ' dismissed');
      if (parts.length > 0) {
        statusSpan.append(txt(parts.join(', ') + ' \u2014 ' + hiddenCount + ' hidden'));
      }
      resetBtn.style.display = dismissedCount > 0 ? '' : 'none';
      toggleAutoBtn.className = showAutoExcluded ? 'active' : '';
      toggleAutoBtn.replaceChildren();
      toggleAutoBtn.append(txt(showAutoExcluded ? 'Hide auto-excluded' : 'Show auto-excluded (' + autoCount + ')'));
    }

    toggleAutoBtn.addEventListener('click', function() {
      showAutoExcluded = !showAutoExcluded;
      renderTable();
    });
    resetBtn.addEventListener('click', function() {
      dismissed = {};
      renderTable();
    });

    renderTable();
    card.append(tableWrap);
    container.append(card);
    return container;
  }
"#;
