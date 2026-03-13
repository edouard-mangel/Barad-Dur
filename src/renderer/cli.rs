use colored::Colorize;

use crate::scorer::AnalysisReport;

/// Render the analysis report as a colored CLI string.
pub fn render(report: &AnalysisReport, verbosity: u8) -> String {
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

    // Overall score
    out.push_str(&format!(
        "\n  {} {} {}\n",
        "Overall Score:".bold(),
        format_score_bar(report.overall_score, 20),
        format_score_number(report.overall_score)
    ));

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
    use super::*;
    use crate::metrics::{CategoryResult, MetricValue, RawValue};

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

    #[test]
    fn render_contains_header() {
        let report = make_report();
        let output = render(&report, 0);
        assert!(output.contains("Barad-dur"));
    }

    #[test]
    fn render_contains_repo_info() {
        let report = make_report();
        let output = render(&report, 0);
        assert!(output.contains("test-repo"));
        assert!(output.contains("main"));
    }

    #[test]
    fn render_contains_category() {
        let report = make_report();
        let output = render(&report, 0);
        assert!(output.contains("Health"));
    }

    #[test]
    fn render_verbose_shows_metrics() {
        let report = make_report();
        let output = render(&report, 1);
        assert!(output.contains("Bus factor"));
        assert!(output.contains("50/100"));
    }

    #[test]
    fn render_very_verbose_shows_raw_value() {
        let report = make_report();
        let output = render(&report, 2);
        assert!(output.contains("value:"));
        assert!(output.contains('2'));
    }

    #[test]
    fn render_contains_actions() {
        let report = make_report();
        let output = render(&report, 0);
        assert!(output.contains("Top Actions"));
    }
}
