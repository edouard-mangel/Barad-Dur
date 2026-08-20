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
        growth_balance(snapshot),
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
            score: None,
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
        score: Some(score),
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
            score: None,
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
            score: None,
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
        score: Some(score),
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

fn age_description(_age_months: f64) -> &'static str {
    "months (median code age)"
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
            score: None,
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
        score: Some(score),
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
            score: None,
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
        score: Some(score),
    }
}

/// Code/test growth balance (Crime Scene Ch. 9, trends design M2): lines
/// added per `file_role` partition (Source vs Test) across the window,
/// split into two halves by timestamp so a test partition falling behind
/// *recently* is visible. Annotation-first: never scored in v1 — a growth
/// ratio has no defensible universal band, and `score: None` metrics stay
/// out of the category average. Merge commits excluded (first-parent
/// diffs double-count merged MRs); a commit exactly at the midpoint
/// belongs to the second half.
fn growth_balance(snapshot: &RepoSnapshot) -> MetricValue {
    use crate::metrics::file_role::{classify, FileRole};
    let name = "Code/test growth balance".to_string();
    let na = |description: &str| MetricValue {
        name: name.clone(),
        description: description.to_string(),
        raw_value: RawValue::Text("N/A".to_string()),
        score: None,
    };
    let role_of: std::collections::HashMap<&std::path::PathBuf, FileRole> = snapshot
        .files
        .iter()
        .map(|f| (&f.path, classify(&f.path)))
        .collect();
    if !role_of.values().any(|r| *r == FileRole::Test) {
        return na("No test files detected — not applicable");
    }
    if !role_of.values().any(|r| *r == FileRole::Source) {
        return na("No source files detected — not applicable");
    }
    let commits: Vec<&crate::snapshot::Commit> =
        snapshot.commits.iter().filter(|c| !c.is_merge).collect();
    let (Some(min_ts), Some(max_ts)) = (
        commits.iter().map(|c| c.timestamp).min(),
        commits.iter().map(|c| c.timestamp).max(),
    ) else {
        return na("No commits in window");
    };
    let midpoint = min_ts + (max_ts - min_ts) / 2;

    // Per half: (source additions, test additions).
    let mut halves = [(0u64, 0u64); 2];
    // Source files' second-half additions, and whether each source file
    // ever co-changed with a Test-role file in the window.
    let mut second_half_source: std::collections::HashMap<&std::path::PathBuf, u64> =
        std::collections::HashMap::new();
    let mut has_test_partner: std::collections::HashSet<&std::path::PathBuf> =
        std::collections::HashSet::new();
    for c in &commits {
        let half = usize::from(c.timestamp >= midpoint);
        let touches_test = c
            .files_changed
            .iter()
            .any(|fc| role_of.get(&fc.path) == Some(&FileRole::Test));
        for fc in &c.files_changed {
            match role_of.get(&fc.path) {
                Some(FileRole::Source) => {
                    halves[half].0 += u64::from(fc.additions);
                    if half == 1 && fc.additions > 0 {
                        *second_half_source.entry(&fc.path).or_insert(0) += u64::from(fc.additions);
                    }
                    if touches_test {
                        has_test_partner.insert(&fc.path);
                    }
                }
                Some(FileRole::Test) => halves[half].1 += u64::from(fc.additions),
                _ => {}
            }
        }
    }
    // Bare ratio ("4.0:1") or the zero-test wording; the word "ratio"
    // is prefixed only on the second-half slot, matching the design's
    // pinned description format.
    let bare = |(src, test): (u64, u64)| {
        if test == 0 {
            "no test growth".to_string()
        } else {
            format!("{:.1}:1", src as f64 / test as f64)
        }
    };
    let second = if halves[1].1 == 0 {
        bare(halves[1])
    } else {
        format!("ratio {}", bare(halves[1]))
    };
    let source_total = halves[0].0 + halves[1].0;
    let test_total = halves[0].1 + halves[1].1;
    let mut untested: Vec<(&std::path::PathBuf, u64)> = second_half_source
        .into_iter()
        .filter(|(path, _)| !has_test_partner.contains(*path))
        .collect();
    untested.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let list: Vec<String> = untested
        .into_iter()
        .take(10)
        .map(|(path, added)| {
            format!(
                "{} — +{added} lines (2nd half), no test co-change",
                path.display()
            )
        })
        .collect();
    MetricValue {
        name,
        description: format!(
            "source +{source_total} / test +{test_total} lines this window; second half {} (first half {})",
            second,
            bare(halves[0]),
        ),
        raw_value: RawValue::List(list),
        score: None,
    }
}

#[cfg(test)]
mod tests;
