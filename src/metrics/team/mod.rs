use crate::metrics::{author_line_counts, CategoryResult, MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;
use std::collections::HashMap;

const MIN_TEAM_SIZE: usize = 4;

pub fn compute_team(
    snapshot: &RepoSnapshot,
    thresholds: &crate::config::TeamThresholds,
) -> CategoryResult {
    if snapshot.authors.len() < MIN_TEAM_SIZE {
        let na = |name: &str| MetricValue {
            name: name.to_string(),
            description: format!(
                "Small team ({} authors, need {MIN_TEAM_SIZE}+) — not applicable",
                snapshot.authors.len()
            ),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
        return CategoryResult {
            name: "Team".to_string(),
            score: 0,
            metrics: vec![
                na("Knowledge distribution"),
                na("Contributor activity"),
                na("Ownership clarity"),
                na("Collaboration patterns"),
                na("Merge patterns"),
            ],
        }
        .compute_score();
    }

    let metrics = vec![
        knowledge_distribution(snapshot, thresholds),
        contributor_activity(snapshot, thresholds),
        ownership_clarity(snapshot, thresholds),
        collaboration_patterns(snapshot, thresholds),
        merge_patterns(snapshot, thresholds),
    ];

    CategoryResult {
        name: "Team".to_string(),
        score: 0,
        metrics,
    }
    .compute_score()
}

/// Gini coefficient of code ownership across authors.
/// For solo projects (single author), this metric is not applicable and scores 100.
fn knowledge_distribution(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::TeamThresholds,
) -> MetricValue {
    if snapshot.authors.len() <= 1 {
        return MetricValue {
            name: "Knowledge distribution".to_string(),
            description: "Solo project — not applicable".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
    }

    if snapshot.blame_map.is_empty() {
        return MetricValue {
            name: "Knowledge distribution".to_string(),
            description: "No blame data available".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
    }

    // Sum blame lines per author across all files
    let mut lines_per_author: HashMap<usize, usize> = HashMap::new();
    for blame_lines in snapshot.blame_map.values() {
        for line in blame_lines {
            *lines_per_author.entry(line.author_id).or_insert(0) += line.line_count;
        }
    }

    let mut counts: Vec<f64> = lines_per_author.values().map(|&v| v as f64).collect();
    if counts.is_empty() {
        return MetricValue {
            name: "Knowledge distribution".to_string(),
            description: "No blame data".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
    }

    counts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = counts.len() as f64;
    let total: f64 = counts.iter().sum();

    let gini = if total == 0.0 {
        0.0
    } else {
        let numerator: f64 = counts
            .iter()
            .enumerate()
            .map(|(i, x)| (i as f64 + 1.0) * x)
            .sum();
        (2.0 * numerator) / (n * total) - (n + 1.0) / n
    };

    let score = if gini > 0.7 {
        20
    } else if gini > 0.5 {
        50
    } else if gini > 0.3 {
        75
    } else {
        100
    };

    let label = if gini > 0.7 {
        "highly concentrated"
    } else if gini > 0.5 {
        "concentrated"
    } else if gini > 0.3 {
        "moderate"
    } else {
        "well distributed"
    };

    MetricValue {
        name: "Knowledge distribution".to_string(),
        description: format!("Gini {:.2} ({})", gini, label),
        raw_value: RawValue::Float(gini),
        score: Some(score),
    }
}

/// Percentage of known authors with commits in the time window.
fn contributor_activity(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::TeamThresholds,
) -> MetricValue {
    if snapshot.authors.is_empty() {
        return MetricValue {
            name: "Contributor activity".to_string(),
            description: "No authors".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
    }

    let total_authors = snapshot.authors.len();
    let active_authors = snapshot
        .authors
        .iter()
        .filter(|author| {
            snapshot
                .commits_by_author
                .get(&author.id)
                .map(|commit_ids| {
                    commit_ids.iter().any(|cid| {
                        snapshot
                            .commits
                            .iter()
                            .any(|c| c.id == *cid && snapshot.time_window.contains(&c.timestamp))
                    })
                })
                .unwrap_or(false)
        })
        .count();

    let pct = (active_authors as f64 / total_authors as f64) * 100.0;

    let score = if pct < 30.0 {
        25
    } else if pct < 50.0 {
        50
    } else if pct < 70.0 {
        75
    } else {
        100
    };

    MetricValue {
        name: "Contributor activity".to_string(),
        description: format!(
            "{}/{} authors active ({:.0}%)",
            active_authors, total_authors, pct
        ),
        raw_value: RawValue::Percentage(pct),
        score: Some(score),
    }
}

/// Percentage of files with a clear owner (>50% blame to one author).
/// For solo projects (single author), ownership is trivially clear.
fn ownership_clarity(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::TeamThresholds,
) -> MetricValue {
    if snapshot.authors.len() <= 1 {
        return MetricValue {
            name: "Ownership clarity".to_string(),
            description: "Solo project — not applicable".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
    }

    if snapshot.blame_map.is_empty() {
        return MetricValue {
            name: "Ownership clarity".to_string(),
            description: "No blame data".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
    }

    let mut files_with_owner = 0;
    let total_files = snapshot.blame_map.len();

    for blame_lines in snapshot.blame_map.values() {
        if blame_lines.is_empty() {
            continue;
        }
        let author_counts = author_line_counts(blame_lines);
        let total: usize = author_counts.values().sum();
        let max: usize = *author_counts.values().max().unwrap_or(&0);
        if total > 0 && (max as f64 / total as f64) > 0.5 {
            files_with_owner += 1;
        }
    }

    let pct = if total_files > 0 {
        (files_with_owner as f64 / total_files as f64) * 100.0
    } else {
        0.0
    };

    // High ownership clarity is good (clear responsibility)
    let score = if pct > 80.0 {
        90
    } else if pct > 60.0 {
        75
    } else if pct > 40.0 {
        60
    } else {
        40
    };

    MetricValue {
        name: "Ownership clarity".to_string(),
        description: format!("{:.0}% of files have a clear owner", pct),
        raw_value: RawValue::Percentage(pct),
        score: Some(score),
    }
}

/// Detect directory silos where one author dominates.
/// For solo projects (single author), silos are expected and not a concern.
fn collaboration_patterns(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::TeamThresholds,
) -> MetricValue {
    if snapshot.authors.len() <= 1 {
        return MetricValue {
            name: "Collaboration patterns".to_string(),
            description: "Solo project — not applicable".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
    }

    if snapshot.blame_map.is_empty() {
        return MetricValue {
            name: "Collaboration patterns".to_string(),
            description: "No blame data".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
    }

    // Group blame lines by top-level directory
    let mut dir_author_lines: HashMap<String, HashMap<usize, usize>> = HashMap::new();

    for (path, blame_lines) in &snapshot.blame_map {
        let dir = path
            .components()
            .next()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());

        let entry = dir_author_lines.entry(dir).or_default();
        for line in blame_lines {
            *entry.entry(line.author_id).or_insert(0) += line.line_count;
        }
    }

    let mut silos: Vec<String> = Vec::new();
    for (dir, author_lines) in &dir_author_lines {
        let total: usize = author_lines.values().sum();
        let max: usize = *author_lines.values().max().unwrap_or(&0);
        if total > 0 && (max as f64 / total as f64) > 0.8 {
            silos.push(dir.clone());
        }
    }

    let count = silos.len();
    let total_dirs = dir_author_lines.len();

    let score = if total_dirs == 0 {
        50
    } else {
        let silo_pct = (count as f64 / total_dirs as f64) * 100.0;
        if silo_pct > 60.0 {
            25
        } else if silo_pct > 30.0 {
            50
        } else if silo_pct > 10.0 {
            75
        } else {
            100
        }
    };

    MetricValue {
        name: "Collaboration patterns".to_string(),
        description: format!("{} directory silos detected out of {}", count, total_dirs),
        raw_value: RawValue::Count(count),
        score: Some(score),
    }
}

/// Merge commit frequency and estimated branch lifetime.
fn merge_patterns(
    snapshot: &RepoSnapshot,
    _thresholds: &crate::config::TeamThresholds,
) -> MetricValue {
    if snapshot.commits.is_empty() {
        return MetricValue {
            name: "Merge patterns".to_string(),
            description: "No commits".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: None,
        };
    }

    let total = snapshot.commits.len();
    let merge_count = snapshot.commits.iter().filter(|c| c.is_merge).count();
    let merge_pct = (merge_count as f64 / total as f64) * 100.0;

    // Estimate avg branch lifetime from merge-to-merge intervals
    let mut merge_timestamps: Vec<_> = snapshot
        .commits
        .iter()
        .filter(|c| c.is_merge)
        .map(|c| c.timestamp)
        .collect();
    merge_timestamps.sort();

    let avg_days = if merge_timestamps.len() >= 2 {
        let intervals: Vec<f64> = merge_timestamps
            .windows(2)
            .map(|w| (w[1] - w[0]).num_hours() as f64 / 24.0)
            .collect();
        let sum: f64 = intervals.iter().sum();
        Some(sum / intervals.len() as f64)
    } else {
        None
    };

    let description = if let Some(days) = avg_days {
        format!(
            "{} merges ({:.0}%), avg {:.1} days between merges",
            merge_count, merge_pct, days
        )
    } else {
        format!("{} merges ({:.0}%)", merge_count, merge_pct)
    };

    // Moderate merge rate is healthy; too many or too few is a smell
    let score = if merge_count == 0 && total > 20 {
        50 // No merges in a big project = might be trunk-based (neutral)
    } else if merge_pct > 50.0 {
        40 // Merge-heavy
    } else {
        80
    };

    MetricValue {
        name: "Merge patterns".to_string(),
        description,
        raw_value: RawValue::Count(merge_count),
        score: Some(score),
    }
}

/// The author holding a *strict* majority (> 50%) of a file's blamed lines
/// — the "main developer" proxy from the org-coupling design (Decision 1).
/// `None` when blame is empty or no author clears the majority (a
/// collectively-owned file has no single owner to mismatch against).
/// Same strict-majority rule as `bus_factor`'s `is_file_author_dominated`,
/// but returns *which* author instead of discarding it.
fn primary_author(lines: &[crate::snapshot::BlameLine]) -> Option<usize> {
    let counts = author_line_counts(lines);
    let total: usize = counts.values().sum();
    counts
        .into_iter()
        .find(|&(_, count)| count * 2 > total)
        .map(|(author, _)| author)
}

#[cfg(test)]
mod tests;
