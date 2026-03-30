pub const JS: &str = r#"
  /* ---- Overview tab ---- */
  function buildOverviewTab() {
    var wrapper = el('div');
    wrapper.append(buildTabInfo(
      'Overview \u2014 Repository health at a glance',
      'The overall score (0\u2013100) is a weighted average of four categories: Health (40%), Team (15%), Evolution (25%), and Git Hygiene (20%). Each category aggregates several metrics scored individually. The radar chart shows balance across categories \u2014 a lopsided shape reveals areas needing attention. Recommendations below target the lowest-scoring metrics.',
      defaultScoreHints
    ));
    var div = el('div', { className: 'overview-grid' });

    // Left column
    var left = el('div', { className: 'left-col' });

    // Category cards
    (R.categories || []).forEach(function(cat) {
      left.append(buildCatCard(cat));
    });

    // Top actions
    left.append(buildActions(R.top_actions));

    // Right column
    var right = el('div', { className: 'right-col' });

    // Gauge
    var gaugeWrap = el('div', { className: 'gauge-wrap' });
    gaugeWrap.append(buildGauge(R.overall_score || 0));
    var gaugeLabel = el('div', { className: 'gauge-label' });
    gaugeLabel.append(txt('Overall Score'));
    gaugeWrap.append(gaugeLabel);
    right.append(gaugeWrap);

    // Radar chart
    if (R.categories && R.categories.length > 0) {
      var radarWrap = el('div', { className: 'gauge-wrap' });
      radarWrap.append(buildRadar(R.categories));
      right.append(radarWrap);
    }

    // Remote meta
    var remoteMeta = buildRemoteMeta(R.remote_meta);
    if (remoteMeta) right.append(remoteMeta);

    div.append(left, right);
    wrapper.append(div);
    return wrapper;
  }
"#;
