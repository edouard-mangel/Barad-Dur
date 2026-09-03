
  /* ---- Tab chrome: info banners, explainers, tooltips, tip texts ---- */
  function buildTabInfo(title, description, scoreHints) {
    var info = el('div', { className: 'tab-info' });
    var t = el('div', { className: 'tab-info-title' });
    t.append(txt(title));
    info.append(t);
    var d = el('div');
    d.append(txt(description));
    info.append(d);
    if (scoreHints && scoreHints.length > 0) {
      var hints = el('div', { className: 'score-hint' });
      scoreHints.forEach(function(h) {
        var s = el('span');
        var dot = el('span', { className: 'dot', style: { background: h.color } });
        s.append(dot, txt(h.label));
        hints.append(s);
      });
      info.append(hints);
    }
    return info;
  }

  function buildExplainer(summaryText, sections) {
    var details = el('details', { className: 'explainer' });
    var summary = document.createElement('summary');
    summary.append(txt(summaryText));
    details.append(summary);
    var body = el('div', { className: 'explainer-body' });
    sections.forEach(function(s) {
      var h = el('h4');
      h.append(txt(s.heading));
      body.append(h);
      if (s.items) {
        var ul = el('ul');
        s.items.forEach(function(item) {
          var li = el('li');
          li.append(txt(item));
          ul.append(li);
        });
        body.append(ul);
      }
      if (s.text) {
        var p = el('div', { style: { marginTop: '4px' } });
        p.append(txt(s.text));
        body.append(p);
      }
    });
    details.append(body);
    return details;
  }

  var defaultScoreHints = [
    { color: 'var(--c-danger)', label: '0\u201339 Critical' },
    { color: 'var(--c-warn)',   label: '40\u201369 Needs work' },
    { color: 'var(--c-good-lo)', label: '70\u2013100 Healthy' }
  ];

  /* ---- Shared floating tooltip singleton ---- */
  var _floatingTip = el('div', { className: 'cp-tooltip' });
  document.body.append(_floatingTip);

  function showFloatingTip(icon, text) {
    _floatingTip.replaceChildren();
    _floatingTip.append(txt(text));
    _floatingTip.style.display = 'block';

    function onMove(e) {
      var vw = window.innerWidth, vh = window.innerHeight;
      var tw = _floatingTip.offsetWidth, th = _floatingTip.offsetHeight;
      var x = e.clientX + 14;
      var y = e.clientY - th / 2;
      if (x + tw > vw - 8) x = e.clientX - tw - 14;
      if (y < 8) y = 8;
      if (y + th > vh - 8) y = vh - th - 8;
      _floatingTip.style.left = x + 'px';
      _floatingTip.style.top  = y + 'px';
    }
    function onLeave() {
      _floatingTip.style.display = 'none';
      icon.removeEventListener('mousemove', onMove);
      icon.removeEventListener('mouseleave', onLeave);
    }
    icon.addEventListener('mousemove', onMove);
    icon.addEventListener('mouseleave', onLeave);
  }

  function tipIcon(text) {
    var icon = el('span', { className: 'th-tip' });
    icon.append(txt('?'));
    icon.addEventListener('mouseenter', function() { showFloatingTip(icon, text); });
    return icon;
  }

  function thWithTip(label, tipText) {
    var th = el('th');
    th.append(txt(label));
    if (tipText) th.append(tipIcon(tipText));
    return th;
  }

  /* ---- Metric & category tooltip definitions ---- */
  var CAT_TIPS = {
    'Health':     'Code quality and maintainability indicators — 35% of the overall score.',
    'Team':       'Team knowledge spread, activity, and collaboration health — 10% of the overall score.',
    'Evolution':  'How the codebase is growing and changing over time — 20% of the overall score.',
    'Git Hygiene':'Commit discipline, message quality, and history cleanliness — 15% of the overall score.',
    'Coupling':   'Structural and change-based coupling between modules — 20% of the overall score.',
    'Dependencies': 'Dependency freshness, vulnerability exposure, and licence risk (scored separately when --deps is used).'
  };

  var METRIC_TIPS = {
    // Health
    'Bus factor':           'Number of active contributors needed to cover 80% of attributable lines. A low score means critical knowledge is concentrated in too few people. Scoring: 1 → 25, 2 → 50, 3 → 75, 4+ → 100.',
    'God objects':          'Production-source files with LOC > 500, or LOC > 300 with >15 public methods, or that structurally dominate the import graph as a connectivity hub. Large repositories are scored by affected-file prevalence.',
    'Complex hotspots':     'Production-source files above the 75th percentile in both cyclomatic complexity and churn. Large repositories are scored by affected-file prevalence.',
    'Long methods':         'Functions with LOC > 40 or cyclomatic complexity > 10. Long or complex functions are harder to test, understand, and maintain (Fowler: Long Method).',
    'Code biomarkers':      'Files with nesting depth > 4 or nesting variance > 2.0. Deeply nested code signals accumulated complexity; high variance indicates erratic structure (Tornhill: Code Biomarkers).',
    'Churn-ownership risk': 'Production-source files that are both above the churn quartile and >80% owned by one author. Advisory only: clear ownership is useful evidence, but is not continuity risk without team context.',
    // Team
    'Knowledge distribution':  'How evenly commit knowledge is spread across contributors. Concentration in one or two people is a bus-factor risk for the whole team.',
    'Contributor activity':    'Percentage of contributors who committed in the last 3 months. High churn means accumulated context is regularly lost.',
    'Ownership clarity':       'Percentage of files with a clear owner (one author > 50% of blame lines). Clear ownership improves accountability and review quality.',
    'Collaboration patterns':  'Top-level directories with >80% line ownership by one author. Advisory only because directory boundaries do not necessarily represent teams or knowledge silos.',
    'Merge patterns':          'Ratio of merge commits and history irregularities. Excessive merges obscure history; very few may mean force-pushes that hide context.',
    'Code/test growth balance': 'Lines added to source vs test-code files per window half; unscored by design (informational). Inline unit tests inside source files count as source, and renames appear as new-file additions.',
    'Cross-team coupling':     'File pairs that repeatedly change on the same author-day but have different primary owners. Advisory until real team boundaries are configured; different authors do not necessarily mean different teams.',
    'Knowledge loss':          'Share of blamed lines written by authors not active in the analysis window — code nobody currently on the project can answer questions about (Tornhill, Ch. 13).',
    // Evolution
    'Growth trend':      'Net file and line additions in the analysis window. Informational and unscored: growth can reflect healthy product activity and is not maintainability debt by itself.',
    'Refactoring ratio': 'Percentage of commits that invest in structure (refactor / clean / improve keywords). A low ratio means technical debt is accumulating without dedicated paydown.',
    'Code age':          'Median age of code weighted by lines. Very old untouched code may be stale or dangerously stable \u2014 worth verifying it is still intentional.',
    'Commit cadence':    'Average commits per day. Irregular cadence (bursts then silence) can signal integration problems or batch-and-dump workflows.',
    // Git Hygiene
    'Commit message quality': 'Percentage of commits with meaningful messages (\u2265 20 chars). Conventional commit format (feat:, fix:, chore:) scores higher.',
    'History cleanliness':    'Presence of merge commits, octopus merges, and empty messages. A clean history makes git bisect, blame, and revert reliable.',
    'Gitignore coverage':     'Tracked files whose path shape suggests generated artefacts, OS metadata, local environment files, or credential material (for example .env, *.pem, *.key, a secrets.yaml). Source, documentation, binary, and *.example template files are not flagged on a filename word like secret or credentials alone. Review each finding before untracking anything.',
    'Firefighting ratio':     'Percentage of commits containing revert / hotfix / emergency / rollback keywords. A high ratio means the team is mostly reacting to incidents rather than building.',
    'Friction language ratio':'Percentage of commits admitting technical-debt friction (hack / workaround / kludge / temporary / fixme / sorry keywords). Unlike the firefighting ratio’s reactive-incident-response signal, this tracks debt knowingly shipped rather than something that broke.',
    // Coupling
    'Afferent coupling':      'Incoming dependencies (Ca) — how many files import this one. Detected via import graph built from use/import/require statements. Scored on the median Ca across ALL files, including the majority that have zero incoming deps. A value of 0.00 is normal and healthy for most repos. Scoring: median \u22642 \u2192 100, \u22645 \u2192 75, \u226410 \u2192 50, >10 \u2192 25.',
    'Efferent coupling':      'Outgoing dependencies (Ce) — how many files this one imports. Scored on the median Ce across ALL files. Most healthy codebases have median Ce near 0 because most files are leaf nodes that import few others. Scoring: median \u22643 \u2192 100, \u22646 \u2192 75, \u226412 \u2192 50, >12 \u2192 25.',
    'Circular dependencies':  'Production-source files that mutually depend on each other: A\u2192B and B\u2192A (depth 1), or A\u2192B\u2192C\u2192A (depth 2). Self-imports are ignored. Below 100 source files the absolute count bands apply, at 300+ affected-file prevalence governs alone, and between them the two are blended in proportion to the population.',
    'Change coupling smells': 'Cross-boundary file pairs that co-change above the configured ratio. When community corroboration is enabled, only affected production-source files in cross-community pairs influence the prevalence score; raw pair counts remain evidence.',
    'Co-change reach trend':  'Files whose distinct co-change partner count at least doubled from the first to the second half of the window and reached decay_min_partners (default 8) \u2014 Tornhill\u2019s "architectural decay in progress". Evidence only, never scored; flagged files carry a partner badge in the Hotspots tab.',
    'Test safety net': 'Source files whose compatible same-project, same-language naming-convention-paired test has stopped co-changing with them. Below 100 source files the absolute count bands apply, at 300+ eroding-pair prevalence governs alone, and between them the two are blended in proportion to the population.'
  };
