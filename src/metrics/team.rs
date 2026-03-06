use std::collections::HashMap;
use std::path::PathBuf;

use crate::metrics::{CategoryResult, MetricValue, RawValue};
use crate::snapshot::RepoSnapshot;

pub fn compute_team(snapshot: &RepoSnapshot) -> CategoryResult {
    let metrics = vec![
        knowledge_distribution(snapshot),
        contributor_activity(snapshot),
        ownership_clarity(snapshot),
        collaboration_patterns(snapshot),
        merge_patterns(snapshot),
    ];

    CategoryResult {
        name: "Team".to_string(),
        score: 0,
        metrics,
    }
    .compute_score()
}

/// Gini coefficient of code ownership across authors.
fn knowledge_distribution(snapshot: &RepoSnapshot) -> MetricValue {
    if snapshot.blame_map.is_empty() || snapshot.authors.is_empty() {
        return MetricValue {
            name: "Knowledge distribution".to_string(),
            description: "No blame data available".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
        };
    }

    // Sum blame lines per author across all files
    let mut lines_per_author: HashMap<usize, usize> = HashMap::new();
    for blame_lines in snapshot.blame_map.values() {
        for line in blame_lines {
            *lines_per_author.entry(line.author_id).or_insert(0) += 1;
        }
    }

    let mut counts: Vec<f64> = lines_per_author.values().map(|&v| v as f64).collect();
    if counts.is_empty() {
        return MetricValue {
            name: "Knowledge distribution".to_string(),
            description: "No blame data".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
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
        score,
    }
}

/// Percentage of known authors with commits in the time window.
fn contributor_activity(snapshot: &RepoSnapshot) -> MetricValue {
    if snapshot.authors.is_empty() {
        return MetricValue {
            name: "Contributor activity".to_string(),
            description: "No authors".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
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
                            .any(|c| &c.id == cid && snapshot.time_window.contains(&c.timestamp))
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
        score,
    }
}

/// Percentage of files with a clear owner (>50% blame to one author).
fn ownership_clarity(snapshot: &RepoSnapshot) -> MetricValue {
    if snapshot.blame_map.is_empty() {
        return MetricValue {
            name: "Ownership clarity".to_string(),
            description: "No blame data".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
        };
    }

    let mut files_with_owner = 0;
    let total_files = snapshot.blame_map.len();

    for blame_lines in snapshot.blame_map.values() {
        if blame_lines.is_empty() {
            continue;
        }
        let mut author_lines: HashMap<usize, usize> = HashMap::new();
        for line in blame_lines {
            *author_lines.entry(line.author_id).or_insert(0) += 1;
        }
        let total: usize = author_lines.values().sum();
        let max: usize = *author_lines.values().max().unwrap_or(&0);
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
        score,
    }
}

/// Detect directory silos where one author dominates.
fn collaboration_patterns(snapshot: &RepoSnapshot) -> MetricValue {
    if snapshot.blame_map.is_empty() {
        return MetricValue {
            name: "Collaboration patterns".to_string(),
            description: "No blame data".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
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
            *entry.entry(line.author_id).or_insert(0) += 1;
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
        score,
    }
}

/// Merge commit frequency and estimated branch lifetime.
fn merge_patterns(snapshot: &RepoSnapshot) -> MetricValue {
    if snapshot.commits.is_empty() {
        return MetricValue {
            name: "Merge patterns".to_string(),
            description: "No commits".to_string(),
            raw_value: RawValue::Text("N/A".to_string()),
            score: 50,
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
        score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::*;
    use chrono::{Duration, Utc};

    #[test]
    fn knowledge_distribution_detects_concentration() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        snapshot.authors = vec![
            Author { id: 0, name: "Alice".into(), email: "a@t.com".into() },
            Author { id: 1, name: "Bob".into(), email: "b@t.com".into() },
            Author { id: 2, name: "Carol".into(), email: "c@t.com".into() },
        ];

        let now = Utc::now();
        // Alice owns 95 lines, Bob 4, Carol 1 → very high Gini
        let mut blame = Vec::new();
        for _ in 0..95 {
            blame.push(BlameLine { author_id: 0, commit_id: "c1".into(), timestamp: now });
        }
        for _ in 0..4 {
            blame.push(BlameLine { author_id: 1, commit_id: "c2".into(), timestamp: now });
        }
        for _ in 0..1 {
            blame.push(BlameLine { author_id: 2, commit_id: "c3".into(), timestamp: now });
        }
        snapshot.blame_map.insert(PathBuf::from("file.rs"), blame);

        let result = knowledge_distribution(&snapshot);
        match result.raw_value {
            RawValue::Float(gini) => assert!(gini >= 0.5, "Expected Gini >= 0.5, got {}", gini),
            _ => panic!("Expected Float"),
        }
        assert!(result.score <= 50);
    }

    #[test]
    fn contributor_activity_counts_active() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        snapshot.authors = vec![
            Author { id: 0, name: "Alice".into(), email: "a@t.com".into() },
            Author { id: 1, name: "Bob".into(), email: "b@t.com".into() },
            Author { id: 2, name: "Carol".into(), email: "c@t.com".into() },
            Author { id: 3, name: "Dave".into(), email: "d@t.com".into() },
            Author { id: 4, name: "Eve".into(), email: "e@t.com".into() },
        ];

        let now = Utc::now();
        // Only 3 authors have recent commits
        for i in 0..3 {
            snapshot.commits.push(Commit {
                id: format!("c{}", i),
                author: i,
                timestamp: now - Duration::days(10),
                message: "msg".into(),
                files_changed: vec![],
                is_merge: false,
                parent_count: 1,
            });
        }
        snapshot.build_indexes();

        let result = contributor_activity(&snapshot);
        match result.raw_value {
            RawValue::Percentage(p) => assert!((p - 60.0).abs() < 1.0, "Expected ~60%, got {}", p),
            _ => panic!("Expected Percentage"),
        }
    }

    #[test]
    fn ownership_clarity_detects_owners() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        let now = Utc::now();
        // File 1: Alice 80%, Bob 20% → clear owner
        let mut blame1 = Vec::new();
        for _ in 0..80 {
            blame1.push(BlameLine { author_id: 0, commit_id: "c1".into(), timestamp: now });
        }
        for _ in 0..20 {
            blame1.push(BlameLine { author_id: 1, commit_id: "c2".into(), timestamp: now });
        }
        snapshot.blame_map.insert(PathBuf::from("f1.rs"), blame1);

        // File 2: 50/50 → no clear owner
        let mut blame2 = Vec::new();
        for _ in 0..50 {
            blame2.push(BlameLine { author_id: 0, commit_id: "c1".into(), timestamp: now });
        }
        for _ in 0..50 {
            blame2.push(BlameLine { author_id: 1, commit_id: "c2".into(), timestamp: now });
        }
        snapshot.blame_map.insert(PathBuf::from("f2.rs"), blame2);

        let result = ownership_clarity(&snapshot);
        // 1 out of 2 files has clear owner = 50%
        match result.raw_value {
            RawValue::Percentage(p) => assert!((p - 50.0).abs() < 1.0),
            _ => panic!("Expected Percentage"),
        }
    }

    #[test]
    fn collaboration_detects_silos() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        let now = Utc::now();
        // "auth" directory: 100% Alice → silo
        let mut blame_auth = Vec::new();
        for _ in 0..100 {
            blame_auth.push(BlameLine { author_id: 0, commit_id: "c1".into(), timestamp: now });
        }
        snapshot.blame_map.insert(PathBuf::from("auth/login.rs"), blame_auth);

        // "api" directory: 60/40 split → NOT a silo
        let mut blame_api = Vec::new();
        for _ in 0..60 {
            blame_api.push(BlameLine { author_id: 0, commit_id: "c1".into(), timestamp: now });
        }
        for _ in 0..40 {
            blame_api.push(BlameLine { author_id: 1, commit_id: "c2".into(), timestamp: now });
        }
        snapshot.blame_map.insert(PathBuf::from("api/routes.rs"), blame_api);

        let result = collaboration_patterns(&snapshot);
        match result.raw_value {
            RawValue::Count(c) => assert_eq!(c, 1, "Should detect 1 silo (auth)"),
            _ => panic!("Expected Count"),
        }
    }

    #[test]
    fn merge_patterns_counts_merges() {
        let mut snapshot = RepoSnapshot::new(
            PathBuf::from("/tmp"),
            "test".into(),
            "main".into(),
            TimeWindow::default(),
        );

        let now = Utc::now();
        for i in 0..20 {
            snapshot.commits.push(Commit {
                id: format!("c{}", i),
                author: 0,
                timestamp: now - Duration::hours(i * 24),
                message: "msg".into(),
                files_changed: vec![],
                is_merge: i < 5, // First 5 are merges
                parent_count: if i < 5 { 2 } else { 1 },
            });
        }

        let result = merge_patterns(&snapshot);
        match result.raw_value {
            RawValue::Count(c) => assert_eq!(c, 5),
            _ => panic!("Expected Count"),
        }
    }
}
