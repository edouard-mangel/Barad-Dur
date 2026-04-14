pub const JS: &str = r#"
  /* ---- Dependencies tab ---- */
  function buildDepsTab() {
    var container = document.createElement('div');
    var reports = (window.R.dep_ecosystem_reports || []);
    if (!reports.length) {
      var msg = document.createElement('p');
      msg.className = 'muted';
      msg.textContent = 'No lock files found. Run with --deps to enable dependency analysis.';
      container.appendChild(msg);
      return container;
    }
    reports.forEach(function(eco) {
      var card = document.createElement('div');
      card.className = 'card';

      var title = document.createElement('h3');
      title.textContent = eco.ecosystem + ' \u2014 ' + eco.total_deps + ' deps';
      card.appendChild(title);

      var drift = document.createElement('p');
      drift.textContent = 'Mean drift: ' + eco.mean_drift_years.toFixed(1) + ' libyears';
      card.appendChild(drift);

      var criticals = eco.critical_deps || [];
      if (criticals.length) {
        var ul = document.createElement('ul');
        ul.className = 'critical-list';
        criticals.forEach(function(dep) {
          var li = document.createElement('li');
          var label = '\u26A0 ' + dep.name + ' ' + dep.current_version
              + ' \u2014 ' + dep.drift_years.toFixed(1) + 'y behind';
          if (dep.vulnerabilities.length) {
            label += ' [' + dep.vulnerabilities.length + ' CVE(s)]';
          }
          li.textContent = label;
          ul.appendChild(li);
        });
        card.appendChild(ul);
      }
      container.appendChild(card);
    });
    return container;
  }
"#;
