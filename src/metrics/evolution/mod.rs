use std::collections::HashMap;

use chrono::Utc;

use crate::metrics::{CategoryResult, MetricValue, RawValue};
use crate::snapshot::{ChangeType, RepoSnapshot};

pub fn compute_evolution(
    snapshot: &RepoSnapshot,
    thresholds: &crate::config::EvolutionThresholds,
) -> CategoryResult {
    let metrics = vec![
        growth_trend(snapshot, thresholds),
        refactoring_ratio(snapshot, thresholds),
        code_age(snapshot, thresholds),
        commit_cadence(snapshot, thresholds),
    ];

    CategoryResult {
        name: "Evolution".to_string(),
        score: 0,
        metrics,
    }
    .compute_score()
}

fn growth_score(growth_pct: f64) -> u32 {
    if growth_pct.abs() > 50.0 {
        40 // Rapid change (growth or shrink)
    } else if growth_pct.abs() > 20.0 {
        65
    } else {
        90 // Stable
    }
}

/// Net file count change over the time window.
fn growth_trend(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::EvolutionThresholds,
) -> MetricValue {
    if snapshot.commits.is_empty() {
        return MetricValue {
            name: "Growth trend".to_string(),
            description: "No commits".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
        };
    }

    let mut files_added: i64 = 0;
    let mut files_deleted: i64 = 0;
    let mut lines_added: i64 = 0;
    let mut lines_deleted: i64 = 0;

    for commit in &snapshot.commits {
        if !snapshot.time_window.contains(&commit.timestamp) {
            continue;
        }
        for fc in &commit.files_changed {
            match fc.change_type {
                ChangeType::Added => files_added += 1,
                ChangeType::Deleted => files_deleted += 1,
                _ => {}
            }
            lines_added += fc.additions as i64;
            lines_deleted += fc.deletions as i64;
        }
    }

    let net_files = files_added - files_deleted;
    let net_lines = lines_added - lines_deleted;

    // Rapid growth can be a smell (more code = more maintenance)
    let total_files = snapshot.files.len() as i64;
    let growth_pct = if total_files > 0 {
        (net_files as f64 / total_files as f64) * 100.0
    } else {
        0.0
    };

    let score = growth_score(growth_pct);

    MetricValue {
        name: "Growth trend".to_string(),
        description: format!("{:+} files, {:+} lines in window", net_files, net_lines),
        raw_value: RawValue::Integer(net_files),
        score,
    }
}

const STRUCTURAL_KEYWORDS: &[&str] = &[
    "refactor",
    "restructur",
    "reorganiz",
    "extract",
    "tidy",
    "clean up",
    "simplif",
    "consolidat",
    "rename",
    "move",
    "dedup",
    "remove dead",
    "dead code",
];

fn is_structural_investment(commit: &crate::snapshot::Commit) -> bool {
    let msg = commit.message.to_lowercase();
    if STRUCTURAL_KEYWORDS.iter().any(|kw| msg.contains(kw)) {
        return true;
    }
    let total_del: u32 = commit.files_changed.iter().map(|fc| fc.deletions).sum();
    let total_add: u32 = commit.files_changed.iter().map(|fc| fc.additions).sum();
    for fc in &commit.files_changed {
        match fc.change_type {
            ChangeType::Renamed | ChangeType::Deleted => return true,
            _ => {}
        }
    }
    if total_del > 50 {
        let denom = total_add + total_del;
        if denom > 0 && (total_del as f64 / denom as f64) > 0.40 {
            return true;
        }
    }
    false
}

fn refactoring_score(ratio: f64) -> u32 {
    if ratio < 0.05 {
        25 // structural debt accumulating
    } else if ratio < 0.15 {
        55 // low investment
    } else if ratio < 0.30 {
        80 // healthy investment
    } else {
        92 // strong investment
    }
}

/// Ratio of commits that invest in structural maintenance (refactoring, cleanup, reorganization).
fn refactoring_ratio(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::EvolutionThresholds,
) -> MetricValue {
    if snapshot.commits.is_empty() {
        return MetricValue {
            name: "Refactoring ratio".to_string(),
            description: "No commits".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
        };
    }

    let window_commits: Vec<_> = snapshot
        .commits
        .iter()
        .filter(|c| snapshot.time_window.contains(&c.timestamp) && !c.is_merge)
        .collect();

    if window_commits.is_empty() {
        return MetricValue {
            name: "Refactoring ratio".to_string(),
            description: "No commits in window".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
        };
    }

    let total = window_commits.len();
    let n_structural = window_commits
        .iter()
        .filter(|c| is_structural_investment(c))
        .count();

    let ratio = n_structural as f64 / total as f64;
    let pct = ratio * 100.0;

    let score = refactoring_score(ratio);

    MetricValue {
        name: "Refactoring ratio".to_string(),
        description: format!(
            "{} of {} commits invest in structure ({:.0}%)",
            n_structural, total, pct
        ),
        raw_value: RawValue::Float(ratio),
        score,
    }
}

fn age_score(age_months: f64) -> u32 {
    if age_months > 24.0 {
        40
    } else if age_months > 12.0 {
        60
    } else if age_months > 3.0 {
        90 // Sweet spot
    } else {
        70 // Very new
    }
}

fn age_description(age_months: f64) -> &'static str {
    if age_months > 12.0 {
        "months (median code age)"
    } else {
        "months (median code age)"
    }
}

/// Median age of code based on blame timestamps.
fn code_age(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::EvolutionThresholds,
) -> MetricValue {
    // Collect (timestamp, line_count) pairs and compute weighted median
    let mut weighted: Vec<_> = snapshot
        .blame_map
        .values()
        .flat_map(|lines| lines.iter().map(|l| (l.timestamp, l.line_count)))
        .collect();

    if weighted.is_empty() {
        return MetricValue {
            name: "Code age".to_string(),
            description: "No blame data".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
        };
    }

    weighted.sort_by_key(|&(ts, _)| ts);
    let total_lines: usize = weighted.iter().map(|&(_, c)| c).sum();
    let mid = total_lines / 2;
    let mut cumulative = 0;
    let median = weighted
        .iter()
        .find(|&&(_, c)| {
            cumulative += c;
            cumulative > mid
        })
        .map(|&(ts, _)| ts)
        .unwrap_or(weighted[0].0);
    let now = Utc::now();
    let age_days = (now - median).num_days();
    let age_months = age_days as f64 / 30.0;

    let description = if age_months > 12.0 {
        format!("{:.0} {}", age_months, age_description(age_months))
    } else {
        format!("{:.1} {}", age_months, age_description(age_months))
    };

    let score = age_score(age_months);

    MetricValue {
        name: "Code age".to_string(),
        description,
        raw_value: RawValue::Float(age_months),
        score,
    }
}

fn cadence_score(cv: f64) -> u32 {
    if cv < 0.5 {
        90
    } else if cv < 1.0 {
        70
    } else {
        50
    }
}

fn regularity_label(cv: f64) -> &'static str {
    if cv < 0.5 {
        "regular"
    } else if cv < 1.0 {
        "moderate"
    } else {
        "irregular"
    }
}

/// Commit frequency and regularity.
fn commit_cadence(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::EvolutionThresholds,
) -> MetricValue {
    let window_commits: Vec<_> = snapshot
        .commits
        .iter()
        .filter(|c| snapshot.time_window.contains(&c.timestamp))
        .collect();

    if window_commits.is_empty() {
        return MetricValue {
            name: "Commit cadence".to_string(),
            description: "No commits in window".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
        };
    }

    // Group commits by day
    let mut daily_counts: HashMap<i64, usize> = HashMap::new();
    for commit in &window_commits {
        let day = commit.timestamp.date_naive().and_hms_opt(0, 0, 0).unwrap();
        let day_key = day.and_utc().timestamp() / 86400;
        *daily_counts.entry(day_key).or_insert(0) += 1;
    }

    let counts: Vec<f64> = daily_counts.values().map(|&c| c as f64).collect();
    let n = counts.len() as f64;
    let mean = counts.iter().sum::<f64>() / n;

    let variance = counts.iter().map(|c| (c - mean).powi(2)).sum::<f64>() / n;
    let std_dev = variance.sqrt();
    let cv = if mean > 0.0 { std_dev / mean } else { 0.0 }; // Coefficient of variation

    let regularity = regularity_label(cv);

    let total_days = if let (Some(first), Some(last)) = (
        window_commits.iter().map(|c| c.timestamp).min(),
        window_commits.iter().map(|c| c.timestamp).max(),
    ) {
        ((last - first).num_days() + 1).max(1) as f64
    } else {
        1.0
    };
    let commits_per_day = window_commits.len() as f64 / total_days;

    let score = cadence_score(cv);

    MetricValue {
        name: "Commit cadence".to_string(),
        description: format!("{:.1} commits/day, {} pattern", commits_per_day, regularity),
        raw_value: RawValue::Float(commits_per_day),
        score,
    }
}

#[cfg(test)]
mod tests;
