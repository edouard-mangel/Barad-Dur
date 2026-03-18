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

    // Header
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

    // First-run notice or trend delta
    let is_first = trend.map(|t| t.delta.is_first).unwrap_or(true);
    if is_first {
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

    // Delta and direction (only when a prior run exists on this branch)
    if let Some(summary) = trend {
        if !summary.delta.is_first {
            let delta = summary.delta.overall;
            let delta_str = if delta >= 0 {
                format!("+{} vs last run", delta)
            } else {
                format!("{} vs last run", delta)
            };
            out.push_str(&format!("  {}\n", delta_str.dimmed()));

            if let Some(velocity) = &summary.velocity {
                use crate::trend::VelocityDirection;
                let direction_str = match velocity.direction {
                    VelocityDirection::Improving => "improving",
                    VelocityDirection::Declining => "declining",
                    VelocityDirection::Stable => "stable",
                };
                out.push_str(&format!("  {}\n", direction_str.dimmed()));
            }
        }
    }

    // Categories
    out.push_str(&format!(
        "\n{}\n",
        "───────────────────────────────────────────────────".dimmed()
    ));

    for cat in &report.categories {
        out.push_str(&format!(
            "\n  {} {} {}\n",
            format!("▸ {}", cat.name).bold(),
            format_score_bar(cat.score, 12),
            format_score_number(cat.score)
        ));

        if verbosity > 0 {
            for metric in &cat.metrics {
                let score_indicator = format_score_dot(metric.score);
                out.push_str(&format!(
                    "    {} {} {}  {}\n",
                    score_indicator,
                    metric.name,
                    format_score_number(metric.score),
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
    }

    // Top actions
    if !report.top_actions.is_empty() {
        out.push_str(&format!(
            "\n{}\n",
            "───────────────────────────────────────────────────".dimmed()
        ));
        out.push_str(&format!("  {}\n", "Top Actions:".bold()));
        for (i, action) in report.top_actions.iter().enumerate() {
            out.push_str(&format!("  {}. {}\n", i + 1, action));
        }
    }

    out.push_str(&format!(
        "{}\n\n",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bold()
    ));

    out
}

fn format_score_bar(score: u32, width: usize) -> String {
    let filled = (score as usize * width) / 100;
    let empty = width - filled;
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

    if score >= 71 {
        bar.green().to_string()
    } else if score >= 41 {
        bar.yellow().to_string()
    } else {
        bar.red().to_string()
    }
}

fn format_score_number(score: u32) -> String {
    let text = format!("{}/100", score);
    if score >= 71 {
        text.green().bold().to_string()
    } else if score >= 41 {
        text.yellow().bold().to_string()
    } else {
        text.red().bold().to_string()
    }
}

fn format_score_dot(score: u32) -> String {
    if score >= 71 {
        "●".green().to_string()
    } else if score >= 41 {
        "●".yellow().to_string()
    } else {
        "●".red().to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use super::*;
    use crate::metrics::{CategoryResult, MetricValue, RawValue};
    use crate::trend::{TrendDelta, TrendSummary, TrendVelocity, VelocityDirection};

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
                    score: 50,
                }],
            }],
            top_actions: vec!["[Health] Bus factor (score: 50) — Improve".into()],
            remote_meta: None,
            file_hotspots: vec![],
            coupling_pairs: vec![],
            author_ownership: vec![],
            file_ages: vec![],
            history: vec![],
        }
    }

    fn make_first_run_trend() -> TrendSummary {
        TrendSummary {
            delta: TrendDelta { overall: 0, categories: HashMap::new(), is_first: true },
            sparkline: vec![],
            velocity: None,
            branch_mismatch_warning: false,
            history: vec![],
        }
    }

    fn make_subsequent_run_trend(delta: i32) -> TrendSummary {
        TrendSummary {
            delta: TrendDelta { overall: delta, categories: HashMap::new(), is_first: false },
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
}
