pub(crate) mod format;
use format::*;

use colored::Colorize;

use crate::scorer::AnalysisReport;
use crate::trend::TrendSummary;

/// Render the analysis report as a colored CLI string.
///
/// When `trend` is `Some` and `!trend.delta.is_first`, a delta line and direction
/// indicator are injected after the overall score line. When `trend` is `None` or
/// `trend.delta.is_first`, the first-snapshot notice is emitted instead.
pub fn render(report: &AnalysisReport, verbosity: u8, trend: Option<&TrendSummary>) -> String {
    let mut out = String::new();
    out.push_str(&render_repo_info(report));
    out.push_str(&render_score_and_trend(report, trend));
    out.push_str(&render_categories(report, trend, verbosity));
    out.push_str(&render_actions_and_footer(report));
    out
}

fn render_repo_info(report: &AnalysisReport) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "\n{}\n",
        "━━━ Barad-dur ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bold()
    ));
    out.push_str(&format!(
        "  {} {} on {}\n",
        "Repository:".dimmed(),
        report.repo_name.bold(),
        report.branch
    ));
    out.push_str(&format!(
        "  {} {} commits, {} authors, {} files\n",
        "Scope:".dimmed(),
        report.total_commits,
        report.total_authors,
        report.total_files
    ));
    if report.time_window_months > 0 {
        out.push_str(&format!(
            "  {} last {} months\n",
            "Window:".dimmed(),
            report.time_window_months
        ));
    } else {
        out.push_str(&format!("  {} full history\n", "Window:".dimmed()));
    }
    if let Some(meta) = &report.remote_meta {
        out.push_str(&format!("  {} {}\n", "Source:".dimmed(), meta.url.bold()));
        let mut details = Vec::new();
        if let Some(stars) = meta.stars {
            details.push(format!("Stars: {}", stars));
        }
        if let Some(lang) = &meta.language {
            details.push(format!("Language: {}", lang));
        }
        if let Some(issues) = meta.open_issues {
            details.push(format!("Issues: {}", issues));
        }
        if !details.is_empty() {
            out.push_str(&format!("  {}\n", details.join("   ").dimmed()));
        }
        if let Some(desc) = &meta.description {
            if !desc.is_empty() {
                out.push_str(&format!("  {}\n", desc.dimmed()));
            }
        }
    }

    out
}

fn render_trend_line(trend: &TrendSummary, report: &AnalysisReport) -> String {
    let mut out = String::new();

    if !trend.delta.is_first && !trend.branch_mismatch_warning {
        let delta = trend.delta.overall;
        let delta_str = if delta >= 0 {
            format!("+{} vs last run", delta)
        } else {
            format!("{} vs last run", delta)
        };
        out.push_str(&format!("  {}\n", delta_str.dimmed()));

        if let Some(velocity) = &trend.velocity {
            let direction_str = direction_word(&velocity.direction);
            let arrow = direction_arrow(&velocity.direction);
            out.push_str(&format!("  {} {}\n", arrow, direction_str.dimmed()));
        }
    }

    let _ = report; // kept for API symmetry
    out
}

fn render_score_and_trend(report: &AnalysisReport, trend: Option<&TrendSummary>) -> String {
    let mut out = String::new();

    // Branch mismatch warning — suppresses delta and notifies the user.
    if let Some(summary) = trend {
        if summary.branch_mismatch_warning {
            let prior_branch = summary
                .history
                .last()
                .map(|e| e.branch.as_str())
                .unwrap_or("unknown");
            out.push_str(&format!(
                "  {}\n",
                format!(
                    "Warning: prior history on '{}'; current branch is '{}'",
                    prior_branch, report.branch
                )
                .yellow()
            ));
        }
    }

    // First-run notice or trend delta
    let is_first = trend.map(|t| t.delta.is_first).unwrap_or(true);
    let is_branch_mismatch = trend.map(|t| t.branch_mismatch_warning).unwrap_or(false);
    if is_first && !is_branch_mismatch {
        out.push_str(&format!(
            "  {}\n",
            "Trend: first snapshot recorded".dimmed()
        ));
    }

    // Overall score
    out.push_str(&format!(
        "\n  {} {} {}\n",
        "Overall Score:".bold(),
        format_score_bar(report.overall_score, 20),
        format_score_number(report.overall_score)
    ));

    // Delta and direction (only when a prior run exists on this branch, and no branch mismatch)
    if let Some(summary) = trend {
        out.push_str(&render_trend_line(summary, report));
    }

    out
}

fn render_single_category(
    cat: &crate::metrics::CategoryResult,
    delta: Option<i32>,
    verbosity: u8,
) -> String {
    let mut out = String::new();

    let delta_suffix = delta
        .map(|d| {
            if d >= 0 {
                format!("  (+{})", d)
            } else {
                format!("  ({})", d)
            }
        })
        .unwrap_or_default();

    out.push_str(&format!(
        "\n  {} {} {}{}\n",
        format!("▸ {}", cat.name).bold(),
        format_score_bar(cat.score, 12),
        format_score_number(cat.score),
        delta_suffix.dimmed()
    ));

    if verbosity > 0 {
        for metric in &cat.metrics {
            // Unscored metric (insufficient data): dimmed dot + dash.
            let score_indicator = metric
                .score
                .map(format_score_dot)
                .unwrap_or_else(|| "○".dimmed().to_string());
            let score_label = metric
                .score
                .map(format_score_number)
                .unwrap_or_else(|| "—".dimmed().to_string());
            out.push_str(&format!(
                "    {} {} {}  {}\n",
                score_indicator,
                metric.name,
                score_label,
                metric.description.dimmed()
            ));
            if verbosity > 1 {
                out.push_str(&format!(
                    "      {} {}\n",
                    "value:".dimmed(),
                    metric.raw_value.to_string().bold()
                ));
            }
        }
    }

    out
}

fn render_dep_section(dep_reports: &[crate::deps::EcosystemReport]) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "\n{}\n",
        "───────────────────────────────────────────────────".dimmed()
    ));
    out.push_str(&format!("  {}\n", "Dependencies:".bold()));
    for eco in dep_reports {
        out.push_str(&format!(
            "  {} {}: {:.1} libyears avg ({} deps)\n",
            "▸".bright_blue(),
            eco.ecosystem.display_name(),
            eco.mean_drift_years,
            eco.total_deps
        ));
        for dep in &eco.critical_deps {
            let suffix = if dep.vulnerabilities.is_empty() {
                String::new()
            } else {
                format!(" [{} CVE(s)]", dep.vulnerabilities.len())
            };
            out.push_str(&format!(
                "    {} {} {} — {:.1}y behind{}\n",
                "⚠".yellow(),
                dep.name,
                dep.current_version,
                dep.drift_years,
                suffix
            ));
        }
    }

    out
}

fn render_top_unstable_files(report: &AnalysisReport) -> String {
    if report.per_file_coupling.is_empty() {
        return String::new();
    }

    report.per_file_coupling.iter().take(5).fold(
        String::from("    Top unstable files:\n"),
        |acc, m| {
            acc + &format!(
                "      {:.2}  {}  (Ca={}, Ce={})\n",
                m.instability, m.path, m.ca, m.ce
            )
        },
    )
}

fn render_categories(
    report: &AnalysisReport,
    trend: Option<&TrendSummary>,
    verbosity: u8,
) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "\n{}\n",
        "───────────────────────────────────────────────────".dimmed()
    ));

    let category_deltas = trend
        .filter(|t| !t.delta.is_first && !t.branch_mismatch_warning)
        .map(|t| &t.delta.categories);

    for cat in &report.categories {
        let delta = category_deltas
            .and_then(|deltas| deltas.get(&cat.name))
            .copied();
        out.push_str(&render_single_category(cat, delta, verbosity));
    }

    out.push_str(&render_top_unstable_files(report));

    if !report.dep_ecosystem_reports.is_empty() {
        out.push_str(&render_dep_section(&report.dep_ecosystem_reports));
    }

    out
}

fn render_actions_and_footer(report: &AnalysisReport) -> String {
    let mut out = String::new();

    if !report.top_actions.is_empty() {
        out.push_str(&format!(
            "\n{}\n",
            "───────────────────────────────────────────────────".dimmed()
        ));
        out.push_str(&format!("  {}\n", "Top Actions:".bold()));
        for (i, action) in report.top_actions.iter().enumerate() {
            out.push_str(&format!("  {}. {}\n", i + 1, action.text));
        }
    }

    out.push_str(&format!(
        "{}\n\n",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bold()
    ));

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{CategoryResult, MetricValue, RawValue};
    use crate::scorer::ActionItem;
    use crate::trend::{TrendDelta, TrendSummary, TrendVelocity, VelocityDirection};
    use std::collections::HashMap;

    fn make_report() -> AnalysisReport {
        AnalysisReport {
            repo_name: "test-repo".into(),
            branch: "main".into(),
            time_window_months: 6,
            total_commits: 100,
            total_authors: 5,
            total_files: 50,
            overall_score: 72,
            categories: vec![CategoryResult {
                name: "Health".into(),
                score: 72,
                metrics: vec![MetricValue {
                    name: "Bus factor".into(),
                    description: "2 (risky)".into(),
                    raw_value: RawValue::Integer(2),
                    score: Some(50),
                }],
            }],
            top_actions: vec![ActionItem {
                text: "[Health] Bus factor (score: 50) — Improve".into(),
                target_tab: Some("ownership".into()),
                sort_by: None,
            }],
            remote_meta: None,
            file_hotspots: vec![],
            coupling_pairs: vec![],
            author_ownership: vec![],
            file_ages: vec![],
            author_cards: vec![],
            history: vec![],
            dep_ecosystem_reports: vec![],
            audit: None,
            per_file_coupling: vec![],
            import_edges: vec![],
            import_cycles: vec![],
            coupling_finding_counts: None,
            score_thresholds: Default::default(),
        }
    }

    fn make_first_run_trend() -> TrendSummary {
        TrendSummary {
            delta: TrendDelta {
                overall: 0,
                delta_vs_oldest: 0,
                categories: HashMap::new(),
                is_first: true,
            },
            sparkline: vec![],
            velocity: None,
            branch_mismatch_warning: false,
            history: vec![],
        }
    }

    fn make_subsequent_run_trend(delta: i32) -> TrendSummary {
        TrendSummary {
            delta: TrendDelta {
                overall: delta,
                delta_vs_oldest: delta,
                categories: HashMap::new(),
                is_first: false,
            },
            sparkline: vec![],
            velocity: Some(TrendVelocity {
                direction: VelocityDirection::Stable,
                points_per_run: delta as f64,
                window_size: 2,
            }),
            branch_mismatch_warning: false,
            history: vec![],
        }
    }

    #[test]
    fn render_contains_header() {
        let report = make_report();
        let output = render(&report, 0, None);
        assert!(output.contains("Barad-dur"));
    }

    #[test]
    fn render_verbose_shows_dash_for_unscored_metric() {
        let mut report = make_report();
        report.categories[0].metrics.push(MetricValue {
            name: "Knowledge distribution".into(),
            description: "Solo project — not applicable".into(),
            raw_value: RawValue::Text("N/A".into()),
            score: None,
        });
        let output = render(&report, 1, None);
        assert!(
            output.contains("—"),
            "unscored metrics must render a dash in verbose CLI output"
        );
        assert!(
            !output.contains("Knowledge distribution 100"),
            "unscored metrics must not display a fake score"
        );
    }

    #[test]
    fn render_contains_repo_info() {
        let report = make_report();
        let output = render(&report, 0, None);
        assert!(output.contains("test-repo"));
        assert!(output.contains("main"));
    }

    #[test]
    fn render_contains_category() {
        let report = make_report();
        let output = render(&report, 0, None);
        assert!(output.contains("Health"));
    }

    #[test]
    fn render_verbose_shows_metrics() {
        let report = make_report();
        let output = render(&report, 1, None);
        assert!(output.contains("Bus factor"));
        assert!(output.contains("50/100"));
    }

    #[test]
    fn render_very_verbose_shows_raw_value() {
        let report = make_report();
        let output = render(&report, 2, None);
        assert!(output.contains("value:"));
        assert!(output.contains('2'));
    }

    #[test]
    fn render_first_run_shows_trend_notice() {
        let report = make_report();
        let trend = make_first_run_trend();
        let output = render(&report, 0, Some(&trend));
        assert!(
            output.contains("Trend: first snapshot recorded"),
            "first run should show trend notice"
        );
    }

    #[test]
    fn render_subsequent_run_no_trend_notice() {
        let report = make_report();
        let trend = make_subsequent_run_trend(0);
        let output = render(&report, 0, Some(&trend));
        assert!(
            !output.contains("Trend: first snapshot recorded"),
            "subsequent run should not show first-run trend notice"
        );
    }

    #[test]
    fn render_contains_actions() {
        let report = make_report();
        let output = render(&report, 0, None);
        assert!(output.contains("Top Actions"));
    }

    #[test]
    fn render_category_row_includes_delta_on_subsequent_run() {
        let report = make_report(); // Has "Health" category at score 72
        let mut category_deltas = HashMap::new();
        category_deltas.insert("Health".to_string(), 3_i32);
        let trend = TrendSummary {
            delta: TrendDelta {
                overall: 3,
                delta_vs_oldest: 3,
                categories: category_deltas,
                is_first: false,
            },
            sparkline: vec![],
            velocity: Some(TrendVelocity {
                direction: VelocityDirection::Improving,
                points_per_run: 3.0,
                window_size: 2,
            }),
            branch_mismatch_warning: false,
            history: vec![],
        };
        let output = render(&report, 0, Some(&trend));

        // The Health category row must contain the delta marker "+3"
        let health_line = output.lines().find(|l| l.contains("Health")).unwrap_or("");
        assert!(
            health_line.contains("+3"),
            "Health category row should show delta '+3', got: {health_line:?}"
        );
    }

    #[test]
    fn render_category_row_shows_negative_delta() {
        let report = make_report(); // Has "Health" category at score 72
        let mut category_deltas = HashMap::new();
        category_deltas.insert("Health".to_string(), -5_i32);
        let trend = TrendSummary {
            delta: TrendDelta {
                overall: -5,
                delta_vs_oldest: -5,
                categories: category_deltas,
                is_first: false,
            },
            sparkline: vec![],
            velocity: Some(TrendVelocity {
                direction: VelocityDirection::Declining,
                points_per_run: -5.0,
                window_size: 2,
            }),
            branch_mismatch_warning: false,
            history: vec![],
        };
        let output = render(&report, 0, Some(&trend));

        let health_line = output.lines().find(|l| l.contains("Health")).unwrap_or("");
        assert!(
            health_line.contains("-5"),
            "Health category row should show delta '-5', got: {health_line:?}"
        );
    }

    #[test]
    fn direction_arrow_maps_correctly() {
        assert_eq!(direction_arrow(&VelocityDirection::Improving), "↑");
        assert_eq!(direction_arrow(&VelocityDirection::Declining), "↓");
        assert_eq!(direction_arrow(&VelocityDirection::Stable), "→");
    }

    #[test]
    fn render_category_row_no_delta_on_first_run() {
        let report = make_report();
        let trend = make_first_run_trend();
        let output = render(&report, 0, Some(&trend));

        // On first run, category rows should NOT have delta markers
        let health_line = output.lines().find(|l| l.contains("Health")).unwrap_or("");
        // No (+N) or (-N) pattern should appear on the category line for a first run
        assert!(
            !health_line.contains("(+") && !health_line.contains("(-"),
            "Health category row should not show delta on first run, got: {health_line:?}"
        );
    }

    #[test]
    fn render_shows_ecosystem_summary_line() {
        use crate::deps::{Ecosystem, EcosystemReport};
        let mut report = make_report();
        report.dep_ecosystem_reports = vec![EcosystemReport {
            ecosystem: Ecosystem::Cargo,
            total_deps: 3,
            mean_drift_years: 1.5,
            total_drift_years: 4.5,
            critical_deps: vec![],
        }];
        let output = render(&report, 0, None);
        assert!(
            output.contains("Cargo"),
            "output should contain ecosystem name"
        );
        assert!(
            output.contains("1.5 libyears avg"),
            "output should contain mean drift"
        );
        assert!(output.contains("3 deps"), "output should contain dep count");
    }

    #[test]
    fn render_shows_critical_dep_with_cve_count() {
        use crate::deps::{DepAge, DepTier, Ecosystem, EcosystemReport, Vuln};
        let mut report = make_report();
        report.dep_ecosystem_reports = vec![EcosystemReport {
            ecosystem: Ecosystem::Cargo,
            total_deps: 1,
            mean_drift_years: 6.2,
            total_drift_years: 6.2,
            critical_deps: vec![DepAge {
                name: "old-crate".into(),
                ecosystem: Ecosystem::Cargo,
                current_version: "0.1.0".into(),
                drift_years: 6.2,
                tier: DepTier::Critical,
                vulnerabilities: vec![Vuln {
                    id: "RUSTSEC-0000-0001".into(),
                    severity: "HIGH".into(),
                    description: "a vulnerability".into(),
                }],
            }],
        }];
        let output = render(&report, 0, None);
        assert!(
            output.contains("old-crate"),
            "output should contain dep name"
        );
        assert!(
            output.contains("0.1.0"),
            "output should contain dep version"
        );
        assert!(output.contains("6.2y"), "output should contain drift years");
        assert!(output.contains("1 CVE"), "output should contain CVE count");
    }

    #[test]
    fn cli_shows_top_unstable_files_sorted_by_instability() {
        use crate::scorer::FileCouplingMetrics;
        let mut report = make_report();
        // Builder (step 02-02) produces vec sorted descending by instability
        report.per_file_coupling = vec![
            FileCouplingMetrics {
                path: "src/unstable.rs".into(),
                ca: 1,
                ce: 9,
                instability: 0.9,
            },
            FileCouplingMetrics {
                path: "src/mid.rs".into(),
                ca: 2,
                ce: 3,
                instability: 0.5,
            },
            FileCouplingMetrics {
                path: "src/stable.rs".into(),
                ca: 5,
                ce: 0,
                instability: 0.1,
            },
        ];
        let output = render(&report, 0, None);

        // Section header must appear
        assert!(
            output.contains("unstable"),
            "output should contain 'unstable' section header"
        );

        // All 3 file paths must appear
        assert!(
            output.contains("src/unstable.rs"),
            "output should contain most unstable file"
        );
        assert!(
            output.contains("src/mid.rs"),
            "output should contain mid-instability file"
        );
        assert!(
            output.contains("src/stable.rs"),
            "output should contain stable file"
        );

        // Instability values must appear in descending order
        let pos_unstable = output
            .find("src/unstable.rs")
            .expect("unstable.rs not found");
        let pos_mid = output.find("src/mid.rs").expect("mid.rs not found");
        let pos_stable = output.find("src/stable.rs").expect("stable.rs not found");
        assert!(
            pos_unstable < pos_mid && pos_mid < pos_stable,
            "files must appear in descending instability order: unstable={pos_unstable}, mid={pos_mid}, stable={pos_stable}"
        );

        // Instability values and Ca/Ce values must appear
        assert!(
            output.contains("0.90"),
            "output should contain instability 0.90"
        );
        assert!(
            output.contains("0.50"),
            "output should contain instability 0.50"
        );
        assert!(
            output.contains("0.10"),
            "output should contain instability 0.10"
        );
        assert!(output.contains("Ca=1"), "output should contain Ca=1");
        assert!(output.contains("Ce=9"), "output should contain Ce=9");
    }

    #[test]
    fn cli_skips_unstable_section_when_no_data() {
        let report = make_report(); // per_file_coupling is empty by default
        let output = render(&report, 0, None);
        assert!(
            !output.contains("unstable"),
            "output should not contain 'unstable' when per_file_coupling is empty"
        );
    }
}
